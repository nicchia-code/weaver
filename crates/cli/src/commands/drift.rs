//! `weaver drift new | show | list | edit | move`. The rules (the fingerprint, the prepared
//! question, the decisions and their reasons) live in `weaver_core::project::drift`; this file reads
//! the input, applies the changes and prints.
//!
//! The decisions of `move` are the developer's: the same functions the app runs for the buttons
//! of the card, under the same lock.

use std::collections::BTreeSet;
use std::convert::Infallible;

use serde_json::{json, Map, Value};
use weaver_core::command::CommandReply;
use weaver_core::jobs::JobKind;
use weaver_core::project::board::Refused;
use weaver_core::project::cards::{Card, CardKind};
use weaver_core::project::changes::{self, ChangeSet, MutationError};
use weaver_core::project::clock::{now_instant, SystemClock};
use weaver_core::project::drift::create::{self, EditDrift, NewDrift, PreparedInput};
use weaver_core::project::drift::decide::{self, Decision};
use weaver_core::project::drift::prepared::{self, Preparation, PreparedDecision};
use weaver_core::project::drift::{self, drift_options, reasons, FixOption};
use weaver_core::project::image::ProjectImage;
use weaver_core::project::proposal::data::{self, Status};
use weaver_core::project::random::OsRandom;
use weaver_core::project::resolve::resolve_card;
use weaver_core::project::shortnum::{self, ShortNumbers};

use super::{card_with, local, question_fields, reply, written};
use crate::spec::*;

const STATES: &[&str] = &["code-to-be-fixed", "proposal-accepted", "dismissed"];

