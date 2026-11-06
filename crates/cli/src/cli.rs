//! The flow of one `weaver` invocation: arguments, help, root, input, access, where the command
//! runs, and what is printed.
//!
//! [`run_with`] takes everything from its parameters (arguments, environment, folders, streams)
//! so that it can be tested in process; [`crate::run`] feeds it the real ones.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};
use weaver_core::command::{code, CommandReply, CommandRequest, KIND};
use weaver_core::lock::{self, LockError};
use weaver_core::paths::{self, AppDirs};

use crate::client::{self, CallError};
use crate::exec::{self, access_failure, NoJobs};
use crate::help;
use crate::interrupts::Interrupts;
use crate::registry;
use crate::round;
use crate::spec::*;

/// The `WEAVER_*` variables the command line reads.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Environment {
    pub job: Option<String>,
    pub general: Option<String>,
    pub ask: Option<String>,
    pub endpoint: Option<String>,
    pub run: Option<String>,
    pub task: Option<String>,
}

impl Environment {
    /// The variables of this process; an empty one counts as not set.
    pub fn from_process() -> Self {
        let get = |name: &str| std::env::var(name).ok().filter(|value| !value.is_empty());
        Self {
            job: get("WEAVER_JOB"),
            general: get("WEAVER_GENERAL"),
            ask: get("WEAVER_ASK"),
            endpoint: get("WEAVER_ENDPOINT"),
            run: get("WEAVER_RUN"),
            task: get("WEAVER_TASK"),
        }
    }

    /// The caller: the variables, and the registry record of the job when the app's folders are
    /// known and the record can be read (`job show` reads nothing else).
    fn caller(&self, dirs: Option<&AppDirs>) -> Caller {
        let job_record = self.job.as_deref().zip(dirs).and_then(|(job, dirs)| {
            weaver_core::jobs::JobStore::new(dirs)
                .read(job)
                .ok()
                .flatten()
        });
        Caller {
            job: self.job.clone(),
            general: self.general.clone(),
            ask: self.ask.clone(),
            run: self.run.clone(),
            task: self.task.clone(),
            job_record,
            ..Caller::default()
        }
    }
}

/// The streams of the process.
pub struct Streams<'a> {
    pub stdin: &'a mut dyn Read,
    pub out: &'a mut dyn Write,
    pub err: &'a mut dyn Write,
    /// What stops a command that waits (SIGINT, SIGTERM, a closed standard output).
    pub interrupts: &'a dyn Interrupts,
}

/// What the arguments say, before the registry is asked.
struct Arguments {
    help: bool,
    json: bool,
    input: Option<String>,
    root: Option<String>,
    words: Vec<String>,
}

fn parse_arguments(args: &[String]) -> Result<Arguments, String> {
    use clap::{Arg, ArgAction};
    let parser = clap::Command::new("weaver")
        .disable_help_flag(true)
        .disable_version_flag(true)
        .disable_help_subcommand(true)
        .arg(
            Arg::new("help")
                .short('h')
                .long("help")
                .action(ArgAction::SetTrue),
        )
        .arg(Arg::new("json").long("json").action(ArgAction::SetTrue))
        .arg(Arg::new("input").long("input").num_args(1))
        .arg(Arg::new("root").long("root").num_args(1))
        .arg(Arg::new("words").num_args(0..));
    let matches = parser
        .try_get_matches_from(std::iter::once("weaver".to_owned()).chain(args.iter().cloned()))
        .map_err(|e| {
            let text = e.to_string();
            let first = text.lines().next().unwrap_or("wrong usage");
            first.strip_prefix("error: ").unwrap_or(first).to_owned()
        })?;
    Ok(Arguments {
        help: matches.get_flag("help"),
        json: matches.get_flag("json"),
        input: matches.get_one::<String>("input").cloned(),
        root: matches.get_one::<String>("root").cloned(),
        words: matches
            .get_many::<String>("words")
            .map(|w| w.cloned().collect())
            .unwrap_or_default(),
    })
}

fn put(stream: &mut dyn Write, text: &str) {
    // A closed pipe is not an error of the command.
    let _ = stream.write_all(text.as_bytes());
    let _ = stream.flush();
}

fn put_line(stream: &mut dyn Write, text: &str) {
    put(stream, &format!("{}\n", text.trim_end_matches('\n')));
}

