//! `weaver job new | show | list | edit`. `job show` without an `<id>` reads the environment and the
//! registry record of the job; the others need the job registry of the app, which runs them
//! through [`crate::exec::Authority::run`].

use serde_json::{json, Map, Value};
use weaver_core::command::CommandReply;
use weaver_core::jobs::JobRecord;

use super::{app_only, in_app, reply};
use crate::spec::*;

pub fn commands() -> Vec<Command> {
    vec![
        Command::new("job", "new", "Starts a job of Weaver.", app_only)
            .notes(&[
                "`design` needs `request` (a written, unsent, non-empty design request), `scan` covers the whole System, `run` takes one `task` (or runs all the runnable Tasks without it) and `resolve` needs `drift`. Weaver applies the developer's refusals. Run it only when the developer explicitly asked for that job in the conversation.",
            ])
            .access(Access::General)
            .input(vec![
                req("kind", one_of(&["design", "scan", "run", "resolve"]), "The kind of job."),
                opt("request", line(), "The design request; with `design`."),
                opt("task", line(), "The Task to run; with `run`."),
                opt("drift", line(), "The Drift to resolve; with `resolve`."),
            ])
            .output(vec![
                req("id", line(), "The job identity."),
                req("kind", line(), "The kind of job."),
            ])
            .check(check_new)
            .place(in_app)
            .example(r#"weaver job new --input '{"kind":"scan"}'"#),
        Command::new("job", "show", "Prints the job of the caller, or the job `<id>`.", show)
            .notes(&[
                "Without `<id>` it reads only the environment and the registry record of the job of the caller: `id`, `kind`, `target`, `merged` (only for a scan after a merge: the files the merge brought, relative to the project root, sorted), whether the session is a Run (`WEAVER_RUN`) and its `task` (`WEAVER_TASK`); outside a job it says so, with exit 0. `<id>` asks Weaver.",
            ])
            .arg(Arg::Job)
            .output(vec![
                req("job", nullable(object(vec![
                    req("id", line(), "The job identity."),
                    req("kind", line(), "The kind of job."),
                    req("target", nullable(line()), "What it works on."),
                    req("run", boolean(), "Whether the session is a Run."),
                    req("task", nullable(line()), "The Task of a Run of one Task."),
                    opt("merged", list(line()), "The files a merge brought; only for a scan after a merge."),
                ])), "The job, or null outside a job."),
            ])
            .place(|_, arg| if arg.is_some() { Place::App } else { Place::Local })
            .example("weaver job show"),
        Command::new("job", "list", "Lists the jobs of the project.", app_only)
            .input(vec![opt("all", boolean(), "Every job, finished ones included.")])
            .output(vec![req("jobs", list(object(vec![
                req("id", line(), "The job identity."),
                req("kind", line(), "The kind of job."),
                req("state", line(), "The state of the job."),
            ])), "The jobs.")])
            .place(in_app)
            .example("weaver job list"),
        Command::new("job", "edit", "Declares what the job is doing now, as a short note.", app_only)
            .notes(&[
                "Line breaks become spaces and the ends are trimmed; an empty note is invalid input. A note over 120 characters is cut to 119 plus `…`, and the output says so. The last 6 notes are kept with their instants; a repeat of the last one only renews its instant.",
            ])
            .access(Access::Job)
            .input(vec![req("note", nonblank(), "What the job is doing.")])
            .output(vec![
                req("note", line(), "The note as kept."),
                req("cut", boolean(), "Whether the note was cut."),
            ])
            .place(in_app)
            .example(r#"weaver job edit --input '{"note":"Reading the order Machine"}'"#),
    ]
}

/// The fields of `job new` that go with each kind.
fn check_new(input: &Map<String, Value>) -> Vec<Problem> {
    let kind = input
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let mut problems = Vec::new();
    let mut field = |name: &str, with: &str, required: bool| {
        let given = input.contains_key(name);
        if kind == with {
            if required && !given {
                problems.push(Problem::new(
                    name,
                    format!("is required with kind `{kind}`"),
                ));
            }
        } else if given {
            problems.push(Problem::new(
                name,
                format!("does not go with kind `{kind}`"),
            ));
        }
    };
    field("request", "design", true);
    field("drift", "resolve", true);
    field("task", "run", false);
    problems
}

/// The job as `job show` prints it: what the record says and what the environment says about the
/// session.
pub fn job_view(record: &JobRecord, run: bool, task: Option<&str>) -> CommandReply {
    let target = record.target.as_deref();
    let mut text = format!(
        "# Job `{}`\n\n- kind: `{}`\n",
        record.id,
        record.kind.as_str()
    );
    if let Some(target) = target {
        text.push_str(&format!("- target: `{target}`\n"));
    }
    if let Some(merged) = &record.merged {
        text.push_str("- merged:\n");
        for path in merged {
            text.push_str(&format!("  - `{path}`\n"));
        }
    }
    text.push_str(&format!("- run: {}\n", if run { "yes" } else { "no" }));
    if let Some(task) = task {
        text.push_str(&format!("- task: `{task}`\n"));
    }
    let mut job = json!({ "id": record.id, "kind": record.kind.as_str(), "target": target, "run": run, "task": task });
    if let Some(merged) = &record.merged {
        job["merged"] = json!(merged);
    }
    reply(text, json!({ "job": job }))
}

/// `job show` without an `<id>` reads the environment and the registry record of the job.
fn show(call: &Call) -> Result<CommandReply, Failure> {
    if call.arg.is_some() {
        return app_only(call);
    }
    let Some(job) = call.caller.job.as_deref() else {
        return Ok(reply(
            "Not running inside a Weaver job.\n".to_owned(),
            json!({ "job": null }),
        ));
    };
    let Some(record) = call.caller.job_record.as_ref() else {
        return Err(Failure::refused(format!(
            "the registry of Weaver has no readable record of the job {job}"
        )));
    };
    Ok(job_view(
        record,
        call.caller.run.is_some(),
        call.caller.task.as_deref(),
    ))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use weaver_core::jobs::{JobKind, JobState};
    use weaver_core::project::image::ProjectImage;

    use super::*;

    fn run(caller: &Caller, arg: Option<&str>) -> Result<CommandReply, Failure> {
        let commands = commands();
        let command = commands.iter().find(|c| c.verb == "show").unwrap();
        let image = ProjectImage::empty(Path::new("/p"));
        let input = Map::new();
        let call = Call {
            command,
            root: Path::new("/p"),
            image: &image,
            arg,
            state: None,
            input: &input,
            caller,
        };
        (command.run)(&call)
    }

    fn record() -> JobRecord {
        JobRecord {
            id: "jjjjjjjj".into(),
            kind: JobKind::Run,
            name: "Run T1".into(),
            project: "/p".into(),
            target: Some("aaaaaaaa".into()),
            merged: None,
            state: JobState::Running,
            reason: None,
            created: "2026-11-02T07:00:00.000Z".into(),
            started: None,
            ended: None,
            session_id: None,
            omp: String::new(),
            args: Vec::new(),
            env: Default::default(),
            announcements: Vec::new(),
            notes: Vec::new(),
            pid: None,
            pid_started: None,
            empty_model_at_start: false,
            overlay: String::new(),
        }
    }

    #[test]
    fn outside_a_job_it_says_so_with_a_null_job() {
        let reply = run(&Caller::default(), None).unwrap();
        assert_eq!(reply.markdown, "Not running inside a Weaver job.\n");
        assert_eq!(reply.json, json!({ "job": null }));
        assert_eq!(reply.exit, 0);
    }

    #[test]
    fn inside_a_job_it_prints_the_record_and_the_environment_only() {
        let caller = Caller {
            job: Some("jjjjjjjj".into()),
            run: Some("1".into()),
            task: Some("aaaaaaaa".into()),
            job_record: Some(record()),
            ..Caller::default()
        };
        let reply = run(&caller, None).unwrap();
        assert_eq!(
            reply.json,
            json!({ "job": { "id": "jjjjjjjj", "kind": "run", "target": "aaaaaaaa", "run": true, "task": "aaaaaaaa" } })
        );
        assert!(
            reply.markdown.contains("- kind: `run`")
                && reply.markdown.contains("- task: `aaaaaaaa`")
        );

        let design = Caller {
            job: Some("jjjjjjjj".into()),
            job_record: Some(JobRecord {
                kind: JobKind::Design,
                target: None,
                ..record()
            }),
            ..Caller::default()
        };
        assert_eq!(
            run(&design, None).unwrap().json,
            json!({ "job": { "id": "jjjjjjjj", "kind": "design", "target": null, "run": false, "task": null } })
        );
    }

    #[test]
    fn a_scan_after_a_merge_shows_the_files_the_merge_brought() {
        let caller = Caller {
            job: Some("jjjjjjjj".into()),
            job_record: Some(JobRecord {
                kind: JobKind::Scan,
                target: None,
                merged: Some(vec!["src/a.rs".into(), "src/b/c.rs".into()]),
                ..record()
            }),
            ..Caller::default()
        };
        let reply = run(&caller, None).unwrap();
        assert_eq!(
            reply.json,
            json!({ "job": { "id": "jjjjjjjj", "kind": "scan", "target": null, "run": false, "task": null, "merged": ["src/a.rs", "src/b/c.rs"] } })
        );
        assert!(reply
            .markdown
            .contains("- merged:\n  - `src/a.rs`\n  - `src/b/c.rs`\n"));
    }

    #[test]
    fn a_job_without_a_readable_record_is_refused_and_an_id_needs_weaver() {
        let caller = Caller {
            job: Some("jjjjjjjj".into()),
            ..Caller::default()
        };
        assert_eq!(
            run(&caller, None).unwrap_err().code,
            weaver_core::command::code::REFUSED
        );
        assert_eq!(
            run(&caller, Some("kkkkkkkk")).unwrap_err().code,
            weaver_core::command::code::APP_NOT_RUNNING
        );
    }

    #[test]
    fn the_fields_of_job_new_go_with_their_kind() {
        let check = |input: Value| check_new(input.as_object().unwrap());
        assert!(check(json!({ "kind": "scan" })).is_empty());
        assert!(check(json!({ "kind": "run" })).is_empty());
        assert!(check(json!({ "kind": "run", "task": "t3" })).is_empty());
        assert!(check(json!({ "kind": "design", "request": "r" })).is_empty());
        assert!(check(json!({ "kind": "resolve", "drift": "d1" })).is_empty());
        let paths = |input: Value| check(input).into_iter().map(|p| p.path).collect::<Vec<_>>();
        assert_eq!(paths(json!({ "kind": "design" })), ["request"]);
        assert_eq!(paths(json!({ "kind": "resolve" })), ["drift"]);
        assert_eq!(
            paths(json!({ "kind": "scan", "request": "r", "drift": "d" })),
            ["request", "drift"]
        );
        assert_eq!(
            paths(json!({ "kind": "design", "request": "r", "task": "t" })),
            ["task"]
        );
    }
}