pub fn commands() -> Vec<Command> {
    vec![
        Command::new("drift", "new", "Files a Drift: the code and the Model or the Requirements disagree.", new)
            .notes(&[
                "`options` are 2 to 5 fix options, the recommended first; they may be left out only with `proposal` (a Draft or Open Proposal no other Drift holds). `task` is the Task the Drift was found in; `previous` is set automatically when that Task is a fix Task. `merge` is only `true` and only for the agent of a scan job started after a merge (the instruction says so), for a Drift whose difference lies in the files the merge brought; from any other caller it is refused and nothing is written, and no command changes it afterwards. `announcement` is the identity printed by `weaver announcement new`. The job that files it is recorded when `WEAVER_JOB` is set.",
            ])
            .input(vec![
                req("title", line(), "The title."),
                req("refs", list_min(line(), 1), "What the Drift concerns: Model elements, Requirement sections, Specs fields."),
                req("happens", nonblank(), "What the code does."),
                req("should", nonblank(), "What the Model or the Requirements say it should do."),
                req("where", nonblank(), "Where in the code."),
                opt("options", list_between(object(vec![req("title", line(), "The option."), req("text", nonblank(), "What fixing the code this way means.")]), 2, 5), "The fix options, the recommended first."),
                opt("task", line(), "The Task the Drift was found in."),
                opt("proposal", line(), "A Draft or Open Proposal that resolves it."),
                opt("previous", line(), "The Drift this one follows."),
                opt("merge", boolean(), "`true` when the difference lies in the files the merge brought; only from a scan after a merge."),
                opt("announcement", line(), "The announcement of the card."),
            ])
            .output(card_with(vec![req("status", line(), "The status: `open`."), opt("merge", boolean(), "`true` for a Drift from a merge.")]))
            .check(check_new)
            .place(written)
            .example("weaver drift new --input -"),
        Command::new("drift", "show", "Prints a Drift with its options, Task, Proposal and prepared question.", show)
            .notes(&[
                "`previous` lists the Drifts this one repeats, the nearest first, with the decision taken on each. `prepared` is the prepared question with `valid`; a block that does not read is reported in `problem`.",
            ])
            .arg(Arg::Card(CardKind::Drift))
            .output(card_with(vec![
                req("status", line(), "The status."),
                opt("merge", boolean(), "`true` when the Drift comes from a merge."),
                req("refs", list(line()), "What it concerns."),
                req("happens", text(), "What the code does."),
                req("should", text(), "What it should do."),
                req("where", text(), "Where in the code."),
                req("options", list(object(vec![req("title", line(), "The option."), req("text", text(), "What it means.")])), "The fix options."),
                req("created", line(), "The instant it was filed."),
                opt("origin", object(card_with(vec![])), "The Task it was found in."),
                req("previous", list(object(card_with(vec![req("status", line(), "Its status."), opt("decision", Ty::Any, "The decision taken: `{decision, option?, note?}`.")]))), "The Drifts it repeats, the nearest first."),
                opt("proposal", object(card_with(vec![req("status", line(), "Its status.")])), "The Proposal attached to it."),
                opt("task", object(card_with(vec![])), "Its fix Task."),
                opt("job", line(), "The job that filed it."),
                opt("decided", line(), "The instant of the decision."),
                opt("prepared", object(vec![
                    req("valid", boolean(), "Whether Resolve asks it at once."),
                    opt("problem", text(), "Why it is not valid, or why it does not read."),
                    opt("question", object(question_fields()), "The question."),
                    opt("decisions", list(object(vec![req("decision", line(), "The decision."), opt("option", int_min(1), "The fix option.")])), "The decision of each option."),
                    opt("prepared", line(), "The instant it was prepared."),
                ]), "The prepared question."),
            ]))
            .example("weaver drift show d3"),
        Command::new("drift", "list", "Lists Drifts.", list_drifts)
            .notes(&[
                "Open Drifts only, unless `all`. `refs` lists the Drifts that concern those references, as the Drifts that block a Task, and marks (`unchanged`) the Dismissed Drifts that concern them whose text is unchanged since the dismissal: the agent never files those differences again. `currentJob` uses `WEAVER_JOB` and lists nothing outside a job. `unprepared` lists the open Drifts that have no valid prepared question and are in no resolution being written or open.",
            ])
            .input(vec![
                opt("all", boolean(), "Every Drift, decided ones included."),
                opt("refs", list(line()), "Only the Drifts that concern these references."),
                opt("job", line(), "Only the Drifts filed by this job."),
                opt("currentJob", boolean(), "Only the Drifts filed by the job of the caller."),
                opt("unprepared", boolean(), "Only the open Drifts without a valid prepared question."),
            ])
            .output(vec![req("drifts", list(object(card_with(vec![
                req("status", line(), "The status."),
                opt("merge", boolean(), "`true` when the Drift comes from a merge."),
                opt("unchanged", boolean(), "A Dismissed Drift whose references have the same text as when it was dismissed."),
            ]))), "The Drifts.")])
            .place(local)
            .example(r#"weaver drift list --input '{"currentJob":true,"unprepared":true}'"#),
        Command::new("drift", "edit", "Records the prepared question of a Drift, or attaches a Proposal to it.", edit)
            .notes(&[
                "`prepared` is `{question, decisions}`: `question` is a question of `weaver question new` whose `id` is the Drift identity and whose `multi` is false, and `decisions` has one decision per option, in the same order: every fix option once as `fix-code` with its `option`, plus one `proposal` or `dismissed` (both when the Drift has no fix option). Weaver adds the fingerprint and the instant. `proposal` attaches a Proposal to a Drift that has none.",
            ])
            .arg(Arg::Card(CardKind::Drift))
            .input(vec![
                opt("prepared", object(vec![
                    req("question", object(question_fields()), "The question."),
                    req("decisions", list_min(object(vec![
                        req("decision", one_of(&["fix-code", "proposal", "dismissed"]), "The decision the option stands for."),
                        opt("option", int_min(1), "The fix option, from 1; with `fix-code`."),
                    ]), 1), "One decision per option of the question."),
                ]), "The prepared question."),
                opt("proposal", line(), "A Proposal to attach."),
            ])
            .output(card_with(vec![
                req("status", line(), "The status."),
                req("prepared", boolean(), "Whether the prepared question was recorded."),
                opt("proposal", line(), "The identity of the Proposal attached by this call."),
            ]))
            .place(written)
            .example("weaver drift edit d3 --input -"),
        Command::new("drift", "move", "Decides a Drift.", move_drift)
            .notes(&[
                "The developer's decisions: the agent runs them only when the developer explicitly asked for that decision in the conversation. `code-to-be-fixed` needs `option` (from 1) when the Drift has options; `proposal-accepted` and `dismissed` take none.",
            ])
            .arg(Arg::Card(CardKind::Drift))
            .states(STATES)
            .input(vec![
                opt("option", int_min(1), "The fix option, from 1; with `code-to-be-fixed`."),
                opt("note", nonblank(), "A note for the fix Task."),
            ])
            .output(card_with(vec![
                req("status", line(), "The status after the decision."),
                opt("task", object(card_with(vec![])), "The fix Task created by `code-to-be-fixed`."),
            ]))
            .place(written)
            .example("weaver drift move d3 dismissed"),
    ]
}

// ---------------------------------------------------------------------------------------------
// Shared
// ---------------------------------------------------------------------------------------------

fn strings(input: &Map<String, Value>, key: &str) -> Vec<String> {
    input
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn str_of<'a>(input: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    input.get(key).and_then(Value::as_str)
}

fn flag(input: &Map<String, Value>, key: &str) -> bool {
    input.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn now() -> Result<String, Failure> {
    now_instant(&SystemClock).map_err(|e| Failure::failed(e.to_string()))
}

fn name(numbers: &ShortNumbers, kind: CardKind, id: &str) -> String {
    numbers.label_of(kind, id).unwrap_or_else(|| id.to_owned())
}

fn status_of(card: &Card) -> &str {
    card.str_key("status").unwrap_or("unknown")
}

fn card_json(numbers: &ShortNumbers, kind: CardKind, card: &Card) -> Value {
    json!({ "id": card.id, "number": numbers.number_of(kind, &card.id), "title": card.title })
}

fn drift_json(numbers: &ShortNumbers, card: &Card) -> Value {
    let mut value = card_json(numbers, CardKind::Drift, card);
    value["status"] = json!(status_of(card));
    if drift::is_merge(card) {
        value["merge"] = json!(true);
    }
    value
}

/// `merge` is only `true`.
fn check_new(input: &Map<String, Value>) -> Vec<Problem> {
    match input.get("merge") {
        Some(Value::Bool(false)) => vec![Problem::new(
            "merge",
            "must be true (leave it out otherwise)",
        )],
        _ => Vec::new(),
    }
}

/// The caller is the agent of a scan job started after a merge (`WEAVER_JOB`, and the `merged`
/// files of the job's registry record).
fn is_scan_after_merge(call: &Call) -> bool {
    call.caller.job.is_some()
        && call
            .caller
            .job_record
            .as_ref()
            .is_some_and(|record| record.kind == JobKind::Scan && record.merged.is_some())
}

fn bullets(prefix: &str, problems: &[(String, String)]) -> String {
    let mut message = prefix.to_owned();
    for (path, why) in problems {
        if path.is_empty() {
            message.push_str(&format!("\n  - {why}"));
        } else {
            message.push_str(&format!("\n  - {path}: {why}"));
        }
    }
    message
}

fn problems_of(found: Vec<create::Problem>) -> Vec<(String, String)> {
    found.into_iter().map(|p| (p.path, p.message)).collect()
}

fn write(call: &Call, changes: &ChangeSet) -> Result<ProjectImage, Failure> {
    changes::apply(call.root, changes, |_| Ok::<(), Infallible>(())).map_err(|error| match error {
        MutationError::Refused(never) => match never {},
        other => Failure::failed(other.to_string()),
    })
}

fn refused_failure(error: MutationError<Refused>) -> Failure {
    match error {
        MutationError::Refused(refused) => Failure::refused(refused.notice),
        other => Failure::failed(other.to_string()),
    }
}

/// A card named in the input, resolved to its identity: `key[index]: <why>` when it cannot be.
fn resolve_named(call: &Call, key: &str, kind: CardKind) -> Result<Option<String>, Failure> {
    match str_of(call.input, key) {
        Some(given) => resolve_card(call.image, kind, given)
            .map(Some)
            .map_err(|e| Failure::refused(format!("{key}: {e}"))),
        None => Ok(None),
    }
}

// ---------------------------------------------------------------------------------------------
// new
// ---------------------------------------------------------------------------------------------

fn new(call: &Call) -> Result<CommandReply, Failure> {
    let options: Vec<FixOption> = call
        .input
        .get("options")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| FixOption {
                    title: item
                        .get("title")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                    text: item
                        .get("text")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                })
                .collect()
        })
        .unwrap_or_default();
    let merge = flag(call.input, "merge");
    if merge && !is_scan_after_merge(call) {
        return Err(Failure::refused(
            "`merge` is only for the agent of a scan job started after a merge: nothing was written.",
        ));
    }
    let input = NewDrift {
        title: str_of(call.input, "title").unwrap_or_default().to_owned(),
        refs: strings(call.input, "refs"),
        happens: str_of(call.input, "happens").unwrap_or_default().to_owned(),
        should: str_of(call.input, "should").unwrap_or_default().to_owned(),
        where_in_code: str_of(call.input, "where").unwrap_or_default().to_owned(),
        options,
        task: resolve_named(call, "task", CardKind::Task)?,
        proposal: resolve_named(call, "proposal", CardKind::Proposal)?,
        previous: resolve_named(call, "previous", CardKind::Drift)?,
        job: call.caller.job.clone(),
        merge,
    };
    let created = now()?;
    let filed = create::plan_new(call.image, &input, &created, &mut OsRandom)
        .map_err(|found| Failure::refused(bullets("Nothing was written:", &problems_of(found))))?;
    let after = write(call, &filed.changes)?;
    let numbers = after.short_numbers();
    let card = after.card(CardKind::Drift, &filed.id).ok_or_else(|| {
        Failure::failed(format!(
            "the Drift file {} was written but cannot be read back",
            filed.path
        ))
    })?;
    let label = name(&numbers, CardKind::Drift, &card.id);
    let markdown = format!(
        "# Drift {label} filed\n\n`{}` {}\n\nFile: `{}`\n",
        card.id, card.title, filed.path
    );
    Ok(reply(markdown, drift_json(&numbers, card)))
}

