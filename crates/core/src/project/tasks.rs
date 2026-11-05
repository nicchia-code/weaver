//! The Task: a unit of work for the agent (machine `task`, feature `work`).
//!
//! Everything here is a pure function over a [`ProjectImage`] that answers with the
//! [`ChangeSet`] of the operation (the caller applies it under the project's write lock) or with
//! the reason it is refused; the instants, the identities and the job that acts come in as
//! arguments. The only reads from disk are the lists of the files of `task-images/<id>/`.
//!
//! * **Blocking** ([`BlockingIndex`]): worked out at every read, never stored. An open Drift that
//!   names the Task as its origin or concerns the same thing blocks it; the origin of a Drift
//!   whose code is to be fixed waits for the fix Task. A Done Task is never blocked, and blocking
//!   never moves a Task.
//! * **Taking, releasing, finishing** ([`plan_take`], [`plan_release`], [`plan_finish`]), and what
//!   ends a hold without the job's word ([`plan_job_ended`], [`plan_orphans`],
//!   [`plan_origin_waits`]).
//! * **Runs**: the runnable Tasks and the next one ([`runnable`]), the free places ([`pick_slots`]),
//!   the groups of one sub-agent ([`groups`], [`pick_groups`]), `Run all` by model
//!   ([`run_all_groups`], [`split_sub_agents`]) and the reasons a card's `Run` is disabled
//!   ([`run_reasons`]).
//! * **Creation** ([`TaskDraft`], [`validate_draft`], [`plan_new_task`]) of the Tasks the agent
//!   files, of the fix Task of a Drift ([`fix_task_body`]) and of the Tasks an approved Proposal
//!   carries ([`approved_task_file`]).
//! * **Retention** ([`plan_forgetting`]): a Done Task that 20 other Tasks finished after, and that
//!   no History entry cites, is forgotten unless an open Drift names it as origin. A Done Task a
//!   History entry cites is recorded there and removed by [`super::history::plan_clear`].

use std::collections::{BTreeMap, BTreeSet};

use pulldown_cmark::{Event, Parser};
use serde_yaml::{Mapping, Value};

use super::board::{self, AgentFacts};
use super::cards::{Card, CardKind};
use super::changes::{Change, ChangeSet};
use super::concerns::{card_keys, key_lists_concern, Key};
use super::drift::{fingerprint, DRIFT_KEYS};
use super::frontmatter;
use super::history;
use super::ids::{card_stem, slugify};
use super::image::ProjectImage;
use super::layout;
use super::order::compare;
use super::random::RandomSource;
use super::refs::{parse_reference, Reference};
use super::shortnum::ShortNumbers;

/// Keys of a Task file, in the order `requirements/data#task` lists them.
pub const KEYS: [&str; 10] = [
    "column",
    "technical",
    "refs",
    "proposal",
    "drift",
    "job",
    "holder",
    "created",
    "finished",
    "unchanged",
];

/// A Done Task not cited by a History entry is forgotten when this many other Tasks have
/// finished after it.
pub const FORGET_AFTER: usize = 20;

/// A Task shares its group with at most this many other Tasks of the group per sub-agent.
pub const GROUP_EXTRAS: usize = 2;

// ---------------------------------------------------------------------------------------------
// What a Task file says
// ---------------------------------------------------------------------------------------------

/// The `column` of a Task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Column {
    Todo,
    Doing,
    Done,
}

impl Column {
    pub fn parse(text: &str) -> Option<Column> {
        match text {
            "todo" => Some(Column::Todo),
            "doing" => Some(Column::Doing),
            "done" => Some(Column::Done),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Column::Todo => "todo",
            Column::Doing => "doing",
            Column::Done => "done",
        }
    }

    /// The name of the state (`To do`, `In progress`, `Done`).
    pub fn state_name(self) -> &'static str {
        match self {
            Column::Todo => "To do",
            Column::Doing => "In progress",
            Column::Done => "Done",
        }
    }
}

pub fn column_of(task: &Card) -> Option<Column> {
    task.str_key("column").and_then(Column::parse)
}

pub fn is_technical(task: &Card) -> bool {
    task.bool_key("technical").unwrap_or(false)
}

/// The `refs` of a card, as written.
pub fn refs_of(card: &Card) -> Vec<String> {
    card.list_key("refs").unwrap_or_default()
}

/// The Task is the fix Task of a Drift.
pub fn is_fix_task(task: &Card) -> bool {
    task.str_key("drift").is_some()
}

/// The instant the Task reached Done.
pub fn finished_of(task: &Card) -> &str {
    task.str_key("finished").unwrap_or("")
}

/// The body without its first H1 line (the title).
pub fn body_after_title(card: &Card) -> String {
    match card.body.trim_start().strip_prefix("# ") {
        Some(rest) => rest
            .split_once('\n')
            .map_or("", |(_, after)| after)
            .trim_matches('\n')
            .to_owned(),
        None => card.body.trim_matches('\n').to_owned(),
    }
}

/// The label of the Proposal whose design the Task belongs to.
pub fn proposal_label(image: &ProjectImage, task: &Card) -> Option<String> {
    let proposal = image.card(CardKind::Proposal, task.str_key("proposal")?)?;
    proposal.str_key("label").map(str::to_owned)
}

fn label(numbers: &ShortNumbers, kind: CardKind, id: &str) -> String {
    numbers.label_of(kind, id).unwrap_or_else(|| id.to_owned())
}

fn created_of(card: &Card) -> &str {
    card.created.as_deref().unwrap_or("")
}

/// The Tasks in To do, In progress or Done, in creation order.
pub fn tasks_in(image: &ProjectImage, column: Column) -> Vec<&Card> {
    image
        .cards_of(CardKind::Task)
        .filter(|t| column_of(t) == Some(column))
        .collect()
}

/// A refused operation; nothing was written.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct Refusal(pub String);

fn refuse<T>(message: impl Into<String>) -> Result<T, Refusal> {
    Err(Refusal(message.into()))
}

fn task_card<'a>(image: &'a ProjectImage, id: &str) -> Result<&'a Card, Refusal> {
    match image.card(CardKind::Task, id) {
        Some(task) => Ok(task),
        None => refuse(format!("no Task has the identity {id}")),
    }
}

/// The text of `card` after an edit of its front matter.
fn rewritten(card: &Card, keys: &[&str], edit: impl FnOnce(&mut Mapping)) -> String {
    let mut front = card.front.clone();
    edit(&mut front);
    frontmatter::render(&front, keys, &card.body)
}

fn set(front: &mut Mapping, key: &str, value: impl Into<Value>) {
    front.insert(Value::String(key.to_owned()), value.into());
}

fn absorb(into: &mut ChangeSet, other: ChangeSet) {
    for (rel, change) in other.iter() {
        match change {
            Change::Write(content) => into.write(rel, content.clone()),
            Change::Delete => into.delete(rel),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Blocking
// ---------------------------------------------------------------------------------------------

/// What blocks a Task.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Blocking {
    /// Open Drifts that name it as origin or concern the same thing, in creation order.
    pub drifts: Vec<String>,
    /// Fix Tasks (not Done) of Drifts whose code is to be fixed, for which it is the origin.
    pub fix_tasks: Vec<String>,
}

impl Blocking {
    pub fn is_blocked(&self) -> bool {
        !self.drifts.is_empty() || !self.fix_tasks.is_empty()
    }
}

/// The facts blocking needs, read once from an image, so that every Task of the project is
/// answered without comparing every file with every other.
pub struct BlockingIndex<'a> {
    image: &'a ProjectImage,
    open: Vec<(&'a Card, Vec<Key>)>,
    /// `(origin Task, fix Task)` of the Drifts whose code is to be fixed and whose fix Task is
    /// not Done, in the creation order of the Drifts.
    waits: Vec<(&'a str, &'a str)>,
}

impl<'a> BlockingIndex<'a> {
    pub fn new(image: &'a ProjectImage) -> Self {
        let live: BTreeSet<&str> = image
            .cards_of(CardKind::Task)
            .filter(|t| column_of(t) != Some(Column::Done))
            .map(|t| t.id.as_str())
            .collect();
        let open = image
            .cards_of(CardKind::Drift)
            .filter(|d| d.str_key("status") == Some("open"))
            .map(|d| (d, card_keys(d)))
            .collect();
        let waits = image
            .cards_of(CardKind::Drift)
            .filter(|d| d.str_key("status") == Some("code-to-be-fixed"))
            .filter_map(|d| Some((d.str_key("origin")?, d.str_key("task")?)))
            .filter(|(_, fix)| live.contains(fix))
            .collect();
        Self { image, open, waits }
    }

    /// What blocks `task`; nothing for a Done Task.
    pub fn of(&self, task: &Card) -> Blocking {
        if column_of(task) == Some(Column::Done) {
            return Blocking::default();
        }
        let keys = card_keys(task);
        let model = self.image.model();
        Blocking {
            drifts: self
                .open
                .iter()
                .filter(|(drift, drift_keys)| {
                    drift.str_key("origin") == Some(task.id.as_str())
                        || key_lists_concern(model, &keys, drift_keys)
                })
                .map(|(drift, _)| drift.id.clone())
                .collect(),
            fix_tasks: self
                .waits
                .iter()
                .filter(|(origin, _)| *origin == task.id)
                .map(|(_, fix)| (*fix).to_owned())
                .collect(),
        }
    }
}

/// What blocks `task` in `image`.
pub fn blocking_of(image: &ProjectImage, task: &Card) -> Blocking {
    BlockingIndex::new(image).of(task)
}

/// `Blocked by Drift D3, D7` and `Waiting for Task T4` (the tooltips and the reasons of `Run`).
pub fn blocked_text(drifts: &[String]) -> String {
    format!("Blocked by Drift {}", drifts.join(", "))
}

pub fn waiting_text(fix_tasks: &[String]) -> String {
    format!("Waiting for Task {}", fix_tasks.join(", "))
}

fn blocking_sentence(image: &ProjectImage, numbers: &ShortNumbers, blocking: &Blocking) -> String {
    let mut parts = Vec::new();
    for id in &blocking.drifts {
        let title = image
            .card(CardKind::Drift, id)
            .map_or("", |d| d.title.as_str());
        parts.push(format!(
            "blocked by Drift {} \"{title}\"",
            label(numbers, CardKind::Drift, id)
        ));
    }
    for id in &blocking.fix_tasks {
        let title = image
            .card(CardKind::Task, id)
            .map_or("", |t| t.title.as_str());
        parts.push(format!(
            "waiting for the fix Task {} \"{title}\"",
            label(numbers, CardKind::Task, id)
        ));
    }
    parts.join("; ")
}

// ---------------------------------------------------------------------------------------------
// Taking, releasing, finishing
// ---------------------------------------------------------------------------------------------

/// `agent.TAKE_TASK`: To do → In progress, held by `job`. Refused unless the Task is in To do and
/// not blocked; the refusal names the Drift (number and title) or the fix Task.
pub fn plan_take(
    image: &ProjectImage,
    numbers: &ShortNumbers,
    task_id: &str,
    job: &str,
) -> Result<ChangeSet, Refusal> {
    let task = task_card(image, task_id)?;
    let name = label(numbers, CardKind::Task, &task.id);
    match column_of(task) {
        Some(Column::Todo) => {}
        Some(Column::Doing) => return refuse(format!("{name} is already in progress")),
        Some(Column::Done) => return refuse(format!("{name} is Done")),
        None => return refuse(format!("{name} is in no known column")),
    }
    let blocking = blocking_of(image, task);
    if blocking.is_blocked() {
        return refuse(format!(
            "{name} cannot be taken: {}",
            blocking_sentence(image, numbers, &blocking)
        ));
    }
    let mut changes = ChangeSet::new();
    changes.write(
        &task.path,
        rewritten(task, &KEYS, |front| {
            set(front, "column", "doing");
            set(front, "holder", job);
        }),
    );
    Ok(changes)
}

/// The Task is In progress and held by `job`; otherwise the refusal says why not.
fn held_by_job<'a>(
    image: &'a ProjectImage,
    numbers: &ShortNumbers,
    task_id: &str,
    job: &str,
) -> Result<&'a Card, Refusal> {
    let task = task_card(image, task_id)?;
    let name = label(numbers, CardKind::Task, &task.id);
    if column_of(task) != Some(Column::Doing) {
        let state = column_of(task).map_or("in no known column", |c| c.state_name());
        return refuse(format!("{name} is not in progress (it is {state}): only the job that holds a Task moves it out of progress"));
    }
    if task.str_key("holder") != Some(job) {
        return refuse(format!("{name} is held by another job: only the job that holds a Task moves it out of progress"));
    }
    Ok(task)
}

