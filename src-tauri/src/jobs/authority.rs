//! What the app answers to the `weaver` commands that need its own state: who may run a command
//! (a working job, the working General), the commands of the jobs themselves (`job new | show |
//! list | edit`, `announcement new`), and the hooks other parts plug into.
//!
//! [`AppAuthority`] implements [`weaver_cli::exec::Authority`]; the socket handler hands it to
//! `exec::execute`, which calls it after every check of the command and inside the project's
//! queue.

use std::path::Path;
use std::sync::{Arc, RwLock};

use serde_json::{json, Value};
use weaver_cli::exec::Authority;
use weaver_cli::spec::{Call, Failure};
use weaver_core::command::CommandReply;
use weaver_core::jobs::JobRecord;
use weaver_core::paths::project_hash_of_canonical;
use weaver_core::project::board::AgentFacts;

use super::registry::Jobs;
use super::starts::{Ctx, StartGate};

/// Who may use the commands only the working General's agent or a working Ask more conversation's
/// agent may (`job new`). Installed by the General (`Jobs::set_general`).
pub trait GeneralAuthority: Send + Sync {
    /// Whether `token` is the identity of the working General of the project at `root`.
    fn accepts(&self, root: &Path, token: &str) -> bool;

    /// Whether `token` is the identity of a working Ask more conversation of the project at
    /// `root`.
    fn accepts_ask(&self, root: &Path, token: &str) -> bool;

    /// `agent.START_JOB`: the agent of the General or of the Ask more conversation `token` asked
    /// for a job of `kind` (the app
    /// already accepted `token`). `outcome` is the jobs that started, as `(identity, name)`, or the
    /// reason none did. Called right after the start, on the thread of the command.
    fn job_requested(
        &self,
        root: &Path,
        token: &str,
        kind: &str,
        outcome: Result<&[(String, String)], &str>,
    );
}

/// A command only the app can answer, added by the part of the app that owns it (`question new`,
/// the commands of the Proposal and the Drift that start jobs...). Asked in the order installed,
/// after the checks of the command and inside the project's queue; `None` leaves it to the next.
pub trait CommandExtension: Send + Sync {
    fn run(&self, call: &Call) -> Option<Result<CommandReply, Failure>>;
}

/// `weaver job new design`: sends a written design request (the design request's own rules
/// decide whether it can go). Installed by the design requests.
pub trait RequestSender: Send + Sync {
    /// Sends the request and returns the identity of the job that works on it.
    fn send(&self, project_id: &str, root: &Path, request: &str) -> Result<String, String>;
}

/// The hooks of the jobs.
#[derive(Default)]
pub struct Hooks {
    pub(crate) general: RwLock<Option<Arc<dyn GeneralAuthority>>>,
    pub(crate) extensions: RwLock<Vec<Arc<dyn CommandExtension>>>,
    pub(crate) requests: RwLock<Option<Arc<dyn RequestSender>>>,
    pub(crate) gates: RwLock<Vec<Arc<dyn StartGate>>>,
}

impl Jobs {
    pub fn set_general(&self, general: Arc<dyn GeneralAuthority>) {
        *self.hooks.general.write().expect("hooks poisoned") = Some(general);
    }

    pub fn add_extension(&self, extension: Arc<dyn CommandExtension>) {
        self.hooks
            .extensions
            .write()
            .expect("hooks poisoned")
            .push(extension);
    }

    pub fn set_request_sender(&self, sender: Arc<dyn RequestSender>) {
        *self.hooks.requests.write().expect("hooks poisoned") = Some(sender);
    }
}

/// The authority of the running app.
pub struct AppAuthority {
    pub jobs: Arc<Jobs>,
}

fn project_of(root: &Path) -> String {
    project_hash_of_canonical(root)
}

impl Authority for AppAuthority {
    fn accepts_job(&self, root: &Path, job: &str) -> bool {
        self.jobs.accepts_job(root, job)
    }

    fn accepts_general(&self, root: &Path, token: &str) -> bool {
        self.jobs
            .hooks
            .general
            .read()
            .expect("hooks poisoned")
            .as_ref()
            .is_some_and(|general| general.accepts(root, token))
    }

