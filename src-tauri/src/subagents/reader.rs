//! Reads the transcripts of the sub-agents from omp's session files, incrementally: from the last
//! offset read, never the whole file again, and only up to the last complete line (a line omp is
//! still writing is read again at the next pass).
//!
//! Blocking file I/O: the poller runs it on a blocking thread, outside every lock.

use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;

use weaver_core::jobs::subagents;

/// Where the transcripts are: omp's sessions folder, the session of the owner and the name of the
/// owner, who labels the assignment of its direct sub-agents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    pub root: PathBuf,
    pub session: String,
    pub owner_name: String,
}

/// One transcript to read now.
#[derive(Debug, Clone)]
pub struct ReadJob {
    pub index: usize,
    pub id: String,
    /// The transcript file, once known.
    pub file: Option<PathBuf>,
    /// The transcript of the parent sub-agent, for a nested one.
    pub parent_file: Option<PathBuf>,
    pub direct: bool,
    pub offset: u64,
    /// The sub-agent had ended when this was planned: nothing more will be written to it.
    pub final_pass: bool,
}

/// The complete lines read, and the offset after them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chunk {
    pub lines: Vec<String>,
    pub end: u64,
}

/// What one pass over one transcript found.
#[derive(Debug)]
pub struct Done {
    pub index: usize,
    pub file: Option<PathBuf>,
    /// The main session file of the owner, when it was looked for.
    pub main: Option<PathBuf>,
    pub final_pass: bool,
    pub outcome: Result<Chunk, String>,
}

/// Reads what omp wrote to the transcript of `job` since its offset. `main` is the main session
/// file of the owner when it is known already.
pub fn read(location: &Location, main: Option<PathBuf>, job: &ReadJob) -> Done {
    let mut found_main = None;
    let file = match &job.file {
        Some(file) => Ok(file.clone()),
        None => resolve(location, main, job, &mut found_main),
    };
    let outcome = file.clone().and_then(|file| read_from(&file, job.offset));
    Done {
        index: job.index,
        file: file.ok(),
        main: found_main,
        final_pass: job.final_pass,
        outcome,
    }
}

fn resolve(
    location: &Location,
    main: Option<PathBuf>,
    job: &ReadJob,
    found_main: &mut Option<PathBuf>,
) -> Result<PathBuf, String> {
    let parent = if job.direct {
        match main {
            Some(main) => main,
            None => {
                let main = subagents::find_main_file(&location.root, &location.session)
                    .ok_or("the session file of the agent was not found")?;
                *found_main = Some(main.clone());
                main
            }
        }
    } else {
        job.parent_file
            .clone()
            .ok_or("the transcript of its parent was not found")?
    };
    if !subagents::is_valid_agent_id(&job.id) {
        return Err("its name is not a valid file name".to_owned());
    }
    subagents::agent_file(&parent, &job.id)
        .ok_or_else(|| "the transcript file was not found".to_owned())
}

fn read_from(file: &PathBuf, offset: u64) -> Result<Chunk, String> {
    let mut open = std::fs::File::open(file).map_err(|error| describe(&error))?;
    let length = open.metadata().map_err(|error| describe(&error))?.len();
    if length < offset {
        return Err("the transcript was shortened".to_owned());
    }
    open.seek(SeekFrom::Start(offset))
        .map_err(|error| describe(&error))?;
    let mut bytes = Vec::with_capacity(usize::try_from(length - offset).unwrap_or(0));
    open.take(length - offset)
        .read_to_end(&mut bytes)
        .map_err(|error| describe(&error))?;
    // Only the complete lines: the last one may still be written.
    let complete = bytes
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |last| last + 1);
    let mut lines = Vec::new();
    for line in bytes[..complete].split(|byte| *byte == b'\n') {
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let line =
            std::str::from_utf8(line).map_err(|_| "the transcript is not valid text".to_owned())?;
        lines.push(line.to_owned());
    }
    Ok(Chunk {
        lines,
        end: offset + complete as u64,
    })
}

/// A reason a person can read.
fn describe(error: &std::io::Error) -> String {
    match error.kind() {
        std::io::ErrorKind::NotFound => "the transcript file was not found".to_owned(),
        std::io::ErrorKind::PermissionDenied => "the transcript file cannot be read".to_owned(),
        _ => format!("the transcript file cannot be read ({error})"),
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    fn job(file: &std::path::Path, offset: u64) -> ReadJob {
        ReadJob {
            index: 0,
            id: "a".into(),
            file: Some(file.to_path_buf()),
            parent_file: None,
            direct: true,
            offset,
            final_pass: false,
        }
    }

    fn location(root: &std::path::Path) -> Location {
        Location {
            root: root.to_path_buf(),
            session: "s".into(),
            owner_name: "Run T1".into(),
        }
    }

    #[test]
    fn a_transcript_is_read_from_the_last_offset_and_never_past_an_incomplete_line() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.jsonl");
        std::fs::write(&file, "{\"n\":1}\n{\"n\":2}\n{\"n\":3").unwrap();
        let place = location(dir.path());
        let first = read(&place, None, &job(&file, 0));
        let chunk = first.outcome.unwrap();
        assert_eq!(chunk.lines, ["{\"n\":1}", "{\"n\":2}"]);
        assert_eq!(chunk.end, 16);

        // Nothing new: nothing read, the offset stays.
        let again = read(&place, None, &job(&file, chunk.end)).outcome.unwrap();
        assert!(again.lines.is_empty());
        assert_eq!(again.end, 16);

        std::fs::OpenOptions::new()
            .append(true)
            .open(&file)
            .unwrap()
            .write_all(b"}\n\n{\"n\":4}\n")
            .unwrap();
        let more = read(&place, None, &job(&file, chunk.end)).outcome.unwrap();
        assert_eq!(
            more.lines,
            ["{\"n\":3}", "{\"n\":4}"],
            "the incomplete line is read again, whole"
        );
    }

    #[test]
    fn a_missing_or_shortened_file_is_a_reason_not_a_panic() {
        let dir = tempfile::tempdir().unwrap();
        let place = location(dir.path());
        let missing = dir.path().join("none.jsonl");
        let done = read(&place, None, &job(&missing, 0));
        assert_eq!(
            done.outcome.unwrap_err(),
            "the transcript file was not found"
        );

        let file = dir.path().join("a.jsonl");
        std::fs::write(&file, "{}\n").unwrap();
        assert_eq!(
            read(&place, None, &job(&file, 99)).outcome.unwrap_err(),
            "the transcript was shortened"
        );

        let mut unresolved = job(&file, 0);
        unresolved.file = None;
        unresolved.id = "../escape".into();
        let done = read(&place, Some(file.clone()), &unresolved);
        assert!(done.outcome.is_err() && done.file.is_none());
    }
}
