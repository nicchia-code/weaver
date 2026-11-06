//! Running one command: the same function serves the CLI process (reads, and writes under the
//! project lock when no app answers) and the app (writes, in the project's queue).
//!
//! The caller holds the project lock for a write; this module never takes it. Order: the command
//! exists, the project is initialised, the input is valid, the caller may run it, the card is
//! named and found, then the handler runs.

use std::path::Path;

use weaver_core::project::board::AgentFacts;

use weaver_core::command::{code, CommandReply, CommandRequest};
use weaver_core::history_commit;
use weaver_core::project::image::ProjectImage;
use weaver_core::project::layout::{weaver_dir, WEAVER_DIR};
use weaver_core::project::resolve::resolve_card;
use weaver_core::solo_mode::Git;

use crate::registry;
use crate::spec::*;
use crate::validate::{describe_problems, validate};

/// Who vouches for the identities a command carries. The app answers from its job registry; a
/// CLI process that runs a command itself vouches for nobody.
pub trait Authority {
    /// Whether `job` is a job of the project at `root` that is Running or Waiting for the
    /// developer.
    fn accepts_job(&self, root: &Path, job: &str) -> bool;
    /// Whether `token` is the identity of the working General of the project at `root`.
    fn accepts_general(&self, root: &Path, token: &str) -> bool;
    /// Whether `token` is the identity of a working Ask more conversation of the project at
    /// `root`.
    fn accepts_ask(&self, _root: &Path, _token: &str) -> bool {
        false
    }
    /// The registry record of `job`, which `accepts_job` accepted: what a command needs to know
    /// about the job (its kind and what it works on). Nothing when no app answers.
    fn job_record(&self, _root: &Path, _job: &str) -> Option<weaver_core::jobs::JobRecord> {
        None
    }
    /// What the app knows about the agent's work in the project at `root`: which jobs work, which
    /// Run or resolve is in progress. Nothing when no app answers.
    fn facts(&self, _root: &Path) -> AgentFacts {
        AgentFacts::default()
    }
    /// Runs a command that needs the state of the app itself (the jobs, the rounds of questions),
    /// which the handlers of the registry cannot reach. `None` leaves the command to its handler.
    /// It is called after every check of [`execute`] and runs in the project's queue.
    fn run(&self, _call: &Call) -> Option<Result<CommandReply, Failure>> {
        None
    }
    /// A card was created with `announcement`: the ghost of the job waits for the card to appear.
    /// Returns a note for the agent when the announcement is unknown or another job's.
    fn card_filed(
        &self,
        _root: &Path,
        _job: Option<&str>,
        _kind: &str,
        _card: &str,
        _announcement: &str,
    ) -> Option<String> {
        None
    }
    /// A Task reached Done by `FINISHED`.
    fn task_finished(&self, _root: &Path) {}
}

/// An authority that accepts nothing: no app answers, or the app has no job to accept yet.
pub struct NoJobs;

impl Authority for NoJobs {
    fn accepts_job(&self, _root: &Path, _job: &str) -> bool {
        false
    }

    fn accepts_general(&self, _root: &Path, _token: &str) -> bool {
        false
    }
}

/// The failure of a command refused for its caller.
pub fn access_failure(command: &Command) -> Failure {
    let who = match command.access {
        Access::General => {
            "the agent of the General or of an Ask more conversation that Weaver started for this project"
        }
        _ => "the agent of a job that Weaver started",
    };
    Failure::new(
        code::FORBIDDEN,
        format!(
            "Only {who} can run `weaver {}`. Nothing was written.",
            command.name()
        ),
    )
}

/// `.weaver/` must exist in the project folder.
pub fn ensure_initialised(root: &Path) -> Result<(), Failure> {
    if weaver_dir(root).is_dir() {
        Ok(())
    } else {
        Err(Failure::new(
            code::NOT_INITIALISED,
            format!("{} has no `{WEAVER_DIR}` folder: initialise the project first with `weaver project new --input '{{}}'`.", root.display()),
        ))
    }
}

fn is_project_new(command: &Command) -> bool {
    command.object == "project" && command.verb == "new"
}