    fn accepts_ask(&self, root: &Path, token: &str) -> bool {
        self.jobs
            .hooks
            .general
            .read()
            .expect("hooks poisoned")
            .as_ref()
            .is_some_and(|general| general.accepts_ask(root, token))
    }

    fn facts(&self, root: &Path) -> AgentFacts {
        self.jobs.facts_of(&project_of(root))
    }

    fn run(&self, call: &Call) -> Option<Result<CommandReply, Failure>> {
        let command = call.command;
        match (command.object, command.verb) {
            ("job", "new") => Some(job_new(&self.jobs, call)),
            ("job", "list") => Some(job_list(&self.jobs, call)),
            ("job", "edit") => Some(job_edit(&self.jobs, call)),
            ("job", "show") if call.arg.is_some() => Some(job_show(&self.jobs, call)),
            ("announcement", "new") => Some(announcement_new(&self.jobs, call)),
            _ => {
                let extensions: Vec<Arc<dyn CommandExtension>> = self
                    .jobs
                    .hooks
                    .extensions
                    .read()
                    .expect("hooks poisoned")
                    .clone();
                extensions.iter().find_map(|extension| extension.run(call))
            }
        }
    }

    fn card_filed(
        &self,
        _root: &Path,
        job: Option<&str>,
        kind: &str,
        card: &str,
        announcement: &str,
    ) -> Option<String> {
        self.jobs.card_filed(job, kind, card, announcement)
    }

    fn task_finished(&self, root: &Path) {
        self.jobs.task_finished(root);
    }
}

fn reply(markdown: String, json: Value) -> CommandReply {
    CommandReply {
        markdown,
        json,
        exit: 0,
        note: None,
    }
}

fn job_id<'a>(call: &'a Call<'_>) -> Result<&'a str, Failure> {
    call.caller.job.as_deref().ok_or_else(|| {
        Failure::new(
            weaver_core::command::code::FORBIDDEN,
            "this command is for the agent of a job",
        )
    })
}

fn text<'a>(call: &'a Call<'_>, field: &str) -> &'a str {
    call.input
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or_default()
}

fn announcement_new(jobs: &Jobs, call: &Call) -> Result<CommandReply, Failure> {
    let id = jobs
        .announce(job_id(call)?, text(call, "kind"), text(call, "title"))
        .map_err(Failure::refused)?;
    Ok(reply(
        format!("Announced. Pass `{id}` as `announcement` when you create the card.\n"),
        json!({ "id": id }),
    ))
}

fn job_edit(jobs: &Jobs, call: &Call) -> Result<CommandReply, Failure> {
    let kept = jobs
        .declare_note(job_id(call)?, text(call, "note"))
        .map_err(Failure::refused)?;
    let mut markdown = format!("Note: {}\n", kept.text);
    if kept.cut {
        markdown.push_str("The note was over 120 characters and was cut to 119 plus `…`.\n");
    }
    Ok(reply(
        markdown,
        json!({ "note": kept.text, "cut": kept.cut }),
    ))
}

fn describe(record: &JobRecord) -> String {
    format!(
        "- `{}` {} · {} · {}\n",
        record.id,
        record.name,
        record.kind.as_str(),
        record.state.as_str()
    )
}

fn job_list(jobs: &Jobs, call: &Call) -> Result<CommandReply, Failure> {
    let all = call
        .input
        .get("all")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let records = jobs.list_for(call.root, all);
    let mut markdown = String::from("# Jobs\n\n");
    if records.is_empty() {
        markdown.push_str("No jobs.\n");
    }
    for record in &records {
        markdown.push_str(&describe(record));
    }
    let list: Vec<Value> = records
        .iter()
        .map(|r| json!({ "id": r.id, "kind": r.kind.as_str(), "state": r.state.as_str() }))
        .collect();
    Ok(reply(markdown, json!({ "jobs": list })))
}