fn back_to_todo(task: &Card) -> String {
    rewritten(task, &KEYS, |front| {
        set(front, "column", "todo");
        front.remove("holder");
    })
}

/// `agent.RELEASE_TASK`: In progress → To do, only by the job that holds it.
pub fn plan_release(
    image: &ProjectImage,
    numbers: &ShortNumbers,
    task_id: &str,
    job: &str,
) -> Result<ChangeSet, Refusal> {
    let task = held_by_job(image, numbers, task_id, job)?;
    let mut changes = ChangeSet::new();
    changes.write(&task.path, back_to_todo(task));
    Ok(changes)
}

/// Which event a finished Task emits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// `task.FINISHED`: the Drift whose fix Task it is goes to Fixed; Done Tasks are counted.
    Finished,
    /// `task.CLOSED_UNCHANGED`: the Drift whose fix Task it is goes to Dismissed.
    ClosedUnchanged,
}

/// What finishing a Task changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinishPlan {
    pub changes: ChangeSet,
    pub outcome: Outcome,
    /// The Drift that went to Fixed or Dismissed.
    pub drift: Option<String>,
    /// The Done Tasks forgotten because this one finished (`FINISHED` only).
    pub forgotten: Vec<String>,
}

/// The Drift whose code is to be fixed and whose fix Task is `task`.
fn drift_of_fix_task<'a>(image: &'a ProjectImage, task: &Card) -> Option<&'a Card> {
    image.cards_of(CardKind::Drift).find(|d| {
        d.str_key("status") == Some("code-to-be-fixed")
            && d.str_key("task") == Some(task.id.as_str())
    })
}

/// `agent.FINISH_TASK` (`unchanged` is `None`) and `agent.CLOSE_TASK_UNCHANGED` (`unchanged` is
/// the non-blank note, only for a fix Task): In progress → Done, only by the job that holds it,
/// at the instant `finished`.
///
/// The Drift whose fix Task it is goes to Fixed (`FINISHED`) or Dismissed (`CLOSED_UNCHANGED`),
/// and a finish forgets the Done Tasks 20 other Tasks have finished after.
pub fn plan_finish(
    image: &ProjectImage,
    numbers: &ShortNumbers,
    task_id: &str,
    job: &str,
    finished: &str,
    unchanged: Option<&str>,
) -> Result<FinishPlan, Refusal> {
    let task = held_by_job(image, numbers, task_id, job)?;
    let name = label(numbers, CardKind::Task, &task.id);
    if let Some(note) = unchanged {
        if note.trim().is_empty() {
            return refuse("`unchanged` needs a note that says what was checked");
        }
        if !is_fix_task(task) {
            return refuse(format!(
                "{name} is not the fix Task of a Drift: only a fix Task is closed without changes"
            ));
        }
    }
    let mut changes = ChangeSet::new();
    changes.write(
        &task.path,
        rewritten(task, &KEYS, |front| {
            set(front, "column", "done");
            front.remove("holder");
            set(front, "finished", finished);
            if let Some(note) = unchanged {
                set(front, "unchanged", note.trim());
            }
        }),
    );
    let outcome = if unchanged.is_some() {
        Outcome::ClosedUnchanged
    } else {
        Outcome::Finished
    };
    let drift = drift_of_fix_task(image, task).map(|drift| {
        let status = if outcome == Outcome::Finished {
            "fixed"
        } else {
            "dismissed"
        };
        changes.write(
            &drift.path,
            rewritten(drift, &DRIFT_KEYS, |front| {
                set(front, "status", status);
                // `task.CLOSED_UNCHANGED` dismisses the Drift with the note that says why nothing changed,
                // and a dismissal records the text fingerprint of what the Drift concerns.
                if let Some(note) = unchanged {
                    set(front, "note", note.trim());
                    set(front, "decided", finished);
                    set(
                        front,
                        "fingerprint",
                        fingerprint::stored_of(image, drift).as_str(),
                    );
                }
            }),
        );
        drift.id.clone()
    });
    let mut forgotten = Vec::new();
    if outcome == Outcome::Finished {
        let after = image.with_changes(&changes);
        let forgetting = plan_forgetting(&after);
        forgotten = forgetting.forgotten;
        absorb(&mut changes, forgetting.changes);
    }
    Ok(FinishPlan {
        changes,
        outcome,
        drift,
        forgotten,
    })
}

/// `job.ENDED`: every Task `job` held goes back to To do, whatever the reason it ended.
pub fn plan_job_ended(image: &ProjectImage, job: &str) -> ChangeSet {
    let mut changes = ChangeSet::new();
    for task in image.cards_of(CardKind::Task) {
        if column_of(task) == Some(Column::Doing) && task.str_key("holder") == Some(job) {
            changes.write(&task.path, back_to_todo(task));
        }
    }
    changes
}

/// When a project opens: a Task In progress whose `holder` is unknown to the job registry
/// (`known` says whether a job is known), or without a `holder`, counts as held by a job that
/// ended and goes back to To do.
pub fn plan_orphans(image: &ProjectImage, known: &dyn Fn(&str) -> bool) -> ChangeSet {
    let mut changes = ChangeSet::new();
    for task in image.cards_of(CardKind::Task) {
        if column_of(task) == Some(Column::Doing) && !task.str_key("holder").is_some_and(known) {
            changes.write(&task.path, back_to_todo(task));
        }
    }
    changes
}

/// `drift.CODE_TO_BE_FIXED`: the Task named as origin of the Drift goes back to To do when it is
/// In progress; it waits for the fix Task. Nothing changes for any other column.
pub fn plan_origin_waits(image: &ProjectImage, origin_task: &str) -> ChangeSet {
    let mut changes = ChangeSet::new();
    if let Some(task) = image.card(CardKind::Task, origin_task) {
        if column_of(task) == Some(Column::Doing) {
            changes.write(&task.path, back_to_todo(task));
        }
    }
    changes
}

// ---------------------------------------------------------------------------------------------
// Runnable, slots, groups
// ---------------------------------------------------------------------------------------------

/// The Tasks in To do and not blocked, oldest first.
pub fn runnable(image: &ProjectImage) -> Vec<&Card> {
    let index = BlockingIndex::new(image);
    tasks_in(image, Column::Todo)
        .into_iter()
        .filter(|t| !index.of(t).is_blocked())
        .collect()
}

/// The next Task to run: the oldest runnable one.
pub fn next_runnable(image: &ProjectImage) -> Option<&Card> {
    runnable(image).into_iter().next()
}

/// Why a runnable Task was not picked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeldBack {
    /// It concerns the same thing as a Task already picked (its identity).
    SameAsPicked(String),
    /// It concerns the same thing as a Task In progress (its identity).
    SameAsInProgress(String),
    /// The places were all taken.
    NoPlace,
}

impl HeldBack {
    pub fn text(&self, numbers: &ShortNumbers) -> String {
        match self {
            HeldBack::SameAsPicked(id) => {
                format!(
                    "it concerns the same thing as {}, picked for this run",
                    label(numbers, CardKind::Task, id)
                )
            }
            HeldBack::SameAsInProgress(id) => {
                format!(
                    "it concerns the same thing as {}, which is in progress",
                    label(numbers, CardKind::Task, id)
                )
            }
            HeldBack::NoPlace => "all the places are taken".to_owned(),
        }
    }
}

/// The result of [`pick_slots`].
#[derive(Debug, Clone, PartialEq)]
pub struct Slots<'a> {
    pub picked: Vec<&'a Card>,
    pub held_back: Vec<(&'a Card, HeldBack)>,
}