/// Prints a failure: the message on stderr and, when `json`, `{error: {code, message}}` on stdout.
fn fail(io: &mut Streams, json: bool, failure: &Failure) -> u8 {
    put_line(io.err, &format!("error: {}", failure.message));
    if json {
        put_line(
            io.out,
            &json!({ "error": { "code": failure.code, "message": failure.message } }).to_string(),
        );
    }
    failure.exit_code()
}

fn usage_failure(message: impl Into<String>) -> Failure {
    Failure::new(code::USAGE, message)
}

/// Prints what a command answered and returns its exit code.
fn print_reply(io: &mut Streams, json: bool, reply: &CommandReply) -> u8 {
    if json {
        put_line(io.out, &reply.json.to_string());
    } else {
        put_line(io.out, &reply.markdown);
    }
    if let Some(note) = &reply.note {
        put_line(io.err, note);
    }
    reply.exit
}

fn help_flow(arguments: &Arguments, io: &mut Streams) -> u8 {
    match arguments.words.as_slice() {
        [] => {
            put(io.out, &help::top_help());
            0
        }
        [object] => match help::object_help(object) {
            Some(text) => {
                put(io.out, &text);
                0
            }
            None => fail(io, arguments.json, &usage_failure(unknown_object(object))),
        },
        [object, verb, ..] => match registry::find(object, verb) {
            Some(command) => {
                if arguments.json {
                    put_line(io.out, &crate::schema::help_json(command).to_string());
                } else {
                    put(io.out, &help::command_help(command));
                }
                0
            }
            None => fail(
                io,
                arguments.json,
                &usage_failure(unknown_verb(object, verb)),
            ),
        },
    }
}

fn unknown_object(object: &str) -> String {
    let objects: Vec<&str> = registry::objects()
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    format!(
        "`{object}` is not an object of `weaver`: {}",
        objects.join(", ")
    )
}

fn unknown_verb(object: &str, verb: &str) -> String {
    match registry::objects()
        .into_iter()
        .find(|(name, _)| *name == object)
    {
        None => unknown_object(object),
        Some((_, commands)) => {
            let verbs: Vec<&str> = commands.iter().map(|c| c.verb).collect();
            format!(
                "`{verb}` is not a verb of `weaver {object}`: {}",
                verbs.join(", ")
            )
        }
    }
}

/// Reads the input object: `-` is standard input, anything else is the JSON text itself. A
/// leading BOM is dropped.
fn read_input(given: Option<&str>, stdin: &mut dyn Read) -> Result<Map<String, Value>, Failure> {
    let Some(given) = given else {
        return Ok(Map::new());
    };
    let text = if given == "-" {
        let mut bytes = Vec::new();
        stdin.read_to_end(&mut bytes).map_err(|e| {
            Failure::invalid(format!("cannot read the input from standard input: {e}"))
        })?;
        String::from_utf8(bytes).map_err(|_| Failure::invalid("the input is not valid UTF-8"))?
    } else {
        given.to_owned()
    };
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    let value: Value = serde_json::from_str(text)
        .map_err(|e| Failure::invalid(format!("the input is not valid JSON: {e}")))?;
    match value {
        Value::Object(map) => Ok(map),
        other => {
            let what = match other {
                Value::Array(_) => "a list",
                Value::String(_) => "a string",
                Value::Number(_) => "a number",
                Value::Bool(_) => "a boolean",
                _ => "null",
            };
            Err(Failure::invalid(format!(
                "the input must be one JSON object, found {what}"
            )))
        }
    }
}

/// The folder the command works on.
fn project_root(given: Option<&str>, cwd: &Path) -> Result<PathBuf, Failure> {
    let wanted = match given {
        Some(given) => {
            let path = Path::new(given);
            if path.is_absolute() {
                path.to_path_buf()
            } else {
                cwd.join(path)
            }
        }
        None => cwd.to_path_buf(),
    };
    paths::canonical_root(&wanted)
        .map_err(|e| Failure::refused(format!("cannot use the folder {}: {e}", wanted.display())))
}

/// What asking the app came to.
enum Asked {
    Answered(Result<CommandReply, Failure>),
    /// No app answers.
    NoApp,
}