// ---------------------------------------------------------------------------------------------
// show
// ---------------------------------------------------------------------------------------------

/// The decision taken on a decided Drift, for the chain of `previous`.
fn decision_of(card: &Card) -> Option<Value> {
    match status_of(card) {
        "code-to-be-fixed" | "fixed" => {
            let option = drift::option_of(card);
            let title = option.and_then(|o| {
                drift_options(card)
                    .get((o as usize).saturating_sub(1))
                    .map(|o| o.title.clone())
            });
            let mut value = json!({ "decision": "fix-code" });
            if let Some(option) = option {
                value["option"] = json!(option);
            }
            if let Some(title) = title {
                value["optionTitle"] = json!(title);
            }
            if let Some(note) = card.str_key("note") {
                value["note"] = json!(note);
            }
            Some(value)
        }
        "proposal-accepted" => Some(json!({ "decision": "proposal" })),
        "dismissed" => {
            let mut value = json!({ "decision": "dismissed" });
            if let Some(note) = card.str_key("note") {
                value["note"] = json!(note);
            }
            Some(value)
        }
        _ => None,
    }
}

/// The Drifts `card` repeats, nearest first; a loop in the files ends the chain.
fn chain_of<'a>(image: &'a ProjectImage, card: &'a Card) -> Vec<&'a Card> {
    let mut chain = Vec::new();
    let mut seen: BTreeSet<&str> = BTreeSet::from([card.id.as_str()]);
    let mut next = card.str_key("previous");
    while let Some(id) = next {
        let Some(found) = image
            .card(CardKind::Drift, id)
            .filter(|c| seen.insert(c.id.as_str()))
        else {
            break;
        };
        chain.push(found);
        next = found.str_key("previous");
    }
    chain
}

fn prepared_json(preparation: &Preparation) -> Option<Value> {
    let (valid, problem, prepared) = match preparation {
        Preparation::Missing => return None,
        Preparation::Unreadable(why) => (false, Some(why.as_str()), None),
        Preparation::Invalid(prepared, why) => (false, Some(why.as_str()), Some(prepared)),
        Preparation::Valid(prepared) => (true, None, Some(prepared)),
    };
    let mut value = json!({ "valid": valid });
    if let Some(problem) = problem {
        value["problem"] = json!(problem);
    }
    if let Some(prepared) = prepared {
        value["question"] = serde_json::to_value(&prepared.question).unwrap_or(Value::Null);
        value["decisions"] = serde_json::to_value(&prepared.decisions).unwrap_or(Value::Null);
        value["prepared"] = json!(prepared.prepared);
    }
    Some(value)
}

