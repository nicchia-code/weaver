//! `proposal new` and `proposal edit`: the files they write.

use std::collections::BTreeSet;
use std::path::Path;

use serde_yaml::Value;

use crate::project::board::{resolution_rows, ResolutionRow};
use crate::project::cards::{Card, CardKind};
use crate::project::changes::{Change, ChangeSet};
use crate::project::ids::{card_stem, draw_identity, slugify, split_card_stem};
use crate::project::image::ProjectImage;
use crate::project::layout::{self, classify, FileClass, PROPOSAL_FILE};
use crate::project::random::RandomSource;
use crate::project::shortnum::ShortNumbers;
use crate::project::text;

use super::analysis;
use super::data::{self, base_value, set, strings, Kind, Status, NEW_FILE};
use super::files::{Folder, Folders};
use super::label::validate_label;
use super::resolution;

// ---------------------------------------------------------------------------------------------
// new
// ---------------------------------------------------------------------------------------------

/// Who writes a Proposal, as far as `request`, `reworks` and the kind are concerned
/// (`requirements/rules#proposal`, **Bindings**). Only what the app vouches for counts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Writer {
    /// Nobody the app vouches for (no identity, one it does not recognise, or no app answering):
    /// binds nothing.
    #[default]
    Other,
    /// The agent of a working Weaver job. `request` is the request the instruction of a setup or
    /// design job names, `reworks` the rejected Proposal the instruction of a rework job names.
    Job {
        request: Option<String>,
        reworks: Option<String>,
    },
}

/// What `proposal new` was asked.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NewProposal {
    pub title: String,
    pub body: Option<String>,
    pub kind: Option<Kind>,
    pub label: Option<String>,
    /// Identities of the Proposals it builds on.
    pub requires: Vec<String>,
    /// The identity of the design request it answers.
    pub request: Option<String>,
    /// The identity of the rejected Proposal it replaces.
    pub reworks: Option<String>,
    /// The job that creates it.
    pub job: Option<String>,
    /// Who writes it.
    pub writer: Writer,
}

/// A Proposal about to be filed as a Draft.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Created {
    pub id: String,
    pub kind: Kind,
    /// `proposals/<id>-<slug>/proposal.md`, relative to `.weaver/`.
    pub path: String,
    pub changes: ChangeSet,
    /// Why it is not bound to its request or to the Proposal it reworks.
    pub note: Option<String>,
}

/// The Proposal `request` or `reworks` was bound to already, as a name.
fn bound_note(numbers: &ShortNumbers, what: &str, other: &Card) -> String {
    let name = numbers
        .label_of(CardKind::Proposal, &other.id)
        .unwrap_or_else(|| other.id.clone());
    format!("The Proposal is not bound to {what}: Proposal {name} is.")
}

/// Binds `request`, or says in `note` why not: a setup or design job binds only the request its
/// instruction names; any other caller binds none. Only the first Proposal binds a request.
fn bind_request(
    image: &ProjectImage,
    numbers: &ShortNumbers,
    writer: &Writer,
    request: &str,
    note: &mut Option<String>,
) -> Option<String> {
    let unbound = |why: &str| format!("The Proposal is not bound to the request {request}: {why}");
    match writer {
        Writer::Other => {
            *note = Some(unbound(
                "only the agent of the job that received the request binds a request.",
            ));
            return None;
        }
        Writer::Job { request: named, .. } if named.as_deref() != Some(request) => {
            *note = Some(unbound("the job did not receive that request."));
            return None;
        }
        _ => {}
    }
    let bound = image.cards_of(CardKind::Proposal).find(|p| {
        data::request_of(p) == Some(request)
            && !matches!(
                data::status_of(p),
                Some(Status::Rejected | Status::Discarded)
            )
    });
    if let Some(other) = bound {
        *note = Some(bound_note(numbers, "the request", other));
        return None;
    }
    if layout::absolute(image.root(), &format!("requests/{request}.md")).is_file() {
        Some(request.to_owned())
    } else {
        *note = Some(unbound("no such request."));
        None
    }
}

