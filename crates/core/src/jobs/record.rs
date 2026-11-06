//! The registry record of a job, `<app local data>/jobs/<job id>.json` (`requirements/data#job`,
//! `requirements/tech#job`).
//!
//! It holds what Weaver needs to list the job and to resume its session: no conversation text
//! (omp keeps it in its own session files). It is written atomically at every change of state.
//! Records of finished jobs stay for the Jobs list, the 50 most recent per project.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::kinds::{JobKind, JobState};
use super::notes::Note;
use crate::atomic;
use crate::paths::AppDirs;
use crate::project::clock::parse_instant;

/// How many finished records the registry keeps per project.
pub const KEPT_FINISHED: usize = 50;

/// The card a job filed with an announcement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FiledCard {
    /// `proposal`, `task` or `drift`.
    pub kind: String,
    /// The permanent identity of the card.
    pub id: String,
}

/// A card the job announced. It waits until its card is on the Board and is withdrawn, with the
/// others, when the job ends.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnnouncementRecord {
    pub id: String,
    /// `proposal`, `task` or `drift`.
    pub kind: String,
    pub title: String,
    /// The card filed with this announcement, written when it is created, so that after a
    /// restart Weaver still knows which announced cards already arrived.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card: Option<FiledCard>,
}

/// One job, as the registry keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobRecord {
    pub id: String,
    pub kind: JobKind,
    /// The fixed name set at creation (`Run T3`, `All tasks`, `Resolve D3`...).
    pub name: String,
    /// The canonical project root.
    pub project: String,
    /// What the job works on, when it works on one thing: a request, a Task, a Drift or a Proposal
    /// identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// Only for a scan after a merge: the files the merge brought (`requirements/rules#solo-merge`),
    /// relative to the project root with `/`, sorted. `weaver job show` reads them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merged: Option<Vec<String>>,
    pub state: JobState,
    /// Why a Failed job failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// UTC ISO 8601 instants.
    pub created: String,
    /// When the session started (`omp.SESSION_STARTED`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended: Option<String>,
    /// omp's ACP session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// The program path of omp.
    #[serde(default)]
    pub omp: String,
    /// The arguments and the Weaver-set environment omp was started with; never inherited variables.
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub announcements: Vec<AnnouncementRecord>,
    #[serde(default)]
    pub notes: Vec<Note>,
    /// The pid of the omp process and its start time, to end a process left by a crash.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid_started: Option<String>,
    /// The Model had no Feature and no Actor when a setup or an import job started running (the
    /// job that found it empty may hand a Proposal over to be approved by itself).
    #[serde(default)]
    pub empty_model_at_start: bool,
    /// The text of the overlay file omp was started with, to write it again when the session is
    /// reloaded.
    #[serde(default)]
    pub overlay: String,
    /// Only for a measured job: the time spent in Running (`requirements/data#job`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub running: Option<RunningTime>,
    /// Weaver was found stopped unexpectedly while the job was Running or Waiting for the
    /// developer: the time since its last change of state is unknown and never counted.
    #[serde(default, skip_serializing_if = "is_false")]
    pub running_incomplete: bool,
}

fn is_false(value: &bool) -> bool {
    !*value
}

/// The time a measured job has spent in Running, so that a job suspended and resumed goes on
/// counting.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunningTime {
    /// The milliseconds spent in Running before `since`.
    pub ms: u64,
    /// The instant the job entered Running, while it is Running.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
}

impl JobRecord {
    /// The project hash of the record's project (the label of its window is `p-<hash>`).
    pub fn project_hash(&self) -> String {
        crate::paths::project_hash_of_canonical(Path::new(&self.project))
    }

    /// `WEAVER_TASK` of a Run of one Task.
    pub fn run_task(&self) -> Option<&str> {
        self.env.get("WEAVER_TASK").map(String::as_str)
    }

    /// Whether the session is a Run (`WEAVER_RUN`).
    pub fn is_run(&self) -> bool {
        self.env.contains_key("WEAVER_RUN")
    }

    /// The latest note, when there is one.
    pub fn latest_note(&self) -> Option<&Note> {
        self.notes.last()
    }

    /// The job enters Running at `now`: for a measured job, the time starts to count.
    pub fn entered_running(&mut self, now: &str) {
        if self.kind.is_measured() {
            self.running.get_or_insert_with(RunningTime::default).since = Some(now.to_owned());
        }
    }