fn show(call: &Call) -> Result<CommandReply, Failure> {
    let id = call.arg.expect("a Drift is named");
    let image = call.image;
    let card = image
        .card(CardKind::Drift, id)
        .ok_or_else(|| Failure::refused(format!("no Drift has the identity {id}")))?;
    let numbers = shortnum::peek(call.root, image.cards());
    let label = name(&numbers, CardKind::Drift, id);
    let options = drift_options(card);
    let refs = drift::refs_of(card);
    let happens = drift::section_text(&card.body, drift::HAPPENS);
    let should = drift::section_text(&card.body, drift::SHOULD);
    let where_in_code = drift::section_text(&card.body, drift::WHERE_IN_CODE);
    let preparation = prepared::check(image, card);
    let chain = chain_of(image, card);

    let mut markdown = format!("# {label} {}\n\n`{id}` · {}", card.title, status_of(card));
    if !chain.is_empty() {
        markdown.push_str(" · recurring");
    }
    if drift::is_merge(card) {
        markdown.push_str(" · merge");
    }
    if let Some(job) = card.str_key("job") {
        markdown.push_str(&format!(" · filed by job `{job}`"));
    }
    markdown.push('\n');
    markdown.push_str("\n## Concerns\n\n");
    for reference in &refs {
        markdown.push_str(&format!("- `{reference}`\n"));
    }
    markdown.push_str(&format!(
        "\n## What happens\n\n{happens}\n\n## What should happen\n\n{should}\n"
    ));
    if options.is_empty() {
        markdown.push_str("\n## Fix options\n\nNone: no change of the code can respect both the Model and the Requirements. An attached Proposal is required.\n");
    } else {
        markdown.push_str("\n## Fix options\n");
        for (index, option) in options.iter().enumerate() {
            let recommended = if index == 0 { " (recommended)" } else { "" };
            markdown.push_str(&format!(
                "\n### {}. {}{recommended}\n\n{}\n",
                index + 1,
                option.title,
                option.text
            ));
        }
    }
    markdown.push_str(&format!("\n## Where in the code\n\n{where_in_code}\n"));

    let origin = card
        .str_key("origin")
        .and_then(|task| image.card(CardKind::Task, task).map(|t| (task, t)));
    if let Some(task) = card.str_key("origin") {
        let title = origin.map_or(String::new(), |(_, t)| format!(" {}", t.title));
        markdown.push_str(&format!(
            "\n## Origin\n\nFound while working on Task {}{title}.\n",
            name(&numbers, CardKind::Task, task)
        ));
    }
    if !chain.is_empty() {
        markdown.push_str("\n## Repeats\n\n");
        for earlier in &chain {
            let decision = decision_of(earlier).map_or(String::new(), |d| {
                let mut text = d["decision"].as_str().unwrap_or_default().to_owned();
                if let Some(title) = d["optionTitle"].as_str() {
                    text.push_str(&format!(" ({title})"));
                }
                format!(" — {text}")
            });
            markdown.push_str(&format!(
                "- {} `{}` {} ({}){decision}\n",
                name(&numbers, CardKind::Drift, &earlier.id),
                earlier.id,
                earlier.title,
                status_of(earlier)
            ));
        }
    }
    let attached = card
        .str_key("proposal")
        .and_then(|p| image.card(CardKind::Proposal, p));
    if let Some(proposal) = attached {
        let status = data::status_of(proposal).map_or("unknown", Status::as_str);
        markdown.push_str(&format!(
            "\n## Attached Proposal\n\n{} `{}` {} ({status}). Read it with `weaver proposal show {}`.\n",
            name(&numbers, CardKind::Proposal, &proposal.id),
            proposal.id,
            proposal.title,
            proposal.id
        ));
    }
    let fix_task = card
        .str_key("task")
        .and_then(|t| image.card(CardKind::Task, t));
    if let Some(task) = card.str_key("task") {
        let title = fix_task.map_or(String::new(), |t| format!(" {}", t.title));
        markdown.push_str(&format!(
            "\n## Fix Task\n\n{} `{task}`{title}.\n",
            name(&numbers, CardKind::Task, task)
        ));
    }
    if let Some(decided) = card.str_key("decided") {
        markdown.push_str(&format!("\n## Decision\n\nDecided at {decided}."));
        if let Some(option) = drift::option_of(card) {
            markdown.push_str(&format!(" Fix option {option}."));
        }
        if let Some(note) = card.str_key("note") {
            markdown.push_str(&format!("\n\nNote: {note}"));
        }
        markdown.push('\n');
    }
    match &preparation {
        Preparation::Missing => markdown.push_str("\n## Prepared question\n\nNone.\n"),
        Preparation::Unreadable(why) => markdown.push_str(&format!("\n## Prepared question\n\nNot valid: it does not read ({why}). It counts as no prepared question.\n")),
        Preparation::Invalid(prepared, why) => {
            markdown.push_str(&format!("\n## Prepared question\n\nNot valid: {why}.\n\n{}\n", prepared.question.question));
        }
        Preparation::Valid(prepared) => {
            markdown.push_str(&format!("\n## Prepared question\n\nValid: Resolve asks it at once.\n\n{}\n", prepared.question.question));
            for (index, option) in prepared.question.options.iter().enumerate() {
                let decision = prepared.decisions.get(index).map_or(String::new(), |d| match d.option {
                    Some(n) => format!("{} {n}", d.decision),
                    None => d.decision.clone(),
                });
                let recommended = if index == prepared.question.recommended { ", recommended" } else { "" };
                markdown.push_str(&format!("- {} → {decision}{recommended}\n", option.label));
            }
        }
    }

    let mut json = drift_json(&numbers, card);
    json["refs"] = json!(refs);
    json["happens"] = json!(happens);
    json["should"] = json!(should);
    json["where"] = json!(where_in_code);
    json["options"] = json!(options
        .iter()
        .map(|o| json!({ "title": o.title, "text": o.text }))
        .collect::<Vec<_>>());
    json["created"] = json!(card.created.clone().unwrap_or_default());
    if let Some(task) = card.str_key("origin") {
        json["origin"] = origin.map_or_else(
            || json!({ "id": task, "number": Value::Null, "title": "" }),
            |(_, t)| card_json(&numbers, CardKind::Task, t),
        );
    }
    json["previous"] = json!(chain
        .iter()
        .map(|earlier| {
            let mut value = drift_json(&numbers, earlier);
            if let Some(decision) = decision_of(earlier) {
                value["decision"] = decision;
            }
            value
        })
        .collect::<Vec<_>>());
    if let Some(proposal) = attached {
        let mut value = card_json(&numbers, CardKind::Proposal, proposal);
        value["status"] = json!(data::status_of(proposal).map_or("unknown", Status::as_str));
        json["proposal"] = value;
    }
    if let Some(task) = card.str_key("task") {
        json["task"] = fix_task.map_or_else(
            || json!({ "id": task, "number": Value::Null, "title": "" }),
            |t| card_json(&numbers, CardKind::Task, t),
        );
    }
    if let Some(job) = card.str_key("job") {
        json["job"] = json!(job);
    }
    if let Some(decided) = card.str_key("decided") {
        json["decided"] = json!(decided);
    }
    if let Some(value) = prepared_json(&preparation) {
        json["prepared"] = value;
    }
    Ok(reply(markdown, json))
}

// ---------------------------------------------------------------------------------------------
// list
// ---------------------------------------------------------------------------------------------

fn list_drifts(call: &Call) -> Result<CommandReply, Failure> {
    let image = call.image;
    let numbers = shortnum::peek(call.root, image.cards());
    let all = flag(call.input, "all");
    let unprepared = flag(call.input, "unprepared");
    let refs = strings(call.input, "refs");
    let filtering_refs = call.input.contains_key("refs");
    let job = str_of(call.input, "job");
    let current_job = flag(call.input, "currentJob");

    let mut markdown = String::from("# Drifts\n\n");
    let mut entries = Vec::new();
    for card in image.cards_of(CardKind::Drift) {
        let open = drift::is_open(card);
        // With `refs`, the Dismissed Drifts that concern the same thing and whose text has not
        // changed are shown too, marked: the agent never files those differences again.
        let standing = filtering_refs && reasons::dismissal_stands(image, card);
        if !(all || open || standing) {
            continue;
        }
        if unprepared && !reasons::is_unprepared(image, card) {
            continue;
        }
        if filtering_refs && !reasons::concerns(image, card, &refs) {
            continue;
        }
        if let Some(job) = job {
            if card.str_key("job") != Some(job) {
                continue;
            }
        }
        if current_job
            && !(call.caller.job.is_some() && card.str_key("job") == call.caller.job.as_deref())
        {
            continue;
        }
        let mark = if standing {
            ", unchanged since the dismissal"
        } else {
            ""
        };
        markdown.push_str(&format!(
            "- `{}` {} ({}{mark}) {}\n",
            name(&numbers, CardKind::Drift, &card.id),
            card.id,
            status_of(card),
            card.title
        ));
        let mut entry = drift_json(&numbers, card);
        if standing {
            entry["unchanged"] = json!(true);
        }
        entries.push(entry);
    }
    if entries.is_empty() {
        markdown.push_str("No Drift.\n");
    }
    Ok(reply(markdown, json!({ "drifts": entries })))
}

// ---------------------------------------------------------------------------------------------
// edit
// ---------------------------------------------------------------------------------------------

fn prepared_input(value: &Value) -> PreparedInput {
    let decisions = value
        .get("decisions")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| PreparedDecision {
                    decision: item
                        .get("decision")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                    option: item
                        .get("option")
                        .and_then(Value::as_u64)
                        .and_then(|n| u32::try_from(n).ok()),
                })
                .collect()
        })
        .unwrap_or_default();
    PreparedInput {
        question: value.get("question").cloned().unwrap_or(Value::Null),
        decisions,
    }
}

