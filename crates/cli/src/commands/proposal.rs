//! `weaver proposal new | show | list | edit | move`. The rules (scope, coverage, staleness, the
//! reasons, the transitions) live in `weaver_core::project::proposal`; this file reads the input,
//! applies the changes and prints.
//!
//! A decision that starts a job (approving a stale Proposal, a rework) needs the running app: the
//! app answers it itself before it reaches the handler, and a handler that meets one means no app
//! answered.

use std::collections::BTreeSet;
use std::convert::Infallible;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};
use weaver_core::command::{code, CommandReply};
use weaver_core::jobs::JobKind;
use weaver_core::project::cards::{Card, CardKind};
use weaver_core::project::changes::{self, ChangeSet, MutationError};
use weaver_core::project::clock::{now_instant, SystemClock};
use weaver_core::project::ids::split_card_stem;
use weaver_core::project::image::ProjectImage;
use weaver_core::project::layout::{self, TASK_IMAGES_DIR};
use weaver_core::project::proposal::analysis::{self, Analysis};
use weaver_core::project::proposal::create::{
    self, DriftDecision, EditRequest, NewProposal, Writer,
};
use weaver_core::project::proposal::data::{self, Kind, Status};
use weaver_core::project::proposal::files;
use weaver_core::project::proposal::scope::Coverage;
use weaver_core::project::proposal::transitions::{
    self, Approval, Decider, Refusal, SubmitContext, Submitted,
};
use weaver_core::project::random::OsRandom;
use weaver_core::project::resolve::resolve_card;
use weaver_core::project::shortnum::{self, ShortNumbers};
use weaver_core::project::tasks;

use super::{card_with, cards, local, reply, written};
use crate::spec::*;

const KINDS: &[&str] = &["change", "setup", "import", "resolution"];

/// Image types a Task may carry.
const IMAGE_EXTENSIONS: [&str; 5] = ["png", "jpg", "jpeg", "gif", "webp"];