fn ask_app(request: &CommandRequest, env: &Environment, dirs: Option<&AppDirs>) -> Asked {
    let Some(dirs) = dirs else {
        return Asked::NoApp;
    };
    let payload = match serde_json::to_value(request) {
        Ok(payload) => payload,
        Err(e) => {
            return Asked::Answered(Err(Failure::failed(format!(
                "cannot encode the command: {e}"
            ))))
        }
    };
    let sent = client::call(
        dirs,
        client::Call {
            job: env.job.as_deref(),
            kind: KIND,
            payload,
            expected_endpoint: env.endpoint.as_deref(),
        },
    );
    match sent {
        Ok(response) => match response.error {
            Some(error) => Asked::Answered(Err(Failure::new(&error.code, error.message))),
            None => Asked::Answered(
                serde_json::from_value::<CommandReply>(response.payload).map_err(|e| {
                    Failure::failed(format!("Weaver sent an answer that cannot be read: {e}"))
                }),
            ),
        },
        Err(CallError::Unavailable(_)) => Asked::NoApp,
        Err(CallError::NoReply) => Asked::Answered(Err(Failure::failed(
            "Weaver closed the connection without an answer; the command may not have run",
        ))),
        Err(CallError::Send(e)) => {
            Asked::Answered(Err(Failure::failed(format!("cannot reach Weaver: {e}"))))
        }
    }
}

fn app_not_running(command: &Command) -> Failure {
    Failure::new(
        code::APP_NOT_RUNNING,
        format!(
            "`weaver {}` needs Weaver, and Weaver is not running.",
            command.name()
        ),
    )
}

/// Runs the command in this process under the project lock.
fn run_locked(
    request: &CommandRequest,
    caller: &Caller,
    dirs: Option<&AppDirs>,
) -> Result<CommandReply, Failure> {
    let Some(dirs) = dirs else {
        return Err(Failure::failed(
            "cannot find the folders of Weaver, so the project lock cannot be taken",
        ));
    };
    let hash = paths::project_hash_of_canonical(Path::new(&request.root));
    match lock::with_project_lock(dirs, &hash, || exec::execute(request, caller, &NoJobs)) {
        // The lock is released: a commit that follows a History entry is made now.
        Ok(result) => result.map(|reply| exec::settle(Path::new(&request.root), reply)),
        Err(LockError::Busy) => Err(Failure::new(
            code::PROJECT_BUSY,
            "the project is busy: another Weaver or weaver command holds it",
        )),
        Err(LockError::Io(e)) => Err(Failure::failed(format!(
            "cannot take the project lock: {e}"
        ))),
    }
}

/// Runs one `weaver` invocation and returns its exit code.
pub fn run_with(
    args: &[String],
    env: &Environment,
    dirs: Option<&AppDirs>,
    cwd: &Path,
    io: &mut Streams,
) -> u8 {
    let arguments = match parse_arguments(args) {
        Ok(arguments) => arguments,
        Err(message) => {
            let json = args.iter().any(|a| a == "--json");
            return fail(
                io,
                json,
                &usage_failure(format!("{message}. Run `weaver --help` for the usage.")),
            );
        }
    };
    if arguments.help {
        return help_flow(&arguments, io);
    }
    let json_flag = arguments.json;
    let Some(object) = arguments.words.first() else {
        put_line(io.err, "error: a command is needed");
        put(io.err, &format!("\n{}", help::top_help()));
        return code_usage();
    };
    let Some(verb) = arguments.words.get(1) else {
        let message = match registry::objects()
            .into_iter()
            .find(|(name, _)| name == object)
        {
            Some((_, commands)) => {
                let verbs: Vec<&str> = commands.iter().map(|c| c.verb).collect();
                format!("`weaver {object}` needs a verb: {}", verbs.join(", "))
            }
            None => unknown_object(object),
        };
        return fail(io, json_flag, &usage_failure(message));
    };
    let Some(command) = registry::find(object, verb) else {
        return fail(io, json_flag, &usage_failure(unknown_verb(object, verb)));
    };
    let json = json_flag || command.json_only;

    let (card, state) = split_words(command, &arguments.words[2..]);
    if let Err(failure) = exec::check_words(command, card.as_deref(), state.as_deref()) {
        return fail(io, json, &failure);
    }
    let extra = arguments
        .words
        .len()
        .saturating_sub(2 + usize::from(card.is_some()) + usize::from(state.is_some()));
    if extra > 0 {
        return fail(
            io,
            json,
            &usage_failure(format!(
                "`weaver {}` got {extra} more argument{} than it takes: {}",
                command.name(),
                if extra == 1 { "" } else { "s" },
                command.usage()
            )),
        );
    }

    let root = match project_root(arguments.root.as_deref(), cwd) {
        Ok(root) => root,
        Err(failure) => return fail(io, json, &failure),
    };
    if !(command.object == "project" && command.verb == "new") {
        if let Err(failure) = exec::ensure_initialised(&root) {
            return fail(io, json, &failure);
        }
    }
    let input = match read_input(arguments.input.as_deref(), io.stdin) {
        Ok(input) => input,
        Err(failure) => return fail(io, json, &failure),
    };
    if let Err(failure) = exec::check_input(command, &input) {
        return fail(io, json, &failure);
    }

    let caller = env.caller(dirs);
    let outcome = dispatch(
        command,
        &card,
        &state,
        input,
        &root,
        env,
        &caller,
        dirs,
        io.interrupts,
    );
    match outcome {
        Ok(reply) => print_reply(io, json, &reply),
        Err(failure)
            if command.access == Access::JobOrUnavailable
                && matches!(
                    failure.code.as_str(),
                    code::FORBIDDEN | code::APP_NOT_RUNNING
                ) =>
        {
            print_reply(io, json, &round::unavailable())
        }
        Err(failure) => fail(io, json, &failure),
    }
}

