//! What a command is: its words, arguments, input fields, output fields, who may run it and
//! where it runs. One [`Command`] value per command in [`crate::registry`] drives parsing,
//! validation, dispatch, help, the JSON Schemas and the command reference of the skills.

use std::path::Path;

use serde_json::{Map, Value};
use weaver_core::command::{code, CommandReply};
use weaver_core::project::cards::CardKind;
use weaver_core::project::image::ProjectImage;

// ---------------------------------------------------------------------------------------------
// Types of input and output fields

/// Rules of a string.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StrRule {
    /// More than whitespace.
    pub nonblank: bool,
    /// No line break.
    pub line: bool,
    /// At most this many characters (Unicode scalar values).
    pub max: Option<usize>,
}

/// The type of a field.
#[derive(Debug, Clone, PartialEq)]
pub enum Ty {
    Str(StrRule),
    Int {
        min: Option<i64>,
        max: Option<i64>,
    },
    Bool,
    Enum(&'static [&'static str]),
    List {
        item: Box<Ty>,
        min: usize,
        max: Option<usize>,
    },
    Object(Vec<Field>),
    /// Output only: the value or `null`.
    Nullable(Box<Ty>),
    /// Output only: any JSON value.
    Any,
}

/// A field of the input object, or of the output.
#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub name: &'static str,
    pub ty: Ty,
    pub required: bool,
    pub doc: &'static str,
}

/// Any string.
pub fn text() -> Ty {
    Ty::Str(StrRule::default())
}

/// A string with something in it.
pub fn nonblank() -> Ty {
    Ty::Str(StrRule {
        nonblank: true,
        ..StrRule::default()
    })
}

/// A non-blank string on one line.
pub fn line() -> Ty {
    Ty::Str(StrRule {
        nonblank: true,
        line: true,
        max: None,
    })
}

/// A non-blank string on one line of at most `max` characters.
pub fn line_max(max: usize) -> Ty {
    Ty::Str(StrRule {
        nonblank: true,
        line: true,
        max: Some(max),
    })
}

/// An integer of at least `min`.
pub fn int_min(min: i64) -> Ty {
    Ty::Int {
        min: Some(min),
        max: None,
    }
}

pub fn boolean() -> Ty {
    Ty::Bool
}

pub fn one_of(values: &'static [&'static str]) -> Ty {
    Ty::Enum(values)
}

/// A list of any length.
pub fn list(item: Ty) -> Ty {
    Ty::List {
        item: Box::new(item),
        min: 0,
        max: None,
    }
}

/// A list of at least `min` items.
pub fn list_min(item: Ty, min: usize) -> Ty {
    Ty::List {
        item: Box::new(item),
        min,
        max: None,
    }
}

/// A list of `min` to `max` items.
pub fn list_between(item: Ty, min: usize, max: usize) -> Ty {
    Ty::List {
        item: Box::new(item),
        min,
        max: Some(max),
    }
}

pub fn object(fields: Vec<Field>) -> Ty {
    Ty::Object(fields)
}

pub fn nullable(ty: Ty) -> Ty {
    Ty::Nullable(Box::new(ty))
}

/// A required field.
pub fn req(name: &'static str, ty: Ty, doc: &'static str) -> Field {
    Field {
        name,
        ty,
        required: true,
        doc,
    }
}

/// An optional field.
pub fn opt(name: &'static str, ty: Ty, doc: &'static str) -> Field {
    Field {
        name,
        ty,
        required: false,
        doc,
    }
}

// ---------------------------------------------------------------------------------------------
// Who, where, with what