pub fn commands() -> Vec<Command> {
    vec![
        Command::new("proposal", "new", "Files a Proposal as a Draft.", new)
            .notes(&[
                "`kind` is `change` (default), `setup`, `import` or `resolution`. `label` is one line of 1 to 30 characters. A `setup` is refused on a Model that has a Feature or an Actor, `request` is refused together with `reworks`, and `setup` and `import` are refused from the agent of Chat, which writes only a `change` or a `resolution`; a Proposal the agent of Chat files carries `chat: true` and is marked as written by Chat until it is handed over.",
                "`request` and `reworks` bind only as Weaver allows for the caller: a setup or design job binds only the request its instruction names, and only with its first Proposal; a rework job only the Proposal its instruction names; the agent of Chat binds a request that is written, not sent, not consumed and not held by another Draft, with a `change`, and any rejected `change` or `resolution` (with or without Rework) that nothing replaced and no rework job works on, with a Proposal of the same kind; any other caller binds nothing. A binding that is not allowed leaves the Proposal unbound, with a note saying why. `announcement` is the identity printed by `weaver announcement new`.",
            ])
            .input(vec![
                req("title", line(), "The title."),
                opt("body", text(), "Why the change is needed, in Markdown."),
                opt("kind", one_of(KINDS), "`change` (default), `setup`, `import` or `resolution`."),
                opt("label", line_max(30), "A short label shown on the card."),
                opt("requires", list(line()), "Proposals this one builds on, by identity or short number."),
                opt("request", line(), "The design request it answers."),
                opt("reworks", line(), "The rejected Proposal it replaces."),
                opt("announcement", line(), "The announcement of the card."),
            ])
            .output(card_with(vec![
                req("kind", one_of(KINDS), "The kind."),
                req("status", line(), "The status: `draft`."),
                opt("note", line(), "Why the Proposal is not bound to its request or to the Proposal it reworks."),
            ]))
            .place(written)
            .example(r#"weaver proposal new --input '{"title":"Cancel an order","announcement":"<id>"}'"#),
        Command::new("proposal", "show", "Prints a Proposal: text, status, files, Tasks, scope, blocking and validation.", show)
            .notes(&[
                "Prints the text, the status, the required Proposals, the files, `## Tasks`, `## Created on approval`, `## Scope` (each entry's reason and coverage), `## Blocked` (every reason) and the validation of the project after the Proposal.",
            ])
            .arg(Arg::Card(CardKind::Proposal))
            .output(card_with(vec![
                req("kind", one_of(KINDS), "The kind."),
                req("status", line(), "The status."),
                cards("The Proposals it requires.", "requires"),
                req("files", list(line()), "The files it adds, changes or removes."),
                cards("The Tasks it creates.", "tasks"),
                req("blocked", list(line()), "Every reason it is blocked."),
                req("validation", list(line()), "Every problem the project would have after it."),
            ]))
            .example("weaver proposal show p3"),
        Command::new("proposal", "list", "Lists the Proposals.", list_proposals)
            .notes(&["Lists the Drafts, the Open and the Refining Proposals; with `all`, every Proposal."])
            .input(vec![opt("all", boolean(), "List every Proposal, decided ones included.")])
            .output(vec![req("proposals", list(object(card_with(vec![req("status", line(), "The status.")]))), "The Proposals.")])
            .place(local)
            .example("weaver proposal list"),
        Command::new("proposal", "edit", "Edits a Proposal: makes files editable, records Drift decisions and images.", edit)
            .notes(&[
                "Every call, even without input, sends an Open Proposal back to Draft and says so. From the agent of Chat it also marks the Proposal as written by Chat (`chatWriting: true`) until it is handed over, a Proposal it did not create included.",
                "`edit` makes paths editable (`model/...`, `requirements/<area>/<fragment>.md`, `specs.yaml`, `tasks/<slug>.md`), copying the current content and recording `base` the first time, and prints the path to write for each. A new Task gets its identity here; Task paths are refused in `import` and `resolution`. `delete` is refused for a path the Proposal holds. `drifts` and `removeDrifts` apply only to a resolution; `option` and `note` go only with `fix-code`. `images` copies files into `.weaver/task-images/<task>/`. `rebase: true`, for any kind, attached to a Drift or not, drops broken `requires` and, in a resolution, the rows of Drifts no longer open, then records `base` again and sets `refining`; it prints what it dropped.",
            ])
            .arg(Arg::Card(CardKind::Proposal))
            .input(vec![
                opt("label", line_max(30), "A short label shown on the card."),
                opt("edit", list(line()), "Paths to make editable."),
                opt("delete", list(line()), "Paths to delete when the Proposal is approved."),
                opt("drifts", list(object(vec![
                    req("drift", line(), "The Drift."),
                    req("decision", one_of(&["proposal", "dismissed", "fix-code"]), "The decision."),
                    opt("option", int_min(1), "The fix option, from 1; only with `fix-code`."),
                    opt("note", nonblank(), "A note; only with `fix-code`."),
                ])), "Decisions on Drifts; only in a resolution."),
                opt("removeDrifts", list(line()), "Drifts to take out of the resolution."),
                opt("images", list(object(vec![req("task", line(), "The Task."), req("path", line(), "The image file.")])), "Images to copy for Tasks."),
                opt("removeImages", list(object(vec![req("task", line(), "The Task."), req("name", line(), "The image name.")])), "Images to remove."),
                opt("rebase", boolean(), "Record `base` again and set `refining`; drops broken `requires` and, in a resolution, the rows of Drifts no longer open."),
            ])
            .output(card_with(vec![
                req("status", line(), "The status after the call."),
                req("sentBackToDraft", boolean(), "Whether an Open Proposal went back to Draft."),
                req("droppedRequires", list(line()), "What a `rebase` dropped from `requires`, one line per Proposal."),
                req("droppedDrifts", list(line()), "What a `rebase` dropped from the rows of a resolution, one line per Drift."),
                req("files", list(object(vec![req("path", line(), "The path in `.weaver/`."), req("file", line(), "The file to write.")])), "The files made editable."),
            ]))
            .place(written)
            .example(r#"weaver proposal edit p3 --input '{"edit":["model/checkout/order.md"]}'"#),
        Command::new("proposal", "move", "Hands a Proposal over, or decides it.", move_proposal)
            .notes(&[
                "`open` hands the Proposal over, clears the mark of a Proposal written by Chat, and prints the warnings and the `show` text. `approved`, `rejected` and `discarded` are the developer's decisions: any caller can run them under the same conditions, and the agent runs them only when the developer explicitly asked for that decision in the conversation. `rejected` takes `feedback` and `rework`; `rework: true`, and approving a stale Proposal (of any kind), need Weaver to be running.",
            ])
            .arg(Arg::Card(CardKind::Proposal))
            .states(&["open", "approved", "rejected", "discarded"])
            .input(vec![
                opt("feedback", nonblank(), "Why it is rejected; only with `rejected`."),
                opt("rework", boolean(), "Ask for a rework; only with `rejected`."),
            ])
            .output(card_with(vec![
                req("kind", one_of(KINDS), "The kind."),
                req("status", line(), "The status after the move."),
                req("warnings", list(line()), "Warnings of the hand-over."),
            ]))
            .place(|input, _| if input.get("rework").and_then(Value::as_bool) == Some(true) { Place::App } else { Place::Write })
            .example("weaver proposal move p3 open"),
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

fn flag(input: &Map<String, Value>, key: &str) -> bool {
    input.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn str_of<'a>(input: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    input.get(key).and_then(Value::as_str)
}

fn now() -> Result<String, Failure> {
    now_instant(&SystemClock).map_err(|e| Failure::failed(e.to_string()))
}

fn name(numbers: &ShortNumbers, kind: CardKind, id: &str) -> String {
    numbers.label_of(kind, id).unwrap_or_else(|| id.to_owned())
}

fn card_json(numbers: &ShortNumbers, card: &Card) -> Value {
    json!({ "id": card.id, "number": numbers.number_of(CardKind::Proposal, &card.id), "title": card.title })
}

fn bullets(prefix: &str, problems: &[String]) -> String {
    let mut message = prefix.to_owned();
    for problem in problems {
        message.push_str(&format!("\n  - {problem}"));
    }
    message
}

/// A failure of the files: the change was not made as a whole, or not at all.
fn failed<E>(error: MutationError<E>) -> Failure {
    Failure::failed(error.to_string())
}

fn refused_failure(error: MutationError<weaver_core::project::board::Refused>) -> Failure {
    match error {
        MutationError::Refused(refused) => Failure::refused(refused.notice),
        other => Failure::failed(other.to_string()),
    }
}

fn write(call: &Call, changes: &ChangeSet) -> Result<(), Failure> {
    changes::apply(call.root, changes, |_| Ok::<(), Infallible>(()))
        .map(|_| ())
        .map_err(|error| match error {
            MutationError::Refused(never) => match never {},
            other => failed(other),
        })
}

fn status_of(image: &ProjectImage, id: &str) -> &'static str {
    image
        .card(CardKind::Proposal, id)
        .and_then(data::status_of)
        .map_or("draft", Status::as_str)
}

fn proposal<'a>(call: &'a Call, id: &str) -> Result<&'a Card, Failure> {
    call.image
        .card(CardKind::Proposal, id)
        .ok_or_else(|| Failure::refused(format!("no Proposal has the identity {id}")))
}

// ---------------------------------------------------------------------------------------------
// new
// ---------------------------------------------------------------------------------------------

fn resolve_proposals(call: &Call, key: &str) -> Result<Vec<String>, Failure> {
    let mut ids = Vec::new();
    for (index, given) in strings(call.input, key).iter().enumerate() {
        let id = resolve_card(call.image, CardKind::Proposal, given)
            .map_err(|e| Failure::refused(format!("{key}[{index}]: {e}")))?;
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    Ok(ids)
}

/// Who writes the Proposal, for the bindings of `request` and `reworks` (`requirements/rules#proposal`):
/// only a caller the app vouches for counts, and a job binds what its own instruction names.
fn writer_of(caller: &Caller) -> Writer {
    match caller.who {
        Who::Chat => Writer::Chat {
            reworking: caller.agent.reworking_proposals.clone(),
        },
        Who::Job => {
            let record = caller.job_record.as_ref();
            let target = record.and_then(|record| record.target.clone());
            match record.map(|record| record.kind) {
                Some(JobKind::Setup | JobKind::Design) => Writer::Job {
                    request: target,
                    reworks: None,
                },
                Some(JobKind::Rework) => Writer::Job {
                    request: None,
                    reworks: target,
                },
                _ => Writer::Job {
                    request: None,
                    reworks: None,
                },
            }
        }
        Who::Other => Writer::Other,
    }
}

fn new(call: &Call) -> Result<CommandReply, Failure> {
    let numbers = shortnum::peek(call.root, call.image.cards());
    let reworks = match str_of(call.input, "reworks") {
        Some(given) => Some(
            resolve_card(call.image, CardKind::Proposal, given)
                .map_err(|e| Failure::refused(format!("reworks: {e}")))?,
        ),
        None => None,
    };
    let input = NewProposal {
        title: str_of(call.input, "title").unwrap_or_default().to_owned(),
        body: str_of(call.input, "body").map(str::to_owned),
        kind: str_of(call.input, "kind").and_then(Kind::parse),
        label: str_of(call.input, "label").map(str::to_owned),
        requires: resolve_proposals(call, "requires")?,
        request: str_of(call.input, "request").map(str::to_owned),
        reworks,
        job: call.caller.job.clone(),
        writer: writer_of(call.caller),
    };
    let created = now()?;
    let planned = create::plan_new(call.image, &numbers, &input, &created, &mut OsRandom)
        .map_err(|problems| Failure::refused(bullets("Nothing was written:", &problems)))?;
    let after = changes::apply(call.root, &planned.changes, |_| Ok::<(), Infallible>(())).map_err(
        |error| match error {
            MutationError::Refused(never) => match never {},
            other => failed(other),
        },
    )?;
    let numbers = after.short_numbers();
    let card = after.card(CardKind::Proposal, &planned.id).ok_or_else(|| {
        Failure::failed(format!(
            "the Proposal file {} was written but cannot be read back",
            planned.path
        ))
    })?;
    let mut markdown = format!(
        "# Proposal {} filed as a Draft\n\n`{}` {}\n\nFile: `{}`\n",
        name(&numbers, CardKind::Proposal, &card.id),
        card.id,
        card.title,
        planned.path
    );
    if let Some(note) = &planned.note {
        markdown.push_str(&format!("\n{note}\n"));
    }
    let mut json = card_json(&numbers, card);
    json["kind"] = json!(planned.kind.as_str());
    json["status"] = json!("draft");
    if let Some(note) = &planned.note {
        json["note"] = json!(note);
    }
    Ok(reply(markdown, json))
}

// ---------------------------------------------------------------------------------------------
// show
// ---------------------------------------------------------------------------------------------

fn analysis_of(
    root: &Path,
    image: &ProjectImage,
    numbers: &ShortNumbers,
    id: &str,
) -> Option<Analysis> {
    let folders = files::read_folders(root, image);
    analysis::analyse(image, &folders, numbers, id)
}

fn refs_text(refs: &[String]) -> String {
    if refs.is_empty() {
        "no reference".to_owned()
    } else {
        refs.iter()
            .map(|r| format!("`{r}`"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// The `show` text and JSON of a Proposal in `image`.
fn show_view(
    root: &Path,
    image: &ProjectImage,
    caller: &Caller,
    id: &str,
) -> Result<(String, Value), Failure> {
    let card = image
        .card(CardKind::Proposal, id)
        .ok_or_else(|| Failure::refused(format!("no Proposal has the identity {id}")))?;
    let numbers = shortnum::peek(root, image.cards());
    let analysis = analysis_of(root, image, &numbers, id)
        .ok_or_else(|| Failure::refused(format!("no Proposal has the identity {id}")))?;
    let kind = data::kind_of(card);
    let status = data::status_of(card).map_or("unknown", Status::as_str);
    let title_name = name(&numbers, CardKind::Proposal, id);

    let mut markdown = format!(
        "# {title_name} {}\n\n`{id}` · {status} · {}",
        card.title,
        kind.as_str()
    );
    if let Some(label) = data::label_of(card) {
        markdown.push_str(&format!(" · label {label}"));
    }
    markdown.push('\n');
    let body = tasks::body_after_title(card);
    if !body.is_empty() {
        markdown.push_str(&format!("\n{body}\n"));
    }

    markdown.push_str("\n## Requires\n\n");
    let mut required_json = Vec::new();
    if analysis.requires.is_empty() {
        markdown.push_str("None.\n");
    }
    for required in &analysis.requires {
        let found = image.card(CardKind::Proposal, &required.id);
        let state = match (&required.problem, found) {
            (Some(problem), _) => problem.text().to_owned(),
            (None, _) => "approved".to_owned(),
        };
        markdown.push_str(&format!(
            "- {} `{}` {} — {state}\n",
            name(&numbers, CardKind::Proposal, &required.id),
            required.id,
            found.map_or("", |c| c.title.as_str())
        ));
        required_json.push(json!({
            "id": required.id,
            "number": numbers.number_of(CardKind::Proposal, &required.id),
            "title": found.map_or("", |c| c.title.as_str()),
        }));
    }

    markdown.push_str("\n## Files\n\n");
    let mut files_json: Vec<String> = Vec::new();
    for (mark, word, paths) in [
        ("+", "added", &analysis.files.added),
        ("~", "changed", &analysis.files.changed),
        ("-", "removed", &analysis.files.removed),
    ] {
        for path in paths {
            markdown.push_str(&format!("- {word} `{path}`\n"));
            files_json.push(format!("{mark} {path}"));
        }
    }
    if files_json.is_empty() {
        markdown.push_str("None.\n");
    }

    markdown.push_str("\n## Tasks\n\n");
    if analysis.tasks.is_empty() {
        markdown.push_str("None.\n");
    }
    let mut tasks_json = Vec::new();
    for task in &analysis.tasks {
        markdown.push_str(&format!(
            "- `{}` {}{} — {}\n",
            task.id,
            task.title,
            if task.technical { " (technical)" } else { "" },
            refs_text(&task.refs)
        ));
        tasks_json.push(json!({ "id": task.id, "number": null, "title": task.title }));
    }

    markdown.push_str("\n## Created on approval\n\n");
    let mut created = 0;
    for task in &analysis.tasks {
        markdown.push_str(&format!("- {} ({})\n", task.title, refs_text(&task.refs)));
        created += 1;
    }
    for removal in &analysis.removals {
        markdown.push_str(&format!(
            "- {} ({})\n",
            removal.title(),
            refs_text(&removal.refs())
        ));
        created += 1;
    }
    if kind == Kind::Resolution {
        for row in weaver_core::project::board::resolution_rows(card) {
            let Some(drift) = image
                .card(CardKind::Drift, &row.drift)
                .filter(|d| d.str_key("status") == Some("open"))
            else {
                continue;
            };
            if row.decision == "fix-code" {
                markdown.push_str(&format!(
                    "- Fix the code: {} (Drift {})\n",
                    drift.title,
                    name(&numbers, CardKind::Drift, &drift.id)
                ));
                created += 1;
            }
        }
    }
    if created == 0 {
        markdown.push_str("None.\n");
    }

    markdown.push_str("\n## Scope\n\n");
    if analysis.scope.is_empty() {
        markdown.push_str("None.\n");
    }
    for covered in &analysis.scope {
        let coverage = match &covered.coverage {
            Some(Coverage::Task(task)) => {
                format!(
                    "covered by Task {}",
                    analysis
                        .tasks
                        .iter()
                        .find(|t| &t.id == task)
                        .map_or(task.as_str(), |t| t.title.as_str())
                )
            }
            Some(Coverage::AlreadyPresent(reference)) => format!("already present (`{reference}`)"),
            Some(Coverage::Removal) => "covered by a code-removal Task".to_owned(),
            None => "not covered".to_owned(),
        };
        markdown.push_str(&format!(
            "- `{}` — {} — {coverage}\n",
            covered.entry.reference,
            covered.entry.reason.text()
        ));
    }

    let gate = transitions::gate_of(&caller.agent, card);
    let blocked: Vec<String> = analysis
        .approve_reasons(gate)
        .iter()
        .map(|r| r.text(&numbers))
        .collect();
    markdown.push_str("\n## Blocked\n\n");
    if blocked.is_empty() {
        markdown.push_str("Nothing blocks it.\n");
    }
    for reason in &blocked {
        markdown.push_str(&format!("- {reason}\n"));
    }

    let validation: Vec<String> = analysis
        .invalid
        .iter()
        .chain(&analysis.uncovered)
        .chain(&analysis.fixes)
        .map(|r| r.text(&numbers))
        .chain(analysis.handover_only.iter().cloned())
        .collect();
    markdown.push_str("\n## Validation\n\n");
    if validation.is_empty() {
        markdown.push_str("No problem found.\n");
    }
    for problem in &validation {
        markdown.push_str(&format!("- {problem}\n"));
    }

    let mut json = card_json(&numbers, card);
    json["kind"] = json!(kind.as_str());
    json["status"] = json!(status);
    json["requires"] = json!(required_json);
    json["files"] = json!(files_json);
    json["tasks"] = json!(tasks_json);
    json["blocked"] = json!(blocked);
    json["validation"] = json!(validation);
    Ok((markdown, json))
}

fn show(call: &Call) -> Result<CommandReply, Failure> {
    let id = call.arg.expect("a Proposal is named");
    let (markdown, json) = show_view(call.root, call.image, call.caller, id)?;
    Ok(reply(markdown, json))
}

// ---------------------------------------------------------------------------------------------
// list
// ---------------------------------------------------------------------------------------------

fn list_proposals(call: &Call) -> Result<CommandReply, Failure> {
    let numbers = shortnum::peek(call.root, call.image.cards());
    let all = flag(call.input, "all");
    let shown: Vec<&Card> = call
        .image
        .cards_of(CardKind::Proposal)
        .filter(|p| all || data::status_of(p).is_some_and(Status::is_live))
        .collect();
    let mut markdown = String::from("# Proposals\n\n");
    if shown.is_empty() {
        markdown.push_str("No Proposal.\n");
    }
    let mut entries = Vec::new();
    for card in shown {
        let status = data::status_of(card).map_or("unknown", Status::as_str);
        markdown.push_str(&format!(
            "- `{}` {} ({status}, {}) {}\n",
            name(&numbers, CardKind::Proposal, &card.id),
            card.id,
            data::kind_of(card).as_str(),
            card.title
        ));
        let mut entry = card_json(&numbers, card);
        entry["status"] = json!(status);
        entries.push(entry);
    }
    Ok(reply(markdown, json!({ "proposals": entries })))
}

// ---------------------------------------------------------------------------------------------
// edit
// ---------------------------------------------------------------------------------------------

/// What the images of one call do: the files to copy and the files to remove.
#[derive(Default)]
struct ImagePlan {
    copies: Vec<(PathBuf, PathBuf)>,
    removals: Vec<PathBuf>,
}

fn is_image(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| IMAGE_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

/// The Task of the Proposal that `given` names: its identity or a unique prefix of it.
fn proposal_task(ids: &BTreeSet<String>, given: &str) -> Result<String, String> {
    if ids.contains(given) {
        return Ok(given.to_owned());
    }
    let found: Vec<&String> = ids.iter().filter(|id| id.starts_with(given)).collect();
    match found.as_slice() {
        [one] => Ok((*one).clone()),
        [] => Err(format!("`{given}` is not a Task of this Proposal")),
        _ => Err(format!(
            "`{given}` names more than one Task of this Proposal"
        )),
    }
}

fn plan_images(call: &Call, task_ids: &BTreeSet<String>) -> Result<ImagePlan, Vec<String>> {
    let mut problems = Vec::new();
    let mut plan = ImagePlan::default();
    let folder = layout::absolute(call.root, TASK_IMAGES_DIR);
    let items = |key: &str| {
        call.input
            .get(key)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    };
    let mut taken: BTreeSet<PathBuf> = BTreeSet::new();
    for (index, item) in items("images").iter().enumerate() {
        let at = format!("images[{index}]");
        let task = item.get("task").and_then(Value::as_str).unwrap_or_default();
        let given = item.get("path").and_then(Value::as_str).unwrap_or_default();
        let task = match proposal_task(task_ids, task) {
            Ok(task) => task,
            Err(why) => {
                problems.push(format!("{at}.task: {why}"));
                continue;
            }
        };
        let source = {
            let path = Path::new(given);
            if path.is_absolute() {
                path.to_path_buf()
            } else {
                call.root.join(path)
            }
        };
        if !is_image(&source) {
            problems.push(format!(
                "{at}.path: `{given}` is not an image (png, jpg, jpeg, gif or webp)"
            ));
            continue;
        }
        if !source.is_file() {
            problems.push(format!("{at}.path: `{given}` does not exist"));
            continue;
        }
        let file_name = source
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let (stem, extension) = match file_name.rsplit_once('.') {
            Some((stem, extension)) => (stem.to_owned(), format!(".{extension}")),
            None => (file_name.clone(), String::new()),
        };
        let mut destination = folder.join(&task).join(&file_name);
        let mut copy = 2;
        while destination.exists() || taken.contains(&destination) {
            destination = folder.join(&task).join(format!("{stem}-{copy}{extension}"));
            copy += 1;
        }
        taken.insert(destination.clone());
        plan.copies.push((source, destination));
    }
    for (index, item) in items("removeImages").iter().enumerate() {
        let at = format!("removeImages[{index}]");
        let task = item.get("task").and_then(Value::as_str).unwrap_or_default();
        let image_name = item.get("name").and_then(Value::as_str).unwrap_or_default();
        let task = match proposal_task(task_ids, task) {
            Ok(task) => task,
            Err(why) => {
                problems.push(format!("{at}.task: {why}"));
                continue;
            }
        };
        if image_name.contains(['/', '\\']) || image_name.starts_with('.') {
            problems.push(format!("{at}.name: `{image_name}` is a file name"));
            continue;
        }
        let path = folder.join(&task).join(image_name);
        if !path.is_file() {
            problems.push(format!(
                "{at}.name: Task {task} has no image `{image_name}`"
            ));
            continue;
        }
        plan.removals.push(path);
    }
    if problems.is_empty() {
        Ok(plan)
    } else {
        Err(problems)
    }
}

fn apply_images(root: &Path, plan: &ImagePlan) -> Result<(), Failure> {
    if !plan.copies.is_empty() {
        layout::ensure_local_folder(root, TASK_IMAGES_DIR)
            .map_err(|e| Failure::failed(format!("cannot prepare the images folder: {e}")))?;
    }
    for (source, destination) in &plan.copies {
        let bytes = fs::read(source)
            .map_err(|e| Failure::failed(format!("cannot read {}: {e}", source.display())))?;
        weaver_core::atomic::write(destination, &bytes, None)
            .map_err(|e| Failure::failed(format!("cannot write {}: {e}", destination.display())))?;
    }
    for path in &plan.removals {
        fs::remove_file(path)
            .map_err(|e| Failure::failed(format!("cannot remove {}: {e}", path.display())))?;
    }
    Ok(())
}

fn drift_decisions(call: &Call) -> Result<Vec<DriftDecision>, Failure> {
    let mut decisions = Vec::new();
    let items = call
        .input
        .get("drifts")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for (index, item) in items.iter().enumerate() {
        let given = item
            .get("drift")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let drift = resolve_card(call.image, CardKind::Drift, given)
            .map_err(|e| Failure::refused(format!("drifts[{index}].drift: {e}")))?;
        decisions.push(DriftDecision {
            drift,
            decision: item
                .get("decision")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            option: item
                .get("option")
                .and_then(Value::as_u64)
                .and_then(|n| u32::try_from(n).ok()),
            note: item.get("note").and_then(Value::as_str).map(str::to_owned),
        });
    }
    Ok(decisions)
}

fn edit(call: &Call) -> Result<CommandReply, Failure> {
    let id = call.arg.expect("a Proposal is named");
    let numbers = shortnum::peek(call.root, call.image.cards());
    proposal(call, id)?;
    let mut remove_drifts = Vec::new();
    for (index, given) in strings(call.input, "removeDrifts").iter().enumerate() {
        remove_drifts.push(
            resolve_card(call.image, CardKind::Drift, given)
                .map_err(|e| Failure::refused(format!("removeDrifts[{index}]: {e}")))?,
        );
    }
    let request = EditRequest {
        label: str_of(call.input, "label").map(str::to_owned),
        edit: strings(call.input, "edit"),
        delete: strings(call.input, "delete"),
        drifts: drift_decisions(call)?,
        remove_drifts,
        rebase: flag(call.input, "rebase"),
        chat: call.caller.who == Who::Chat,
    };
    let folders = files::read_folders(call.root, call.image);
    let read = |path: &str| create::read_project_file(call.root, path);
    let edited = create::plan_edit(call.image, &folders, id, &request, &read, &mut OsRandom)
        .map_err(|problems| Failure::refused(bullets("Nothing was written:", &problems)))?;

    // The Tasks the images can belong to: those of the Proposal and the ones this call creates.
    let mut task_ids: BTreeSet<String> = data::tasks_of(call.image, id)
        .iter()
        .map(|t| t.id.clone())
        .collect();
    for file in &edited.files {
        if let Some(stem) = file
            .path
            .strip_prefix("tasks/")
            .and_then(|n| n.strip_suffix(".md"))
        {
            if let Some((task, _)) = split_card_stem(stem) {
                task_ids.insert(task.to_owned());
            }
        }
    }
    let images = plan_images(call, &task_ids)
        .map_err(|problems| Failure::refused(bullets("Nothing was written:", &problems)))?;

    write(call, &edited.changes)?;
    apply_images(call.root, &images)?;

    let label = name(&numbers, CardKind::Proposal, id);
    let mut markdown = format!(
        "# {label} edited\n\n{label} is {}.\n",
        edited.status.as_str()
    );
    if edited.sent_back_to_draft {
        markdown.push_str(&format!(
            "\n{label} was Open and is back in Draft: hand it over again with `weaver proposal move {label} open` when it is ready.\n"
        ));
    }
    let mut files_json = Vec::new();
    if !edited.files.is_empty() {
        markdown.push_str("\nWrite each file at the path below:\n\n");
    }
    for file in &edited.files {
        let absolute = layout::absolute(call.root, &file.file)
            .to_string_lossy()
            .into_owned();
        markdown.push_str(&format!("- `{}`: `{absolute}`\n", file.path));
        files_json.push(json!({ "path": file.path, "file": absolute }));
    }
    // What a rebase dropped: every requirement and every row.
    let dropped_requires: Vec<String> = edited
        .dropped_requires
        .iter()
        .filter_map(|required| {
            required.problem.map(|problem| {
                format!(
                    "Proposal {} {}",
                    name(&numbers, CardKind::Proposal, &required.id),
                    problem.text()
                )
            })
        })
        .collect();
    let dropped_drifts: Vec<String> = edited
        .dropped_drifts
        .iter()
        .map(|row| {
            format!(
                "Drift {} ({}) is no longer open",
                name(&numbers, CardKind::Drift, &row.drift),
                row.decision
            )
        })
        .collect();
    if !dropped_requires.is_empty() {
        markdown.push_str("\nDropped from `requires`:\n\n");
        for line in &dropped_requires {
            markdown.push_str(&format!("- {line}\n"));
        }
    }
    if !dropped_drifts.is_empty() {
        markdown.push_str("\nDropped decisions of the resolution:\n\n");
        for line in &dropped_drifts {
            markdown.push_str(&format!("- {line}\n"));
        }
    }
    if !images.copies.is_empty() {
        markdown.push_str(&format!("\n{} image(s) copied.\n", images.copies.len()));
    }
    if !images.removals.is_empty() {
        markdown.push_str(&format!("\n{} image(s) removed.\n", images.removals.len()));
    }
    let mut json = card_json(
        &numbers,
        call.image
            .card(CardKind::Proposal, id)
            .expect("checked above"),
    );
    json["status"] = json!(edited.status.as_str());
    json["sentBackToDraft"] = json!(edited.sent_back_to_draft);
    json["files"] = json!(files_json);
    json["droppedRequires"] = json!(dropped_requires);
    json["droppedDrifts"] = json!(dropped_drifts);
    Ok(reply(markdown, json))
}

// ---------------------------------------------------------------------------------------------
// move
// ---------------------------------------------------------------------------------------------

/// Whether the job that hands a Proposal over found the Model with no Feature and no Actor when
/// it reached Running.
fn job_found_empty_model(caller: &Caller) -> bool {
    caller
        .job_record
        .as_ref()
        .is_some_and(|record| record.empty_model_at_start)
}

/// Consumes the design request a handed-over Proposal is bound to: `requests/<id>.md` and
/// `requests/<id>.images/` go in one step. Recording the request as consumed in
/// `requests/.state.json` belongs to the design-request code, which calls this one's twin when
/// it is registered.
fn consume_request(root: &Path, request: &str) -> Result<(), Failure> {
    let file = layout::absolute(root, &format!("{}/{request}.md", layout::REQUESTS_DIR));
    let images = layout::absolute(root, &format!("{}/{request}.images", layout::REQUESTS_DIR));
    match fs::remove_file(&file) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            return Err(Failure::failed(format!(
                "the request {request} cannot be consumed: {e}"
            )));
        }
        _ => {}
    }
    match fs::remove_dir_all(&images) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(Failure::failed(format!(
            "the images of the request {request} cannot be removed: {e}"
        ))),
        _ => Ok(()),
    }
}

fn refusal_lines(refusal: &Refusal) -> Vec<String> {
    refusal.0.clone()
}

fn open(call: &Call) -> Result<CommandReply, Failure> {
    let id = call.arg.expect("a Proposal is named");
    let created = now()?;
    let context = SubmitContext {
        now: &created,
        job_found_empty_model: job_found_empty_model(call.caller),
        run_in_progress: call.caller.agent.run_in_progress,
    };
    let (after, submission) = transitions::submit(call.root, id, &context, &mut OsRandom).map_err(
        |error| match error {
            MutationError::Refused(refusal) => Failure::refused(bullets(
                "Nothing was handed over:",
                &refusal_lines(&refusal),
            )),
            other => failed(other),
        },
    )?;
    if let Some(request) = &submission.consumed_request {
        consume_request(call.root, request)?;
    }
    let numbers = shortnum::peek(call.root, after.cards());
    let label = name(&numbers, CardKind::Proposal, id);
    let mut markdown = match &submission.outcome {
        Submitted::Open => format!("# {label} handed over\n\n{label} is Open: the developer decides.\n"),
        Submitted::Approved(plan) => {
            let mut text = format!("# {label} approved\n\n{label} is approved at once and its changes are in the Model.\n");
            if !plan.created_tasks.is_empty() || !plan.removal_tasks.is_empty() {
                text.push_str(&format!("\n{} Task(s) are in To do.\n", plan.created_tasks.len() + plan.removal_tasks.len()));
            }
            text
        }
        Submitted::WaitsForRun => format!("# {label} handed over\n\n{label} is refined and waits for the Run to end before it is approved.\n"),
    };
    if !submission.warnings.is_empty() {
        markdown.push_str("\n## Warnings\n\n");
        for warning in &submission.warnings {
            markdown.push_str(&format!("- {warning}\n"));
        }
    }
    let (view, _) = show_view(call.root, &after, call.caller, id)?;
    markdown.push_str(&format!("\n---\n\n{view}"));
    let kind = after
        .card(CardKind::Proposal, id)
        .map_or(Kind::Change, data::kind_of);
    let mut json = card_json(
        &numbers,
        after
            .card(CardKind::Proposal, id)
            .ok_or_else(|| Failure::failed("the Proposal is gone"))?,
    );
    json["kind"] = json!(kind.as_str());
    json["status"] = json!(status_of(&after, id));
    json["warnings"] = json!(submission.warnings);
    Ok(reply(markdown, json))
}

/// The answer of a decision.
fn decided(call: &Call, image: &ProjectImage, id: &str, markdown: String) -> CommandReply {
    let numbers = shortnum::peek(call.root, image.cards());
    let card = call.image.card(CardKind::Proposal, id);
    let mut json = json!({
        "id": id,
        "number": numbers.number_of(CardKind::Proposal, id),
        "title": card.map_or("", |c| c.title.as_str()),
        "kind": card.map_or(Kind::Change, data::kind_of).as_str(),
        "status": status_of(image, id),
        "warnings": [],
    });
    if card.is_none() {
        json["title"] = json!("");
    }
    reply(markdown, json)
}

fn needs_the_app(what: &str) -> Failure {
    Failure::new(code::APP_NOT_RUNNING, format!("{what} starts a job, which only the running Weaver can do: Weaver is not running. Nothing was written."))
}

fn approve(call: &Call) -> Result<CommandReply, Failure> {
    let id = call.arg.expect("a Proposal is named");
    let numbers = shortnum::peek(call.root, call.image.cards());
    let label = name(&numbers, CardKind::Proposal, id);
    let created = now()?;
    // Approving a stale Proposal of any kind, or a Draft a refinement left half-way, starts a
    // refinement job.
    let folders = files::read_folders(call.root, call.image);
    let gate = proposal(call, id).map(|card| transitions::gate_of(&call.caller.agent, card))?;
    match transitions::plan_approve(
        call.image,
        &folders,
        &numbers,
        id,
        gate,
        &created,
        &mut OsRandom,
        Decider::Developer,
    ) {
        Err(refusal) => {
            return Err(Failure::refused(format!(
                "Could not approve {label}: {refusal}."
            )))
        }
        Ok(Approval::Approve(_)) => {}
        Ok(Approval::Refine(_)) => {
            return Err(needs_the_app(&format!(
                "Refining {label} on the current Model (it is stale)"
            )));
        }
        Ok(Approval::Continue(_)) => {
            return Err(needs_the_app(&format!(
                "Continuing the refinement of {label}"
            )));
        }
    }
    let (after, approval) = transitions::approve(
        call.root,
        &call.caller.agent,
        id,
        Decider::Developer,
        &created,
        &mut OsRandom,
    )
    .map_err(refused_failure)?;
    let mut markdown = format!("# {label} approved\n\nThe changes of {label} are in the Model, the Requirements and the Specs.\n");
    if let Approval::Approve(plan) = &approval {
        let created = plan.created_tasks.len() + plan.removal_tasks.len() + plan.fix_tasks.len();
        if created > 0 {
            markdown.push_str(&format!("\n{created} Task(s) are in To do.\n"));
        }
        for (drift, status) in &plan.decided_drifts {
            markdown.push_str(&format!(
                "\nDrift {} is now {status}.\n",
                name(&numbers, CardKind::Drift, drift)
            ));
        }
    }
    Ok(decided(call, &after, id, markdown))
}

fn reject(call: &Call) -> Result<CommandReply, Failure> {
    let id = call.arg.expect("a Proposal is named");
    let numbers = shortnum::peek(call.root, call.image.cards());
    let label = name(&numbers, CardKind::Proposal, id);
    let rework = flag(call.input, "rework");
    if rework {
        return Err(needs_the_app(&format!("A rework of {label}")));
    }
    let (after, _) = transitions::reject(
        call.root,
        &call.caller.agent,
        id,
        str_of(call.input, "feedback"),
        false,
        Decider::Developer,
    )
    .map_err(refused_failure)?;
    Ok(decided(
        call,
        &after,
        id,
        format!("# {label} rejected\n\n{label} is Rejected.\n"),
    ))
}

fn discard(call: &Call) -> Result<CommandReply, Failure> {
    let id = call.arg.expect("a Proposal is named");
    let numbers = shortnum::peek(call.root, call.image.cards());
    let label = name(&numbers, CardKind::Proposal, id);
    let after = transitions::discard(call.root, &call.caller.agent, id, Decider::Developer)
        .map_err(refused_failure)?;
    Ok(decided(
        call,
        &after,
        id,
        format!("# {label} discarded\n\n{label} is Discarded.\n"),
    ))
}

fn move_proposal(call: &Call) -> Result<CommandReply, Failure> {
    let state = call.state.expect("a state is named");
    if state != "rejected"
        && (call.input.contains_key("feedback") || call.input.contains_key("rework"))
    {
        return Err(Failure::invalid(
            "Invalid input:\n  - feedback, rework: go with the state `rejected`",
        ));
    }
    match state {
        "open" => open(call),
        "approved" => approve(call),
        "rejected" => reject(call),
        _ => discard(call),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use weaver_core::jobs::JobRecord;
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

    fn hash(text: &str) -> String {
        sha256_hex(&file_content(text))
    }

    /// A Proposal folder: `proposal.md`, held files and Tasks `(id, slug, text)`.
    fn proposal_at(
        root: &Path,
        id: &str,
        front: &str,
        body: &str,
        held: &[(&str, &str)],
        tasks: &[(&str, &str, &str)],
    ) {
        put(
            root,
            &format!("proposals/{id}-t/proposal.md"),
            &format!("---\n{front}created: 2026-11-02T07:00:00.000Z\n---\n{body}"),
        );
        for (path, text) in held {
            put(root, &format!("proposals/{id}-t/{path}"), text);
        }
        for (task, slug, text) in tasks {
            put(
                root,
                &format!("proposals/{id}-t/tasks/{task}-{slug}.md"),
                text,
            );
        }
    }

    fn changed_order() -> String {
        ORDER.replace("### Paid\n", "### Paid\nThe customer paid.\n")
    }

    fn task_text(title: &str, refs: &[&str]) -> String {
        let refs: String = refs.iter().map(|r| format!("  - {r}\n")).collect();
        format!("---\ntechnical: false\nrefs:\n{refs}---\n# {title}\n\nDo it.\n")
    }

    /// An Open change with one Task covering the whole scope (`order` covers its States).
    fn ready(root: &Path, id: &str, status: &str) {
        let front = format!(
            "status: {status}\nkind: change\nbase:\n  model/shop/order.md: {}\nlabel: Shop\n",
            hash(ORDER)
        );
        proposal_at(
            root,
            id,
            &front,
            "# Pay\n\nWhy.\n",
            &[("model/shop/order.md", &changed_order())],
            &[("tttttttt", "pay", &task_text("Pay", &["order"]))],
        );
    }

    fn call_with(
        verb: &str,
        root: &Path,
        arg: Option<&str>,
        state: Option<&str>,
        caller: &Caller,
        input: Value,
    ) -> Result<CommandReply, Failure> {
        let all = commands();
        let command = all.iter().find(|c| c.verb == verb).unwrap();
        let image = ProjectImage::load(root);
        let input: Map<String, Value> = input.as_object().unwrap().clone();
        (command.run)(&Call {
            command,
            root,
            image: &image,
            arg,
            state,
            input: &input,
            caller,
        })
    }

    fn run(
        verb: &str,
        root: &Path,
        arg: Option<&str>,
        state: Option<&str>,
        input: Value,
    ) -> Result<CommandReply, Failure> {
        call_with(verb, root, arg, state, &Caller::default(), input)
    }

    fn snapshot(root: &Path) -> BTreeMap<String, Vec<u8>> {
        fn walk(dir: &Path, base: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
            let Ok(listing) = fs::read_dir(dir) else {
                return;
            };
            for item in listing {
                let path = item.unwrap().path();
                let key = path
                    .strip_prefix(base)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned();
                if path.is_dir() {
                    out.insert(format!("{key}/"), Vec::new());
                    walk(&path, base, out);
                } else {
                    out.insert(key, fs::read(&path).unwrap());
                }
            }
        }
        let mut out = BTreeMap::new();
        walk(root, root, &mut out);
        out
    }

    #[test]
    fn new_files_a_draft_recording_the_job_with_its_number_and_kind() {
        let dir = project();
        let caller = Caller {
            job: Some("jjjjjjjj".into()),
            ..Caller::default()
        };
        let reply = call_with(
            "new",
            dir.path(),
            None,
            None,
            &caller,
            json!({ "title": "Cancel an order", "body": "Why.", "label": "Orders" }),
        )
        .unwrap();
        assert_eq!(
            (
                reply.json["status"].as_str(),
                reply.json["kind"].as_str(),
                reply.json["number"].as_u64()
            ),
            (Some("draft"), Some("change"), Some(1))
        );
        let id = reply.json["id"].as_str().unwrap();
        let image = ProjectImage::load(dir.path());
        let card = image.card(CardKind::Proposal, id).unwrap();
        assert_eq!(card.str_key("job"), Some("jjjjjjjj"));
        assert_eq!(card.str_key("label"), Some("Orders"));
        assert!(
            reply.markdown.contains("filed as a Draft"),
            "{}",
            reply.markdown
        );
        assert!(dir
            .path()
            .join(format!(
                ".weaver/proposals/{id}-cancel-an-order/proposal.md"
            ))
            .is_file());
    }

    #[test]
    fn new_refuses_a_setup_on_a_model_with_a_feature_and_writes_nothing() {
        let dir = project();
        let before = snapshot(dir.path());
        let error = run(
            "new",
            dir.path(),
            None,
            None,
            json!({ "title": "Shop", "kind": "setup", "request": "r", "reworks": "aaaaaaaa" }),
        )
        .unwrap_err();
        assert_eq!(error.code, code::REFUSED);
        assert_eq!(snapshot(dir.path()), before);
        let error = run(
            "new",
            dir.path(),
            None,
            None,
            json!({ "title": "Shop", "kind": "setup" }),
        )
        .unwrap_err();
        assert!(
            error
                .message
                .contains("`setup` is for a project whose Model has no Feature and no Actor"),
            "{}",
            error.message
        );
    }

    /// A caller the app vouches for as the agent of a design job that received `request`.
    fn design_job(request: &str) -> Caller {
        let mut record = record(false);
        record.kind = weaver_core::jobs::JobKind::Design;
        record.target = Some(request.to_owned());
        Caller {
            job: Some(record.id.clone()),
            job_record: Some(record),
            who: Who::Job,
            ..Caller::default()
        }
    }

    fn chat_caller() -> Caller {
        Caller {
            general: Some("secret".into()),
            who: Who::Chat,
            ..Caller::default()
        }
    }

    #[test]
    fn new_says_why_it_did_not_bind_the_request_and_resolves_requires() {
        let dir = project();
        ready(dir.path(), "pppppppp", "open");
        put(
            dir.path(),
            "requests/2026-11-02T07-00-00.md",
            "# A request\n",
        );
        let job = design_job("2026-11-02T07-00-00");
        let reply = call_with(
            "new",
            dir.path(),
            None,
            None,
            &job,
            json!({ "title": "Second", "requires": ["p1"], "request": "2026-11-02T07-00-00" }),
        )
        .unwrap();
        assert!(
            reply.json.get("note").is_none(),
            "no Proposal is bound to it yet"
        );
        let image = ProjectImage::load(dir.path());
        let card = image
            .card(CardKind::Proposal, reply.json["id"].as_str().unwrap())
            .unwrap();
        assert_eq!(card.list_key("requires"), Some(vec!["pppppppp".to_owned()]));
        assert_eq!(card.str_key("request"), Some("2026-11-02T07-00-00"));
        let third = call_with(
            "new",
            dir.path(),
            None,
            None,
            &job,
            json!({ "title": "Third", "request": "2026-11-02T07-00-00" }),
        )
        .unwrap();
        assert!(
            third.json["note"]
                .as_str()
                .unwrap()
                .contains("is not bound to the request"),
            "{}",
            third.json
        );
        assert!(third.markdown.contains("is not bound to the request"));
        let missing = run(
            "new",
            dir.path(),
            None,
            None,
            json!({ "title": "Fourth", "requires": ["p99"] }),
        )
        .unwrap_err();
        assert!(
            missing.message.starts_with("requires[0]:"),
            "{}",
            missing.message
        );
    }

    #[test]
    fn a_caller_the_app_does_not_vouch_for_binds_nothing_even_with_a_job_identity() {
        let dir = project();
        put(
            dir.path(),
            "requests/2026-11-02T07-00-00.md",
            "# A request\n",
        );
        let mut unvouched = design_job("2026-11-02T07-00-00");
        unvouched.who = Who::Other;
        let reply = call_with(
            "new",
            dir.path(),
            None,
            None,
            &unvouched,
            json!({ "title": "Not bound", "request": "2026-11-02T07-00-00" }),
        )
        .unwrap();
        assert!(reply.json["note"]
            .as_str()
            .unwrap()
            .contains("only the agent of the job that received the request"));
        let image = ProjectImage::load(dir.path());
        let card = image
            .card(CardKind::Proposal, reply.json["id"].as_str().unwrap())
            .unwrap();
        assert_eq!(card.str_key("request"), None);
    }

    #[test]
    fn chat_files_a_draft_it_marks_binds_a_free_request_and_cannot_write_a_setup_or_an_import() {
        let dir = project();
        put(
            dir.path(),
            "requests/2026-11-02T07-00-00.md",
            "# A request\n",
        );
        let reply = call_with(
            "new",
            dir.path(),
            None,
            None,
            &chat_caller(),
            json!({ "title": "From Chat", "request": "2026-11-02T07-00-00" }),
        )
        .unwrap();
        assert!(reply.json.get("note").is_none(), "{}", reply.json);
        let id = reply.json["id"].as_str().unwrap().to_owned();
        let image = ProjectImage::load(dir.path());
        let card = image.card(CardKind::Proposal, &id).unwrap();
        assert_eq!(card.bool_key("chat"), Some(true));
        assert_eq!(card.bool_key("chatWriting"), Some(true));
        assert_eq!(card.str_key("request"), Some("2026-11-02T07-00-00"));
        assert_eq!(card.str_key("job"), None);

        for kind in ["setup", "import"] {
            let error = call_with(
                "new",
                dir.path(),
                None,
                None,
                &chat_caller(),
                json!({ "title": "Shop", "kind": kind }),
            )
            .unwrap_err();
            assert_eq!(error.code, code::REFUSED);
            assert!(
                error
                    .message
                    .contains("the agent of Chat writes only a `change` or a `resolution`"),
                "{}",
                error.message
            );
        }
    }

    #[test]
    fn chat_edit_marks_any_proposal_it_touches_and_the_hand_over_clears_the_mark() {
        let dir = project();
        ready(dir.path(), "pppppppp", "open");
        let reply = call_with(
            "edit",
            dir.path(),
            Some("pppppppp"),
            None,
            &chat_caller(),
            json!({}),
        )
        .unwrap();
        assert_eq!(reply.json["sentBackToDraft"], true);
        let image = ProjectImage::load(dir.path());
        let card = image.card(CardKind::Proposal, "pppppppp").unwrap();
        assert_eq!(card.str_key("status"), Some("draft"));
        assert_eq!(card.bool_key("chatWriting"), Some(true));
        assert_eq!(card.bool_key("chat"), None, "only reopened, not created");

        // Another caller's edit does not mark it.
        ready(dir.path(), "qqqqqqqq", "open");
        call_with(
            "edit",
            dir.path(),
            Some("qqqqqqqq"),
            None,
            &Caller::default(),
            json!({}),
        )
        .unwrap();
        let image = ProjectImage::load(dir.path());
        assert_eq!(
            image
                .card(CardKind::Proposal, "qqqqqqqq")
                .unwrap()
                .bool_key("chatWriting"),
            None
        );

        let handed = call_with(
            "move",
            dir.path(),
            Some("pppppppp"),
            Some("open"),
            &chat_caller(),
            json!({}),
        )
        .unwrap();
        assert_eq!(handed.json["status"], "open");
        let image = ProjectImage::load(dir.path());
        let card = image.card(CardKind::Proposal, "pppppppp").unwrap();
        assert_eq!(card.bool_key("chatWriting"), None);
    }

    #[test]
    fn list_shows_the_live_proposals_and_all_with_all() {
        let dir = project();
        ready(dir.path(), "pppppppp", "open");
        proposal_at(
            dir.path(),
            "qqqqqqqq",
            "status: draft\n",
            "# Drafted\n",
            &[],
            &[],
        );
        proposal_at(
            dir.path(),
            "rrrrrrrr",
            "status: rejected\n",
            "# Rejected\n",
            &[],
            &[],
        );
        let live = run("list", dir.path(), None, None, json!({})).unwrap();
        let titles = |reply: &CommandReply| {
            reply.json["proposals"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| p["title"].as_str().unwrap().to_owned())
                .collect::<Vec<_>>()
        };
        assert_eq!(titles(&live), ["Pay", "Drafted"]);
        assert_eq!(live.json["proposals"][0]["number"], 1);
        assert_eq!(live.json["proposals"][0]["status"], "open");
        let all = run("list", dir.path(), None, None, json!({ "all": true })).unwrap();
        assert_eq!(titles(&all), ["Pay", "Drafted", "Rejected"]);
        assert_eq!(all.json["proposals"][2]["number"], Value::Null);
        let empty = run(
            "list",
            tempfile::tempdir().unwrap().path(),
            None,
            None,
            json!({}),
        )
        .unwrap();
        assert!(empty.markdown.contains("No Proposal."));
    }

    #[test]
    fn show_prints_every_section_and_every_reason() {
        let dir = project();
        ready(dir.path(), "pppppppp", "open");
        // A required Proposal that is still a Draft, and a Model file changed since the start.
        let front = format!("status: open\nrequires:\n  - qqqqqqqq\nbase:\n  model/shop/order.md: {}\nlabel: Shop\n", hash("something else"));
        proposal_at(
            dir.path(),
            "pppppppp",
            &front,
            "# Pay\n\nWhy.\n",
            &[("model/shop/order.md", &changed_order())],
            &[("tttttttt", "pay", &task_text("Pay", &["order"]))],
        );
        proposal_at(
            dir.path(),
            "qqqqqqqq",
            "status: draft\n",
            "# Needed first\n",
            &[],
            &[],
        );
        let reply = run("show", dir.path(), Some("pppppppp"), None, json!({})).unwrap();
        for section in [
            "## Requires",
            "## Files",
            "## Tasks",
            "## Created on approval",
            "## Scope",
            "## Blocked",
            "## Validation",
        ] {
            assert!(
                reply.markdown.contains(section),
                "{section}: {}",
                reply.markdown
            );
        }
        assert!(
            reply.markdown.contains("- changed `model/shop/order.md`"),
            "{}",
            reply.markdown
        );
        assert!(
            reply
                .markdown
                .contains("- `order/Paid` — changed — covered by Task Pay"),
            "{}",
            reply.markdown
        );
        assert!(
            reply.markdown.contains("- Pay (`order`)"),
            "{}",
            reply.markdown
        );
        let blocked: Vec<&str> = reply.json["blocked"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(blocked.len(), 2, "{blocked:?}");
        assert!(
            blocked[0].ends_with("is still being written"),
            "{blocked:?}"
        );
        assert!(
            blocked[1].starts_with("Stale: `model/shop/order.md`"),
            "{blocked:?}"
        );
        assert_eq!(reply.json["files"], json!(["~ model/shop/order.md"]));
        assert_eq!(reply.json["requires"][0]["id"], "qqqqqqqq");
        assert_eq!(reply.json["tasks"][0]["title"], "Pay");
        assert_eq!(reply.json["tasks"][0]["number"], Value::Null);
        assert_eq!(reply.json["validation"], json!([]));
        assert_eq!(reply.json["kind"], "change");
        assert!(run("show", dir.path(), Some("nnnnnnnn"), None, json!({})).is_err());
    }

    #[test]
    fn edit_prints_the_path_to_write_and_sends_an_open_proposal_back_to_draft() {
        let dir = project();
        ready(dir.path(), "pppppppp", "open");
        let reply = run(
            "edit",
            dir.path(),
            Some("pppppppp"),
            None,
            json!({ "edit": ["model/shop/order.md", "tasks/more.md"], "label": "Shop v2" }),
        )
        .unwrap();
        assert_eq!(reply.json["sentBackToDraft"], true);
        assert_eq!(reply.json["status"], "draft");
        let files = reply.json["files"].as_array().unwrap();
        let order = files[0]["file"].as_str().unwrap();
        assert!(
            Path::new(order).ends_with("proposals/pppppppp-t/model/shop/order.md"),
            "{order}"
        );
        assert!(Path::new(order).is_file());
        assert!(files[1]["path"].as_str().unwrap().ends_with("-more.md"));
        assert!(
            reply.markdown.contains("back in Draft"),
            "{}",
            reply.markdown
        );
        let image = ProjectImage::load(dir.path());
        assert_eq!(
            image
                .card(CardKind::Proposal, "pppppppp")
                .unwrap()
                .str_key("label"),
            Some("Shop v2")
        );
        // Without input it still goes to Draft only when it is Open: now it is a Draft.
        let again = run("edit", dir.path(), Some("pppppppp"), None, json!({})).unwrap();
        assert_eq!(again.json["sentBackToDraft"], false);
    }

    #[test]
    fn edit_refuses_with_every_problem_and_writes_nothing() {
        let dir = project();
        proposal_at(
            dir.path(),
            "pppppppp",
            "status: open\nkind: import\n",
            "# Import\n",
            &[],
            &[],
        );
        let before = snapshot(dir.path());
        let error = run("edit", dir.path(), Some("pppppppp"), None, json!({ "edit": ["tasks/a.md", "elsewhere.md"], "drifts": [{ "drift": "dddddddd", "decision": "dismissed" }] })).unwrap_err();
        assert!(
            error.message.starts_with("drifts[0].drift:"),
            "{}",
            error.message
        );
        let error = run(
            "edit",
            dir.path(),
            Some("pppppppp"),
            None,
            json!({ "edit": ["tasks/a.md", "elsewhere.md"] }),
        )
        .unwrap_err();
        assert_eq!(error.code, code::REFUSED);
        assert!(
            error.message.contains("carries no Task")
                && error.message.contains("is not a file of the Model"),
            "{}",
            error.message
        );
        assert_eq!(
            snapshot(dir.path()),
            before,
            "the Open Proposal is not even sent back to Draft"
        );
    }

    #[test]
    fn edit_copies_and_removes_images_of_tasks_inside_the_local_folder() {
        let dir = project();
        proposal_at(
            dir.path(),
            "pppppppp",
            "status: draft\nlabel: L\n",
            "# P\n",
            &[],
            &[("aaaaaaaa", "known", &task_text("Known", &["order"]))],
        );
        let picture = dir.path().join("shot.png");
        fs::write(&picture, b"\x89PNG").unwrap();
        let args = json!({ "images": [{ "task": "aaaa", "path": picture.to_string_lossy() }, { "task": "aaaaaaaa", "path": "shot.png" }] });
        let reply = run("edit", dir.path(), Some("pppppppp"), None, args).unwrap();
        assert!(
            reply.markdown.contains("2 image(s) copied"),
            "{}",
            reply.markdown
        );
        let folder = dir.path().join(".weaver/task-images/aaaaaaaa");
        assert_eq!(fs::read(folder.join("shot.png")).unwrap(), b"\x89PNG");
        assert!(
            folder.join("shot-2.png").is_file(),
            "a second image of the same name is kept apart"
        );
        assert_eq!(
            fs::read_to_string(dir.path().join(".weaver/task-images/.gitignore")).unwrap(),
            "*\n"
        );

        let removed = run(
            "edit",
            dir.path(),
            Some("pppppppp"),
            None,
            json!({ "removeImages": [{ "task": "aaaaaaaa", "name": "shot.png" }] }),
        )
        .unwrap();
        assert!(removed.markdown.contains("1 image(s) removed"));
        assert!(!folder.join("shot.png").exists() && folder.join("shot-2.png").exists());

        let bad = run(
            "edit",
            dir.path(),
            Some("pppppppp"),
            None,
            json!({ "images": [{ "task": "zzzz", "path": "shot.png" }, { "task": "aaaaaaaa", "path": "notes.txt" }, { "task": "aaaaaaaa", "path": "gone.png" }], "removeImages": [{ "task": "aaaaaaaa", "name": "../x.png" }] }),
        )
        .unwrap_err();
        for expected in [
            "images[0].task",
            "images[1].path",
            "images[2].path",
            "removeImages[0].name",
        ] {
            assert!(
                bad.message.contains(expected),
                "{expected}: {}",
                bad.message
            );
        }
    }

    #[test]
    fn open_hands_a_draft_over_prints_the_show_text_and_consumes_its_request() {
        let dir = project();
        put(
            dir.path(),
            "requests/2026-11-02T07-00-00.md",
            "# A request\n",
        );
        put(dir.path(), "requests/2026-11-02T07-00-00.images/a.png", "x");
        let front = format!("status: draft\nrequest: 2026-11-02T07-00-00\nbase:\n  model/shop/order.md: {}\nlabel: Shop\n", hash(ORDER));
        proposal_at(
            dir.path(),
            "pppppppp",
            &front,
            "# Pay\n\nWhy.\n",
            &[("model/shop/order.md", &changed_order())],
            &[("tttttttt", "pay", &task_text("Pay", &["order"]))],
        );
        let reply = run(
            "move",
            dir.path(),
            Some("pppppppp"),
            Some("open"),
            json!({}),
        )
        .unwrap();
        assert_eq!(reply.json["status"], "open");
        assert_eq!(reply.json["warnings"], json!([]));
        assert!(
            reply.markdown.contains("is Open: the developer decides")
                && reply.markdown.contains("## Scope"),
            "{}",
            reply.markdown
        );
        assert!(!dir
            .path()
            .join(".weaver/requests/2026-11-02T07-00-00.md")
            .exists());
        assert!(!dir
            .path()
            .join(".weaver/requests/2026-11-02T07-00-00.images")
            .exists());
    }

    #[test]
    fn open_is_refused_naming_every_problem_and_writes_nothing_not_even_the_request() {
        let dir = project();
        put(
            dir.path(),
            "requests/2026-11-02T07-00-00.md",
            "# A request\n",
        );
        let front = format!(
            "status: draft\nrequest: 2026-11-02T07-00-00\nbase:\n  model/shop/order.md: {}\n",
            hash(ORDER)
        );
        // No Task and no label for the change, and a file nobody registered.
        proposal_at(
            dir.path(),
            "pppppppp",
            &front,
            "# Pay\n",
            &[("model/shop/order.md", &changed_order()), ("stray.md", "x")],
            &[],
        );
        let before = snapshot(dir.path());
        let error = run(
            "move",
            dir.path(),
            Some("pppppppp"),
            Some("open"),
            json!({}),
        )
        .unwrap_err();
        assert_eq!(error.code, code::REFUSED);
        for expected in [
            "Nothing was handed over:",
            "`order/Paid` (changed) is not covered by a Task",
            "`stray.md` is not registered",
        ] {
            assert!(
                error.message.contains(expected),
                "{expected}: {}",
                error.message
            );
        }
        assert_eq!(snapshot(dir.path()), before);
    }

    /// A job record as the registry writes it; the fields with a default are left out, so a new one
    /// never breaks this fixture.
    fn record(empty: bool) -> JobRecord {
        serde_json::from_value(json!({
            "id": "jjjjjjjj",
            "kind": "setup",
            "name": "Setup",
            "project": "/p",
            "state": "running",
            "created": "2026-11-02T07:00:00.000Z",
            "emptyModelAtStart": empty,
        }))
        .expect("a valid record")
    }

    #[test]
    fn a_setup_of_a_job_that_found_an_empty_model_is_approved_at_the_hand_over() {
        let dir = tempfile::tempdir().unwrap();
        put(dir.path(), "model/system.md", SYSTEM);
        let front = "status: draft\nkind: setup\nbase:\n  model/shop/feature.md: new\n  model/shop/order.md: new\nlabel: Shop\n";
        let tasks = task_text("Shop", &["shop"]);
        proposal_at(
            dir.path(),
            "pppppppp",
            front,
            "# Shop\n",
            &[
                ("model/shop/feature.md", FEATURE),
                ("model/shop/order.md", ORDER),
            ],
            &[("tttttttt", "shop", &tasks)],
        );

        let other = Caller {
            job: Some("jjjjjjjj".into()),
            job_record: Some(record(false)),
            ..Caller::default()
        };
        let open = call_with(
            "move",
            dir.path(),
            Some("pppppppp"),
            Some("open"),
            &other,
            json!({}),
        )
        .unwrap();
        assert_eq!(open.json["status"], "open", "{}", open.markdown);

        proposal_at(
            dir.path(),
            "pppppppp",
            front,
            "# Shop\n",
            &[
                ("model/shop/feature.md", FEATURE),
                ("model/shop/order.md", ORDER),
            ],
            &[("tttttttt", "shop", &tasks)],
        );
        let empty = Caller {
            job: Some("jjjjjjjj".into()),
            job_record: Some(record(true)),
            ..Caller::default()
        };
        let approved = call_with(
            "move",
            dir.path(),
            Some("pppppppp"),
            Some("open"),
            &empty,
            json!({}),
        )
        .unwrap();
        assert_eq!(
            (
                approved.json["status"].as_str(),
                approved.json["kind"].as_str()
            ),
            (Some("approved"), Some("setup"))
        );
        assert!(dir.path().join(".weaver/model/shop/order.md").is_file());
        assert!(dir.path().join(".weaver/tasks/tttttttt-shop.md").is_file());
        assert!(
            approved.markdown.contains("approved at once"),
            "{}",
            approved.markdown
        );
    }

    #[test]
    fn approved_creates_the_tasks_and_a_stale_change_needs_the_app() {
        let dir = project();
        ready(dir.path(), "pppppppp", "open");
        // The Model moved on after the Proposal started.
        put(
            dir.path(),
            "model/shop/order.md",
            &ORDER.replace("Choosing.", "Browsing."),
        );
        let before = snapshot(dir.path());
        let error = run(
            "move",
            dir.path(),
            Some("pppppppp"),
            Some("approved"),
            json!({}),
        )
        .unwrap_err();
        assert_eq!(error.code, code::APP_NOT_RUNNING);
        assert!(
            error.message.contains("Weaver is not running"),
            "{}",
            error.message
        );
        assert_eq!(
            snapshot(dir.path()),
            before,
            "nothing, not even Refining, was written"
        );

        // Back where it started: approved at once.
        put(dir.path(), "model/shop/order.md", ORDER);
        let reply = run(
            "move",
            dir.path(),
            Some("pppppppp"),
            Some("approved"),
            json!({}),
        )
        .unwrap();
        assert_eq!(reply.json["status"], "approved");
        assert!(
            reply.markdown.contains("1 Task(s) are in To do"),
            "{}",
            reply.markdown
        );
        assert!(dir.path().join(".weaver/tasks/tttttttt-pay.md").is_file());
        assert!(
            fs::read_to_string(dir.path().join(".weaver/model/shop/order.md"))
                .unwrap()
                .contains("The customer paid.")
        );
    }

    #[test]
    fn approved_is_refused_with_every_reason_and_writes_nothing() {
        let dir = project();
        ready(dir.path(), "pppppppp", "open");
        let mut caller = Caller::default();
        caller.agent.run_in_progress = true;
        let before = snapshot(dir.path());
        let error = call_with(
            "move",
            dir.path(),
            Some("pppppppp"),
            Some("approved"),
            &caller,
            json!({}),
        )
        .unwrap_err();
        assert_eq!(error.code, code::REFUSED);
        assert_eq!(error.message, "Could not approve P1: A run is in progress.");
        assert_eq!(snapshot(dir.path()), before);
    }

    #[test]
    fn rejected_keeps_the_feedback_discarded_is_for_a_draft_and_rework_needs_the_app() {
        let dir = project();
        ready(dir.path(), "pppppppp", "open");
        let place = |input: Value| {
            let all = commands();
            let command = all.iter().find(|c| c.verb == "move").unwrap();
            command.place_for(input.as_object().unwrap(), Some("p1"))
        };
        assert_eq!(place(json!({ "rework": true })), Place::App);
        assert_eq!(place(json!({ "rework": false })), Place::Write);
        let error = run(
            "move",
            dir.path(),
            Some("pppppppp"),
            Some("rejected"),
            json!({ "rework": true, "feedback": "Split it" }),
        )
        .unwrap_err();
        assert_eq!(error.code, code::APP_NOT_RUNNING);

        let reply = run(
            "move",
            dir.path(),
            Some("pppppppp"),
            Some("rejected"),
            json!({ "feedback": "Too big" }),
        )
        .unwrap();
        assert_eq!(reply.json["status"], "rejected");
        let text = fs::read_to_string(dir.path().join(".weaver/proposals/pppppppp-t/proposal.md"))
            .unwrap();
        assert!(
            text.contains("status: rejected") && text.ends_with("## Feedback\n\nToo big\n"),
            "{text}"
        );

        proposal_at(
            dir.path(),
            "qqqqqqqq",
            "status: draft\n",
            "# Draft\n",
            &[],
            &[],
        );
        let discarded = run(
            "move",
            dir.path(),
            Some("qqqqqqqq"),
            Some("discarded"),
            json!({}),
        )
        .unwrap();
        assert_eq!(discarded.json["status"], "discarded");
        proposal_at(
            dir.path(),
            "rrrrrrrr",
            "status: open\n",
            "# Open\n",
            &[],
            &[],
        );
        let refused = run(
            "move",
            dir.path(),
            Some("rrrrrrrr"),
            Some("discarded"),
            json!({}),
        )
        .unwrap_err();
        assert!(
            refused
                .message
                .contains("an Open Proposal is rejected, not discarded"),
            "{}",
            refused.message
        );
        let misplaced = run(
            "move",
            dir.path(),
            Some("rrrrrrrr"),
            Some("open"),
            json!({ "feedback": "x" }),
        )
        .unwrap_err();
        assert_eq!(misplaced.code, code::INVALID_INPUT);
    }
}