fn code_usage() -> u8 {
    weaver_core::command::exit_code(code::USAGE)
}

/// The card and the state among the words after the verb.
fn split_words(command: &Command, rest: &[String]) -> (Option<String>, Option<String>) {
    let mut rest = rest.iter();
    let card = (command.arg != Arg::None)
        .then(|| rest.next().cloned())
        .flatten();
    let state = (!command.states.is_empty())
        .then(|| rest.next().cloned())
        .flatten();
    (card, state)
}

#[allow(clippy::too_many_arguments)]
fn dispatch(
    command: &Command,
    card: &Option<String>,
    state: &Option<String>,
    input: Map<String, Value>,
    root: &Path,
    env: &Environment,
    caller: &Caller,
    dirs: Option<&AppDirs>,
    interrupts: &dyn Interrupts,
) -> Result<CommandReply, Failure> {
    let needs_job = match command.access {
        Access::Job => env.job.is_none(),
        // The agent of a job, or of the Chat: the app decides which of them it accepts.
        Access::JobOrUnavailable => env.job.is_none() && env.general.is_none(),
        Access::Anyone | Access::General => false,
    };
    let needs_general =
        command.access == Access::General && env.general.is_none() && env.ask.is_none();
    if needs_job || needs_general {
        return Err(access_failure(command));
    }
    if command.object == "question" && command.verb == "new" {
        // A round waits with no time limit, so it never goes through the project's queue.
        return round::run(input, env, dirs, interrupts);
    }
    let place = command.place_for(&input, card.as_deref());
    let request = CommandRequest {
        object: command.object.to_owned(),
        verb: command.verb.to_owned(),
        root: root.to_string_lossy().into_owned(),
        card: card.clone(),
        state: state.clone(),
        input,
        general: env.general.clone(),
        ask: env.ask.clone(),
    };
    match place {
        Place::Local => exec::execute(&request, caller, &NoJobs),
        Place::Write => match ask_app(&request, env, dirs) {
            Asked::Answered(result) => result,
            Asked::NoApp => run_locked(&request, caller, dirs),
        },
        Place::App => match ask_app(&request, env, dirs) {
            Asked::Answered(result) => result,
            Asked::NoApp => Err(app_not_running(command)),
        },
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::thread;

    use weaver_core::endpoint::{new_endpoint, new_secret};
    use weaver_core::ipc::{serve_connection, Listener};
    use weaver_core::protocol::Response;
    use weaver_core::runtime::{self, RuntimeInfo};

    use super::*;
    use crate::interrupts::NoInterrupts;

    struct Sandbox {
        base: tempfile::TempDir,
        root: PathBuf,
        dirs: AppDirs,
    }

    fn sandbox() -> Sandbox {
        let base = tempfile::tempdir().unwrap();
        let root = base.path().join("project");
        fs::create_dir_all(root.join(".weaver")).unwrap();
        let dirs = AppDirs::from_bases(
            &base.path().join("config"),
            &base.path().join("data"),
            Some(base.path()),
        );
        Sandbox {
            root: dunce_root(&root),
            base,
            dirs,
        }
    }

    fn dunce_root(path: &Path) -> PathBuf {
        paths::canonical_root(path).unwrap()
    }

    fn invoke(
        args: &[&str],
        env: &Environment,
        sandbox: &Sandbox,
        stdin: &str,
    ) -> (u8, String, String) {
        let args: Vec<String> = args
            .iter()
            .map(|a| (*a).to_owned())
            .chain(["--root".to_owned(), sandbox.root.display().to_string()])
            .collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let mut input = stdin.as_bytes();
        let mut streams = Streams {
            stdin: &mut input,
            out: &mut out,
            err: &mut err,
            interrupts: &NoInterrupts,
        };
        let exit = run_with(&args, env, Some(&sandbox.dirs), &sandbox.root, &mut streams);
        (
            exit,
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    }

    /// Starts an app that answers one request with `reply` and reports what it received.
    fn app(
        sandbox: &Sandbox,
        reply: Result<CommandReply, (&'static str, &'static str)>,
    ) -> thread::JoinHandle<weaver_core::protocol::Request> {
        let endpoint = new_endpoint(&sandbox.dirs).unwrap();
        let secret = new_secret();
        runtime::write(
            &sandbox.dirs,
            &RuntimeInfo {
                endpoint: endpoint.clone(),
                secret: secret.clone(),
                pid: 1,
                version: "0.1.0".into(),
            },
        )
        .unwrap();
        let listener = Listener::bind(&endpoint).unwrap();
        thread::spawn(move || {
            let stream = listener.accept().unwrap();
            let mut seen = None;
            serve_connection(stream, &secret, |request| {
                seen = Some(request);
                match reply {
                    Ok(reply) => Response::ok(serde_json::to_value(reply).unwrap()),
                    Err((code, message)) => Response::failure(code, message),
                }
            })
            .unwrap();
            seen.unwrap()
        })
    }

    fn done(markdown: &str) -> CommandReply {
        CommandReply {
            markdown: markdown.into(),
            json: json!({ "from": "app" }),
            exit: 0,
            note: None,
        }
    }

    #[test]
    fn a_job_bound_command_goes_to_the_app_with_the_job_and_the_card_as_typed() {
        let sandbox = sandbox();
        let server = app(&sandbox, Ok(done("Moved by the app.")));
        let env = Environment {
            job: Some("abcd1234".into()),
            ..Environment::default()
        };
        let (exit, out, err) = invoke(&["task", "move", "t3", "doing"], &env, &sandbox, "");
        assert_eq!(
            (exit, out.as_str(), err.as_str()),
            (0, "Moved by the app.\n", "")
        );
        let request = server.join().unwrap();
        assert_eq!(request.kind, KIND);
        assert_eq!(request.job.as_deref(), Some("abcd1234"));
        let sent: CommandRequest = serde_json::from_value(request.payload).unwrap();
        assert_eq!((sent.object.as_str(), sent.verb.as_str()), ("task", "move"));
        assert_eq!(
            (sent.card.as_deref(), sent.state.as_deref()),
            (Some("t3"), Some("doing"))
        );
        assert_eq!(Path::new(&sent.root), sandbox.root);
    }

    #[test]
    fn a_write_goes_to_the_app_when_one_answers_and_a_refusal_of_the_app_is_exit_one() {
        let sandbox = sandbox();
        let server = app(&sandbox, Err((code::REFUSED, "the app says no")));
        let (exit, out, err) = invoke(
            &["project", "new", "--input", "{\"force\":true}"],
            &Environment::default(),
            &sandbox,
            "",
        );
        assert_eq!((exit, out.as_str()), (1, ""));
        assert_eq!(err, "error: the app says no\n");
        let sent: CommandRequest = serde_json::from_value(server.join().unwrap().payload).unwrap();
        assert_eq!(sent.input["force"], json!(true));
        assert!(
            !sandbox.root.join(".agents").exists(),
            "the CLI did not run it as well"
        );
    }

    #[test]
    fn a_write_runs_in_the_cli_under_the_lock_when_no_app_answers() {
        let sandbox = sandbox();
        fs::remove_dir_all(sandbox.root.join(".weaver")).unwrap();
        let (exit, out, _) = invoke(&["project", "new"], &Environment::default(), &sandbox, "");
        assert_eq!(exit, 0);
        assert!(out.starts_with("# Project initialised"));
        assert!(sandbox.root.join(".weaver/model/system.md").is_file());
        assert!(
            sandbox.dirs.locks_dir().is_dir(),
            "the project lock was taken"
        );
        let _ = &sandbox.base;
    }

    #[test]
    fn a_reply_with_exit_one_prints_its_output_and_its_note() {
        let sandbox = sandbox();
        let reply = CommandReply {
            markdown: "report".into(),
            json: json!({}),
            exit: 1,
            note: Some("2 errors".into()),
        };
        let _server = app(&sandbox, Ok(reply));
        let env = Environment {
            job: Some("abcd1234".into()),
            ..Environment::default()
        };
        let (exit, out, err) = invoke(
            &["job", "edit", "--input", "{\"note\":\"x\"}"],
            &env,
            &sandbox,
            "",
        );
        assert_eq!(
            (exit, out.as_str(), err.as_str()),
            (1, "report\n", "2 errors\n")
        );
    }

    #[test]
    fn job_new_goes_to_the_app_with_the_ask_identity_and_needs_a_caller() {
        let sandbox = sandbox();
        let server = app(&sandbox, Ok(done("Started.")));
        let env = Environment {
            ask: Some("askaskas".into()),
            ..Environment::default()
        };
        let args = ["job", "new", "--input", "{\"kind\":\"scan\"}"];
        let (exit, out, err) = invoke(&args, &env, &sandbox, "");
        assert_eq!((exit, out.as_str(), err.as_str()), (0, "Started.\n", ""));
        let request = server.join().unwrap();
        let sent: CommandRequest = serde_json::from_value(request.payload).unwrap();
        assert_eq!(sent.ask.as_deref(), Some("askaskas"));
        assert_eq!(sent.general, None);

        let (exit, out, err) = invoke(&args, &Environment::default(), &sandbox, "");
        assert_eq!((exit, out.as_str()), (1, ""));
        assert!(err.contains("Ask more conversation"), "{err}");
    }

    #[test]
    fn input_from_standard_input_loses_its_bom_and_must_be_one_object() {
        let sandbox = sandbox();
        let (exit, out, _) = invoke(
            &["model", "check", "--input", "-"],
            &Environment::default(),
            &sandbox,
            "\u{feff}{}",
        );
        assert_eq!(exit, 0, "{out}");
        for bad in ["", "[]", "{", "\"text\"", "7", "\u{feff}"] {
            let (exit, _, err) = invoke(
                &["model", "check", "--input", "-"],
                &Environment::default(),
                &sandbox,
                bad,
            );
            assert_eq!(exit, 2, "{bad:?}: {err}");
        }
    }

    #[test]
    fn words_and_options_that_do_not_fit_are_wrong_usage() {
        let sandbox = sandbox();
        let env = Environment::default();
        for args in [
            vec!["model"],
            vec!["nothing", "show"],
            vec!["model", "nothing"],
            vec!["model", "check", "extra"],
            vec!["task", "show"],
            vec!["task", "move", "t1"],
            vec!["task", "show", "t1", "extra"],
            vec!["model", "check", "--no-such-option"],
            vec!["model", "check", "-x"],
            vec![],
        ] {
            let (exit, out, err) = invoke(&args, &env, &sandbox, "");
            assert_eq!(exit, 2, "{args:?}: {out}{err}");
            assert!(!err.is_empty() && out.is_empty(), "{args:?}");
        }
    }

    #[test]
    fn help_reads_nothing_and_needs_no_project() {
        let base = tempfile::tempdir().unwrap();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let mut empty: &[u8] = b"";
        let mut streams = Streams {
            stdin: &mut empty,
            out: &mut out,
            err: &mut err,
            interrupts: &NoInterrupts,
        };
        let args: Vec<String> = ["task", "move", "--help", "--json", "--root", "/nonexistent"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        assert_eq!(
            run_with(
                &args,
                &Environment::default(),
                None,
                base.path(),
                &mut streams
            ),
            0
        );
        let help: Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(
            help["command"]["properties"]["state"]["enum"],
            json!(["doing", "todo", "done"])
        );
        assert!(err.is_empty());
    }
}