/// Checks the words of a request against its command: the argument and the state.
pub fn check_words(
    command: &Command,
    card: Option<&str>,
    state: Option<&str>,
) -> Result<(), Failure> {
    let usage = |message: String| Failure::new(code::USAGE, message);
    match (command.arg, card) {
        (Arg::Card(_), None) => {
            return Err(usage(format!(
                "`weaver {}` needs the card: {}",
                command.name(),
                command.usage()
            )))
        }
        (Arg::None, Some(extra)) => {
            return Err(usage(format!(
                "`weaver {}` takes no argument (found `{extra}`)",
                command.name()
            )))
        }
        _ => {}
    }
    match (command.states.is_empty(), state) {
        (false, None) => {
            return Err(usage(format!(
                "`weaver {}` needs the state: {}",
                command.name(),
                command.states.join(", ")
            )))
        }
        (false, Some(state)) if !command.states.contains(&state) => {
            return Err(usage(format!(
                "`{state}` is not a state of `weaver {}`: {}",
                command.name(),
                command.states.join(", ")
            )))
        }
        (true, Some(extra)) => {
            return Err(usage(format!(
                "`weaver {}` takes no state (found `{extra}`)",
                command.name()
            )))
        }
        _ => {}
    }
    Ok(())
}

/// Validates the input of `command`: the types, then the rules over the whole input.
pub fn check_input(
    command: &Command,
    input: &serde_json::Map<String, serde_json::Value>,
) -> Result<(), Failure> {
    let mut problems = validate(&command.input, input);
    if let Some(check) = command.check {
        problems.extend(check(input));
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(Failure::invalid(describe_problems(&problems)))
    }
}

/// Finishes the answer of a command that announces a commit: `history new` and `history edit`,
/// while Atomic commits is on, answer `commit.status` `pending` and leave the commit for after the
/// project's lock is released, so that a slow hook never holds the project. This makes the commit,
/// in the queue of the working copy, and puts how it ended in the answer (`commit` and the last
/// line of the text). Any other answer is returned as it is. Call it right after the lock is
/// released, and never on the thread of an interface.
pub fn settle(root: &Path, mut reply: CommandReply) -> CommandReply {
    history_commit::settle(&Git::system(), root, &mut reply.json, &mut reply.markdown);
    reply
}

/// Runs the command of `request` for `caller`.
pub fn execute(
    request: &CommandRequest,
    caller: &Caller,
    authority: &dyn Authority,
) -> Result<CommandReply, Failure> {
    let command = registry::find(&request.object, &request.verb).ok_or_else(|| {
        Failure::new(
            code::USAGE,
            format!(
                "`weaver {} {}` is not a command",
                request.object, request.verb
            ),
        )
    })?;
    let root = Path::new(&request.root);
    if !is_project_new(command) {
        ensure_initialised(root)?;
    }
    check_words(command, request.card.as_deref(), request.state.as_deref())?;
    check_input(command, &request.input)?;

    let allowed = match command.access {
        Access::Anyone => true,
        Access::Job => caller
            .job
            .as_deref()
            .is_some_and(|job| authority.accepts_job(root, job)),
        Access::JobOrUnavailable => {
            caller
                .job
                .as_deref()
                .is_some_and(|job| authority.accepts_job(root, job))
                || caller
                    .general
                    .as_deref()
                    .is_some_and(|token| authority.accepts_general(root, token))
        }
        Access::General => {
            caller
                .general
                .as_deref()
                .is_some_and(|token| authority.accepts_general(root, token))
                || caller
                    .ask
                    .as_deref()
                    .is_some_and(|token| authority.accepts_ask(root, token))
        }
    };
    if !allowed {
        return Err(access_failure(command));
    }

    let image = if weaver_dir(root).is_dir() {
        ProjectImage::load(root)
    } else {
        ProjectImage::empty(root)
    };
    let arg: Option<String> = match command.arg {
        Arg::None => None,
        Arg::Card(kind) => {
            let given = request.card.as_deref().expect("checked by check_words");
            Some(resolve_card(&image, kind, given).map_err(|e| Failure::refused(e.to_string()))?)
        }
        Arg::Reference | Arg::Job => request.card.clone(),
    };
    // The handler sees what the app knows about the agent's work, at the moment it runs, and who
    // the app vouches the caller is: an identity it does not accept counts as none.
    let who = if caller
        .job
        .as_deref()
        .is_some_and(|job| authority.accepts_job(root, job))
    {
        Who::Job
    } else if caller
        .general
        .as_deref()
        .is_some_and(|token| authority.accepts_general(root, token))
    {
        Who::Chat
    } else {
        Who::Other
    };
    let job_record = match (who, caller.job.as_deref()) {
        (Who::Job, Some(job)) => authority
            .job_record(root, job)
            .or_else(|| caller.job_record.clone()),
        _ => caller.job_record.clone(),
    };
    let caller = Caller {
        agent: authority.facts(root),
        who,
        job_record,
        ..caller.clone()
    };
    let call = Call {
        command,
        root,
        image: &image,
        arg: arg.as_deref(),
        state: request.state.as_deref(),
        input: &request.input,
        caller: &caller,
    };
    if let Some(result) = authority.run(&call) {
        return result;
    }
    let mut result = (command.run)(&call);
    if let Ok(reply) = result.as_mut() {
        after_success(request, &caller, authority, reply);
    }
    result
}