fn edit(call: &Call) -> Result<CommandReply, Failure> {
    let id = call.arg.expect("a Drift is named");
    let request = EditDrift {
        prepared: call.input.get("prepared").map(prepared_input),
        proposal: resolve_named(call, "proposal", CardKind::Proposal)?,
    };
    if request.prepared.is_none() && request.proposal.is_none() {
        return Err(Failure::invalid(
            "Invalid input:\n  - prepared, proposal: give at least one",
        ));
    }
    let numbers = shortnum::peek(call.root, call.image.cards());
    let label = name(&numbers, CardKind::Drift, id);
    let recorded = now()?;
    let edited = create::plan_edit(call.image, id, &request, &recorded).map_err(|found| {
        Failure::refused(bullets(
            &format!("Could not edit {label}. Nothing was written:"),
            &problems_of(found),
        ))
    })?;
    let after = write(call, &edited.changes)?;
    let numbers = after.short_numbers();
    let card = after.card(CardKind::Drift, id).ok_or_else(|| {
        Failure::failed(format!(
            "the Drift {id} was written but cannot be read back"
        ))
    })?;
    let mut markdown = format!("# {label} edited\n");
    if edited.prepared {
        markdown.push_str("\nThe prepared question is recorded: Resolve asks it at once while the Model and the Requirements the Drift concerns do not change.\n");
    }
    if let Some(proposal) = &edited.attached {
        markdown.push_str(&format!("\nThe Proposal `{proposal}` is attached.\n"));
    }
    let mut json = drift_json(&numbers, card);
    json["prepared"] = json!(edited.prepared);
    if let Some(proposal) = &edited.attached {
        json["proposal"] = json!(proposal);
    }
    Ok(reply(markdown, json))
}

// ---------------------------------------------------------------------------------------------
// move
// ---------------------------------------------------------------------------------------------