/// Slots: walks the runnable Tasks `eligible` says yes to, oldest first, picking each that
/// concerns the same thing as no Task already picked and no Task In progress, until `max` are
/// picked or none is left. Every other eligible runnable Task is held back, with the reason.
pub fn pick_slots<'a>(
    image: &'a ProjectImage,
    max: usize,
    eligible: &dyn Fn(&Card) -> bool,
) -> Slots<'a> {
    let model = image.model();
    let doing: Vec<(&Card, Vec<Key>)> = tasks_in(image, Column::Doing)
        .into_iter()
        .map(|t| (t, card_keys(t)))
        .collect();
    let mut picked: Vec<(&Card, Vec<Key>)> = Vec::new();
    let mut held_back = Vec::new();
    for task in runnable(image).into_iter().filter(|t| eligible(t)) {
        let keys = card_keys(task);
        if let Some((other, _)) = doing
            .iter()
            .find(|(_, other_keys)| key_lists_concern(model, &keys, other_keys))
        {
            held_back.push((task, HeldBack::SameAsInProgress(other.id.clone())));
        } else if let Some((other, _)) = picked
            .iter()
            .find(|(_, other_keys)| key_lists_concern(model, &keys, other_keys))
        {
            held_back.push((task, HeldBack::SameAsPicked(other.id.clone())));
        } else if picked.len() < max {
            picked.push((task, keys));
        } else {
            held_back.push((task, HeldBack::NoPlace));
        }
    }
    Slots {
        picked: picked.into_iter().map(|(task, _)| task).collect(),
        held_back,
    }
}

/// The file paths between backticks in a text: a span without spaces that ends in a file
/// extension (a dot and lowercase letters or digits) or in `/`. A Model reference such as
/// `order/Paid` or `user.CANCEL` is not a path.
pub fn backticked_paths(text: &str) -> BTreeSet<String> {
    Parser::new(text)
        .filter_map(|event| match event {
            Event::Code(code) => Some(code.into_string()),
            _ => None,
        })
        .filter(|span| is_file_path(span))
        .map(|span| span.strip_prefix("./").unwrap_or(&span).to_owned())
        .collect()
}

fn is_file_path(span: &str) -> bool {
    if span.is_empty() || span.chars().any(char::is_whitespace) {
        return false;
    }
    if span.ends_with('/') {
        return span.len() > 1;
    }
    let last = span.rsplit('/').next().unwrap_or(span);
    match last.rsplit_once('.') {
        Some((stem, extension)) => {
            !stem.is_empty()
                && !extension.is_empty()
                && extension
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        }
        None => false,
    }
}

/// The groups of the Tasks in To do: Tasks linked transitively by concerning the same thing or
/// by naming the same backticked file path. Each group is in creation order, and the groups are
/// ordered by their oldest Task.
pub fn groups(image: &ProjectImage) -> Vec<Vec<&Card>> {
    let model = image.model();
    let todo = tasks_in(image, Column::Todo);
    let keys: Vec<Vec<Key>> = todo.iter().map(|t| card_keys(t)).collect();
    let paths: Vec<BTreeSet<String>> = todo.iter().map(|t| backticked_paths(&t.body)).collect();
    let mut parent: Vec<usize> = (0..todo.len()).collect();
    fn find(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    // Tasks that name one path are linked through the first Task that names it; the Tasks that
    // concern the same thing are compared two by two (a project holds few Tasks in To do).
    let mut first_with_path: BTreeMap<&str, usize> = BTreeMap::new();
    for i in 0..todo.len() {
        for path in &paths[i] {
            let first = *first_with_path.entry(path.as_str()).or_insert(i);
            let (a, b) = (find(&mut parent, first), find(&mut parent, i));
            parent[b] = a;
        }
        for j in 0..i {
            if key_lists_concern(model, &keys[i], &keys[j]) {
                let (a, b) = (find(&mut parent, j), find(&mut parent, i));
                parent[b] = a;
            }
        }
    }
    let mut by_root: BTreeMap<usize, Vec<&Card>> = BTreeMap::new();
    for (i, card) in todo.iter().enumerate() {
        let root = find(&mut parent, i);
        by_root.entry(root).or_default().push(*card);
    }
    let mut groups: Vec<Vec<&Card>> = by_root.into_values().collect();
    groups.sort_by(|a, b| compare(created_of(a[0]), &a[0].id, created_of(b[0]), &b[0].id));
    groups
}

/// Slots with groups: each Task picked by [`pick_slots`] brings up to [`GROUP_EXTRAS`] more
/// runnable Tasks of its group, oldest first, for the same sub-agent (at most 3 Tasks of a group
/// per sub-agent). The groups come largest first, equal ones by their oldest Task. Also returns
/// the held-back Tasks of the slots.
pub fn pick_groups<'a>(
    image: &'a ProjectImage,
    max: usize,
    eligible: &dyn Fn(&Card) -> bool,
) -> (Vec<Vec<&'a Card>>, Vec<(&'a Card, HeldBack)>) {
    let slots = pick_slots(image, max, eligible);
    let all_groups = groups(image);
    let runnable_ids: BTreeSet<&str> = runnable(image)
        .into_iter()
        .filter(|t| eligible(t))
        .map(|t| t.id.as_str())
        .collect();
    let mut taken: BTreeSet<&str> = slots.picked.iter().map(|t| t.id.as_str()).collect();
    let mut result: Vec<Vec<&Card>> = Vec::new();
    for &pick in &slots.picked {
        let mut group = vec![pick];
        if let Some(members) = all_groups
            .iter()
            .find(|g| g.iter().any(|t| t.id == pick.id))
        {
            for member in members {
                if group.len() > GROUP_EXTRAS {
                    break;
                }
                if runnable_ids.contains(member.id.as_str()) && !taken.contains(member.id.as_str())
                {
                    taken.insert(member.id.as_str());
                    group.push(member);
                }
            }
        }
        result.push(group);
    }
    result.sort_by_key(|group| std::cmp::Reverse(group.len()));
    let mut held_back = slots.held_back;
    held_back.retain(|(task, _)| !taken.contains(task.id.as_str()));
    (result, held_back)
}

// ---------------------------------------------------------------------------------------------
// Run all
// ---------------------------------------------------------------------------------------------

/// The runnable Tasks that share one effective model.
#[derive(Debug, Clone, PartialEq)]
pub struct RunGroup<'a> {
    /// The model, trimmed.
    pub model: String,
    pub tasks: Vec<&'a Card>,
}

/// `Run all`: the runnable Tasks grouped by effective model, in the order of each group's oldest
/// Task. The effective model of a Task is the one chosen for the Proposal of its design
/// (`chosen_model` answers for a Proposal identity), else `execution_model`; models are trimmed
/// and compared exactly. One Run per group, started together.
pub fn run_all_groups<'a>(
    image: &'a ProjectImage,
    chosen_model: &dyn Fn(&str) -> Option<String>,
    execution_model: &str,
) -> Vec<RunGroup<'a>> {
    let mut groups: Vec<RunGroup> = Vec::new();
    for task in runnable(image) {
        let model = task
            .str_key("proposal")
            .and_then(chosen_model)
            .map(|m| m.trim().to_owned())
            .unwrap_or_else(|| execution_model.trim().to_owned());
        match groups.iter_mut().find(|g| g.model == model) {
            Some(group) => group.tasks.push(task),
            None => groups.push(RunGroup {
                model,
                tasks: vec![task],
            }),
        }
    }
    groups
}

/// Shares the sub-agent limit among groups in proportion to their runnable Tasks (`counts`,
/// groups in the order of their oldest Task): at least 1 each, the rest to the largest
/// fractional parts, ties to the group with the oldest Task (the earlier one).
pub fn split_sub_agents(limit: usize, counts: &[usize]) -> Vec<usize> {
    let total: usize = counts.iter().sum();
    if total == 0 {
        return vec![0; counts.len()];
    }
    let mut shares: Vec<usize> = Vec::with_capacity(counts.len());
    let mut remainders: Vec<(usize, usize)> = Vec::with_capacity(counts.len());
    for (index, &count) in counts.iter().enumerate() {
        let exact = limit * count;
        shares.push((exact / total).max(1));
        remainders.push((index, exact % total));
    }
    let given: usize = shares.iter().sum();
    let mut left = limit.saturating_sub(given);
    remainders.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    for (index, remainder) in remainders {
        if left == 0 {
            break;
        }
        // A group raised to 1 already holds more than its floor.
        if remainder > 0 {
            shares[index] += 1;
            left -= 1;
        }
    }
    shares
}

// ---------------------------------------------------------------------------------------------
// Run
// ---------------------------------------------------------------------------------------------

/// The reasons a Task's `Run` is disabled, in the fixed order of the control: `The agent is
/// still finishing`, `A merge is in progress`, `Drifts are being resolved`, `A run is in
/// progress`, `The agent is working on it`, `Blocked by Drift <ids>`, `Waiting for Task <id>`.
/// Another Task held by the agent never disables it. `name` shows a Drift or a Task (`D3`, or its
/// permanent id).
pub fn run_reasons(
    image: &ProjectImage,
    facts: &AgentFacts,
    task: &Card,
    blocking: &Blocking,
    name: &dyn Fn(CardKind, &str) -> String,
) -> Vec<String> {
    let mut reasons = Vec::new();
    if facts.is_finishing(task) {
        reasons.push(board::REASON_AGENT_FINISHING.to_owned());
    }
    if facts.merge_in_progress {
        reasons.push(board::REASON_MERGE_IN_PROGRESS.to_owned());
    }
    if board::drifts_resolving(image, facts) {
        reasons.push(board::REASON_DRIFTS_RESOLVING.to_owned());
    }
    if facts.run_in_progress {
        reasons.push(board::REASON_RUN_IN_PROGRESS.to_owned());
    }
    if facts.holds(task) {
        reasons.push("The agent is working on it".to_owned());
    }
    if !blocking.drifts.is_empty() {
        let names: Vec<String> = blocking
            .drifts
            .iter()
            .map(|id| name(CardKind::Drift, id))
            .collect();
        reasons.push(blocked_text(&names));
    }
    if !blocking.fix_tasks.is_empty() {
        let names: Vec<String> = blocking
            .fix_tasks
            .iter()
            .map(|id| name(CardKind::Task, id))
            .collect();
        reasons.push(waiting_text(&names));
    }
    reasons
}