/// Binds `reworks`, or says in `note` why not: a rework job binds only the Proposal its
/// instruction names, rejected with Rework; any other caller binds none. A Proposal that
/// something already replaced is bound by nobody.
fn bind_reworks(
    image: &ProjectImage,
    numbers: &ShortNumbers,
    writer: &Writer,
    target: &str,
    note: &mut Option<String>,
) -> Option<String> {
    let unbound =
        |why: &str| format!("The Proposal is not bound to the Proposal it reworks: {why}");
    let rejected = image
        .card(CardKind::Proposal, target)
        .filter(|p| data::status_of(p) == Some(Status::Rejected));
    match writer {
        Writer::Other => {
            *note = Some(unbound("only the agent of a rework job binds a Proposal."));
            return None;
        }
        Writer::Job { reworks, .. } => {
            if reworks.as_deref() != Some(target) {
                *note = Some(unbound("the job does not rework that Proposal."));
                return None;
            }
            if !rejected.is_some_and(data::rework_requested) {
                *note = Some(unbound("that one was not rejected with Rework."));
                return None;
            }
        }
    }
    match image
        .cards_of(CardKind::Proposal)
        .find(|p| data::reworks_of(p) == Some(target))
    {
        Some(other) => {
            *note = Some(bound_note(numbers, "the Proposal it reworks", other));
            None
        }
        None => Some(target.to_owned()),
    }
}

/// Plans a new Draft. Refused (nothing is written), with every reason: a `setup` on a Model that
/// has a Feature or an Actor, `request` together with `reworks`, a label that is not valid, a
/// title that is not one line.
///
/// `request` and `reworks` bind only as [`Writer`] allows and as `requirements/rules#proposal`
/// (**Bindings**) says; otherwise the Proposal is created unbound, with a note saying why.
pub fn plan_new(
    image: &ProjectImage,
    numbers: &ShortNumbers,
    input: &NewProposal,
    created: &str,
    rng: &mut dyn RandomSource,
) -> Result<Created, Vec<String>> {
    let kind = input.kind.unwrap_or(Kind::Change);
    let mut problems = Vec::new();
    if input.title.trim().is_empty() || input.title.contains(['\n', '\r']) {
        problems.push("title: one non-blank line".to_owned());
    }
    let label = match input.label.as_deref().map(validate_label) {
        Some(Ok(label)) => Some(label),
        Some(Err(why)) => {
            problems.push(format!("label: {why}"));
            None
        }
        None => None,
    };
    if kind == Kind::Setup && !image.model().is_empty() {
        problems.push("kind: a `setup` is for a project whose Model has no Feature and no Actor; use `change`".to_owned());
    }
    if input.request.is_some() && input.reworks.is_some() {
        problems.push("request: cannot go together with `reworks`".to_owned());
    }
    if !problems.is_empty() {
        return Err(problems);
    }

    let mut note = None;
    let request = match input.request.as_deref() {
        Some(request) => bind_request(image, numbers, &input.writer, request, &mut note),
        None => None,
    };
    let reworks = match input.reworks.as_deref() {
        Some(target) => bind_reworks(image, numbers, &input.writer, target, &mut note),
        None => None,
    };

    let id = image.new_identity(CardKind::Proposal, rng);
    let title = input.title.trim();
    let folder = format!(
        "{}/{}",
        layout::PROPOSALS_DIR,
        card_stem(&id, &slugify(title))
    );
    let path = format!("{folder}/{PROPOSAL_FILE}");
    let mut front = serde_yaml::Mapping::new();
    set(&mut front, "status", Status::Draft.as_str());
    set(&mut front, "kind", kind.as_str());
    if !input.requires.is_empty() {
        set(&mut front, "requires", strings(&input.requires));
    }
    set(&mut front, "created", created);
    if let Some(request) = &request {
        set(&mut front, "request", request.as_str());
    }
    if let Some(reworks) = &reworks {
        set(&mut front, "reworks", reworks.as_str());
    }
    if let Some(label) = &label {
        set(&mut front, "label", label.as_str());
    }
    if let Some(job) = &input.job {
        set(&mut front, "job", job.as_str());
    }
    let body = input
        .body
        .as_deref()
        .map(|b| b.trim_matches('\n'))
        .unwrap_or_default();
    let text = if body.is_empty() {
        format!("# {title}\n")
    } else {
        format!("# {title}\n\n{body}\n")
    };
    let mut changes = ChangeSet::new();
    changes.write(
        &path,
        crate::project::frontmatter::render(&front, &data::KEYS, &text),
    );
    Ok(Created {
        id,
        kind,
        path,
        changes,
        note,
    })
}

// ---------------------------------------------------------------------------------------------
// edit
// ---------------------------------------------------------------------------------------------