fn move_drift(call: &Call) -> Result<CommandReply, Failure> {
    let id = call.arg.expect("a Drift is named");
    let state = call.state.expect("a state is named");
    if state != "code-to-be-fixed"
        && (call.input.contains_key("option") || call.input.contains_key("note"))
    {
        return Err(Failure::invalid(
            "Invalid input:\n  - option, note: go with the state `code-to-be-fixed`",
        ));
    }
    let decision = match state {
        "code-to-be-fixed" => Decision::FixCode {
            option: call
                .input
                .get("option")
                .and_then(Value::as_u64)
                .and_then(|n| u32::try_from(n).ok()),
            note: str_of(call.input, "note").map(str::to_owned),
        },
        "proposal-accepted" => Decision::AcceptProposal,
        _ => Decision::Dismiss,
    };
    let numbers = shortnum::peek(call.root, call.image.cards());
    let label = name(&numbers, CardKind::Drift, id);
    let decided_at = now()?;
    let (after, decided) = decide::decide(
        call.root,
        &call.caller.agent,
        id,
        &decision,
        &decided_at,
        &mut OsRandom,
    )
    .map_err(refused_failure)?;
    let numbers = after.short_numbers();
    let card = after.card(CardKind::Drift, id).ok_or_else(|| {
        Failure::failed(format!(
            "the Drift {id} was written but cannot be read back"
        ))
    })?;
    let mut markdown = match state {
        "code-to-be-fixed" => format!("# {label}: the code is to be fixed\n"),
        "proposal-accepted" => format!("# {label}: Proposal accepted\n\nThe Model and the Requirements now say what the code does.\n"),
        _ => format!("# {label} dismissed\n\nNothing changes in the code or in the Model.\n"),
    };
    let mut json = drift_json(&numbers, card);
    if let Some(task) = decided
        .fix_task
        .as_deref()
        .and_then(|task| after.card(CardKind::Task, task))
    {
        markdown.push_str(&format!(
            "\nThe fix Task {} `{}` is in To do.\n",
            name(&numbers, CardKind::Task, &task.id),
            task.id
        ));
        json["task"] = card_json(&numbers, CardKind::Task, task);
    }
    let mut result = reply(markdown, json);
    if let Some(Err(why)) = &decided.skills {
        result.note = Some(format!(
            "The Requirement skills could not be generated again: {why}. The decision stands."
        ));
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use serde_json::{json, Map, Value};
    use weaver_core::project::text::{file_content, sha256_hex};

    use super::*;

    const SYSTEM: &str = "# Shop\n\nA shop.\n";
    const FEATURE: &str = "# Shopping\n\nBuying things.\n";
    const ORDER: &str = "# Order\n\nAn order.\n\n## States\n\n### Draft (initial)\nChoosing.\n- `CONFIRM` → Paid\n\n### Paid\n\n## Events\n\n### CONFIRM\nConfirmed.\n";

    fn put(root: &Path, rel: &str, text: &str) {
        let path = root.join(".weaver").join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn project() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        put(dir.path(), "model/system.md", SYSTEM);
        put(dir.path(), "model/shop/feature.md", FEATURE);
        put(dir.path(), "model/shop/order.md", ORDER);
        dir
    }

    fn input(value: Value) -> Map<String, Value> {
        value.as_object().cloned().unwrap_or_default()
    }

    fn run(
        root: &Path,
        object: &str,
        verb: &str,
        arg: Option<&str>,
        state: Option<&str>,
        value: Value,
        caller: &Caller,
    ) -> Result<CommandReply, Failure> {
        let request = weaver_core::command::CommandRequest {
            object: object.to_owned(),
            verb: verb.to_owned(),
            card: arg.map(str::to_owned),
            state: state.map(str::to_owned),
            input: input(value),
            root: root.to_string_lossy().into_owned(),
            general: None,
            ask: None,
        };
        crate::exec::execute(&request, caller, &crate::exec::NoJobs)
    }

    fn new_drift(root: &Path, value: Value) -> Result<CommandReply, Failure> {
        run(root, "drift", "new", None, None, value, &Caller::default())
    }

    fn drift_input() -> Value {
        json!({
            "title": "The code ships twice",
            "refs": ["order/Paid"],
            "happens": "It ships twice.",
            "should": "It ships once.",
            "where": "`src/ship.rs`",
            "options": [{"title": "Deduplicate", "text": "Keep one shipment."}, {"title": "Lock", "text": "Lock the order."}],
        })
    }

    fn question(id: &str) -> Value {
        json!({
            "id": id,
            "question": "What now?",
            "options": [
                {"label": "Deduplicate", "description": "d"},
                {"label": "Lock", "description": "d"},
                {"label": "Keep it", "description": "d"},
            ],
            "recommended": 0,
            "multi": false,
        })
    }

    fn hash(text: &str) -> String {
        sha256_hex(&file_content(text))
    }

    /// The registry record of the job `jjjjjjjj`: a scan, after a merge or not, or another kind.
    fn job_caller(kind: &str, merged: Option<Value>) -> Caller {
        let mut record = json!({
            "id": "jjjjjjjj",
            "kind": kind,
            "name": "Scan",
            "project": "/p",
            "state": "running",
            "created": "2026-11-05T10:00:00.000Z",
        });
        if let Some(merged) = merged {
            record["merged"] = merged;
        }
        Caller {
            job: Some("jjjjjjjj".into()),
            job_record: Some(serde_json::from_value(record).expect("a valid record")),
            ..Caller::default()
        }
    }

    fn merge_input() -> Value {
        let mut value = drift_input();
        value["merge"] = json!(true);
        value
    }

    fn drifts_written(root: &Path) -> usize {
        fs::read_dir(root.join(".weaver/drifts")).map_or(0, |entries| entries.count())
    }

    #[test]
    fn merge_is_accepted_only_from_the_agent_of_a_scan_after_a_merge() {
        let dir = project();
        let root = dir.path();
        let file = |caller: &Caller| run(root, "drift", "new", None, None, merge_input(), caller);
        for (why, caller) in [
            ("outside a job", Caller::default()),
            ("a design job", job_caller("design", None)),
            (
                "a design job with files",
                job_caller("design", Some(json!(["a.rs"]))),
            ),
            (
                "a scan that does not follow a merge",
                job_caller("scan", None),
            ),
            (
                "a job without a record",
                Caller {
                    job: Some("jjjjjjjj".into()),
                    ..Caller::default()
                },
            ),
            ("the merge conflicts job", job_caller("merge", None)),
        ] {
            let failure = file(&caller).unwrap_err();
            assert_eq!(failure.exit_code(), 1, "{why}");
            assert!(
                failure.message.contains("scan job started after a merge"),
                "{why}: {}",
                failure.message
            );
            assert_eq!(drifts_written(root), 0, "{why}: nothing is written");
        }

        let filed = file(&job_caller("scan", Some(json!(["src/ship.rs"])))).unwrap();
        assert_eq!(filed.json["merge"], true);
        assert_eq!(drifts_written(root), 1);
        let text = fs::read_to_string(
            fs::read_dir(root.join(".weaver/drifts"))
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .path(),
        )
        .unwrap();
        assert!(
            text.contains("\njob: jjjjjjjj\nmerge: true\ncreated:"),
            "{text}"
        );
    }

    #[test]
    fn merge_is_only_true_and_a_drift_without_it_never_has_the_key() {
        let dir = project();
        let root = dir.path();
        let mut off = drift_input();
        off["merge"] = json!(false);
        let failure = new_drift(root, off).unwrap_err();
        assert_eq!(failure.exit_code(), 2);
        assert!(failure.message.contains("merge"), "{}", failure.message);
        assert_eq!(drifts_written(root), 0);

        let plain = run(
            root,
            "drift",
            "new",
            None,
            None,
            drift_input(),
            &job_caller("scan", Some(json!(["a.rs"]))),
        )
        .unwrap();
        assert!(plain.json.get("merge").is_none());
    }

    #[test]
    fn show_and_list_print_merge() {
        let dir = project();
        let root = dir.path();
        let merged = run(
            root,
            "drift",
            "new",
            None,
            None,
            merge_input(),
            &job_caller("scan", Some(json!(["src/ship.rs"]))),
        )
        .unwrap();
        let plain = new_drift(root, drift_input()).unwrap();
        let (merged, plain) = (
            merged.json["id"].as_str().unwrap().to_owned(),
            plain.json["id"].as_str().unwrap().to_owned(),
        );

        let shown = |card: &str| {
            run(
                root,
                "drift",
                "show",
                Some(card),
                None,
                json!({}),
                &Caller::default(),
            )
            .unwrap()
        };
        let first = shown(&merged);
        assert_eq!(first.json["merge"], true);
        assert!(first.markdown.contains("· merge"), "{}", first.markdown);
        let second = shown(&plain);
        assert!(second.json.get("merge").is_none());
        assert!(!second.markdown.contains("· merge"), "{}", second.markdown);

        let listed = run(
            root,
            "drift",
            "list",
            None,
            None,
            json!({}),
            &Caller::default(),
        )
        .unwrap();
        let marked: Vec<&str> = listed.json["drifts"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d.get("merge") == Some(&json!(true)))
            .map(|d| d["id"].as_str().unwrap())
            .collect();
        assert_eq!(marked, [merged.as_str()]);
    }

    #[test]
    fn new_files_an_open_drift_and_show_prints_it() {
        let dir = project();
        let root = dir.path();
        let filed = new_drift(root, drift_input()).unwrap();
        assert_eq!(filed.json["status"], "open");
        assert_eq!(filed.json["number"], 1);
        assert!(
            filed.markdown.starts_with("# Drift D1 filed"),
            "{}",
            filed.markdown
        );
        let id = filed.json["id"].as_str().unwrap().to_owned();

        let shown = run(
            root,
            "drift",
            "show",
            Some("d1"),
            None,
            json!({}),
            &Caller::default(),
        )
        .unwrap();
        for expected in [
            "# D1 The code ships twice",
            "- `order/Paid`",
            "## What happens\n\nIt ships twice.",
            "### 1. Deduplicate (recommended)",
            "### 2. Lock\n",
            "## Where in the code\n\n`src/ship.rs`",
            "## Prepared question\n\nNone.",
        ] {
            assert!(
                shown.markdown.contains(expected),
                "{expected}\n{}",
                shown.markdown
            );
        }
        assert_eq!(shown.json["id"], id.as_str());
        assert_eq!(shown.json["options"][1]["title"], "Lock");
        assert_eq!(shown.json["previous"], json!([]));
        assert!(shown.json.get("prepared").is_none());
    }

    #[test]
    fn new_refuses_with_every_problem_and_writes_nothing() {
        let dir = project();
        let root = dir.path();
        let mut bad = drift_input();
        bad["refs"] = json!(["ghost"]);
        bad["task"] = json!("t9");
        bad.as_object_mut().unwrap().remove("options");
        let failure = new_drift(root, bad).unwrap_err();
        assert_eq!(failure.exit_code(), 1);
        assert!(
            failure.message.starts_with("Nothing was written:") || failure.message.contains("t9"),
            "{}",
            failure.message
        );
        assert!(
            !root.join(".weaver/drifts").exists(),
            "a refused command writes nothing"
        );

        let mut bad = drift_input();
        bad["refs"] = json!(["ghost"]);
        bad.as_object_mut().unwrap().remove("options");
        let failure = new_drift(root, bad).unwrap_err();
        assert!(
            failure.message.contains("refs[0]") && failure.message.contains("options"),
            "{}",
            failure.message
        );
        assert!(!root.join(".weaver/drifts").exists());
    }

    #[test]
    fn the_job_that_files_a_drift_is_recorded_and_the_list_filters() {
        let dir = project();
        let root = dir.path();
        let job = Caller {
            job: Some("jjjjjjjj".to_owned()),
            ..Caller::default()
        };
        let mine = run(root, "drift", "new", None, None, drift_input(), &job).unwrap();
        let other = new_drift(root, drift_input()).unwrap();
        let list = |value: Value, caller: &Caller| -> Vec<String> {
            let listed = run(root, "drift", "list", None, None, value, caller).unwrap();
            listed.json["drifts"]
                .as_array()
                .unwrap()
                .iter()
                .map(|d| d["id"].as_str().unwrap().to_owned())
                .collect()
        };
        let (mine_id, other_id) = (
            mine.json["id"].as_str().unwrap().to_owned(),
            other.json["id"].as_str().unwrap().to_owned(),
        );
        assert_eq!(
            list(json!({}), &Caller::default()),
            [mine_id.clone(), other_id.clone()]
        );
        assert_eq!(
            list(json!({ "job": "jjjjjjjj" }), &Caller::default()),
            std::slice::from_ref(&mine_id)
        );
        assert_eq!(
            list(json!({ "currentJob": true }), &job),
            std::slice::from_ref(&mine_id)
        );
        assert_eq!(
            list(json!({ "currentJob": true }), &Caller::default()),
            Vec::<String>::new(),
            "outside a job there is nothing"
        );
        assert_eq!(
            list(json!({ "unprepared": true, "currentJob": true }), &job),
            std::slice::from_ref(&mine_id)
        );
        assert_eq!(
            list(json!({ "refs": ["order"] }), &Caller::default()).len(),
            2,
            "a Drift on a State concerns its Machine"
        );
        assert_eq!(
            list(json!({ "refs": ["shop"] }), &Caller::default()).len(),
            2
        );
        assert_eq!(
            list(json!({ "refs": ["requirements/ui"] }), &Caller::default()).len(),
            0
        );
    }

    #[test]
    fn edit_records_the_prepared_question_and_unprepared_stops_listing_the_drift() {
        let dir = project();
        let root = dir.path();
        let id = new_drift(root, drift_input()).unwrap().json["id"]
            .as_str()
            .unwrap()
            .to_owned();
        let unprepared = |root: &Path| {
            run(
                root,
                "drift",
                "list",
                None,
                None,
                json!({ "unprepared": true }),
                &Caller::default(),
            )
            .unwrap()
            .json["drifts"]
                .as_array()
                .unwrap()
                .len()
        };
        assert_eq!(unprepared(root), 1);

        let edited = run(
            root,
            "drift",
            "edit",
            Some(&id),
            None,
            json!({ "prepared": { "question": question(&id), "decisions": [
                {"decision": "fix-code", "option": 1}, {"decision": "fix-code", "option": 2}, {"decision": "dismissed"},
            ] } }),
            &Caller::default(),
        )
        .unwrap();
        assert_eq!(edited.json["prepared"], true);
        assert_eq!(unprepared(root), 0);
        let shown = run(
            root,
            "drift",
            "show",
            Some(&id),
            None,
            json!({}),
            &Caller::default(),
        )
        .unwrap();
        assert_eq!(shown.json["prepared"]["valid"], true);
        assert!(
            shown.markdown.contains("Valid: Resolve asks it at once.")
                && shown.markdown.contains("Keep it → dismissed"),
            "{}",
            shown.markdown
        );
        let file = fs::read_dir(root.join(".weaver/drifts"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        assert!(
            fs::read_to_string(file).unwrap().contains("```json"),
            "the block is in the file"
        );

        // The Model changes: the question is no longer valid, and the Drift is listed as unprepared again.
        put(
            root,
            "model/shop/order.md",
            &ORDER.replace("### Paid\n", "### Paid\nPaid.\n"),
        );
        assert_eq!(unprepared(root), 1);
        let shown = run(
            root,
            "drift",
            "show",
            Some(&id),
            None,
            json!({}),
            &Caller::default(),
        )
        .unwrap();
        assert_eq!(shown.json["prepared"]["valid"], false);
        assert!(shown.json["prepared"]["problem"]
            .as_str()
            .unwrap()
            .contains("changed"));
    }

    #[test]
    fn edit_refuses_a_question_that_does_not_fit_and_an_empty_edit() {
        let dir = project();
        let root = dir.path();
        let id = new_drift(root, drift_input()).unwrap().json["id"]
            .as_str()
            .unwrap()
            .to_owned();
        let before = fs::read_dir(root.join(".weaver/drifts"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let text = fs::read_to_string(&before).unwrap();
        let failure = run(
            root,
            "drift",
            "edit",
            Some(&id),
            None,
            json!({ "prepared": { "question": question("eeeeeeee"), "decisions": [{"decision": "dismissed"}] } }),
            &Caller::default(),
        )
        .unwrap_err();
        assert_eq!(failure.exit_code(), 1);
        assert!(
            failure.message.contains("prepared.question.id")
                && failure.message.contains("prepared.decisions"),
            "{}",
            failure.message
        );
        assert_eq!(fs::read_to_string(&before).unwrap(), text);
        let empty = run(
            root,
            "drift",
            "edit",
            Some(&id),
            None,
            json!({}),
            &Caller::default(),
        )
        .unwrap_err();
        assert_eq!(empty.exit_code(), 2);
    }

    #[test]
    fn move_decides_the_drift() {
        let dir = project();
        let root = dir.path();
        let id = new_drift(root, drift_input()).unwrap().json["id"]
            .as_str()
            .unwrap()
            .to_owned();

        // An option is required when the Drift offers some.
        let failure = run(
            root,
            "drift",
            "move",
            Some("d1"),
            Some("code-to-be-fixed"),
            json!({}),
            &Caller::default(),
        )
        .unwrap_err();
        assert!(
            failure.message.contains("an option is required"),
            "{}",
            failure.message
        );
        let failure = run(
            root,
            "drift",
            "move",
            Some("d1"),
            Some("dismissed"),
            json!({ "option": 1 }),
            &Caller::default(),
        )
        .unwrap_err();
        assert_eq!(failure.exit_code(), 2);

        let fixed = run(
            root,
            "drift",
            "move",
            Some("d1"),
            Some("code-to-be-fixed"),
            json!({ "option": 2, "note": "Mind the lock" }),
            &Caller::default(),
        )
        .unwrap();
        assert_eq!(fixed.json["status"], "code-to-be-fixed");
        assert_eq!(
            fixed.json["number"], 1,
            "a Drift whose code is to be fixed is live"
        );
        assert_eq!(fixed.json["task"]["number"], 1);
        assert!(
            fixed.markdown.contains("The fix Task T1"),
            "{}",
            fixed.markdown
        );
        let task = run(
            root,
            "task",
            "show",
            Some("t1"),
            None,
            json!({}),
            &Caller::default(),
        )
        .unwrap();
        assert!(
            task.markdown.contains("## Chosen option") && task.markdown.contains("Mind the lock"),
            "{}",
            task.markdown
        );

        // The Drift is decided: no other decision.
        let failure = run(
            root,
            "drift",
            "move",
            Some(&id),
            Some("dismissed"),
            json!({}),
            &Caller::default(),
        )
        .unwrap_err();
        assert!(
            failure.message.contains("the Drift is code-to-be-fixed"),
            "{}",
            failure.message
        );
    }

    #[test]
    fn dismissing_records_the_fingerprint_that_the_list_marks() {
        let dir = project();
        let root = dir.path();
        new_drift(root, drift_input()).unwrap();
        let dismissed = run(
            root,
            "drift",
            "move",
            Some("d1"),
            Some("dismissed"),
            json!({}),
            &Caller::default(),
        )
        .unwrap();
        assert_eq!(dismissed.json["status"], "dismissed");
        assert_eq!(
            dismissed.json["number"],
            Value::Null,
            "a closed Drift has no number"
        );

        let listed = run(
            root,
            "drift",
            "list",
            None,
            None,
            json!({ "refs": ["order/Paid"] }),
            &Caller::default(),
        )
        .unwrap();
        assert_eq!(listed.json["drifts"][0]["unchanged"], true);
        assert!(
            listed.markdown.contains("unchanged since the dismissal"),
            "{}",
            listed.markdown
        );
        assert_eq!(
            run(
                root,
                "drift",
                "list",
                None,
                None,
                json!({}),
                &Caller::default()
            )
            .unwrap()
            .json["drifts"],
            json!([]),
            "decided Drifts are listed with `all`"
        );
        assert_eq!(
            run(
                root,
                "drift",
                "list",
                None,
                None,
                json!({ "all": true }),
                &Caller::default()
            )
            .unwrap()
            .json["drifts"]
                .as_array()
                .unwrap()
                .len(),
            1
        );

        // The text changes: the difference may be filed again.
        put(
            root,
            "model/shop/order.md",
            &ORDER.replace("### Paid\n", "### Paid\nPaid.\n"),
        );
        let listed = run(
            root,
            "drift",
            "list",
            None,
            None,
            json!({ "refs": ["order/Paid"] }),
            &Caller::default(),
        )
        .unwrap();
        assert_eq!(listed.json["drifts"], json!([]));
    }

    #[test]
    fn accepting_the_attached_proposal_approves_it() {
        let dir = project();
        let root = dir.path();
        let changed = ORDER.replace("### Paid\n", "### Paid\nThe customer paid.\n");
        put(
            root,
            "proposals/pppppppp-t/proposal.md",
            &format!("---\nstatus: open\nkind: change\nbase:\n  model/shop/order.md: {}\ncreated: 2026-11-02T07:00:00.000Z\n---\n# Paid orders\n\n## Already present\n\n- `order` — src/order.rs\n", hash(ORDER)),
        );
        put(root, "proposals/pppppppp-t/model/shop/order.md", &changed);
        let mut value = drift_input();
        value["proposal"] = json!("pppppppp");
        value.as_object_mut().unwrap().remove("options");
        let id = new_drift(root, value).unwrap().json["id"]
            .as_str()
            .unwrap()
            .to_owned();
        let shown = run(
            root,
            "drift",
            "show",
            Some(&id),
            None,
            json!({}),
            &Caller::default(),
        )
        .unwrap();
        assert_eq!(shown.json["proposal"]["id"], "pppppppp");
        assert!(
            shown.markdown.contains("No change of the code can respect")
                || shown.markdown.contains("None: no change of the code"),
            "{}",
            shown.markdown
        );

        let accepted = run(
            root,
            "drift",
            "move",
            Some(&id),
            Some("proposal-accepted"),
            json!({}),
            &Caller::default(),
        )
        .unwrap();
        assert_eq!(accepted.json["status"], "proposal-accepted");
        assert!(fs::read_to_string(root.join(".weaver/model/shop/order.md"))
            .unwrap()
            .contains("The customer paid."));
    }

    #[test]
    fn a_drift_found_during_a_fix_task_repeats_the_earlier_one_and_show_walks_the_chain() {
        let dir = project();
        let root = dir.path();
        let first = new_drift(root, drift_input()).unwrap().json["id"]
            .as_str()
            .unwrap()
            .to_owned();
        run(
            root,
            "drift",
            "move",
            Some(&first),
            Some("code-to-be-fixed"),
            json!({ "option": 1 }),
            &Caller::default(),
        )
        .unwrap();
        let fix = run(
            root,
            "drift",
            "show",
            Some(&first),
            None,
            json!({}),
            &Caller::default(),
        )
        .unwrap()
        .json["task"]["id"]
            .as_str()
            .unwrap()
            .to_owned();
        // The fix Task is taken by a job, which finds the same difference again.
        let text = fs::read_to_string(root.join(format!(
                ".weaver/tasks/{}",
                fs::read_dir(root.join(".weaver/tasks"))
                    .unwrap()
                    .next()
                    .unwrap()
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
            )))
        .unwrap();
        let path = fs::read_dir(root.join(".weaver/tasks"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        fs::write(
            path,
            text.replace("column: todo", "column: doing\nholder: jjjjjjjj"),
        )
        .unwrap();
        let mut again = drift_input();
        again["task"] = json!(fix);
        let second = new_drift(root, again).unwrap().json["id"]
            .as_str()
            .unwrap()
            .to_owned();
        let shown = run(
            root,
            "drift",
            "show",
            Some(&second),
            None,
            json!({}),
            &Caller::default(),
        )
        .unwrap();
        assert_eq!(shown.json["previous"][0]["id"], first.as_str());
        assert_eq!(
            shown.json["previous"][0]["decision"]["decision"],
            "fix-code"
        );
        assert_eq!(
            shown.json["previous"][0]["decision"]["optionTitle"],
            "Deduplicate"
        );
        assert!(
            shown.markdown.contains("recurring") && shown.markdown.contains("## Repeats"),
            "{}",
            shown.markdown
        );
    }
}