// ---------------------------------------------------------------------------------------------
// Creation
// ---------------------------------------------------------------------------------------------

/// What a new Task says.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TaskDraft {
    pub title: String,
    /// What must be true when it is done, in Markdown; may be empty.
    pub body: String,
    pub refs: Vec<String>,
    pub technical: bool,
    /// The Proposal whose design it belongs to.
    pub proposal: Option<String>,
    /// The Drift whose fix Task it is.
    pub drift: Option<String>,
    /// The job that filed it.
    pub job: Option<String>,
}

/// A problem of a draft, with the JSON path of the input it comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftProblem {
    pub path: String,
    pub message: String,
}

fn problem(path: impl Into<String>, message: impl Into<String>) -> DraftProblem {
    DraftProblem {
        path: path.into(),
        message: message.into(),
    }
}

/// Every problem of a Task the agent files: a title on one line; references that parse and exist;
/// a technical Task names no Model element and every other Task names at least one.
pub fn validate_draft(image: &ProjectImage, draft: &TaskDraft) -> Vec<DraftProblem> {
    let mut problems = Vec::new();
    if draft.title.trim().is_empty() || draft.title.contains(['\n', '\r']) {
        problems.push(problem("title", "the title is one non-blank line"));
    }
    let resolver = image.resolver();
    let mut elements = 0;
    for (index, text) in draft.refs.iter().enumerate() {
        let path = format!("refs[{index}]");
        match parse_reference(text) {
            Err(message) => problems.push(problem(path, message)),
            Ok(reference) => {
                if let Err(message) = resolver.resolve(&reference) {
                    problems.push(problem(path, message));
                } else if matches!(reference, Reference::Element(_)) {
                    elements += 1;
                }
            }
        }
    }
    if draft.technical && elements > 0 {
        problems.push(problem("technical", "a technical Task names no Model element: remove the Model elements from `refs`, or set `technical` to false"));
    }
    if !draft.technical && elements == 0 {
        problems.push(problem("refs", "a Task that is not technical names at least one Model element: add it to `refs`, or set `technical` to true"));
    }
    problems
}

/// The `refs` of a draft without repeats, in order.
fn distinct(refs: &[String]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    refs.iter()
        .filter(|r| seen.insert(r.as_str()))
        .cloned()
        .collect()
}

fn strings(list: &[String]) -> Value {
    Value::Sequence(list.iter().map(|s| Value::String(s.clone())).collect())
}

/// The text of a Task file in To do, created at the instant `created`.
pub fn render_task(draft: &TaskDraft, created: &str) -> String {
    let mut front = Mapping::new();
    set(&mut front, "column", "todo");
    set(&mut front, "technical", draft.technical);
    set(&mut front, "refs", strings(&distinct(&draft.refs)));
    if let Some(proposal) = &draft.proposal {
        set(&mut front, "proposal", proposal.as_str());
    }
    if let Some(drift) = &draft.drift {
        set(&mut front, "drift", drift.as_str());
    }
    if let Some(job) = &draft.job {
        set(&mut front, "job", job.as_str());
    }
    set(&mut front, "created", created);
    let body = draft.body.trim_matches('\n');
    let text = if body.is_empty() {
        format!("# {}\n", draft.title.trim())
    } else {
        format!("# {}\n\n{body}\n", draft.title.trim())
    };
    frontmatter::render(&front, &KEYS, &text)
}

/// A Task about to be filed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewTask {
    pub id: String,
    /// `tasks/<id>-<slug>.md`, relative to `.weaver/`.
    pub path: String,
    pub changes: ChangeSet,
}

/// Files a Task in To do: draws its identity (unique among the Tasks of `tasks/`, of the copies
/// in History and of every Proposal) and writes its file. The draft must pass
/// [`validate_draft`] when the agent files it; the Tasks of a decision are checked by it.
pub fn plan_new_task(
    image: &ProjectImage,
    draft: &TaskDraft,
    created: &str,
    rng: &mut dyn RandomSource,
) -> NewTask {
    let id = image.new_identity(CardKind::Task, rng);
    let path = format!(
        "{}/{}.md",
        layout::TASKS_DIR,
        card_stem(&id, &slugify(&draft.title))
    );
    let mut changes = ChangeSet::new();
    changes.write(&path, render_task(draft, created));
    NewTask { id, path, changes }
}

/// The file a Task of a Proposal becomes when the Proposal is approved: the same identity and
/// slug in `tasks/`, in To do, with the Proposal's `proposal` key, created at the approval
/// instant. `task` is the card read from `proposals/<id>-<slug>/tasks/`. Returns the path and
/// the content.
pub fn approved_task_file(
    task: &Card,
    proposal_id: &str,
    created: &str,
    job: Option<&str>,
) -> (String, String) {
    let draft = TaskDraft {
        title: task.title.clone(),
        body: body_after_title(task),
        refs: refs_of(task),
        technical: is_technical(task),
        proposal: Some(proposal_id.to_owned()),
        drift: None,
        job: job.map(str::to_owned),
    };
    (
        format!(
            "{}/{}.md",
            layout::TASKS_DIR,
            card_stem(&task.id, &task.slug)
        ),
        render_task(&draft, created),
    )
}

/// The body of the fix Task of a Drift: both sides of the Drift, the option the developer chose,
/// the developer's note and where in the code.
pub fn fix_task_body(
    happens: &str,
    should: &str,
    option: Option<(&str, &str)>,
    note: Option<&str>,
    where_in_code: &str,
) -> String {
    let mut body = format!(
        "## What happens\n\n{}\n\n## What should happen\n\n{}\n",
        happens.trim(),
        should.trim()
    );
    if let Some((title, text)) = option {
        body.push_str(&format!(
            "\n## Chosen option\n\n### {}\n\n{}\n",
            title.trim(),
            text.trim()
        ));
    }
    if let Some(note) = note.filter(|n| !n.trim().is_empty()) {
        body.push_str(&format!("\n## Note of the developer\n\n{}\n", note.trim()));
    }
    body.push_str(&format!(
        "\n## Where in the code\n\n{}\n",
        where_in_code.trim()
    ));
    body
}

// ---------------------------------------------------------------------------------------------
// Retention
// ---------------------------------------------------------------------------------------------

/// Deletes the file of a Task and its images.
fn remove_task(image: &ProjectImage, task: &Card, changes: &mut ChangeSet) {
    changes.delete(&task.path);
    for file in board::task_image_files(image.root(), &task.id) {
        changes.delete(&file);
    }
}

/// What forgetting Done Tasks does.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Forgetting {
    pub changes: ChangeSet,
    pub forgotten: Vec<String>,
}

/// The Tasks that reached Done by `FINISHED`, as `(finished, identity)`: the Task files that were
/// not closed without changes, and the copies in History entries (each Task once).
fn finish_marks(image: &ProjectImage) -> BTreeSet<(String, String)> {
    let mut marks = BTreeSet::new();
    for task in image.cards_of(CardKind::Task) {
        if column_of(task) == Some(Column::Done) && task.str_key("unchanged").is_none() {
            marks.insert((finished_of(task).to_owned(), task.id.clone()));
        }
    }
    for entry in image.cards_of(CardKind::History) {
        if let Some(Value::Mapping(cleared)) = entry.front.get("cleared") {
            for (id, copy) in cleared {
                let (Some(id), Some(copy)) = (id.as_str(), copy.as_mapping()) else {
                    continue;
                };
                if copy.contains_key("unchanged") {
                    continue;
                }
                marks.insert((
                    frontmatter::get_str(copy, "finished")
                        .unwrap_or("")
                        .to_owned(),
                    id.to_owned(),
                ));
            }
        }
    }
    marks
}