    /// The job leaves Running at `now`: the time since it entered is added. When that time cannot
    /// be told, it is not counted and the record says so.
    pub fn left_running(&mut self, now: &str) {
        let Some(running) = self.running.as_mut() else {
            return;
        };
        let Some(since) = running.since.take() else {
            return;
        };
        match (parse_instant(&since), parse_instant(now)) {
            (Some(from), Some(to)) => {
                running.ms = running.ms.saturating_add(u64::try_from(to - from).unwrap_or(0));
            }
            _ => self.running_incomplete = true,
        }
    }

    /// Weaver was found stopped unexpectedly while the job was Running or Waiting for the
    /// developer: what it counted so far stays, and the time since its last change of state is
    /// never counted.
    pub fn running_interrupted(&mut self) {
        if !self.kind.is_measured() {
            return;
        }
        if let Some(running) = self.running.as_mut() {
            running.since = None;
        }
        self.running_incomplete = true;
    }
}

/// `<app local data>/jobs`.
pub fn jobs_dir(dirs: &AppDirs) -> PathBuf {
    dirs.local_data_dir().join("jobs")
}

/// `<app local data>/overlays`.
pub fn overlays_dir(dirs: &AppDirs) -> PathBuf {
    dirs.local_data_dir().join("overlays")
}

/// `<app local data>/overlays/<job id>.yml`.
pub fn overlay_file(dirs: &AppDirs, job: &str) -> PathBuf {
    overlays_dir(dirs).join(format!("{job}.yml"))
}

/// The records of a read of the registry: the ones that could be read and, for each other file, its
/// path and why it could not.
#[derive(Debug, Default)]
pub struct Registry {
    pub records: Vec<JobRecord>,
    pub unreadable: Vec<(PathBuf, String)>,
}

/// The registry folder.
#[derive(Debug, Clone)]
pub struct JobStore {
    dir: PathBuf,
}

impl JobStore {
    pub fn new(dirs: &AppDirs) -> Self {
        Self::in_dir(jobs_dir(dirs))
    }

    pub fn in_dir(dir: PathBuf) -> Self {
        Self { dir }
    }

    pub fn file(&self, job: &str) -> PathBuf {
        self.dir.join(format!("{job}.json"))
    }

    /// Writes (atomically) the record of the job.
    pub fn write(&self, record: &JobRecord) -> io::Result<()> {
        let mut json = serde_json::to_vec_pretty(record).map_err(io::Error::other)?;
        json.push(b'\n');
        atomic::write(&self.file(&record.id), &json, Some(0o600))
    }

    /// The record of `job`; `Ok(None)` when there is none.
    pub fn read(&self, job: &str) -> io::Result<Option<JobRecord>> {
        match std::fs::read(self.file(job)) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map(Some)
                .map_err(io::Error::other),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    pub fn delete(&self, job: &str) -> io::Result<()> {
        match std::fs::remove_file(self.file(job)) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
            _ => Ok(()),
        }
    }

    /// Every record. A file that cannot be read or parsed is reported and left alone: its job
    /// counts as ended.
    pub fn read_all(&self) -> Registry {
        let mut registry = Registry::default();
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return registry;
        };
        let mut paths: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|e| e == "json"))
            .collect();
        paths.sort();
        for path in paths {
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .to_owned();
            match std::fs::read(&path)
                .map_err(|e| e.to_string())
                .and_then(|b| serde_json::from_slice::<JobRecord>(&b).map_err(|e| e.to_string()))
            {
                Ok(record) if record.id == stem => registry.records.push(record),
                Ok(record) => registry.unreadable.push((
                    path,
                    format!("it holds the job `{}`, not `{stem}`", record.id),
                )),
                Err(reason) => registry.unreadable.push((path, reason)),
            }
        }
        registry
    }

    /// Deletes the records of the finished jobs of `project` beyond the [`KEPT_FINISHED`] most
    /// recent ones. Returns the identities deleted.
    pub fn prune_finished(&self, project: &str) -> io::Result<Vec<String>> {
        let registry = self.read_all();
        let mut deleted = Vec::new();
        for id in finished_to_prune(&registry.records, project, KEPT_FINISHED) {
            self.delete(&id)?;
            deleted.push(id);
        }
        Ok(deleted)
    }
}