/// One decision of a resolution on a Drift, as `proposal edit` records it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriftDecision {
    /// The identity of the Drift.
    pub drift: String,
    pub decision: String,
    pub option: Option<u32>,
    pub note: Option<String>,
}

/// What `proposal edit` was asked (images are handled by the caller, which copies the files).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EditRequest {
    pub label: Option<String>,
    pub edit: Vec<String>,
    pub delete: Vec<String>,
    pub drifts: Vec<DriftDecision>,
    /// Identities of Drifts to take out of the resolution.
    pub remove_drifts: Vec<String>,
    pub rebase: bool,
}

/// A file the Proposal now has room for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditableFile {
    /// The path the agent asked for, or the Task path with its identity (`tasks/<id>-<slug>.md`).
    pub path: String,
    /// The file to write, relative to `.weaver/`: `proposals/<id>-<slug>/<path>`.
    pub file: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edited {
    pub changes: ChangeSet,
    pub status: Status,
    /// An Open Proposal went back to Draft.
    pub sent_back_to_draft: bool,
    pub files: Vec<EditableFile>,
    /// A rebase dropped these entries of `requires`, each with what was wrong with it.
    pub dropped_requires: Vec<analysis::Required>,
    /// A rebase dropped these rows of a resolution: their Drift is no longer open.
    pub dropped_drifts: Vec<ResolutionRow>,
}

fn is_slug(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= crate::project::ids::SLUG_MAX_LEN
        && text.split('-').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
}

/// The files of the Model, the Requirements and the Specs a Proposal may change.
fn is_model_path(path: &str) -> bool {
    !path.starts_with('/')
        && !path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == ".." || part.starts_with('.'))
        && matches!(
            classify(path),
            FileClass::System
                | FileClass::Actor { .. }
                | FileClass::Feature { .. }
                | FileClass::Machine { .. }
                | FileClass::AreaDoc { .. }
                | FileClass::Fragment { .. }
                | FileClass::Specs
        )
}

fn titled(slug: &str) -> String {
    let mut text = slug.replace('-', " ");
    if let Some(first) = text.get(..1) {
        text = format!("{}{}", first.to_uppercase(), &text[1..]);
    }
    text
}

/// The Task path `tasks/<slug>.md` as it is inside the folder: an existing Task of the Proposal
/// with that slug keeps its identity; a new one gets a new identity.
fn task_file(
    image: &ProjectImage,
    card: &Card,
    asked: &str,
    used: &mut BTreeSet<String>,
    rng: &mut dyn RandomSource,
) -> Result<(String, Option<String>), String> {
    let name = asked
        .strip_prefix("tasks/")
        .and_then(|n| n.strip_suffix(".md"))
        .filter(|n| !n.contains('/'))
        .ok_or_else(|| format!("`{asked}` is not a Task path: `tasks/<slug>.md`"))?;
    // `tasks/<id>-<slug>.md` of a Task of this Proposal.
    if let Some((id, _)) = split_card_stem(name) {
        if data::tasks_of(image, &card.id).iter().any(|t| t.id == id) {
            return Ok((asked.to_owned(), None));
        }
    }
    if !is_slug(name) {
        return Err(format!(
            "`{asked}`: the slug is lowercase letters, digits and `-`, at most 40 characters"
        ));
    }
    if let Some(existing) = data::tasks_of(image, &card.id)
        .into_iter()
        .find(|t| t.slug == name)
    {
        return Ok((
            format!("tasks/{}.md", card_stem(&existing.id, &existing.slug)),
            None,
        ));
    }
    let id = draw_identity(rng, &|candidate| used.contains(candidate));
    used.insert(id.clone());
    let path = format!("tasks/{}.md", card_stem(&id, name));
    let skeleton = format!(
        "---\ntechnical: false\nrefs: []\n---\n# {}\n\nWhat must be true when it is done.\n",
        titled(name)
    );
    Ok((path, Some(skeleton)))
}

