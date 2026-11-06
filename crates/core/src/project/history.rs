//! History entries (`requirements/data#history-entry`): what a job did, told by its agent for one
//! macro-feature, with the finished Tasks it cites.
//!
//! The agent writes the entries; Weaver never writes one by itself. Everything here is pure over a
//! [`ProjectImage`] and returns a [`ChangeSet`], so the caller writes it whole or not at all.
//!
//! * [`plan_new`] and [`plan_edit`] check every field and every cited Task, list every problem and
//!   write nothing when there is one. A Task cited for the first time passes to *Recorded in
//!   History* (`history-entry.TASKS_CITED`): in the same change Weaver writes its copy into every
//!   entry that cites it and deletes its file and its images ([`plan_clear`]). The copy is part of
//!   the entry file and goes before the deletion; when the copy cannot be built the Task stays,
//!   and [`stuck`] tells the Board why.
//! * `tasks` never shrinks, and a copy in `cleared` is written once and never changed; writing it
//!   does not change `updated`, the date of the entry.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use serde_yaml::{Mapping, Value};

use super::board::task_image_files;
use super::cards::{Card, CardKind};
use super::changes::ChangeSet;
use super::clock::{is_instant, now_instant, Clock};
use super::frontmatter;
use super::ids::{card_stem, slugify};
use super::image::ProjectImage;
use super::layout::HISTORY_DIR;
use super::random::RandomSource;
use super::resolve::resolve_card;
use super::shortnum;

/// The longest summary, in characters.
pub const SUMMARY_MAX: usize = 280;

/// Keys of the front matter, in the order of `requirements/data#history-entry`.
pub const KEY_ORDER: [&str; 5] = ["created", "updated", "summary", "tasks", "cleared"];

/// A problem found in a command: the JSON path of the field and what is wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub path: String,
    pub message: String,
}

impl Problem {
    fn new(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
        }
    }
}

/// What the entry keeps of a Task that left `.weaver/tasks/`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TaskCopy {
    pub title: String,
    pub refs: Vec<String>,
    pub technical: bool,
    /// The Proposal whose design the Task belonged to.
    pub proposal: Option<String>,
    /// The label of that Proposal, when it had one.
    pub label: Option<String>,
    /// The instant the Task reached Done.
    pub finished: String,
    /// What the Task said after its title, in Markdown.
    pub body: String,
}

fn text_value(text: &str) -> Value {
    Value::String(text.to_owned())
}

impl TaskCopy {
    fn to_value(&self) -> Value {
        let mut map = Mapping::new();
        map.insert(text_value("title"), text_value(&self.title));
        map.insert(
            text_value("refs"),
            Value::Sequence(self.refs.iter().map(|r| text_value(r)).collect()),
        );
        map.insert(text_value("technical"), Value::Bool(self.technical));
        if let Some(proposal) = &self.proposal {
            map.insert(text_value("proposal"), text_value(proposal));
        }
        if let Some(label) = &self.label {
            map.insert(text_value("label"), text_value(label));
        }
        map.insert(text_value("finished"), text_value(&self.finished));
        map.insert(text_value("body"), text_value(&self.body));
        Value::Mapping(map)
    }

    fn from_mapping(map: &Mapping) -> Option<Self> {
        Some(Self {
            title: frontmatter::get_str(map, "title")?.to_owned(),
            refs: frontmatter::get_str_list(map, "refs").unwrap_or_default(),
            technical: frontmatter::get_bool(map, "technical").unwrap_or(false),
            proposal: frontmatter::get_str(map, "proposal").map(str::to_owned),
            label: frontmatter::get_str(map, "label").map(str::to_owned),
            finished: frontmatter::get_str(map, "finished")?.to_owned(),
            body: frontmatter::get_str(map, "body").unwrap_or("").to_owned(),
        })
    }
}

/// A History entry as read from its file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Entry {
    pub id: String,
    /// Path relative to `.weaver/`.
    pub path: String,
    pub title: String,
    /// The text after the title, in Markdown.
    pub text: String,
    pub created: String,
    /// When it was written or last updated: the date of the entry.
    pub updated: String,
    pub summary: String,
    /// The Tasks it cites, in the order cited.
    pub tasks: Vec<String>,
    /// The copies it keeps, by Task identity.
    pub cleared: BTreeMap<String, TaskCopy>,
}

/// The text of a card after its first H1, without blank lines around it.
pub fn body_after_title(body: &str) -> String {
    match body.trim_start().strip_prefix("# ") {
        Some(rest) => rest
            .split_once('\n')
            .map_or("", |(_, after)| after)
            .trim_matches('\n')
            .to_owned(),
        None => body.trim_matches('\n').to_owned(),
    }
}