/// `task.FINISHED` → Forgotten: every Done Task that no History entry cites counts the Tasks
/// that finished after it (by `finished`, ties by identity, over Task files and copies in
/// History; a Task closed without changes never counts); at [`FORGET_AFTER`] it is deleted with
/// its images, unless an open Drift names it as origin.
pub fn plan_forgetting(image: &ProjectImage) -> Forgetting {
    let marks = finish_marks(image);
    let cited = history::cited_ids(image);
    let origins: BTreeSet<&str> = image
        .cards_of(CardKind::Drift)
        .filter(|d| d.str_key("status") == Some("open"))
        .filter_map(|d| d.str_key("origin"))
        .collect();
    let mut forgetting = Forgetting::default();
    for task in image
        .cards_of(CardKind::Task)
        .filter(|t| column_of(t) == Some(Column::Done))
    {
        if cited.contains(&task.id) || origins.contains(task.id.as_str()) {
            continue;
        }
        let me = (finished_of(task).to_owned(), task.id.clone());
        let after = marks
            .range((std::ops::Bound::Excluded(&me), std::ops::Bound::Unbounded))
            .count();
        if after >= FORGET_AFTER {
            remove_task(image, task, &mut forgetting.changes);
            forgetting.forgotten.push(task.id.clone());
        }
    }
    forgetting
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    use crate::project::model::{ORDER_MACHINE, USER_ACTOR};
    use crate::project::random::ScriptedRandom;
    use crate::project::shortnum::{short_cards, ShortNumbers};
    use crate::project::testing::put;

    fn at(second: u32) -> String {
        format!(
            "2026-11-02T07:00:{:02}.{:03}Z",
            second / 1000,
            second % 1000
        )
    }

    fn project() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        put(root, "model/system.md", "# Shop\n\nA shop.\n");
        put(root, "model/shop/feature.md", "# Shopping\n");
        put(root, "model/shop/order.md", ORDER_MACHINE);
        put(root, "model/actors/user.md", USER_ACTOR);
        dir
    }

    fn task(root: &Path, id: &str, created: u32, column: &str, refs: &[&str], extra: &str) {
        task_with_body(root, id, created, column, refs, extra, "");
    }

    fn task_with_body(
        root: &Path,
        id: &str,
        created: u32,
        column: &str,
        refs: &[&str],
        extra: &str,
        body: &str,
    ) {
        let refs: String = refs.iter().map(|r| format!("  - {r}\n")).collect();
        let refs = if refs.is_empty() {
            "refs: []\n".to_owned()
        } else {
            format!("refs:\n{refs}")
        };
        put(
            root,
            &format!("tasks/{id}-t.md"),
            &format!("---\ncolumn: {column}\ntechnical: false\n{refs}{extra}created: {}\n---\n# Task {id}\n\n{body}\n", at(created)),
        );
    }

    fn drift(root: &Path, id: &str, created: u32, status: &str, refs: &[&str], extra: &str) {
        let refs: String = refs.iter().map(|r| format!("  - {r}\n")).collect();
        put(
            root,
            &format!("drifts/{id}-d.md"),
            &format!("---\nstatus: {status}\nrefs:\n{refs}{extra}created: {}\n---\n# Drift {id}\n\nBody.\n", at(created)),
        );
    }

    fn history(root: &Path, id: &str, created: u32, tasks: &[&str], extra: &str) {
        let list: String = tasks.iter().map(|t| format!("  - {t}\n")).collect();
        put(
            root,
            &format!("history/{id}-h.md"),
            &format!("---\ncreated: {c}\nupdated: {c}\nsummary: Did it\ntasks:\n{list}{extra}---\n# Entry {id}\n", c = at(created)),
        );
    }

    fn image(dir: &tempfile::TempDir) -> ProjectImage {
        ProjectImage::load(dir.path())
    }

    fn numbers(image: &ProjectImage) -> ShortNumbers {
        ShortNumbers::assign(&short_cards(image.cards()), "")
    }

    fn apply(dir: &tempfile::TempDir, image: &ProjectImage, changes: &ChangeSet) -> ProjectImage {
        let _ = image;
        for (rel, change) in changes.iter() {
            let path = layout::absolute(dir.path(), rel);
            match change {
                Change::Write(content) => {
                    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                    std::fs::write(path, crate::project::text::file_content(content)).unwrap();
                }
                Change::Delete => {
                    let _ = std::fs::remove_file(path);
                }
            }
        }
        ProjectImage::load(dir.path())
    }

    fn ids(cards: &[&Card]) -> Vec<String> {
        cards.iter().map(|c| c.id.clone()).collect()
    }

    // --- blocking ---

    #[test]
    fn an_open_drift_blocks_its_origin_and_the_tasks_that_concern_the_same_thing() {
        let dir = project();
        let root = dir.path();
        task(root, "aaaaaaaa", 1, "todo", &["order/Paid"], "");
        task(root, "bbbbbbbb", 2, "todo", &["user"], "");
        task(root, "cccccccc", 3, "todo", &["user"], "");
        task(root, "dddddddd", 4, "todo", &["order"], "");
        drift(
            root,
            "11111111",
            5,
            "open",
            &["order"],
            "origin: cccccccc\n",
        );
        let image = image(&dir);
        let index = BlockingIndex::new(&image);
        let of = |id: &str| index.of(image.card(CardKind::Task, id).unwrap());
        assert_eq!(
            of("aaaaaaaa").drifts,
            ["11111111"],
            "the Drift concerns an element that contains it"
        );
        assert!(!of("bbbbbbbb").is_blocked());
        assert_eq!(
            of("cccccccc").drifts,
            ["11111111"],
            "the origin of an open Drift"
        );
        assert_eq!(of("dddddddd").drifts, ["11111111"]);
    }

    #[test]
    fn a_done_task_is_never_blocked_and_blocking_moves_nothing() {
        let dir = project();
        let root = dir.path();
        task(
            root,
            "aaaaaaaa",
            1,
            "done",
            &["order"],
            "finished: 2026-11-02T08:00:00.000Z\n",
        );
        task(
            root,
            "bbbbbbbb",
            2,
            "doing",
            &["order"],
            "holder: jjjjjjjj\n",
        );
        drift(root, "11111111", 5, "open", &["order"], "");
        let image = image(&dir);
        assert!(!blocking_of(&image, image.card(CardKind::Task, "aaaaaaaa").unwrap()).is_blocked());
        assert!(blocking_of(&image, image.card(CardKind::Task, "bbbbbbbb").unwrap()).is_blocked());
        assert_eq!(
            column_of(image.card(CardKind::Task, "bbbbbbbb").unwrap()),
            Some(Column::Doing)
        );
    }

    #[test]
    fn the_origin_of_a_drift_whose_code_is_to_be_fixed_waits_for_its_fix_task() {
        let dir = project();
        let root = dir.path();
        task(root, "aaaaaaaa", 1, "todo", &["user"], "");
        task(
            root,
            "ffffffff",
            2,
            "todo",
            &["order"],
            "drift: '11111111'\n",
        );
        drift(
            root,
            "11111111",
            3,
            "code-to-be-fixed",
            &["order"],
            "origin: aaaaaaaa\ntask: ffffffff\n",
        );
        let image = image(&dir);
        let blocking = blocking_of(&image, image.card(CardKind::Task, "aaaaaaaa").unwrap());
        assert_eq!(
            blocking,
            Blocking {
                drifts: vec![],
                fix_tasks: vec!["ffffffff".into()]
            }
        );
        assert!(
            !blocking_of(&image, image.card(CardKind::Task, "ffffffff").unwrap()).is_blocked(),
            "the Drift no longer blocks anything else"
        );
        // Once the fix Task is Done the origin is free.
        let done = apply(&dir, &image, &plan_finish_for_test(&image, "ffffffff"));
        assert!(!blocking_of(&done, done.card(CardKind::Task, "aaaaaaaa").unwrap()).is_blocked());
    }

    /// Marks a Task Done directly, as the files would be after a finish.
    fn plan_finish_for_test(image: &ProjectImage, id: &str) -> ChangeSet {
        let task = image.card(CardKind::Task, id).unwrap();
        let mut changes = ChangeSet::new();
        changes.write(
            &task.path,
            rewritten(task, &KEYS, |f| {
                set(f, "column", "done");
                set(f, "finished", "2026-11-02T08:00:00.000Z");
            }),
        );
        changes
    }

    // --- take, release, finish ---

    #[test]
    fn taking_a_blocked_task_is_refused_naming_the_drift_and_nothing_is_written() {
        let dir = project();
        let root = dir.path();
        task(root, "aaaaaaaa", 1, "todo", &["order"], "");
        drift(root, "11111111", 2, "open", &["order/Paid"], "");
        let image = image(&dir);
        let Err(Refusal(message)) = plan_take(&image, &numbers(&image), "aaaaaaaa", "jjjjjjjj")
        else {
            panic!("refused")
        };
        assert!(message.contains("T1"), "{message}");
        assert!(message.contains("Drift D1 \"Drift 11111111\""), "{message}");
    }

    #[test]
    fn taking_a_task_waiting_for_its_fix_task_names_the_fix_task() {
        let dir = project();
        let root = dir.path();
        task(root, "aaaaaaaa", 1, "todo", &["user"], "");
        task(
            root,
            "ffffffff",
            2,
            "todo",
            &["order"],
            "drift: '11111111'\n",
        );
        drift(
            root,
            "11111111",
            3,
            "code-to-be-fixed",
            &["order"],
            "origin: aaaaaaaa\ntask: ffffffff\n",
        );
        let image = image(&dir);
        let Err(Refusal(message)) = plan_take(&image, &numbers(&image), "aaaaaaaa", "jjjjjjjj")
        else {
            panic!("refused")
        };
        assert!(
            message.contains("waiting for the fix Task T2 \"Task ffffffff\""),
            "{message}"
        );
    }

    #[test]
    fn a_free_task_is_taken_and_its_holder_is_the_job() {
        let dir = project();
        let root = dir.path();
        task(root, "aaaaaaaa", 1, "todo", &["order"], "job: kkkkkkkk\n");
        let image = image(&dir);
        let changes = plan_take(&image, &numbers(&image), "aaaaaaaa", "jjjjjjjj").unwrap();
        let after = image.with_changes(&changes);
        let task = after.card(CardKind::Task, "aaaaaaaa").unwrap();
        assert_eq!(task.str_key("column"), Some("doing"));
        assert_eq!(task.str_key("holder"), Some("jjjjjjjj"));
        assert_eq!(task.str_key("job"), Some("kkkkkkkk"), "other keys are kept");
        // A Task already taken cannot be taken again.
        assert!(plan_take(&after, &numbers(&after), "aaaaaaaa", "other000").is_err());
    }

    #[test]
    fn only_the_holder_releases_or_finishes() {
        let dir = project();
        let root = dir.path();
        task(
            root,
            "aaaaaaaa",
            1,
            "doing",
            &["order"],
            "holder: jjjjjjjj\n",
        );
        let image = image(&dir);
        let numbers = numbers(&image);
        assert!(plan_release(&image, &numbers, "aaaaaaaa", "other000").is_err());
        assert!(plan_finish(&image, &numbers, "aaaaaaaa", "other000", &at(9), None).is_err());
        let released = plan_release(&image, &numbers, "aaaaaaaa", "jjjjjjjj").unwrap();
        let after = image.with_changes(&released);
        let task = after.card(CardKind::Task, "aaaaaaaa").unwrap();
        assert_eq!(
            (task.str_key("column"), task.str_key("holder")),
            (Some("todo"), None)
        );
        // A Task in To do is not released.
        assert!(plan_release(&after, &numbers, "aaaaaaaa", "jjjjjjjj").is_err());
    }

    #[test]
    fn finishing_writes_done_and_finished_and_drops_the_holder() {
        let dir = project();
        let root = dir.path();
        task(
            root,
            "aaaaaaaa",
            1,
            "doing",
            &["order"],
            "holder: jjjjjjjj\n",
        );
        let image = image(&dir);
        let plan = plan_finish(
            &image,
            &numbers(&image),
            "aaaaaaaa",
            "jjjjjjjj",
            "2026-11-02T09:00:00.000Z",
            None,
        )
        .unwrap();
        assert_eq!(plan.outcome, Outcome::Finished);
        let task = image.with_changes(&plan.changes);
        let task = task.card(CardKind::Task, "aaaaaaaa").unwrap();
        assert_eq!(task.str_key("column"), Some("done"));
        assert_eq!(task.str_key("finished"), Some("2026-11-02T09:00:00.000Z"));
        assert_eq!(task.str_key("holder"), None);
        assert_eq!(task.str_key("unchanged"), None);
    }

    #[test]
    fn a_fix_task_finished_fixes_its_drift_and_one_closed_unchanged_dismisses_it() {
        for (unchanged, status) in [
            (None, "fixed"),
            (Some("The code already does it"), "dismissed"),
        ] {
            let dir = project();
            let root = dir.path();
            task(root, "aaaaaaaa", 1, "todo", &["user"], "");
            task(
                root,
                "ffffffff",
                2,
                "doing",
                &["order"],
                "drift: '11111111'\nholder: jjjjjjjj\n",
            );
            drift(
                root,
                "11111111",
                3,
                "code-to-be-fixed",
                &["order"],
                "origin: aaaaaaaa\ntask: ffffffff\n",
            );
            let image = image(&dir);
            let plan = plan_finish(
                &image,
                &numbers(&image),
                "ffffffff",
                "jjjjjjjj",
                &at(10),
                unchanged,
            )
            .unwrap();
            assert_eq!(plan.drift.as_deref(), Some("11111111"));
            let after = image.with_changes(&plan.changes);
            assert_eq!(
                after
                    .card(CardKind::Drift, "11111111")
                    .unwrap()
                    .str_key("status"),
                Some(status)
            );
            assert_eq!(
                after
                    .card(CardKind::Task, "ffffffff")
                    .unwrap()
                    .str_key("unchanged"),
                unchanged
            );
            assert_eq!(
                plan.outcome,
                if unchanged.is_some() {
                    Outcome::ClosedUnchanged
                } else {
                    Outcome::Finished
                }
            );
        }
    }

    #[test]
    fn closing_unchanged_needs_a_fix_task_and_a_note() {
        let dir = project();
        let root = dir.path();
        task(
            root,
            "aaaaaaaa",
            1,
            "doing",
            &["order"],
            "holder: jjjjjjjj\n",
        );
        task(
            root,
            "ffffffff",
            2,
            "doing",
            &["order"],
            "drift: '11111111'\nholder: jjjjjjjj\n",
        );
        let image = image(&dir);
        let numbers = numbers(&image);
        assert!(
            plan_finish(
                &image,
                &numbers,
                "aaaaaaaa",
                "jjjjjjjj",
                &at(10),
                Some("checked")
            )
            .is_err(),
            "not a fix Task"
        );
        assert!(
            plan_finish(
                &image,
                &numbers,
                "ffffffff",
                "jjjjjjjj",
                &at(10),
                Some("  ")
            )
            .is_err(),
            "blank note"
        );
        assert!(plan_finish(
            &image,
            &numbers,
            "ffffffff",
            "jjjjjjjj",
            &at(10),
            Some("checked")
        )
        .is_ok());
    }

    #[test]
    fn when_a_job_ends_the_tasks_it_held_go_back_to_todo() {
        let dir = project();
        let root = dir.path();
        task(
            root,
            "aaaaaaaa",
            1,
            "doing",
            &["order"],
            "holder: jjjjjjjj\n",
        );
        task(
            root,
            "bbbbbbbb",
            2,
            "doing",
            &["user"],
            "holder: kkkkkkkk\n",
        );
        task(
            root,
            "cccccccc",
            3,
            "done",
            &["user"],
            "holder: jjjjjjjj\nfinished: 2026-11-02T08:00:00.000Z\n",
        );
        let image = image(&dir);
        let after = image.with_changes(&plan_job_ended(&image, "jjjjjjjj"));
        let column = |id: &str| {
            after
                .card(CardKind::Task, id)
                .unwrap()
                .str_key("column")
                .map(str::to_owned)
        };
        assert_eq!(column("aaaaaaaa").as_deref(), Some("todo"));
        assert_eq!(
            after
                .card(CardKind::Task, "aaaaaaaa")
                .unwrap()
                .str_key("holder"),
            None
        );
        assert_eq!(column("bbbbbbbb").as_deref(), Some("doing"));
        assert_eq!(
            column("cccccccc").as_deref(),
            Some("done"),
            "a Done Task stays Done"
        );
    }

    #[test]
    fn at_opening_an_unknown_holder_or_no_holder_counts_as_a_job_that_ended() {
        let dir = project();
        let root = dir.path();
        task(
            root,
            "aaaaaaaa",
            1,
            "doing",
            &["order"],
            "holder: known000\n",
        );
        task(
            root,
            "bbbbbbbb",
            2,
            "doing",
            &["user"],
            "holder: gone0000\n",
        );
        task(root, "cccccccc", 3, "doing", &["user"], "");
        task(root, "dddddddd", 4, "todo", &["user"], "");
        let image = image(&dir);
        let after = image.with_changes(&plan_orphans(&image, &|job| job == "known000"));
        let column = |id: &str| {
            after
                .card(CardKind::Task, id)
                .unwrap()
                .str_key("column")
                .map(str::to_owned)
        };
        assert_eq!(column("aaaaaaaa").as_deref(), Some("doing"));
        assert_eq!(column("bbbbbbbb").as_deref(), Some("todo"));
        assert_eq!(column("cccccccc").as_deref(), Some("todo"));
    }

    #[test]
    fn the_origin_task_goes_back_to_todo_when_its_drift_code_is_to_be_fixed() {
        let dir = project();
        let root = dir.path();
        task(
            root,
            "aaaaaaaa",
            1,
            "doing",
            &["order"],
            "holder: jjjjjjjj\n",
        );
        let image = image(&dir);
        let after = image.with_changes(&plan_origin_waits(&image, "aaaaaaaa"));
        assert_eq!(
            after
                .card(CardKind::Task, "aaaaaaaa")
                .unwrap()
                .str_key("column"),
            Some("todo")
        );
        assert!(plan_origin_waits(&image, "zzzzzzzz").is_empty());
    }

    // --- runnable, slots, groups ---

    #[test]
    fn the_next_runnable_task_is_the_oldest_one_that_is_not_blocked() {
        let dir = project();
        let root = dir.path();
        task(root, "aaaaaaaa", 1, "todo", &["order"], "");
        task(root, "bbbbbbbb", 2, "todo", &["user"], "");
        task(
            root,
            "cccccccc",
            3,
            "doing",
            &["user"],
            "holder: jjjjjjjj\n",
        );
        drift(root, "11111111", 4, "open", &["order"], "");
        let image = image(&dir);
        assert_eq!(ids(&runnable(&image)), ["bbbbbbbb"]);
        assert_eq!(next_runnable(&image).unwrap().id, "bbbbbbbb");
    }

    #[test]
    fn slots_pick_the_oldest_tasks_that_do_not_concern_the_same_thing() {
        let dir = project();
        let root = dir.path();
        task(root, "aaaaaaaa", 1, "todo", &["order/Paid"], "");
        task(root, "bbbbbbbb", 2, "todo", &["order"], "");
        task(root, "cccccccc", 3, "todo", &["user"], "");
        task(root, "dddddddd", 4, "todo", &["shop"], "");
        let image = image(&dir);
        let slots = pick_slots(&image, 2, &|_| true);
        assert_eq!(ids(&slots.picked), ["aaaaaaaa", "cccccccc"]);
        assert_eq!(
            slots.held_back[0],
            (
                image.card(CardKind::Task, "bbbbbbbb").unwrap(),
                HeldBack::SameAsPicked("aaaaaaaa".into())
            )
        );
        // `shop` (the Feature) contains the Machine of the first pick.
        assert_eq!(
            slots.held_back[1].1,
            HeldBack::SameAsPicked("aaaaaaaa".into())
        );
        let one = pick_slots(&image, 1, &|_| true);
        assert_eq!(ids(&one.picked), ["aaaaaaaa"]);
        assert!(one
            .held_back
            .iter()
            .any(|(t, why)| t.id == "cccccccc" && *why == HeldBack::NoPlace));
    }

    #[test]
    fn slots_skip_what_concerns_a_task_in_progress() {
        let dir = project();
        let root = dir.path();
        task(
            root,
            "aaaaaaaa",
            1,
            "doing",
            &["order/Paid"],
            "holder: jjjjjjjj\n",
        );
        task(root, "bbbbbbbb", 2, "todo", &["order"], "");
        task(root, "cccccccc", 3, "todo", &["user"], "");
        let image = image(&dir);
        let slots = pick_slots(&image, 3, &|_| true);
        assert_eq!(ids(&slots.picked), ["cccccccc"]);
        assert_eq!(
            slots.held_back[0].1,
            HeldBack::SameAsInProgress("aaaaaaaa".into())
        );
    }

    #[test]
    fn groups_link_tasks_transitively_by_the_same_thing_or_the_same_backticked_path() {
        let dir = project();
        let root = dir.path();
        task_with_body(
            root,
            "aaaaaaaa",
            1,
            "todo",
            &["order/Paid"],
            "",
            "Edit `src/shop.rs`.",
        );
        task(root, "bbbbbbbb", 2, "todo", &["user"], "");
        task_with_body(
            root,
            "cccccccc",
            3,
            "todo",
            &["user"],
            "",
            "Also `./src/shop.rs` and `order/Paid`.",
        );
        task(root, "dddddddd", 4, "todo", &["order"], "");
        task(root, "eeeeeeee", 5, "todo", &["user.CANCEL"], "");
        let image = image(&dir);
        let all = groups(&image);
        // a–c share a path, c–b concern `user`, a–d concern `order`; e concerns `user` too.
        assert_eq!(all.len(), 1);
        assert_eq!(
            ids(&all[0]),
            ["aaaaaaaa", "bbbbbbbb", "cccccccc", "dddddddd", "eeeeeeee"]
        );
        // Without the shared path the groups are separate.
        task(root, "cccccccc", 3, "todo", &["user"], "");
        task(root, "aaaaaaaa", 1, "todo", &["order/Paid"], "");
        let image = ProjectImage::load(dir.path());
        let all = groups(&image);
        assert_eq!(
            all.iter().map(|g| ids(g)).collect::<Vec<_>>(),
            [
                vec!["aaaaaaaa", "dddddddd"],
                vec!["bbbbbbbb", "cccccccc", "eeeeeeee"]
            ]
        );
    }

    #[test]
    fn backticked_paths_are_files_not_references() {
        let paths = backticked_paths("`src/a.rs`, `Cargo.toml`, `./lib/`, `order/Paid`, `user.CANCEL`, `two words.rs`, `order`, `a.b.c`");
        assert_eq!(
            paths.into_iter().collect::<Vec<_>>(),
            ["Cargo.toml", "a.b.c", "lib/", "src/a.rs"]
        );
    }

    #[test]
    fn with_groups_each_pick_brings_up_to_two_more_of_its_group() {
        let dir = project();
        let root = dir.path();
        for (index, id) in ["aaaaaaaa", "bbbbbbbb", "cccccccc", "dddddddd"]
            .iter()
            .enumerate()
        {
            task_with_body(
                root,
                id,
                index as u32 + 1,
                "todo",
                &["order"],
                "",
                "Edit `src/shop.rs`.",
            );
        }
        task(root, "eeeeeeee", 5, "todo", &["user"], "");
        let image = image(&dir);
        let (groups, _) = pick_groups(&image, 2, &|_| true);
        assert_eq!(groups.len(), 2);
        assert_eq!(
            ids(&groups[0]),
            ["aaaaaaaa", "bbbbbbbb", "cccccccc"],
            "at most 3 Tasks of a group per sub-agent"
        );
        assert_eq!(ids(&groups[1]), ["eeeeeeee"]);
    }

    // --- Run all ---

    #[test]
    fn run_all_groups_the_runnable_tasks_by_effective_model() {
        let dir = project();
        let root = dir.path();
        put(
            root,
            "proposals/pppppppp-p/proposal.md",
            &format!("---\nstatus: approved\ncreated: {}\n---\n# P\n", at(1)),
        );
        task(
            root,
            "aaaaaaaa",
            2,
            "todo",
            &["order"],
            "proposal: pppppppp\n",
        );
        task(root, "bbbbbbbb", 3, "todo", &["user"], "");
        task(
            root,
            "cccccccc",
            4,
            "todo",
            &["user.CANCEL"],
            "proposal: pppppppp\n",
        );
        task(
            root,
            "dddddddd",
            5,
            "todo",
            &["shop"],
            "proposal: qqqqqqqq\n",
        );
        let image = image(&dir);
        let chosen = |proposal: &str| (proposal == "pppppppp").then(|| " claude-opus ".to_owned());
        let groups = run_all_groups(&image, &chosen, "claude-sonnet");
        let summary: Vec<(String, Vec<String>)> = groups
            .iter()
            .map(|g| (g.model.clone(), ids(&g.tasks)))
            .collect();
        assert_eq!(
            summary,
            [
                (
                    "claude-opus".to_owned(),
                    vec!["aaaaaaaa".to_owned(), "cccccccc".to_owned()]
                ),
                (
                    "claude-sonnet".to_owned(),
                    vec!["bbbbbbbb".to_owned(), "dddddddd".to_owned()]
                ),
            ]
        );
    }

    #[test]
    fn the_sub_agent_limit_is_shared_in_proportion_with_one_at_least() {
        assert_eq!(split_sub_agents(4, &[3, 1]), [3, 1]);
        assert_eq!(
            split_sub_agents(5, &[3, 1]),
            [4, 1],
            "the rest goes to the largest fractional part: 3.75 and 1.25"
        );
        assert_eq!(
            split_sub_agents(6, &[1, 1, 1, 1]),
            [2, 2, 1, 1],
            "equal parts: the group with the oldest Task first"
        );
        assert_eq!(
            split_sub_agents(2, &[10, 1, 1]),
            [1, 1, 1],
            "at least one each, even over the limit"
        );
        assert_eq!(split_sub_agents(10, &[7]), [10]);
        assert_eq!(split_sub_agents(8, &[5, 3, 2]), [4, 2, 2]);
        assert_eq!(split_sub_agents(3, &[]), Vec::<usize>::new());
    }

    // --- Run reasons ---

    #[test]
    fn run_lists_every_reason_in_its_fixed_order() {
        let dir = project();
        let root = dir.path();
        task(root, "aaaaaaaa", 1, "todo", &["order"], "job: jjjjjjjj\n");
        task(root, "bbbbbbbb", 2, "todo", &["user"], "");
        drift(root, "11111111", 3, "open", &["order"], "");
        drift(
            root,
            "22222222",
            4,
            "code-to-be-fixed",
            &["order"],
            "origin: aaaaaaaa\ntask: bbbbbbbb\n",
        );
        let image = image(&dir);
        let facts = AgentFacts {
            resolving_drifts: BTreeSet::new(),
            run_in_progress: true,
            resolve_working: true,
            working_jobs: BTreeSet::from(["jjjjjjjj".to_owned()]),
            done_since_last_scan: 0,
            merge_in_progress: true,
            ..AgentFacts::default()
        };
        let task = image.card(CardKind::Task, "aaaaaaaa").unwrap();
        let blocking = blocking_of(&image, task);
        let name = |kind: CardKind, id: &str| {
            numbers(&image)
                .label_of(kind, id)
                .unwrap_or_else(|| id.to_owned())
        };
        assert_eq!(
            run_reasons(&image, &facts, task, &blocking, &name),
            [
                "The agent is still finishing",
                "A merge is in progress",
                "Drifts are being resolved",
                "A run is in progress",
                "Blocked by Drift D1",
                "Waiting for Task T2",
            ]
        );
        let free = image.card(CardKind::Task, "bbbbbbbb").unwrap();
        assert!(run_reasons(
            &image,
            &AgentFacts::default(),
            free,
            &blocking_of(&image, free),
            &name
        )
        .is_empty());
    }

    #[test]
    fn run_is_disabled_while_the_agent_works_on_the_task_but_not_for_another_one() {
        let dir = project();
        let root = dir.path();
        task(
            root,
            "aaaaaaaa",
            1,
            "doing",
            &["order"],
            "holder: jjjjjjjj\n",
        );
        task(root, "bbbbbbbb", 2, "todo", &["user"], "");
        let image = image(&dir);
        let facts = AgentFacts {
            working_jobs: BTreeSet::from(["jjjjjjjj".to_owned()]),
            ..AgentFacts::default()
        };
        let name = |_: CardKind, id: &str| id.to_owned();
        let held = image.card(CardKind::Task, "aaaaaaaa").unwrap();
        assert_eq!(
            run_reasons(&image, &facts, held, &Blocking::default(), &name),
            ["The agent is working on it"]
        );
        let other = image.card(CardKind::Task, "bbbbbbbb").unwrap();
        assert!(run_reasons(&image, &facts, other, &Blocking::default(), &name).is_empty());
    }

    // --- creation ---

    fn draft(title: &str, refs: &[&str], technical: bool) -> TaskDraft {
        TaskDraft {
            title: title.into(),
            body: "It is done when it works.".into(),
            refs: refs.iter().map(|r| (*r).into()).collect(),
            technical,
            ..TaskDraft::default()
        }
    }

    #[test]
    fn a_task_names_a_model_element_unless_it_is_technical() {
        let dir = project();
        let image = image(&dir);
        assert!(
            validate_draft(&image, &draft("Cancel an order", &["order/Paid"], false)).is_empty()
        );
        assert!(validate_draft(
            &image,
            &draft(
                "Cancel an order",
                &["order", "requirements/ui", "specs"],
                false
            )
        )
        .iter()
        .any(|p| p.path == "refs[1]" || p.path == "refs[2]"));
        let none = validate_draft(&image, &draft("Cancel", &[], false));
        assert_eq!(
            none.iter().map(|p| p.path.as_str()).collect::<Vec<_>>(),
            ["refs"]
        );
        let technical = validate_draft(&image, &draft("Speed up", &["order"], true));
        assert_eq!(
            technical
                .iter()
                .map(|p| p.path.as_str())
                .collect::<Vec<_>>(),
            ["technical"]
        );
        assert!(validate_draft(&image, &draft("Speed up", &[], true)).is_empty());
        let bad = validate_draft(
            &image,
            &draft("Cancel", &["order", "nowhere", "bad_ref"], false),
        );
        assert_eq!(
            bad.iter().map(|p| p.path.as_str()).collect::<Vec<_>>(),
            ["refs[1]", "refs[2]"]
        );
        assert_eq!(
            validate_draft(&image, &draft("two\nlines", &["order"], false))[0].path,
            "title"
        );
    }

    #[test]
    fn a_new_task_is_a_file_in_todo_with_a_fresh_identity() {
        let dir = project();
        let root = dir.path();
        task(
            root,
            "aaaaaaaa",
            1,
            "done",
            &["order"],
            "finished: 2026-11-02T08:00:00.000Z\n",
        );
        let image = image(&dir);
        // The first identity the source draws is the used one: it is drawn again.
        let mut rng = ScriptedRandom::new([vec![0u8; 8], (8u8..16).collect()].concat());
        let mut full = draft("Cancel an order", &["order/Paid"], false);
        full.job = Some("jjjjjjjj".into());
        let new = plan_new_task(&image, &full, "2026-11-02T10:00:00.000Z", &mut rng);
        assert!(
            new.path
                .starts_with(&format!("tasks/{}-cancel-an-order", new.id)),
            "{}",
            new.path
        );
        let after = image.with_changes(&new.changes);
        let created = after.card(CardKind::Task, &new.id).unwrap();
        assert_eq!(created.title, "Cancel an order");
        assert_eq!(created.str_key("column"), Some("todo"));
        assert_eq!(created.bool_key("technical"), Some(false));
        assert_eq!(
            created.list_key("refs"),
            Some(vec!["order/Paid".to_owned()])
        );
        assert_eq!(created.str_key("job"), Some("jjjjjjjj"));
        assert_eq!(created.created.as_deref(), Some("2026-11-02T10:00:00.000Z"));
        assert_eq!(body_after_title(created), "It is done when it works.");
        let text = render_task(&full, "2026-11-02T10:00:00.000Z");
        assert!(
            text.starts_with(
                "---\ncolumn: todo\ntechnical: false\nrefs:\n- order/Paid\njob: jjjjjjjj\ncreated:"
            ),
            "{text}"
        );
    }

    #[test]
    fn an_approved_proposal_task_keeps_its_identity_and_belongs_to_the_design() {
        let dir = project();
        let root = dir.path();
        put(
            root,
            "proposals/pppppppp-p/proposal.md",
            &format!("---\nstatus: open\ncreated: {}\n---\n# P\n", at(1)),
        );
        put(
            root,
            "proposals/pppppppp-p/tasks/tttttttt-cancel.md",
            "---\ntechnical: false\nrefs:\n  - order/Paid\n---\n# Cancel\n\nDone when.\n",
        );
        let image = image(&dir);
        let card = image.cards_of(CardKind::ProposalTask).next().unwrap();
        let (path, content) =
            approved_task_file(card, "pppppppp", "2026-11-02T11:00:00.000Z", None);
        assert_eq!(path, "tasks/tttttttt-cancel.md");
        let mut changes = ChangeSet::new();
        changes.write(&path, content);
        let after = image.with_changes(&changes);
        let created = after.card(CardKind::Task, "tttttttt").unwrap();
        assert_eq!(created.str_key("proposal"), Some("pppppppp"));
        assert_eq!(created.str_key("column"), Some("todo"));
        assert_eq!(created.created.as_deref(), Some("2026-11-02T11:00:00.000Z"));
    }

    #[test]
    fn a_fix_task_body_carries_both_sides_the_option_the_note_and_the_place() {
        let body = fix_task_body(
            "It cancels twice.",
            "It cancels once.",
            Some(("Guard it", "Check the state first.")),
            Some("Be careful."),
            "`src/order.rs`",
        );
        assert_eq!(
            body,
            "## What happens\n\nIt cancels twice.\n\n## What should happen\n\nIt cancels once.\n\n## Chosen option\n\n### Guard it\n\nCheck the state first.\n\n## Note of the developer\n\nBe careful.\n\n## Where in the code\n\n`src/order.rs`\n"
        );
        assert!(!fix_task_body("a", "b", None, None, "c").contains("Chosen option"));
    }

    // --- retention ---

    #[test]
    fn a_done_task_is_forgotten_when_20_others_finished_after_it() {
        let dir = project();
        let root = dir.path();
        // `aaaaaaaa` finished first; 20 Tasks finish after it, 19 after `bbbbbbbb`.
        for n in 0..21u32 {
            let id = format!("{:0>8}", format!("t{n}"));
            task(
                root,
                &id,
                n + 1,
                "done",
                &["order"],
                &format!("finished: {}\n", at(100 + n)),
            );
        }
        let image = image(&dir);
        let forgetting = plan_forgetting(&image);
        assert_eq!(
            forgetting.forgotten,
            ["000000t0"],
            "only the first has 20 after it"
        );
        let after = image.with_changes(&forgetting.changes);
        assert!(after.card(CardKind::Task, "000000t0").is_none());
        assert!(after.card(CardKind::Task, "000000t1").is_some());
        // Ties in `finished` go by identity.
        let dir = project();
        let root = dir.path();
        for n in 0..21u32 {
            let id = format!("{:0>8}", format!("t{n}"));
            task(
                root,
                &id,
                n + 1,
                "done",
                &["order"],
                &format!("finished: {}\n", at(100)),
            );
        }
        let image = ProjectImage::load(root);
        assert_eq!(plan_forgetting(&image).forgotten, ["000000t0"]);
    }

    #[test]
    fn closed_unchanged_tasks_never_count_and_open_drift_origins_stay() {
        let dir = project();
        let root = dir.path();
        task(
            root,
            "aaaaaaaa",
            1,
            "done",
            &["order"],
            &format!("finished: {}\n", at(100)),
        );
        task(
            root,
            "bbbbbbbb",
            2,
            "done",
            &["order"],
            &format!("finished: {}\n", at(101)),
        );
        for n in 0..20u32 {
            let id = format!("{:0>8}", format!("u{n}"));
            task(
                root,
                &id,
                n + 3,
                "done",
                &["order"],
                &format!("finished: {}\nunchanged: checked\n", at(200 + n)),
            );
        }
        let image = image(&dir);
        assert!(
            plan_forgetting(&image).forgotten.is_empty(),
            "20 closed-unchanged Tasks count for nothing"
        );
        // 20 real ones after `aaaaaaaa`, but an open Drift names it as origin.
        for n in 0..20u32 {
            let id = format!("{:0>8}", format!("v{n}"));
            task(
                root,
                &id,
                n + 30,
                "done",
                &["order"],
                &format!("finished: {}\n", at(300 + n)),
            );
        }
        drift(
            root,
            "11111111",
            60,
            "open",
            &["order"],
            "origin: aaaaaaaa\n",
        );
        let image = ProjectImage::load(root);
        let forgotten = plan_forgetting(&image).forgotten;
        assert!(forgotten.contains(&"bbbbbbbb".to_owned()));
        assert!(
            !forgotten.contains(&"aaaaaaaa".to_owned()),
            "an open Drift names it as origin"
        );
    }

    #[test]
    fn copies_in_history_count_for_forgetting_and_cited_tasks_are_never_forgotten() {
        let dir = project();
        let root = dir.path();
        task(
            root,
            "aaaaaaaa",
            1,
            "done",
            &["order"],
            &format!("finished: {}\n", at(100)),
        );
        task(
            root,
            "bbbbbbbb",
            2,
            "done",
            &["order"],
            &format!("finished: {}\n", at(101)),
        );
        let copies: String = (0..20u32)
            .map(|n| format!("  c{n:07}:\n    title: Copy\n    refs: []\n    technical: true\n    finished: {}\n    body: x\n", at(200 + n)))
            .collect();
        history(
            root,
            "hhhhhhhh",
            300,
            &["bbbbbbbb"],
            &format!("cleared:\n{copies}"),
        );
        let image = image(&dir);
        let forgetting = plan_forgetting(&image);
        assert_eq!(
            forgetting.forgotten,
            ["aaaaaaaa"],
            "`bbbbbbbb` is cited by an entry"
        );
    }

    #[test]
    fn a_task_cited_by_history_leaves_tasks_and_stays_readable_in_the_entry() {
        let dir = project();
        let root = dir.path();
        put(
            root,
            "proposals/pppppppp-p/proposal.md",
            &format!(
                "---\nstatus: approved\nlabel: Cancel orders\ncreated: {}\n---\n# P\n",
                at(1)
            ),
        );
        task_with_body(
            root,
            "aaaaaaaa",
            2,
            "done",
            &["order/Paid"],
            &format!("proposal: pppppppp\nfinished: {}\n", at(100)),
            "Done when it cancels.",
        );
        task(
            root,
            "bbbbbbbb",
            3,
            "done",
            &["user"],
            &format!("finished: {}\n", at(101)),
        );
        put(root, "task-images/aaaaaaaa/a.png", "png");
        history(root, "hhhhhhhh", 300, &["aaaaaaaa"], "");
        history(root, "gggggggg", 301, &["aaaaaaaa", "bbbbbbbb"], "");
        let image = image(&dir);
        let plan = history::plan_clear(&image);
        assert_eq!(plan.recorded, ["aaaaaaaa", "bbbbbbbb"]);
        let after = apply(&dir, &image, &plan.changes);
        assert!(
            after.card(CardKind::Task, "aaaaaaaa").is_none(),
            "the Task leaves `.weaver/tasks/`"
        );
        assert!(after.card(CardKind::Task, "bbbbbbbb").is_none());
        assert!(
            !layout::absolute(root, "task-images/aaaaaaaa/a.png").exists(),
            "the images go with the Task"
        );
        for entry_id in ["hhhhhhhh", "gggggggg"] {
            let entry =
                history::entry_of(after.card(CardKind::History, entry_id).unwrap()).unwrap();
            let copy = &entry.cleared["aaaaaaaa"];
            assert_eq!(copy.title, "Task aaaaaaaa");
            assert_eq!(copy.body, "Done when it cancels.");
            assert_eq!(copy.label.as_deref(), Some("Cancel orders"));
            assert_eq!(copy.proposal.as_deref(), Some("pppppppp"));
            assert_eq!(copy.finished, at(100));
            assert!(!copy.technical);
        }
        assert_eq!(
            after.used_ids(CardKind::Task).len(),
            2,
            "the identities stay in use through the copies"
        );
        // A Task that is recorded is never forgotten, and the copies count for the others.
        assert!(plan_forgetting(&after).forgotten.is_empty());
    }

    #[test]
    fn finishing_forgets_what_20_others_finished_after() {
        let dir = project();
        let root = dir.path();
        for n in 0..20u32 {
            let id = format!("{:0>8}", format!("t{n}"));
            task(
                root,
                &id,
                n + 1,
                "done",
                &["order"],
                &format!("finished: {}\n", at(100 + n)),
            );
        }
        task(
            root,
            "zzzzzzzz",
            50,
            "doing",
            &["user"],
            "holder: jjjjjjjj\n",
        );
        let image = image(&dir);
        let plan = plan_finish(
            &image,
            &numbers(&image),
            "zzzzzzzz",
            "jjjjjjjj",
            &at(500),
            None,
        )
        .unwrap();
        assert_eq!(plan.forgotten, ["000000t0"]);
        let after = image.with_changes(&plan.changes);
        assert!(after.card(CardKind::Task, "000000t0").is_none());
        assert!(after.card(CardKind::Task, "zzzzzzzz").is_some());
        // Closing a fix Task without changes forgets nothing.
        task(
            root,
            "ffffffff",
            51,
            "doing",
            &["order"],
            "drift: '11111111'\nholder: jjjjjjjj\n",
        );
        let image = ProjectImage::load(root);
        let plan = plan_finish(
            &image,
            &numbers(&image),
            "ffffffff",
            "jjjjjjjj",
            &at(501),
            Some("checked"),
        )
        .unwrap();
        assert!(plan.forgotten.is_empty());
    }
}