/// Who may run a command (`requirements/cli#agent`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    /// Any caller.
    Anyone,
    /// Only the agent of a Weaver job (`WEAVER_JOB`, accepted by the app); refused for any other
    /// caller.
    Job,
    /// Only the agent of a Weaver job (`WEAVER_JOB`) or, without it, of the project's working
    /// General during a turn (`WEAVER_GENERAL`), both accepted by the app; any other caller gets
    /// the outcome `unavailable` (exit 0).
    JobOrUnavailable,
    /// Only the agent of the project's working General (`WEAVER_GENERAL`) or of a working Ask more
    /// conversation of the project (`WEAVER_ASK`).
    General,
    /// Only the agent of the project's live Chat (`WEAVER_GENERAL`, accepted by the app) during one
    /// of its turns; an Ask more conversation and a job are refused.
    Chat,
}

/// Where a command runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    /// In the CLI process, with no lock.
    Local,
    /// In the app when one answers, else in the CLI under the project lock.
    Write,
    /// In the app only: without it the command fails saying Weaver is not running.
    App,
}

/// What the argument after the verb names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arg {
    None,
    /// A card of this kind, by identity, prefix or short number. Required.
    Card(CardKind),
    /// A Model or Requirement reference. Optional.
    Reference,
    /// A job identity. Optional.
    Job,
}

/// What a handler may use.
pub struct Call<'a> {
    pub command: &'a Command,
    /// The project root (holds `.weaver/`), canonical.
    pub root: &'a Path,
    /// The image of the project, read when the command started (under the lock for writes).
    pub image: &'a ProjectImage,
    /// The card, resolved to its identity; for [`Arg::Reference`] and [`Arg::Job`] the text given.
    pub arg: Option<&'a str>,
    /// The state argument of `move`.
    pub state: Option<&'a str>,
    /// The validated input.
    pub input: &'a Map<String, Value>,
    pub caller: &'a Caller,
}

/// Who the app vouches the caller is (`requirements/cli#agent`): the agent of a Weaver job
/// (`WEAVER_JOB` accepted), the agent of the project's live Chat (`WEAVER_GENERAL` recognised),
/// or anybody else: no identity, one the app does not recognise, or no app answering (a command
/// the CLI runs itself vouches for nobody).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Who {
    #[default]
    Other,
    Job,
    Chat,
}

/// Who runs the command, from the environment of the process that typed it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Caller {
    /// `WEAVER_JOB`.
    pub job: Option<String>,
    /// `WEAVER_GENERAL`.
    pub general: Option<String>,
    /// `WEAVER_ASK`.
    pub ask: Option<String>,
    /// `WEAVER_RUN`.
    pub run: Option<String>,
    /// `WEAVER_TASK`.
    pub task: Option<String>,
    /// What the app knows about the agent's work in the project (which jobs work); empty when no
    /// app answers, so a command run by the CLI itself sees no working job.
    pub agent: weaver_core::project::board::AgentFacts,
    /// The registry record of `WEAVER_JOB`, when the job has one that could be read.
    pub job_record: Option<weaver_core::jobs::JobRecord>,
    /// Who the app vouches the caller is, set by [`crate::exec::execute`] from the identities above;
    /// `Other` until then.
    pub who: Who,
}

/// A command that did not do its work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    /// One of [`weaver_core::command::code`], or `not-implemented`.
    pub code: String,
    pub message: String,
}

impl Failure {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_owned(),
            message: message.into(),
        }
    }

    /// The command is refused: nothing was written.
    pub fn refused(message: impl Into<String>) -> Self {
        Self::new(code::REFUSED, message)
    }

    /// The command failed on a file or the system.
    pub fn failed(message: impl Into<String>) -> Self {
        Self::new(code::FAILED, message)
    }

    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(code::INVALID_INPUT, message)
    }

    pub fn exit_code(&self) -> u8 {
        weaver_core::command::exit_code(&self.code)
    }
}

/// The code of a command whose handler is not written yet.
pub const NOT_IMPLEMENTED: &str = "not-implemented";

pub type Handler = fn(&Call) -> Result<CommandReply, Failure>;