/// What the app learns from a command that wrote: the card a ghost was waiting for was filed (its
/// `announcement`), a Task reached Done by `FINISHED`.
fn after_success(
    request: &CommandRequest,
    caller: &Caller,
    authority: &dyn Authority,
    reply: &mut CommandReply,
) {
    let root = Path::new(&request.root);
    let creates_card =
        request.verb == "new" && matches!(request.object.as_str(), "task" | "drift" | "proposal");
    if creates_card {
        let announcement = request
            .input
            .get("announcement")
            .and_then(serde_json::Value::as_str);
        let card = reply
            .json
            .get("id")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned);
        if let (Some(announcement), Some(card)) = (announcement, card) {
            if let Some(note) = authority.card_filed(
                root,
                caller.job.as_deref(),
                &request.object,
                &card,
                announcement,
            ) {
                reply.note = Some(match reply.note.take() {
                    Some(existing) => format!("{existing}\n{note}"),
                    None => note,
                });
            }
        }
    }
    if request.object == "task"
        && request.verb == "move"
        && request.state.as_deref() == Some("done")
        && !request.input.contains_key("unchanged")
    {
        authority.task_finished(root);
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde_json::{json, Map, Value};

    use super::*;

    fn request(
        object: &str,
        verb: &str,
        root: &Path,
        card: Option<&str>,
        state: Option<&str>,
        input: Value,
    ) -> CommandRequest {
        CommandRequest {
            object: object.into(),
            verb: verb.into(),
            root: root.to_string_lossy().into_owned(),
            card: card.map(Into::into),
            state: state.map(Into::into),
            input: input.as_object().cloned().unwrap_or_else(Map::new),
            general: None,
            ask: None,
        }
    }

    fn project() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".weaver/tasks")).unwrap();
        dir
    }

    fn run(request: &CommandRequest, caller: &Caller) -> Result<CommandReply, Failure> {
        execute(request, caller, &NoJobs)
    }

    #[test]
    fn a_folder_without_weaver_is_refused_for_everything_but_project_new() {
        let dir = tempfile::tempdir().unwrap();
        let error = run(
            &request("model", "check", dir.path(), None, None, json!({})),
            &Caller::default(),
        )
        .unwrap_err();
        assert_eq!(error.code, code::NOT_INITIALISED);
        assert!(error.message.contains("initialise"));
        assert!(!dir.path().join(".weaver").exists());
    }

    #[test]
    fn a_folder_with_only_an_old_trama_folder_is_not_initialised() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".trama/model")).unwrap();
        fs::write(dir.path().join(".trama/model/system.md"), "# Old\n").unwrap();
        let error = run(
            &request("model", "check", dir.path(), None, None, json!({})),
            &Caller::default(),
        )
        .unwrap_err();
        assert_eq!(error.code, code::NOT_INITIALISED);
        assert_eq!(error.exit_code(), 1);
        assert!(error.message.contains("initialise"));
        assert!(!dir.path().join(".weaver").exists());
    }

    #[test]
    fn a_folder_with_both_is_the_project_in_weaver_without_warnings() {
        let dir = tempfile::tempdir().unwrap();
        run(
            &request("project", "new", dir.path(), None, None, json!({})),
            &Caller::default(),
        )
        .unwrap();
        fs::create_dir_all(dir.path().join(".trama/model")).unwrap();
        fs::write(dir.path().join(".trama/model/system.md"), "no title here\n").unwrap();
        let reply = run(
            &request("model", "check", dir.path(), None, None, json!({})),
            &Caller::default(),
        )
        .unwrap();
        assert_eq!(reply.exit, 0);
        assert!(reply.markdown.contains("No problem found."));
        assert_eq!(reply.json["warnings"], 0);
    }

    #[test]
    fn a_task_move_without_a_valid_job_is_refused_and_writes_nothing() {
        let dir = project();
        let card = Some("aaaaaaaa");
        let state = Some("doing");
        let without = run(
            &request("task", "move", dir.path(), card, state, json!({})),
            &Caller::default(),
        )
        .unwrap_err();
        assert_eq!(without.code, code::FORBIDDEN);
        assert_eq!(without.exit_code(), 1);
        assert!(without
            .message
            .contains("agent of a job that Weaver started"));
        // A job the authority does not know is no better than none.
        let unknown = Caller {
            job: Some("zzzzzzzz".into()),
            ..Caller::default()
        };
        let error = run(
            &request("task", "move", dir.path(), card, state, json!({})),
            &unknown,
        )
        .unwrap_err();
        assert_eq!(error.code, code::FORBIDDEN);
        assert_eq!(
            fs::read_dir(dir.path().join(".weaver/tasks"))
                .unwrap()
                .count(),
            0
        );
    }

    /// An authority that vouches for one job and one Chat token, and says what the handler saw.
    struct Vouches;

    impl Authority for Vouches {
        fn accepts_job(&self, _root: &Path, job: &str) -> bool {
            job == "jjjjjjjj"
        }

        fn accepts_general(&self, _root: &Path, token: &str) -> bool {
            token == "chat"
        }

        fn job_record(&self, _root: &Path, job: &str) -> Option<weaver_core::jobs::JobRecord> {
            (job == "jjjjjjjj").then(|| {
                serde_json::from_value(json!({
                    "id": "jjjjjjjj", "kind": "design", "name": "Design", "project": "/p",
                    "state": "running", "created": "2026-11-02T07:00:00.000Z", "target": "rrrr",
                }))
                .unwrap()
            })
        }

        fn run(&self, call: &Call) -> Option<Result<CommandReply, Failure>> {
            Some(Ok(CommandReply {
                markdown: String::new(),
                json: json!({
                    "who": format!("{:?}", call.caller.who),
                    "target": call.caller.job_record.as_ref().and_then(|r| r.target.clone()),
                }),
                exit: 0,
                note: None,
            }))
        }
    }

    #[test]
    fn the_handler_sees_a_job_the_chat_or_another_caller_as_the_app_vouches_for_them() {
        let dir = project();
        let who = |caller: Caller| {
            let reply = execute(
                &request("proposal", "list", dir.path(), None, None, json!({})),
                &caller,
                &Vouches,
            )
            .unwrap();
            (
                reply.json["who"].as_str().unwrap().to_owned(),
                reply.json["target"].as_str().map(str::to_owned),
            )
        };
        let job = |id: &str| Caller {
            job: Some(id.into()),
            ..Caller::default()
        };
        let chat = |token: &str| Caller {
            general: Some(token.into()),
            ..Caller::default()
        };
        assert_eq!(who(job("jjjjjjjj")), ("Job".into(), Some("rrrr".into())));
        assert_eq!(who(chat("chat")), ("Chat".into(), None));
        // An identity the app does not recognise counts as none.
        assert_eq!(who(job("zzzzzzzz")), ("Other".into(), None));
        assert_eq!(who(chat("forged")), ("Other".into(), None));
        assert_eq!(who(Caller::default()), ("Other".into(), None));
        // A command the CLI runs itself vouches for nobody.
        let reply = execute(
            &request("proposal", "list", dir.path(), None, None, json!({})),
            &job("jjjjjjjj"),
            &NoJobs,
        )
        .unwrap();
        assert_eq!(reply.exit, 0);
    }

    #[test]
    fn invalid_input_is_exit_two_before_the_access_gate() {
        let dir = project();
        let error = run(
            &request(
                "task",
                "move",
                dir.path(),
                Some("aaaaaaaa"),
                Some("doing"),
                json!({ "nope": 1 }),
            ),
            &Caller::default(),
        )
        .unwrap_err();
        assert_eq!(error.exit_code(), 2);
        assert!(error.message.contains("nope"));
    }

    #[test]
    fn a_card_that_does_not_exist_is_refused_with_exit_one() {
        let dir = project();
        for name in ["aaaaaaaa", "t99", "p1"] {
            let error = run(
                &request("task", "show", dir.path(), Some(name), None, json!({})),
                &Caller::default(),
            )
            .unwrap_err();
            assert_eq!(error.code, code::REFUSED, "{name}");
        }
        let wrong_letter = run(
            &request("proposal", "show", dir.path(), Some("t3"), None, json!({})),
            &Caller::default(),
        )
        .unwrap_err();
        assert!(
            wrong_letter.message.contains("Task number"),
            "{}",
            wrong_letter.message
        );
    }

    #[test]
    fn words_are_checked_against_the_command() {
        let dir = project();
        let missing = run(
            &request("task", "show", dir.path(), None, None, json!({})),
            &Caller::default(),
        )
        .unwrap_err();
        assert_eq!(missing.exit_code(), 2);
        let state = run(
            &request(
                "task",
                "move",
                dir.path(),
                Some("aaaaaaaa"),
                Some("flying"),
                json!({}),
            ),
            &Caller::default(),
        )
        .unwrap_err();
        assert!(state.message.contains("doing, todo, done"));
        let extra = run(
            &request("model", "check", dir.path(), Some("x"), None, json!({})),
            &Caller::default(),
        )
        .unwrap_err();
        assert!(extra.message.contains("takes no argument"));
    }

    #[test]
    fn model_check_runs_in_a_fresh_project_and_prints_markdown_and_json() {
        let dir = tempfile::tempdir().unwrap();
        run(
            &request("project", "new", dir.path(), None, None, json!({})),
            &Caller::default(),
        )
        .unwrap();
        let reply = run(
            &request("model", "check", dir.path(), None, None, json!({})),
            &Caller::default(),
        )
        .unwrap();
        assert_eq!(reply.exit, 0);
        assert!(reply.markdown.contains("No problem found."));
        assert_eq!(reply.json["ok"], true);
    }

    #[test]
    fn model_check_exits_one_on_errors_but_not_on_warnings() {
        let dir = tempfile::tempdir().unwrap();
        run(
            &request("project", "new", dir.path(), None, None, json!({})),
            &Caller::default(),
        )
        .unwrap();
        let big = format!(
            "---\ndescription: Use when big.\n---\n# Big\n\n## General\n{}\n",
            "- a line of text\n".repeat(1000)
        );
        fs::write(dir.path().join(".weaver/requirements/ui/big.md"), big).unwrap();
        let warn = run(
            &request("model", "check", dir.path(), None, None, json!({})),
            &Caller::default(),
        )
        .unwrap();
        assert_eq!(warn.exit, 0);
        assert_eq!(warn.json["warnings"], 1, "{}", warn.markdown);
        fs::write(
            dir.path().join(".weaver/model/system.md"),
            "no title here\n",
        )
        .unwrap();
        let broken = run(
            &request("model", "check", dir.path(), None, None, json!({})),
            &Caller::default(),
        )
        .unwrap();
        assert_eq!(broken.exit, 1);
        assert_eq!(broken.json["ok"], false);
        assert!(broken.markdown.contains("model/system.md"));
    }
}