/// The identities of the finished jobs of `project` that are not among its `keep` most recent
/// ones (by end instant, then identity).
pub fn finished_to_prune(records: &[JobRecord], project: &str, keep: usize) -> Vec<String> {
    let mut finished: Vec<&JobRecord> = records
        .iter()
        .filter(|r| r.project == project && r.state.is_final())
        .collect();
    finished.sort_by(|a, b| b.ended.cmp(&a.ended).then_with(|| b.id.cmp(&a.id)));
    finished
        .into_iter()
        .skip(keep)
        .map(|r| r.id.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn record(
        id: &str,
        project: &str,
        state: JobState,
        ended: Option<&str>,
    ) -> JobRecord {
        JobRecord {
            id: id.to_owned(),
            kind: JobKind::Scan,
            name: "Scan".to_owned(),
            project: project.to_owned(),
            target: None,
            merged: None,
            state,
            reason: None,
            created: "2026-11-02T07:00:00.000Z".to_owned(),
            started: None,
            ended: ended.map(str::to_owned),
            session_id: Some("s1".to_owned()),
            omp: "/bin/omp".to_owned(),
            args: vec!["acp".to_owned()],
            env: BTreeMap::from([
                ("WEAVER_JOB".to_owned(), id.to_owned()),
                ("WEAVER_RUN".to_owned(), "1".to_owned()),
            ]),
            announcements: vec![AnnouncementRecord {
                id: "aaaaaaaa".into(),
                kind: "task".into(),
                title: "A task".into(),
                card: Some(FiledCard {
                    kind: "task".into(),
                    id: "bbbbbbbb".into(),
                }),
            }],
            notes: vec![Note {
                text: "Reading".into(),
                at: "2026-11-02T07:01:00.000Z".into(),
            }],
            pid: Some(12),
            pid_started: Some("Mon Nov  2 07:00:00 2026".to_owned()),
            empty_model_at_start: false,
            overlay: "bash:\n".to_owned(),
            running: None,
            running_incomplete: false,
        }
    }

    #[test]
    fn a_record_is_written_atomically_and_read_back_whole() {
        let dir = tempfile::tempdir().unwrap();
        let store = JobStore::in_dir(dir.path().join("jobs"));
        let record = record("jjjjjjjj", "/p", JobState::Running, None);
        store.write(&record).unwrap();
        assert_eq!(store.read("jjjjjjjj").unwrap(), Some(record.clone()));
        assert!(record.is_run() && record.run_task().is_none());
        let text = std::fs::read_to_string(store.file("jjjjjjjj")).unwrap();
        assert!(text.contains("\"sessionId\": \"s1\"") && text.contains("\"pidStarted\""));
        assert!(text.contains("\"card\"") && text.contains("\"bbbbbbbb\""));
        assert!(!text.contains("conversation"));
        store.delete("jjjjjjjj").unwrap();
        assert_eq!(store.read("jjjjjjjj").unwrap(), None);
        store.delete("jjjjjjjj").unwrap();
    }

    #[test]
    fn a_record_that_cannot_be_read_is_reported_and_the_others_still_load() {
        let dir = tempfile::tempdir().unwrap();
        let store = JobStore::in_dir(dir.path().to_path_buf());
        store
            .write(&record(
                "aaaaaaaa",
                "/p",
                JobState::Done,
                Some("2026-11-02T08:00:00.000Z"),
            ))
            .unwrap();
        std::fs::write(dir.path().join("bbbbbbbb.json"), "{ nope").unwrap();
        std::fs::write(
            dir.path().join("cccccccc.json"),
            serde_json::to_string(&record("dddddddd", "/p", JobState::Done, None)).unwrap(),
        )
        .unwrap();
        let registry = store.read_all();
        assert_eq!(registry.records.len(), 1);
        assert_eq!(registry.unreadable.len(), 2);
        assert!(registry.unreadable[1].1.contains("not `cccccccc`"));
        assert!(
            dir.path().join("bbbbbbbb.json").exists(),
            "an unreadable record is never rewritten or deleted"
        );
    }

    #[test]
    fn only_the_most_recent_finished_records_of_a_project_are_kept() {
        let records: Vec<JobRecord> = (0..5)
            .map(|n| {
                record(
                    &format!("done000{n}"),
                    "/p",
                    JobState::Done,
                    Some(&format!("2026-11-02T08:0{n}:00.000Z")),
                )
            })
            .chain([
                record(
                    "other000",
                    "/q",
                    JobState::Done,
                    Some("2026-11-02T01:00:00.000Z"),
                ),
                record("running0", "/p", JobState::Running, None),
                record("suspend0", "/p", JobState::Suspended, None),
            ])
            .collect();
        assert_eq!(
            finished_to_prune(&records, "/p", 3),
            ["done0001", "done0000"]
        );
        assert!(finished_to_prune(&records, "/p", 50).is_empty());
        assert!(finished_to_prune(&records, "/q", 0) == ["other000"]);
    }

    #[test]
    fn prune_deletes_the_older_files() {
        let dir = tempfile::tempdir().unwrap();
        let store = JobStore::in_dir(dir.path().to_path_buf());
        for n in 0..(KEPT_FINISHED + 2) {
            let id = format!("job{n:05}");
            store
                .write(&record(
                    &id,
                    "/p",
                    JobState::Stopped,
                    Some(&format!("2026-11-02T07:{:02}:{:02}.000Z", n / 60, n % 60)),
                ))
                .unwrap();
        }
        store
            .write(&record("running0", "/p", JobState::Running, None))
            .unwrap();
        let deleted = store.prune_finished("/p").unwrap();
        assert_eq!(deleted, ["job00001", "job00000"]);
        assert_eq!(store.read_all().records.len(), KEPT_FINISHED + 1);
    }

    fn measured(kind: JobKind) -> JobRecord {
        JobRecord {
            kind,
            ..record("jjjjjjjj", "/p", JobState::Queued, None)
        }
    }

    #[test]
    fn the_time_in_running_is_added_at_each_stay_and_a_resumed_job_goes_on_counting() {
        let mut record = measured(JobKind::Run);
        record.entered_running("2026-11-02T07:00:10.000Z");
        assert_eq!(
            record.running,
            Some(RunningTime {
                ms: 0,
                since: Some("2026-11-02T07:00:10.000Z".into())
            })
        );
        record.left_running("2026-11-02T07:00:15.250Z");
        // Waiting, Suspended and Queued time is never counted: only the next stay is added.
        record.entered_running("2026-11-02T07:10:00.000Z");
        record.left_running("2026-11-02T07:10:02.000Z");
        let running = record.running.clone().unwrap();
        assert_eq!((running.ms, running.since), (7250, None));
        assert!(!record.running_incomplete);
        // Leaving Running twice counts nothing more.
        record.left_running("2026-11-02T08:00:00.000Z");
        assert_eq!(record.running.unwrap().ms, 7250);
    }

    #[test]
    fn a_stop_of_weaver_while_running_keeps_what_was_counted_and_marks_the_record() {
        let mut record = measured(JobKind::Design);
        record.entered_running("2026-11-02T07:00:00.000Z");
        record.left_running("2026-11-02T07:00:03.000Z");
        record.entered_running("2026-11-02T07:05:00.000Z");
        record.running_interrupted();
        let running = record.running.clone().unwrap();
        assert_eq!((running.ms, running.since), (3000, None));
        assert!(record.running_incomplete);
        let text = serde_json::to_string(&record).unwrap();
        assert!(text.contains("\"running\":{\"ms\":3000}") && text.contains("\"runningIncomplete\":true"));
    }

    #[test]
    fn only_measured_kinds_count_their_time() {
        for kind in JobKind::ALL {
            let mut record = measured(kind);
            record.entered_running("2026-11-02T07:00:00.000Z");
            record.left_running("2026-11-02T07:00:01.000Z");
            record.running_interrupted();
            let measured = matches!(
                kind,
                JobKind::Design | JobKind::Rework | JobKind::Refinement | JobKind::Run
            );
            assert_eq!(kind.is_measured(), measured, "{kind:?}");
            assert_eq!(record.running.is_some(), measured, "{kind:?}");
            assert_eq!(record.running_incomplete, measured, "{kind:?}");
            let text = serde_json::to_string(&record).unwrap();
            assert_eq!(text.contains("running\""), measured, "{kind:?}");
        }
    }
}