/// The handler of a command whose Task is not done yet: the command is in the registry with its
/// whole input, so `--help` and validation work, but running it is refused.
pub fn not_implemented(call: &Call) -> Result<CommandReply, Failure> {
    Err(Failure::new(
        NOT_IMPLEMENTED,
        format!(
            "`weaver {} {}` is not available in this version of Weaver yet",
            call.command.object, call.command.verb
        ),
    ))
}

/// A problem found in the input, with the JSON path of the value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub path: String,
    pub message: String,
}

impl Problem {
    pub fn new(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
        }
    }
}

/// A rule over the whole input that types cannot say.
pub type Check = fn(&Map<String, Value>) -> Vec<Problem>;

/// Where a command runs, as a function of its call.
pub type PlaceFn = fn(&Map<String, Value>, Option<&str>) -> Place;

/// One command.
pub struct Command {
    pub object: &'static str,
    pub verb: &'static str,
    /// One line, no first person.
    pub summary: &'static str,
    /// More paragraphs for `--help` and the reference.
    pub notes: &'static [&'static str],
    pub arg: Arg,
    /// Values of the state argument; empty when the command has none.
    pub states: &'static [&'static str],
    pub input: Vec<Field>,
    pub output: Vec<Field>,
    /// A full command line, `weaver ...`.
    pub example: &'static str,
    pub access: Access,
    /// Output is always JSON, with or without `--json` (`question new`).
    pub json_only: bool,
    pub place: PlaceFn,
    pub check: Option<Check>,
    pub run: Handler,
}

impl Command {
    /// A command with the common defaults: no argument, no state, no input, any caller, run in
    /// the CLI.
    pub fn new(
        object: &'static str,
        verb: &'static str,
        summary: &'static str,
        run: Handler,
    ) -> Self {
        Self {
            object,
            verb,
            summary,
            notes: &[],
            arg: Arg::None,
            states: &[],
            input: Vec::new(),
            output: Vec::new(),
            example: "",
            access: Access::Anyone,
            json_only: false,
            place: |_, _| Place::Local,
            check: None,
            run,
        }
    }

    pub fn notes(mut self, notes: &'static [&'static str]) -> Self {
        self.notes = notes;
        self
    }

    pub fn arg(mut self, arg: Arg) -> Self {
        self.arg = arg;
        self
    }

    pub fn states(mut self, states: &'static [&'static str]) -> Self {
        self.states = states;
        self
    }

    pub fn input(mut self, input: Vec<Field>) -> Self {
        self.input = input;
        self
    }

    pub fn output(mut self, output: Vec<Field>) -> Self {
        self.output = output;
        self
    }

    pub fn example(mut self, example: &'static str) -> Self {
        self.example = example;
        self
    }

    pub fn access(mut self, access: Access) -> Self {
        self.access = access;
        self
    }

    pub fn json_only(mut self) -> Self {
        self.json_only = true;
        self
    }

    pub fn place(mut self, place: PlaceFn) -> Self {
        self.place = place;
        self
    }

    pub fn check(mut self, check: Check) -> Self {
        self.check = Some(check);
        self
    }

    /// `object verb`.
    pub fn name(&self) -> String {
        format!("{} {}", self.object, self.verb)
    }

    /// The usage line: `weaver <object> <verb> [<id>] ...`.
    pub fn usage(&self) -> String {
        let mut usage = format!("weaver {} {}", self.object, self.verb);
        match self.arg {
            Arg::None => {}
            Arg::Card(_) => usage.push_str(" <id>"),
            Arg::Reference => usage.push_str(" [<ref>]"),
            Arg::Job => usage.push_str(" [<id>]"),
        }
        if !self.states.is_empty() {
            usage.push(' ');
            usage.push_str(&self.states.join("|"));
        }
        usage.push_str(" [--input <json>|-] [--root <dir>] [--json]");
        usage
    }

    /// Where the command runs for this input and argument.
    pub fn place_for(&self, input: &Map<String, Value>, arg: Option<&str>) -> Place {
        if self.access != Access::Anyone {
            return Place::App;
        }
        (self.place)(input, arg)
    }
}