fn cleared_mapping(card: &Card) -> Option<&Mapping> {
    card.front.get("cleared").and_then(Value::as_mapping)
}

/// Reads the entry of a History card; a missing `updated` falls back to `created`.
pub fn entry_of(card: &Card) -> Option<Entry> {
    if card.kind != CardKind::History {
        return None;
    }
    let created = card.created.clone()?;
    let updated = card
        .str_key("updated")
        .filter(|u| is_instant(u))
        .map_or_else(|| created.clone(), str::to_owned);
    let mut tasks: Vec<String> = Vec::new();
    for id in card.list_key("tasks").unwrap_or_default() {
        if !tasks.contains(&id) {
            tasks.push(id);
        }
    }
    let cleared = cleared_mapping(card)
        .map(|map| {
            map.iter()
                .filter_map(|(key, value)| {
                    Some((
                        key.as_str()?.to_owned(),
                        TaskCopy::from_mapping(value.as_mapping()?)?,
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    Some(Entry {
        id: card.id.clone(),
        path: card.path.clone(),
        title: card.title.clone(),
        text: body_after_title(&card.body),
        created,
        updated,
        summary: card.str_key("summary").unwrap_or("").to_owned(),
        tasks,
        cleared,
    })
}

/// Every entry of the project, newest first by its date (ties: the later created, then the
/// identity in byte order, descending).
pub fn entries(image: &ProjectImage) -> Vec<Entry> {
    let mut all: Vec<Entry> = image
        .cards_of(CardKind::History)
        .filter_map(entry_of)
        .collect();
    all.sort_by(|a, b| {
        (b.updated.as_bytes(), b.created.as_bytes(), b.id.as_bytes()).cmp(&(
            a.updated.as_bytes(),
            a.created.as_bytes(),
            a.id.as_bytes(),
        ))
    });
    all
}

/// The Tasks cited by any entry.
pub fn cited_ids(image: &ProjectImage) -> BTreeSet<String> {
    image
        .cards_of(CardKind::History)
        .flat_map(|card| card.list_key("tasks").unwrap_or_default())
        .collect()
}

/// The copy of a Done Task, or why it cannot be made.
pub fn copy_of(image: &ProjectImage, task: &Card) -> Result<TaskCopy, String> {
    if task.str_key("column") != Some("done") {
        return Err("it is not Done".to_owned());
    }
    let finished = match task.str_key("finished") {
        Some(finished) if is_instant(finished) => finished.to_owned(),
        Some(other) => {
            return Err(format!(
                "its `finished` is `{other}`, not a UTC ISO 8601 instant with milliseconds"
            ))
        }
        None => return Err("it has no `finished` instant".to_owned()),
    };
    let proposal = task.str_key("proposal").map(str::to_owned);
    let label = proposal
        .as_deref()
        .and_then(|id| image.card(CardKind::Proposal, id))
        .and_then(|card| card.str_key("label"))
        .map(str::to_owned);
    Ok(TaskCopy {
        title: task.title.clone(),
        refs: task.list_key("refs").unwrap_or_default(),
        technical: task.bool_key("technical").unwrap_or(false),
        proposal,
        label,
        finished,
        body: body_after_title(&task.body),
    })
}

/// The Task `id` cited by `entry` as the History shows it: from the copy the entry keeps, else from
/// the Task file that is still there (its copy could not be made yet), else `None`.
pub fn shown_copy(image: &ProjectImage, entry: &Entry, id: &str) -> Option<TaskCopy> {
    entry
        .cleared
        .get(id)
        .cloned()
        .or_else(|| copy_of(image, image.card(CardKind::Task, id)?).ok())
}

/// A cited Task that is still a file, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stuck {
    pub task: String,
    pub reason: String,
}

/// What recording the cited Tasks changes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClearPlan {
    pub changes: ChangeSet,
    /// The Tasks whose copy is written and whose file goes, in creation order.
    pub recorded: Vec<String>,
    /// The cited Tasks whose copy cannot be made: they stay.
    pub kept: Vec<Stuck>,
}

fn entry_citing<'a>(image: &'a ProjectImage, task: &str) -> Vec<&'a Card> {
    image
        .cards_of(CardKind::History)
        .filter(|card| {
            card.list_key("tasks")
                .is_some_and(|tasks| tasks.iter().any(|t| t == task))
        })
        .collect()
}

/// Records every cited Task that is still a file: its copy goes into every entry that cites it
/// (a copy already there is never changed, and `updated` stays), then its file and its
/// `task-images/<id>/` files are deleted. The copy is built before anything is deleted: a Task
/// whose copy cannot be built stays, listed in `kept`.
pub fn plan_clear(image: &ProjectImage) -> ClearPlan {
    let mut plan = ClearPlan::default();
    // The new front matter of each entry that gets a copy, by path.
    let mut rewritten: BTreeMap<String, (&Card, Mapping)> = BTreeMap::new();
    for task in image.cards_of(CardKind::Task) {
        let citing = entry_citing(image, &task.id);
        if citing.is_empty() {
            continue;
        }
        let copy = match copy_of(image, task) {
            Ok(copy) => copy,
            Err(reason) => {
                plan.kept.push(Stuck {
                    task: task.id.clone(),
                    reason,
                });
                continue;
            }
        };
        for entry in citing {
            let (_, front) = rewritten
                .entry(entry.path.clone())
                .or_insert_with(|| (entry, entry.front.clone()));
            let mut cleared = front
                .get("cleared")
                .and_then(Value::as_mapping)
                .cloned()
                .unwrap_or_default();
            if !cleared.contains_key(&task.id) {
                cleared.insert(text_value(&task.id), copy.to_value());
                front.insert(text_value("cleared"), Value::Mapping(cleared));
            }
        }
        plan.changes.delete(&task.path);
        for file in task_image_files(image.root(), &task.id) {
            plan.changes.delete(&file);
        }
        plan.recorded.push(task.id.clone());
    }
    for (path, (card, front)) in rewritten {
        if front != card.front {
            plan.changes
                .write(&path, frontmatter::render(&front, &KEY_ORDER, &card.body));
        }
    }
    plan
}

/// The cited Tasks that are still files and why, for the warning of the Board.
pub fn stuck(image: &ProjectImage) -> Vec<Stuck> {
    let cited = cited_ids(image);
    image
        .cards_of(CardKind::Task)
        .filter(|task| cited.contains(&task.id))
        .map(|task| Stuck {
            task: task.id.clone(),
            reason: copy_of(image, task)
                .err()
                .unwrap_or_else(|| "its copy in History is not written yet".to_owned()),
        })
        .collect()
}

/// Removes the folders `task-images/<id>/` that the deletion of the files of the `recorded`
/// Tasks left empty (best effort: a folder that still holds something stays).
pub fn remove_task_folders(project_root: &std::path::Path, recorded: &[String]) {
    for id in recorded {
        super::board::remove_empty_folders(&super::layout::absolute(
            project_root,
            &format!("task-images/{id}"),
        ));
    }
}

// ---------------------------------------------------------------------------------------------
// Writing entries

/// What a command that writes an entry produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Planned {
    /// Everything to write: the entry, the copies and the deletions of the Tasks recorded.
    pub changes: ChangeSet,
    pub id: String,
    pub title: String,
    /// The entry file, relative to `.weaver/`.
    pub file: String,
    /// The Tasks cited for the first time (`history-entry.TASKS_CITED`).
    pub cited: Vec<String>,
    /// The Tasks the write adds to the entry: every Task of a new entry, and for an edit the Tasks
    /// it names that the entry did not cite.
    pub added: Vec<String>,
    /// The Tasks now recorded: copy written, file deleted.
    pub recorded: Vec<String>,
    /// The cited Tasks that stay, with the reason.
    pub kept: Vec<Stuck>,
}

fn one_line(text: &str) -> bool {
    !text.contains('\n') && !text.contains('\r')
}

/// The problems of a title and a summary, when given.
pub fn field_problems(title: Option<&str>, summary: Option<&str>) -> Vec<Problem> {
    let mut problems = Vec::new();
    if let Some(title) = title {
        if title.trim().is_empty() {
            problems.push(Problem::new("title", "the title is empty"));
        } else if !one_line(title) {
            problems.push(Problem::new("title", "the title must be on one line"));
        }
    }
    if let Some(summary) = summary {
        let length = summary.trim().chars().count();
        if length == 0 {
            problems.push(Problem::new("summary", "the summary is empty"));
        } else if !one_line(summary) {
            problems.push(Problem::new("summary", "the summary must be on one line"));
        } else if length > SUMMARY_MAX {
            problems.push(Problem::new(
                "summary",
                format!("the summary has {length} characters; at most {SUMMARY_MAX}"),
            ));
        }
    }
    problems
}

fn column_name(column: Option<&str>) -> String {
    match column {
        Some("todo") => "in To do".to_owned(),
        Some("doing") => "in progress".to_owned(),
        Some(other) => format!("in `{other}`"),
        None => "without a column".to_owned(),
    }
}

/// Names the Tasks of a command: every one must exist, be Done and not be closed without
/// changes, and not be recorded in another entry. Returns the identities not yet cited by
/// `own` (no duplicates, in order given) and every problem found.
fn resolve_tasks(
    image: &ProjectImage,
    names: &[String],
    own: &[String],
) -> (Vec<String>, Vec<Problem>) {
    let numbers = shortnum::peek(image.root(), image.cards());
    let label = |id: &str| {
        numbers
            .label_of(CardKind::Task, id)
            .unwrap_or_else(|| id.to_owned())
    };
    let mut found: Vec<String> = Vec::new();
    let mut problems = Vec::new();
    let recorded_in: BTreeMap<String, String> = entries(image)
        .into_iter()
        .rev()
        .flat_map(|entry| {
            entry
                .cleared
                .keys()
                .map(|task| (task.clone(), entry.id.clone()))
                .collect::<Vec<_>>()
        })
        .collect();
    for (index, name) in names.iter().enumerate() {
        let path = format!("tasks[{index}]");
        if let Some(entry) = recorded_in.get(name) {
            if !own.contains(name) && !found.contains(name) {
                problems.push(Problem::new(&path, format!("{name} is already recorded in the History entry {entry}; it cannot be cited again")));
            }
            continue;
        }
        let id = match resolve_card(image, CardKind::Task, name) {
            Ok(id) => id,
            Err(error) => {
                problems.push(Problem::new(&path, error.to_string()));
                continue;
            }
        };
        let Some(task) = image.card(CardKind::Task, &id) else {
            continue;
        };
        if task.str_key("column") != Some("done") {
            problems.push(Problem::new(
                &path,
                format!(
                    "{} is {}: only a Task that is Done goes into History",
                    label(&id),
                    column_name(task.str_key("column"))
                ),
            ));
            continue;
        }
        if task.str_key("unchanged").is_some() {
            problems.push(Problem::new(
                &path,
                format!(
                    "{} was closed without changes: it does not go into History",
                    label(&id)
                ),
            ));
            continue;
        }
        if !own.contains(&id) && !found.contains(&id) {
            found.push(id);
        }
    }
    (found, problems)
}

fn render_entry(front: &Mapping, title: &str, text: &str) -> String {
    let text = text.trim_matches('\n');
    let body = if text.trim().is_empty() {
        format!("# {}\n", title.trim())
    } else {
        format!("# {}\n\n{text}\n", title.trim())
    };
    frontmatter::render(front, &KEY_ORDER, &body)
}

fn task_sequence(tasks: &[String]) -> Value {
    Value::Sequence(tasks.iter().map(|t| text_value(t)).collect())
}

/// Adds the recording of the cited Tasks to `changes` (the entry as it will be written).
fn finish(
    image: &ProjectImage,
    mut changes: ChangeSet,
    id: String,
    title: String,
    file: String,
    cited: Vec<String>,
    added: Vec<String>,
) -> Planned {
    let after = image.with_changes(&changes);
    let clear = plan_clear(&after);
    for (path, change) in clear.changes.iter() {
        match change {
            super::changes::Change::Write(content) => changes.write(path, content.clone()),
            super::changes::Change::Delete => changes.delete(path),
        }
    }
    Planned {
        changes,
        id,
        title,
        file,
        cited,
        added,
        recorded: clear.recorded,
        kept: clear.kept,
    }
}

/// A new entry citing `tasks` (names: identity, unique prefix or short number).
pub fn plan_new(
    image: &ProjectImage,
    title: &str,
    summary: &str,
    body: Option<&str>,
    tasks: &[String],
    clock: &dyn Clock,
    rng: &mut dyn RandomSource,
) -> Result<Planned, Vec<Problem>> {
    let mut problems = field_problems(Some(title), Some(summary));
    if tasks.is_empty() {
        problems.push(Problem::new("tasks", "the entry needs at least one Task"));
    }
    let (cited, task_problems) = resolve_tasks(image, tasks, &[]);
    problems.extend(task_problems);
    let now = now_instant(clock).map_err(|e| vec![Problem::new("clock", e.to_string())]);
    if !problems.is_empty() {
        return Err(problems);
    }
    let now = now?;
    let title = title.trim().to_owned();
    let id = image.new_identity(CardKind::History, rng);
    let file = format!("{HISTORY_DIR}/{}.md", card_stem(&id, &slugify(&title)));
    let mut front = Mapping::new();
    front.insert(text_value("created"), text_value(&now));
    front.insert(text_value("updated"), text_value(&now));
    front.insert(text_value("summary"), text_value(summary.trim()));
    front.insert(text_value("tasks"), task_sequence(&cited));
    let mut changes = ChangeSet::new();
    changes.write(&file, render_entry(&front, &title, body.unwrap_or("")));
    let first_time = first_time(image, &cited);
    Ok(finish(image, changes, id, title, file, first_time, cited))
}

/// The tasks of `ids` that no entry cites yet.
fn first_time(image: &ProjectImage, ids: &[String]) -> Vec<String> {
    let cited = cited_ids(image);
    ids.iter()
        .filter(|id| !cited.contains(*id))
        .cloned()
        .collect()
}

/// An edit of entry `id`: the fields given replace the old ones and the Tasks given are added;
/// no Task is ever removed. Nothing changes (not even `updated`) when the edit changes nothing.
pub fn plan_edit(
    image: &ProjectImage,
    id: &str,
    title: Option<&str>,
    summary: Option<&str>,
    body: Option<&str>,
    tasks: &[String],
    clock: &dyn Clock,
) -> Result<Planned, Vec<Problem>> {
    let Some(card) = image.card(CardKind::History, id) else {
        return Err(vec![Problem::new(
            "id",
            format!("no History entry has the identity {id}"),
        )]);
    };
    let Some(entry) = entry_of(card) else {
        return Err(vec![Problem::new(
            "id",
            format!("the History entry {id} cannot be read"),
        )]);
    };
    let mut problems = field_problems(title, summary);
    let (added, task_problems) = resolve_tasks(image, tasks, &entry.tasks);
    problems.extend(task_problems);
    if !problems.is_empty() {
        return Err(problems);
    }
    let new_title = title.map_or_else(|| entry.title.clone(), |t| t.trim().to_owned());
    let new_summary = summary.map_or_else(|| entry.summary.clone(), |s| s.trim().to_owned());
    let new_text = body.map_or_else(|| entry.text.clone(), |b| b.trim_matches('\n').to_owned());
    let mut all_tasks = entry.tasks.clone();
    all_tasks.extend(added.iter().cloned());
    let unchanged = new_title == entry.title
        && new_summary == entry.summary
        && new_text == entry.text
        && added.is_empty();
    let first_time = first_time(image, &added);
    if unchanged {
        return Ok(finish(
            image,
            ChangeSet::new(),
            id.to_owned(),
            new_title,
            entry.path,
            first_time,
            added,
        ));
    }
    let now = now_instant(clock).map_err(|e| vec![Problem::new("clock", e.to_string())])?;
    let mut front = card.front.clone();
    front.insert(text_value("updated"), text_value(&now));
    front.insert(text_value("summary"), text_value(&new_summary));
    front.insert(text_value("tasks"), task_sequence(&all_tasks));
    let mut changes = ChangeSet::new();
    changes.write(&entry.path, render_entry(&front, &new_title, &new_text));
    Ok(finish(
        image,
        changes,
        id.to_owned(),
        new_title,
        entry.path,
        first_time,
        added,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::changes::{self, Change};
    use crate::project::clock::ManualClock;
    use crate::project::random::ScriptedRandom;
    use crate::project::testing::put;

    const NOW: &str = "2026-11-02T09:30:00.000Z";

    fn done_task(root: &std::path::Path, id: &str, finished: &str, extra: &str) {
        put(
            root,
            &format!("tasks/{id}-job.md"),
            &format!(
                "---\ncolumn: done\ntechnical: false\nrefs:\n  - order/Paid\nproposal: pppppppp\ncreated: 2026-11-02T07:00:01.000Z\nfinished: {finished}\n{extra}---\n# Job {id}\n\nDone when it works.\n"
            ),
        );
    }

    fn project() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        put(dir.path(), "proposals/pppppppp-design/proposal.md", "---\nstatus: approved\nkind: change\ncreated: 2026-11-02T06:00:00.000Z\nlabel: Checkout\n---\n# Design\n");
        done_task(dir.path(), "aaaaaaaa", "2026-11-02T08:00:00.000Z", "");
        done_task(dir.path(), "bbbbbbbb", "2026-11-02T08:30:00.000Z", "");
        dir
    }

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    fn new_entry(
        dir: &tempfile::TempDir,
        title: &str,
        tasks: &[&str],
        at: &str,
    ) -> Result<Planned, Vec<Problem>> {
        let image = ProjectImage::load(dir.path());
        plan_new(
            &image,
            title,
            "One line.",
            Some("The text."),
            &names(tasks),
            &ManualClock::at(at),
            &mut ScriptedRandom::new([0u8, 1, 2, 3, 4, 5, 6, 7, 9, 11, 13, 17, 19, 23, 29, 31]),
        )
    }

    fn write(dir: &tempfile::TempDir, planned: &Planned) -> ProjectImage {
        changes::apply(dir.path(), &planned.changes, |_| Ok::<(), ()>(())).unwrap()
    }

    #[test]
    fn a_new_entry_is_dated_now_and_cites_its_tasks_in_order() {
        let dir = project();
        let planned = new_entry(&dir, "Checkout works", &["bbbbbbbb", "aaaaaaaa"], NOW).unwrap();
        assert!(
            planned.file.starts_with("history/") && planned.file.ends_with("-checkout-works.md")
        );
        let image = write(&dir, &planned);
        let [entry] = entries(&image).try_into().unwrap();
        assert_eq!((entry.created.as_str(), entry.updated.as_str()), (NOW, NOW));
        assert_eq!(entry.tasks, ["bbbbbbbb", "aaaaaaaa"]);
        assert_eq!(
            (
                entry.title.as_str(),
                entry.summary.as_str(),
                entry.text.as_str()
            ),
            ("Checkout works", "One line.", "The text.")
        );
    }

    #[test]
    fn tasks_cited_for_the_first_time_get_their_copy_written_and_their_file_deleted() {
        let dir = project();
        put(dir.path(), "task-images/aaaaaaaa/shot.png", "png");
        let planned = new_entry(&dir, "Checkout works", &["aaaaaaaa"], NOW).unwrap();
        assert_eq!(planned.cited, ["aaaaaaaa"]);
        assert_eq!(planned.recorded, ["aaaaaaaa"]);
        // The copy is part of the entry file, which goes before the files of the Task.
        let order: Vec<&str> = planned
            .changes
            .iter()
            .map(|(path, _)| path.as_str())
            .collect();
        let entry_at = order
            .iter()
            .position(|p| p.starts_with("history/"))
            .unwrap();
        let task_at = order
            .iter()
            .position(|p| *p == "tasks/aaaaaaaa-job.md")
            .unwrap();
        assert!(entry_at < task_at, "{order:?}");
        assert!(matches!(
            planned.changes.get("tasks/aaaaaaaa-job.md"),
            Some(Change::Delete)
        ));
        assert!(matches!(
            planned.changes.get("task-images/aaaaaaaa/shot.png"),
            Some(Change::Delete)
        ));

        let image = write(&dir, &planned);
        assert!(image.card(CardKind::Task, "aaaaaaaa").is_none());
        assert!(
            image.card(CardKind::Task, "bbbbbbbb").is_some(),
            "a Task nobody cites stays"
        );
        let entry = &entries(&image)[0];
        let copy = &entry.cleared["aaaaaaaa"];
        assert_eq!(copy.title, "Job aaaaaaaa");
        assert_eq!(
            (copy.refs.as_slice(), copy.technical),
            (["order/Paid".to_owned()].as_slice(), false)
        );
        assert_eq!(
            (copy.proposal.as_deref(), copy.label.as_deref()),
            (Some("pppppppp"), Some("Checkout"))
        );
        assert_eq!(
            (copy.finished.as_str(), copy.body.as_str()),
            ("2026-11-02T08:00:00.000Z", "Done when it works.")
        );
        assert_eq!(
            entry.updated, NOW,
            "writing the copy does not change the date"
        );
    }

    #[test]
    fn a_task_already_cited_is_not_cited_for_the_first_time_again_and_goes_into_both_entries() {
        let dir = project();
        // The first entry cites a Task whose copy cannot be made, so it stays a file.
        put(dir.path(), "tasks/cccccccc-job.md", "---\ncolumn: done\ntechnical: true\nrefs: []\ncreated: 2026-11-02T07:00:01.000Z\n---\n# Broken\n");
        let first = new_entry(&dir, "First", &["cccccccc"], "2026-11-02T09:00:00.000Z");
        let first = first.unwrap();
        assert_eq!(
            (first.cited.as_slice(), first.recorded.len()),
            (["cccccccc".to_owned()].as_slice(), 0)
        );
        write(&dir, &first);
        // The Task gets its `finished` later; a second entry cites it and `aaaaaaaa`.
        put(dir.path(), "tasks/cccccccc-job.md", "---\ncolumn: done\ntechnical: true\nrefs: []\ncreated: 2026-11-02T07:00:01.000Z\nfinished: 2026-11-02T08:45:00.000Z\n---\n# Broken\n");
        let second = new_entry(
            &dir,
            "Second",
            &["cccccccc", "aaaaaaaa"],
            "2026-11-02T10:00:00.000Z",
        )
        .unwrap();
        assert_eq!(
            second.cited,
            ["aaaaaaaa"],
            "TASKS_CITED only for the Tasks cited for the first time"
        );
        assert_eq!(second.recorded, ["aaaaaaaa", "cccccccc"]);
        let image = write(&dir, &second);
        let all = entries(&image);
        assert_eq!(
            all.iter().map(|e| e.title.as_str()).collect::<Vec<_>>(),
            ["Second", "First"],
            "newest first"
        );
        assert!(all[0].cleared.contains_key("cccccccc") && all[1].cleared.contains_key("cccccccc"));
        assert_eq!(
            all[1].updated, "2026-11-02T09:00:00.000Z",
            "the copy added later keeps the date of the first entry"
        );
    }

    #[test]
    fn a_task_whose_copy_cannot_be_made_stays_and_is_reported() {
        let dir = project();
        put(dir.path(), "tasks/cccccccc-job.md", "---\ncolumn: done\ntechnical: true\nrefs: []\ncreated: 2026-11-02T07:00:01.000Z\n---\n# Broken\n");
        let planned = new_entry(&dir, "Mixed", &["aaaaaaaa", "cccccccc"], NOW).unwrap();
        assert_eq!(planned.recorded, ["aaaaaaaa"]);
        assert_eq!(planned.kept.len(), 1);
        assert_eq!(planned.kept[0].task, "cccccccc");
        assert!(
            planned.kept[0].reason.contains("finished"),
            "{}",
            planned.kept[0].reason
        );
        let image = write(&dir, &planned);
        assert!(
            image.card(CardKind::Task, "cccccccc").is_some(),
            "the Task stays"
        );
        assert_eq!(
            entries(&image)[0].tasks,
            ["aaaaaaaa", "cccccccc"],
            "the entry still cites it"
        );
        assert_eq!(
            stuck(&image)
                .iter()
                .map(|s| s.task.as_str())
                .collect::<Vec<_>>(),
            ["cccccccc"]
        );
    }

    #[cfg(unix)]
    #[test]
    fn when_the_copy_cannot_be_written_nothing_changes_and_the_task_stays() {
        use std::os::unix::fs::PermissionsExt;
        let dir = project();
        let first = new_entry(&dir, "First", &["aaaaaaaa"], NOW).unwrap();
        let before = write(&dir, &first);
        let entry_path = crate::project::layout::absolute(dir.path(), &first.file);
        let folder = entry_path.parent().unwrap();
        // A second entry also cites `bbbbbbbb`; its copy goes into a read-only folder.
        std::fs::set_permissions(folder, std::fs::Permissions::from_mode(0o555)).unwrap();
        let probe = folder.join(".probe");
        let writable = std::fs::write(&probe, "x").is_ok();
        if writable {
            std::fs::remove_file(&probe).unwrap();
            std::fs::set_permissions(folder, std::fs::Permissions::from_mode(0o755)).unwrap();
            return; // running with rights that ignore the mode: nothing to observe
        }
        let id = entries(&before)[0].id.clone();
        let image = ProjectImage::load(dir.path());
        let planned = plan_edit(
            &image,
            &id,
            None,
            None,
            None,
            &names(&["bbbbbbbb"]),
            &ManualClock::at("2026-11-02T10:00:00.000Z"),
        )
        .unwrap();
        let outcome = changes::apply(dir.path(), &planned.changes, |_| Ok::<(), ()>(()));
        std::fs::set_permissions(folder, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(outcome.is_err());
        let after = ProjectImage::load(dir.path());
        assert!(
            after.card(CardKind::Task, "bbbbbbbb").is_some(),
            "the Task stays when the copy fails"
        );
        assert_eq!(
            entries(&after)[0].tasks,
            ["aaaaaaaa"],
            "the entry is as before"
        );
    }

    #[test]
    fn tasks_never_shrink_and_an_edit_dates_the_entry_when_it_changes_something() {
        let dir = project();
        let image = write(
            &dir,
            &new_entry(&dir, "Checkout works", &["aaaaaaaa"], NOW).unwrap(),
        );
        let id = entries(&image)[0].id.clone();
        let later = ManualClock::at("2026-11-02T11:00:00.000Z");

        let planned = plan_edit(
            &image,
            &id,
            None,
            Some("A new summary."),
            None,
            &names(&["bbbbbbbb"]),
            &later,
        )
        .unwrap();
        assert_eq!(planned.cited, ["bbbbbbbb"]);
        let image = write(&dir, &planned);
        let entry = &entries(&image)[0];
        assert_eq!(entry.tasks, ["aaaaaaaa", "bbbbbbbb"]);
        assert_eq!(
            (entry.created.as_str(), entry.updated.as_str()),
            (NOW, "2026-11-02T11:00:00.000Z")
        );
        assert_eq!(
            (
                entry.summary.as_str(),
                entry.title.as_str(),
                entry.text.as_str()
            ),
            ("A new summary.", "Checkout works", "The text.")
        );
        let old_copy = entry.cleared["aaaaaaaa"].clone();

        // Naming only an old Task removes nothing; an edit that changes nothing writes nothing.
        let again =
            plan_edit(&image, &id, None, None, None, &names(&["aaaaaaaa"]), &later).unwrap();
        assert!(again.changes.is_empty() && again.cited.is_empty());
        // Rewriting the text never changes a copy.
        let planned = plan_edit(
            &image,
            &id,
            Some("Renamed"),
            None,
            Some("Other text."),
            &[],
            &later,
        )
        .unwrap();
        let image = write(&dir, &planned);
        let entry = &entries(&image)[0];
        assert_eq!(
            (entry.title.as_str(), entry.text.as_str(), entry.tasks.len()),
            ("Renamed", "Other text.", 2)
        );
        assert_eq!(entry.cleared["aaaaaaaa"], old_copy);
        assert!(
            planned.file.ends_with("-checkout-works.md"),
            "the slug is fixed when the file is created"
        );
    }

    #[test]
    fn every_problem_is_listed_and_nothing_is_written() {
        let dir = project();
        put(dir.path(), "tasks/dddddddd-job.md", "---\ncolumn: todo\ntechnical: true\nrefs: []\ncreated: 2026-11-02T07:00:02.000Z\n---\n# Open\n");
        put(dir.path(), "tasks/eeeeeeee-fix.md", "---\ncolumn: done\ntechnical: true\nrefs: []\ncreated: 2026-11-02T07:00:03.000Z\nfinished: 2026-11-02T08:50:00.000Z\nunchanged: The code already did it.\n---\n# Fix\n");
        let problems = new_entry(
            &dir,
            "Checkout works",
            &["dddddddd", "eeeeeeee", "zzzzzzzz", "aaaaaaaa"],
            NOW,
        )
        .unwrap_err();
        let paths: Vec<&str> = problems.iter().map(|p| p.path.as_str()).collect();
        assert_eq!(paths, ["tasks[0]", "tasks[1]", "tasks[2]"], "{problems:?}");
        assert!(
            problems[0].message.contains("To do"),
            "{}",
            problems[0].message
        );
        assert!(
            problems[1].message.contains("without changes"),
            "{}",
            problems[1].message
        );

        let image = ProjectImage::load(dir.path());
        let clock = ManualClock::at(NOW);
        let mut rng = ScriptedRandom::new([1u8, 2, 3]);
        let many = plan_new(
            &image,
            "Two\nlines",
            &"x".repeat(281),
            None,
            &[],
            &clock,
            &mut rng,
        )
        .unwrap_err();
        assert_eq!(
            many.iter().map(|p| p.path.as_str()).collect::<Vec<_>>(),
            ["title", "summary", "tasks"]
        );
        assert!(entries(&image).is_empty());
    }

    #[test]
    fn a_task_recorded_in_another_entry_cannot_be_cited_again_but_naming_it_in_its_own_entry_is_fine(
    ) {
        let dir = project();
        let image = write(&dir, &new_entry(&dir, "First", &["aaaaaaaa"], NOW).unwrap());
        let first = entries(&image)[0].id.clone();
        let second = new_entry(&dir, "Second", &["aaaaaaaa"], NOW).unwrap_err();
        assert!(second[0].message.contains(&first), "{}", second[0].message);
        let own = plan_edit(
            &image,
            &first,
            None,
            None,
            None,
            &names(&["aaaaaaaa"]),
            &ManualClock::at(NOW),
        )
        .unwrap();
        assert!(own.changes.is_empty());
    }

    #[test]
    fn identities_of_copied_tasks_stay_taken_for_new_tasks() {
        let dir = project();
        let image = write(&dir, &new_entry(&dir, "First", &["aaaaaaaa"], NOW).unwrap());
        assert!(
            image.used_ids(CardKind::Task).contains("aaaaaaaa"),
            "the copy keeps the identity in use"
        );
    }

    #[test]
    fn the_entry_file_has_its_keys_in_order_and_keeps_unknown_ones() {
        let dir = project();
        let image = write(
            &dir,
            &new_entry(&dir, "Checkout works", &["aaaaaaaa"], NOW).unwrap(),
        );
        let entry = &entries(&image)[0];
        let text =
            std::fs::read_to_string(crate::project::layout::absolute(dir.path(), &entry.path))
                .unwrap();
        let keys: Vec<&str> = text
            .lines()
            .skip(1)
            .take_while(|l| *l != "---")
            .filter(|l| !l.starts_with(' ') && !l.starts_with('-'))
            .map(|l| l.split(':').next().unwrap())
            .collect();
        assert_eq!(keys, ["created", "updated", "summary", "tasks", "cleared"]);
        assert!(text.contains("\n---\n# Checkout works\n\nThe text.\n"));
    }
}
