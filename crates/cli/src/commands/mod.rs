//! The commands, one file per object. Each file returns its [`Command`] values; the registry
//! collects them in the order of `requirements/cli#General`. A Task that builds a command fills
//! the handler of its entry (or replaces the entry) and touches nothing else.

use serde_json::Value;
use weaver_core::command::CommandReply;

use crate::spec::*;

mod announcement;
mod drift;
mod history;
mod job;
mod model;
mod project;
mod proposal;
mod question;
mod request;
mod task;

/// Every command, by object in the order of the table of `requirements/cli#General`.
pub fn all() -> Vec<Command> {
    let mut all = Vec::new();
    all.extend(model::commands());
    all.extend(project::commands());
    all.extend(proposal::commands());
    all.extend(task::commands());
    all.extend(drift::commands());
    all.extend(history::commands());
    all.extend(announcement::commands());
    all.extend(job::commands());
    all.extend(question::commands());
    all.extend(request::commands());
    all
}

/// A successful reply.
pub fn reply(markdown: String, json: Value) -> CommandReply {
    CommandReply {
        markdown,
        json,
        exit: 0,
        note: None,
    }
}

/// The fields every card has in an output.
pub fn card_fields() -> Vec<Field> {
    vec![
        req("id", line(), "The identity."),
        req(
            "number",
            nullable(int_min(1)),
            "The short number; null when the card has no live number.",
        ),
        req("title", line(), "The title."),
    ]
}

/// A card with extra fields.
pub fn card_with(extra: Vec<Field>) -> Vec<Field> {
    let mut fields = card_fields();
    fields.extend(extra);
    fields
}

/// A list of cards.
pub fn cards(doc: &'static str, name: &'static str) -> Field {
    req(name, list(object(card_fields())), doc)
}

/// Where a command runs: in the app when one answers, else in the CLI under the project lock.
pub fn written(_input: &serde_json::Map<String, Value>, _arg: Option<&str>) -> Place {
    Place::Write
}

/// Where a command runs: in the CLI.
pub fn local(_input: &serde_json::Map<String, Value>, _arg: Option<&str>) -> Place {
    Place::Local
}

/// Where a command runs: in the app only.
pub fn in_app(_input: &serde_json::Map<String, Value>, _arg: Option<&str>) -> Place {
    Place::App
}

/// The handler of a command that only the app can run: the app answers it through
/// [`crate::exec::Authority::run`] before the handlers of the registry are asked, so a handler of
/// this kind is reached only with no app at hand.
pub fn app_only(call: &Call) -> Result<CommandReply, Failure> {
    Err(Failure::new(
        weaver_core::command::code::APP_NOT_RUNNING,
        format!(
            "`weaver {}` needs Weaver, and Weaver is not running.",
            call.command.name()
        ),
    ))
}

/// The shape of one question of `question new` (also the question of a prepared Drift).
pub fn question_fields() -> Vec<Field> {
    vec![
        req("id", line(), "Unique in the round, non-empty."),
        req(
            "question",
            nonblank(),
            "The question, in Markdown, self-contained: the developer sees only the round.",
        ),
        opt(
            "context",
            nonblank(),
            "What the developer needs to answer, in Markdown.",
        ),
        req(
            "options",
            list_min(
                object(vec![
                    req("label", line(), "The answer; plain text, not Markdown."),
                    req(
                        "description",
                        nonblank(),
                        "What choosing it means, in Markdown.",
                    ),
                    opt("details", nonblank(), "More detail, in Markdown."),
                ]),
                2,
            ),
            "At least two. Never an \"Other\" option: a free answer is always offered.",
        ),
        req(
            "recommended",
            int_min(0),
            "The index of the recommended option, from 0.",
        ),
        req("multi", boolean(), "Whether several options may be chosen."),
    ]
}