fn job_show(jobs: &Jobs, call: &Call) -> Result<CommandReply, Failure> {
    let given = call.arg.unwrap_or_default();
    let records = jobs.list_for(call.root, true);
    let mut found = records.iter().filter(|r| r.id.starts_with(given));
    let (Some(record), None) = (found.next(), found.next()) else {
        return Err(Failure::refused(format!(
            "no job of this project is named `{given}`, or the name is not unique"
        )));
    };
    let task = record.run_task().map(str::to_owned);
    let markdown = format!(
        "# Job `{}`\n\n- name: {}\n- kind: `{}`\n- state: `{}`\n",
        record.id,
        record.name,
        record.kind.as_str(),
        record.state.as_str()
    );
    let mut job = json!({ "id": record.id, "kind": record.kind.as_str(), "target": record.target, "run": record.is_run(), "task": task });
    if let Some(merged) = &record.merged {
        job["merged"] = json!(merged);
    }
    Ok(reply(markdown, json!({ "job": job })))
}

/// `job new`: only from the working General or a working Ask more conversation, with the
/// developer's refusals. The conversation that asked is told what happened (`agent.START_JOB`), a
/// refusal included.
fn job_new(jobs: &Jobs, call: &Call) -> Result<CommandReply, Failure> {
    let kind = text(call, "kind");
    let result = start_jobs(jobs, call, kind);
    let general = jobs.hooks.general.read().expect("hooks poisoned").clone();
    let token = call
        .caller
        .general
        .as_deref()
        .or(call.caller.ask.as_deref());
    if let (Some(general), Some(token)) = (general, token) {
        match &result {
            Ok(started) => {
                let named: Vec<(String, String)> = started
                    .iter()
                    .map(|id| {
                        (
                            id.clone(),
                            jobs.record(id)
                                .map(|record| record.name)
                                .unwrap_or_default(),
                        )
                    })
                    .collect();
                general.job_requested(call.root, token, kind, Ok(&named));
            }
            Err(failure) => general.job_requested(
                call.root,
                token,
                kind,
                Err(refusal_reason(&failure.message)),
            ),
        }
    }
    let started = result?;
    let first = started.first().cloned().unwrap_or_default();
    let markdown = if started.len() > 1 {
        format!("Started {} jobs. The first is `{first}`.\n", started.len())
    } else {
        format!("Started the job `{first}`.\n")
    };
    Ok(reply(markdown, json!({ "id": first, "kind": kind })))
}

const REFUSED_PREFIX: &str = "Could not start the job: ";

/// What the developer reads after `Weaver did not start <kind>:`: the refusal without its lead-in
/// and its final full stop.
fn refusal_reason(message: &str) -> &str {
    message
        .strip_prefix(REFUSED_PREFIX)
        .unwrap_or(message)
        .trim_end_matches('.')
}

/// The identities of the jobs `job new` started.
fn start_jobs(jobs: &Jobs, call: &Call, kind: &str) -> Result<Vec<String>, Failure> {
    let project_id = project_of(call.root);
    let ctx = Ctx {
        project_id: &project_id,
        root: call.root,
        image: call.image,
    };
    let refused = |refusal: weaver_core::jobs::admission::Refusal| {
        Failure::refused(format!("{REFUSED_PREFIX}{refusal}."))
    };
    Ok(match kind {
        "scan" => vec![jobs.start_scan(&ctx).map_err(refused)?],
        "run" => match call.input.get("task").and_then(Value::as_str) {
            Some(task) => {
                let task = weaver_core::project::resolve::resolve_card(
                    call.image,
                    weaver_core::project::cards::CardKind::Task,
                    task,
                )
                .map_err(|error| Failure::refused(error.to_string()))?;
                vec![jobs.start_run_task(&ctx, &task).map_err(refused)?]
            }
            None => jobs.start_run_all(&ctx).map_err(refused)?,
        },
        "resolve" => {
            let drift = weaver_core::project::resolve::resolve_card(
                call.image,
                weaver_core::project::cards::CardKind::Drift,
                text(call, "drift"),
            )
            .map_err(|error| Failure::refused(error.to_string()))?;
            vec![jobs.start_resolve(&ctx, &drift, None).map_err(refused)?]
        }
        "design" => {
            let sender = jobs.hooks.requests.read().expect("hooks poisoned").clone();
            let Some(sender) = sender else {
                return Err(Failure::refused(
                    "Design requests cannot be sent from here yet.",
                ));
            };
            vec![sender
                .send(&project_id, call.root, text(call, "request"))
                .map_err(Failure::refused)?]
        }
        other => {
            return Err(Failure::invalid(format!(
                "`{other}` is not a kind of job `job new` starts"
            )))
        }
    })
}