/// Plans `proposal edit`. Every call, even without input, sends an Open Proposal back to Draft.
///
/// `read` gives the text of a file of the project on disk (`None` when it does not exist); the
/// current content, with the Proposals this one requires applied below, is copied into the
/// folder the first time a path is made editable, and `base` records its hash (or `new`).
/// Refused, with every problem, and then nothing is written.
pub fn plan_edit(
    image: &ProjectImage,
    folders: &Folders,
    id: &str,
    request: &EditRequest,
    read: &dyn Fn(&str) -> Option<String>,
    rng: &mut dyn RandomSource,
) -> Result<Edited, Vec<String>> {
    let Some(card) = image.card(CardKind::Proposal, id) else {
        return Err(vec![format!("no Proposal has the identity {id}")]);
    };
    let status = data::status_of(card).unwrap_or(Status::Draft);
    let kind = data::kind_of(card);
    if status.is_final() {
        return Err(vec![format!(
            "the Proposal is {}: a decided Proposal never changes",
            status.as_str()
        )]);
    }
    let attached = data::attached_drift(image, id).is_some();
    let empty = Folder::default();
    let folder = folders.get(id).unwrap_or(&empty);

    // A refinement starts from the current files: first it stops requiring what is broken, and
    // everything below (the content copied, `base`) counts only the Proposals that remain.
    let dropped_requires = if request.rebase {
        analysis::broken_requires(image, id)
    } else {
        Vec::new()
    };
    let rebased;
    let image = if dropped_requires.is_empty() {
        image
    } else {
        let kept: Vec<String> = data::requires_of(card)
            .into_iter()
            .filter(|required| !dropped_requires.iter().any(|d| &d.id == required))
            .collect();
        let mut without = ChangeSet::new();
        without.write(
            &card.path,
            data::rewritten(card, |front| {
                if kept.is_empty() {
                    front.remove("requires");
                } else {
                    set(front, "requires", strings(&kept));
                }
            }),
        );
        rebased = image.with_changes(&without);
        &rebased
    };
    let card = image
        .card(CardKind::Proposal, id)
        .expect("the Proposal is still in the image");
    let below = analysis::applied_below(image, folders, id);

    let mut problems: Vec<String> = Vec::new();
    let mut base = data::base_of(card);
    let mut deleted = data::deleted_of(card);
    let mut changes = ChangeSet::new();
    let mut files: Vec<EditableFile> = Vec::new();
    let mut used = image.used_ids(CardKind::Task);

    let label = match request.label.as_deref().map(validate_label) {
        Some(Ok(label)) => Some(label),
        Some(Err(why)) => {
            problems.push(format!("label: {why}"));
            None
        }
        None => None,
    };

    let current = |path: &str| -> Option<String> {
        match below.get(path) {
            Some(Change::Write(content)) => Some(content.clone()),
            Some(Change::Delete) => None,
            None => read(path),
        }
    };

    for (index, path) in request.edit.iter().enumerate() {
        let at = format!("edit[{index}]");
        if path.starts_with("tasks/") {
            if !kind.carries_tasks() {
                problems.push(format!("{at}: a {} carries no Task", kind.as_str()));
                continue;
            }
            if attached {
                problems.push(format!(
                    "{at}: a Proposal attached to a Drift carries no Task"
                ));
                continue;
            }
            match task_file(image, card, path, &mut used, rng) {
                Err(why) => problems.push(format!("{at}: {why}")),
                Ok((task_path, skeleton)) => {
                    let file = data::held_path(card, &task_path);
                    if let Some(text) = skeleton {
                        changes.write(&file, text);
                    }
                    if !files.iter().any(|f| f.path == task_path) {
                        files.push(EditableFile {
                            path: task_path,
                            file,
                        });
                    }
                }
            }
            continue;
        }
        if !is_model_path(path) {
            problems.push(format!("{at}: `{path}` is not a file of the Model, the Requirements or the Specs (`model/…`, `requirements/<area>/<fragment>.md`, `specs.yaml`) or a Task (`tasks/<slug>.md`)"));
            continue;
        }
        if deleted.contains(path) || request.delete.contains(path) {
            problems.push(format!("{at}: `{path}` is removed by this Proposal"));
            continue;
        }
        let file = data::held_path(card, path);
        if !base.contains_key(path) {
            let recorded = match current(path) {
                Some(content) => text::sha256_hex(&text::file_content(&content)),
                None => NEW_FILE.to_owned(),
            };
            base.insert(path.clone(), recorded);
        }
        if !folder.files.contains_key(path) {
            if let Some(content) = current(path) {
                changes.write(&file, content);
            }
        }
        if !files.iter().any(|f| &f.path == path) {
            files.push(EditableFile {
                path: path.clone(),
                file,
            });
        }
    }

    for (index, path) in request.delete.iter().enumerate() {
        let at = format!("delete[{index}]");
        if !is_model_path(path) {
            problems.push(format!(
                "{at}: `{path}` is not a file of the Model, the Requirements or the Specs"
            ));
            continue;
        }
        if folder.files.contains_key(path) || request.edit.contains(path) {
            problems.push(format!(
                "{at}: `{path}` is held by this Proposal: it cannot be removed as well"
            ));
            continue;
        }
        if current(path).is_none() {
            problems.push(format!("{at}: `{path}` does not exist"));
            continue;
        }
        if !base.contains_key(path) {
            let recorded = current(path).map_or_else(
                || NEW_FILE.to_owned(),
                |c| text::sha256_hex(&text::file_content(&c)),
            );
            base.insert(path.clone(), recorded);
        }
        if !deleted.contains(path) {
            deleted.push(path.clone());
        }
    }

    let mut rows: Vec<ResolutionRow> = resolution_rows(card);
    if !request.drifts.is_empty() || !request.remove_drifts.is_empty() {
        if kind != Kind::Resolution {
            problems.push("drifts: only a resolution records decisions on Drifts".to_owned());
        } else {
            for (index, decision) in request.drifts.iter().enumerate() {
                let at = format!("drifts[{index}]");
                if decision.decision != "fix-code"
                    && (decision.option.is_some() || decision.note.is_some())
                {
                    problems.push(format!("{at}: `option` and `note` go only with `fix-code`"));
                    continue;
                }
                let row = ResolutionRow {
                    drift: decision.drift.clone(),
                    decision: decision.decision.clone(),
                    option: decision.option,
                    note: decision.note.clone(),
                };
                match rows.iter_mut().find(|r| r.drift == row.drift) {
                    Some(existing) => *existing = row,
                    None => rows.push(row),
                }
            }
            rows.retain(|r| !request.remove_drifts.contains(&r.drift));
        }
    }
    // A refinement drops the decisions on Drifts no longer open; those still open stay taken.
    let mut dropped_drifts: Vec<ResolutionRow> = Vec::new();
    if request.rebase && kind == Kind::Resolution {
        let (open, closed): (Vec<ResolutionRow>, Vec<ResolutionRow>) =
            rows.into_iter().partition(|row| {
                image
                    .card(CardKind::Drift, &row.drift)
                    .is_some_and(resolution::is_open)
            });
        rows = open;
        dropped_drifts = closed;
    }

    // The refinement's rebase, after the requirements and rows were dropped: `base` again, with
    // the required Proposals that remain applied below.
    if request.rebase {
        let paths: Vec<String> = base.keys().cloned().collect();
        for path in paths {
            let recorded = analysis::current_base(image, folders, id, &path);
            base.insert(path, recorded);
        }
    }

    if !problems.is_empty() {
        return Err(problems);
    }

    let sent_back = status == Status::Open;
    let status_after = if sent_back { Status::Draft } else { status };
    changes.write(
        &card.path,
        data::rewritten(card, |front| {
            set(front, "status", status_after.as_str());
            if !base.is_empty() {
                set(front, "base", base_value(&base));
            }
            if deleted.is_empty() {
                front.remove("delete");
            } else {
                set(front, "delete", strings(&deleted));
            }
            if let Some(label) = &label {
                set(front, "label", label.as_str());
            }
            if request.rebase {
                set(front, "refining", true);
            }
            if kind == Kind::Resolution
                && (!request.drifts.is_empty()
                    || !request.remove_drifts.is_empty()
                    || !dropped_drifts.is_empty())
            {
                let list: Vec<Value> = rows
                    .iter()
                    .map(|row| {
                        let mut map = serde_yaml::Mapping::new();
                        map.insert("drift".into(), row.drift.as_str().into());
                        map.insert("decision".into(), row.decision.as_str().into());
                        if let Some(option) = row.option {
                            map.insert("option".into(), u64::from(option).into());
                        }
                        if let Some(note) = &row.note {
                            map.insert("note".into(), note.as_str().into());
                        }
                        Value::Mapping(map)
                    })
                    .collect();
                set(front, "drifts", Value::Sequence(list));
            }
        }),
    );
    Ok(Edited {
        changes,
        status: status_after,
        sent_back_to_draft: sent_back,
        files,
        dropped_requires,
        dropped_drifts,
    })
}

/// The file of `path` on disk under `.weaver/`, as normalized text; `None` when it is not there.
pub fn read_project_file(project_root: &Path, path: &str) -> Option<String> {
    text::read_text(&layout::absolute(project_root, path)).ok()
}
