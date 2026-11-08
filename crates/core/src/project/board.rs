//! The Board of a project: what it shows, worked out from the image and a few facts only the app
//! knows (which jobs work, whether a Run works).
//!
//! * `Backlog`: open Proposals and open Drifts in one creation order, oldest first. A resolution
//!   gathers the Drifts it lists into one card; a Proposal attached to a Drift is never shown on
//!   its own; a Draft is shown only when a refinement left it half-way.
//! * `Active`: Tasks in To do and In progress, blocked ones included, oldest first.
//! * Blocking is never stored: an open Drift that names a Task as its origin or concerns the
//!   same thing blocks it ([`concerns`]); the origin of a Drift whose code is to be fixed waits
//!   for the fix Task.
//! * `Run all` is disabled for the reasons listed, in the fixed order of the control.
//! * Delete is a decision of the developer: [`delete_task`] checks its conditions under the
//!   project lock, when it is applied, and either removes the Task whole or changes nothing.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use super::cards::{Card, CardKind};
use super::changes::{self, ChangeSet, MutationError};
use super::image::ProjectImage;
use super::layout;
use super::order::compare;
use super::shortnum::{self, ShortNumbers};
use super::tasks::{Blocking, BlockingIndex};

/// `Run all` is disabled while a resolve works or a resolution is being written.
pub const REASON_DRIFTS_RESOLVING: &str = "Drifts are being resolved";
pub const REASON_RUN_IN_PROGRESS: &str = "A run is in progress";
/// A merge of Solo mode is in progress (`merge/merge.json` exists): Runs, Run all, resolves, scans
/// and imports are refused.
pub const REASON_MERGE_IN_PROGRESS: &str = "A merge is in progress";
pub const REASON_NO_TODO: &str = "No Task is in To do";
pub const REASON_ALL_BLOCKED: &str = "All the Tasks in To do are blocked";
/// First reason on every control of a card whose job still works.
pub const REASON_AGENT_FINISHING: &str = "The agent is still finishing";

/// What only the running app knows about the agent's work in one project.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AgentFacts {
    /// A Run works (Run all counts as one Run).
    pub run_in_progress: bool,
    /// A resolve job works.
    pub resolve_working: bool,
    /// Identities of the Drifts the working resolve jobs resolve.
    pub resolving_drifts: BTreeSet<String>,
    /// Identities of the jobs that are working.
    pub working_jobs: BTreeSet<String>,
    /// Tasks that reached Done by `FINISHED` since the last Scan System job started, the deleted
    /// and forgotten ones included.
    pub done_since_last_scan: u32,
    /// A merge of Solo mode is in progress (`.weaver/merge/merge.json` exists).
    pub merge_in_progress: bool,
    /// Some file of the merge in progress still has conflicts. Git tells it, so the caller that
    /// has just asked git sets it; the registry never does.
    pub merge_conflicts: bool,
    /// A `merge` job of the project has not ended.
    pub merge_job_unended: bool,
}

impl AgentFacts {
    /// Whether a working job still works on `card`: it created, filed or wrote it (its `job`
    /// key).
    pub fn is_finishing(&self, card: &Card) -> bool {
        card.str_key("job")
            .is_some_and(|job| self.working_jobs.contains(job))
    }

    /// The working job that holds the Task (its `holder` key), if there is one.
    pub fn holder<'a>(&self, task: &'a Card) -> Option<&'a str> {
        task.str_key("holder")
            .filter(|job| self.working_jobs.contains(*job))
    }

    /// Whether a working job holds the Task.
    pub fn holds(&self, task: &Card) -> bool {
        self.holder(task).is_some()
    }
}

/// How a Proposal in Backlog stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProposalState {
    /// Handed over, waiting for the developer.
    Open,
    /// A refinement job adapts it to the current Model.
    Refining,
    /// A Draft a refinement left half-way.
    Interrupted,
}

/// One Drift listed by a resolution, with the decision recorded for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolutionRow {
    pub drift: String,
    /// `fix-code`, `proposal` or `dismissed`.
    pub decision: String,
    pub option: Option<u32>,
    pub note: Option<String>,
}

/// A card of the Backlog, by identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BacklogEntry {
    Proposal {
        id: String,
        state: ProposalState,
        kind: String,
    },
    /// A resolution Proposal and the Drifts it gathers.
    Resolution {
        id: String,
        state: ProposalState,
        rows: Vec<ResolutionRow>,
    },
    Drift {
        id: String,
    },
}

impl BacklogEntry {
    pub fn id(&self) -> &str {
        match self {
            BacklogEntry::Proposal { id, .. }
            | BacklogEntry::Resolution { id, .. }
            | BacklogEntry::Drift { id } => id,
        }
    }
}

/// A card of Active, by identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveEntry {
    pub id: String,
    /// In progress (`doing`), else To do.
    pub doing: bool,
    /// Open Drifts that block it, in creation order.
    pub blocked_by: Vec<String>,
    /// Fix Tasks it waits for, in creation order of their Drifts.
    pub waiting_for: Vec<String>,
}

impl ActiveEntry {
    /// To do and not blocked.
    pub fn runnable(&self) -> bool {
        !self.doing && self.blocked_by.is_empty() && self.waiting_for.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Board {
    pub backlog: Vec<BacklogEntry>,
    pub active: Vec<ActiveEntry>,
    /// Why `Run all` is disabled, in the order of the control; empty when it can start.
    pub run_all_reasons: Vec<&'static str>,
}

fn created_of(card: &Card) -> &str {
    card.created.as_deref().unwrap_or("")
}

fn is_resolution(card: &Card) -> bool {
    card.str_key("kind") == Some("resolution")
}

fn proposal_state(card: &Card) -> Option<ProposalState> {
    match card.str_key("status")? {
        "open" => Some(ProposalState::Open),
        "refining" => Some(ProposalState::Refining),
        "draft" if card.bool_key("refining") == Some(true) => Some(ProposalState::Interrupted),
        _ => None,
    }
}

/// The rows of a resolution: `drifts` in its front matter, in order.
pub fn resolution_rows(card: &Card) -> Vec<ResolutionRow> {
    let Some(serde_yaml::Value::Sequence(list)) = card.front.get("drifts") else {
        return Vec::new();
    };
    list.iter()
        .filter_map(|item| {
            let map = item.as_mapping()?;
            Some(ResolutionRow {
                drift: map.get("drift")?.as_str()?.to_owned(),
                decision: map.get("decision")?.as_str()?.to_owned(),
                option: map
                    .get("option")
                    .and_then(|v| v.as_u64())
                    .and_then(|n| u32::try_from(n).ok()),
                note: map.get("note").and_then(|v| v.as_str()).map(str::to_owned),
            })
        })
        .collect()
}

/// Whether Drifts are being resolved: a resolve works, or a resolution is being written (a Draft
/// no refinement left half-way) or refined.
pub fn drifts_resolving(image: &ProjectImage, facts: &AgentFacts) -> bool {
    facts.resolve_working
        || image
            .cards_of(CardKind::Proposal)
            .any(|p| is_resolution(p) && is_being_written(p))
}

/// A Proposal an agent is writing: a Draft that no refinement left half-way, or one being refined.
pub fn is_being_written(card: &Card) -> bool {
    match card.str_key("status") {
        Some("refining") => true,
        Some("draft") => card.bool_key("refining") != Some(true),
        _ => false,
    }
}

/// What the developer reads when the Proposal attached to a Drift, or the one a control acts on,
/// is being refined (shared by the Drift and the Proposal controls).
pub const REASON_BEING_REFINED: &str = "Being refined";

/// Computes the Board of `image`.
pub fn compute(image: &ProjectImage, facts: &AgentFacts) -> Board {
    let drifts: Vec<&Card> = image.cards_of(CardKind::Drift).collect();
    let tasks: Vec<&Card> = image.cards_of(CardKind::Task).collect();
    let proposals: Vec<&Card> = image.cards_of(CardKind::Proposal).collect();

    // --- Backlog ---
    let attached: BTreeSet<&str> = drifts
        .iter()
        .filter_map(|d| d.str_key("proposal"))
        .collect();
    let shown: Vec<(&Card, ProposalState)> = proposals
        .iter()
        .filter(|p| !attached.contains(p.id.as_str()))
        .filter_map(|p| proposal_state(p).map(|state| (*p, state)))
        .collect();
    let mut gathered: BTreeSet<String> = BTreeSet::new();
    let mut entries: Vec<(&Card, BacklogEntry)> = Vec::new();
    for &(card, state) in &shown {
        if is_resolution(card) {
            let rows = resolution_rows(card);
            gathered.extend(rows.iter().map(|r| r.drift.clone()));
            entries.push((
                card,
                BacklogEntry::Resolution {
                    id: card.id.clone(),
                    state,
                    rows,
                },
            ));
        } else {
            let kind = card.str_key("kind").unwrap_or("change").to_owned();
            entries.push((
                card,
                BacklogEntry::Proposal {
                    id: card.id.clone(),
                    state,
                    kind,
                },
            ));
        }
    }
    let open_drifts: Vec<&Card> = drifts
        .iter()
        .copied()
        .filter(|d| d.str_key("status") == Some("open"))
        .collect();
    for &drift in &open_drifts {
        if !gathered.contains(&drift.id) {
            entries.push((
                drift,
                BacklogEntry::Drift {
                    id: drift.id.clone(),
                },
            ));
        }
    }
    // A Proposal that reworks a rejected one takes its place in the order of the Board.
    let place = |card: &Card| -> (String, String) {
        let rejected = card
            .str_key("reworks")
            .and_then(|id| image.card(CardKind::Proposal, id));
        let (created, id) = rejected.map_or((created_of(card), card.id.as_str()), |old| {
            (created_of(old), old.id.as_str())
        });
        (created.to_owned(), id.to_owned())
    };
    entries.sort_by_cached_key(|(card, _)| place(card));
    let backlog: Vec<BacklogEntry> = entries.into_iter().map(|(_, entry)| entry).collect();

    // --- Active ---
    let blocking = BlockingIndex::new(image);
    let mut active: Vec<ActiveEntry> = tasks
        .iter()
        .filter(|t| matches!(t.str_key("column"), Some("todo" | "doing")))
        .map(|task| {
            let Blocking {
                drifts: blocked_by,
                fix_tasks: waiting_for,
            } = blocking.of(task);
            ActiveEntry {
                id: task.id.clone(),
                doing: task.str_key("column") == Some("doing"),
                blocked_by,
                waiting_for,
            }
        })
        .collect();
    let created = |id: &str| {
        tasks
            .iter()
            .find(|t| t.id == id)
            .map(|t| created_of(t))
            .unwrap_or("")
    };
    active.sort_by(|a, b| compare(created(&a.id), &a.id, created(&b.id), &b.id));

    // --- Run all ---
    let resolving = drifts_resolving(image, facts);
    let todo = active.iter().filter(|t| !t.doing).count();
    let runnable = active.iter().filter(|t| t.runnable()).count();
    let mut run_all_reasons = Vec::new();
    if facts.merge_in_progress {
        run_all_reasons.push(REASON_MERGE_IN_PROGRESS);
    }
    if resolving {
        run_all_reasons.push(REASON_DRIFTS_RESOLVING);
    }
    if facts.run_in_progress {
        run_all_reasons.push(REASON_RUN_IN_PROGRESS);
    }
    if todo == 0 {
        run_all_reasons.push(REASON_NO_TODO);
    } else if runnable == 0 {
        run_all_reasons.push(REASON_ALL_BLOCKED);
    }

    Board {
        backlog,
        active,
        run_all_reasons,
    }
}

// ---------------------------------------------------------------------------------------
// Delete
// ---------------------------------------------------------------------------------------

/// A reason why a Task cannot be deleted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeleteBlocker {
    /// The job that filed the Task is still working.
    AgentFinishing,
    /// The Task is the origin of an open Drift (its identity).
    OriginOfOpenDrift(String),
    /// The Task is the fix Task of a Drift whose code is to be fixed (the Drift's identity).
    FixTaskOf(String),
}

impl DeleteBlocker {
    /// The text of the disabled control; `label` shows a Drift (`D3`, or its permanent id).
    pub fn control_text(&self, label: impl Fn(&str) -> String) -> String {
        match self {
            DeleteBlocker::AgentFinishing => REASON_AGENT_FINISHING.to_owned(),
            DeleteBlocker::OriginOfOpenDrift(drift) => {
                format!("Origin of open Drift {}", label(drift))
            }
            DeleteBlocker::FixTaskOf(drift) => format!(
                "Fix Task of Drift {}, whose code is to be fixed",
                label(drift)
            ),
        }
    }

    /// The reason in a notice: `Could not delete T4: <reason>.`
    pub fn notice_text(&self, label: impl Fn(&str) -> String) -> String {
        match self {
            DeleteBlocker::AgentFinishing => "the agent that filed it is still working".to_owned(),
            DeleteBlocker::OriginOfOpenDrift(drift) => {
                format!("it is the origin of open Drift {}", label(drift))
            }
            DeleteBlocker::FixTaskOf(drift) => format!(
                "it is the fix Task of Drift {}, whose code is to be fixed",
                label(drift)
            ),
        }
    }
}

/// Every reason `task` cannot be deleted now, in the fixed order of the control.
pub fn delete_blockers(
    image: &ProjectImage,
    facts: &AgentFacts,
    task: &Card,
) -> Vec<DeleteBlocker> {
    let mut blockers = Vec::new();
    if facts.is_finishing(task) {
        blockers.push(DeleteBlocker::AgentFinishing);
    }
    blockers.extend(
        image
            .cards_of(CardKind::Drift)
            .filter(|d| {
                d.str_key("status") == Some("open") && d.str_key("origin") == Some(task.id.as_str())
            })
            .map(|d| DeleteBlocker::OriginOfOpenDrift(d.id.clone())),
    );
    blockers.extend(
        image
            .cards_of(CardKind::Drift)
            .filter(|d| {
                d.str_key("status") == Some("code-to-be-fixed")
                    && d.str_key("task") == Some(task.id.as_str())
            })
            .map(|d| DeleteBlocker::FixTaskOf(d.id.clone())),
    );
    blockers
}

/// A decision refused when it was applied; nothing was written. `notice` is what the developer
/// reads at the top of the Board.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{notice}")]
pub struct Refused {
    pub notice: String,
}

fn label_of(numbers: &ShortNumbers, kind: CardKind, id: &str) -> String {
    numbers.label_of(kind, id).unwrap_or_else(|| id.to_owned())
}

/// Files under `task-images/<id>/`, relative to `.weaver/`, in order.
pub fn task_image_files(project_root: &Path, id: &str) -> Vec<String> {
    fn walk(folder: &Path, rel: &str, out: &mut Vec<String>) {
        let Ok(entries) = fs::read_dir(folder) else {
            return;
        };
        let mut entries: Vec<_> = entries.filter_map(Result::ok).collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let name = entry.file_name().to_string_lossy().into_owned();
            let rel = format!("{rel}/{name}");
            match entry.file_type() {
                Ok(kind) if kind.is_dir() => walk(&entry.path(), &rel, out),
                Ok(_) => out.push(rel),
                Err(_) => {}
            }
        }
    }
    let rel = format!("task-images/{id}");
    let mut out = Vec::new();
    walk(&layout::absolute(project_root, &rel), &rel, &mut out);
    out
}

/// What deleting a Task changes, or why it is refused. Pure over the image: the conditions are
/// those of the moment it is applied.
pub fn plan_delete_task(
    image: &ProjectImage,
    facts: &AgentFacts,
    task_id: &str,
) -> Result<ChangeSet, Refused> {
    let numbers = shortnum::peek(image.root(), image.cards());
    let Some(task) = image.card(CardKind::Task, task_id) else {
        return Err(Refused {
            notice: format!("Could not delete {task_id}: the Task no longer exists."),
        });
    };
    let name = label_of(&numbers, CardKind::Task, &task.id);
    if task.str_key("column") != Some("todo") {
        return Err(Refused {
            notice: format!("Could not delete {name}: it is no longer in To do."),
        });
    }
    let blockers = delete_blockers(image, facts, task);
    if !blockers.is_empty() {
        let drift = |id: &str| label_of(&numbers, CardKind::Drift, id);
        let reasons: Vec<String> = blockers.iter().map(|b| b.notice_text(drift)).collect();
        return Err(Refused {
            notice: format!("Could not delete {name}: {}.", reasons.join("; ")),
        });
    }
    let mut changes = ChangeSet::new();
    changes.delete(&task.path);
    for file in task_image_files(image.root(), &task.id) {
        changes.delete(&file);
    }
    Ok(changes)
}

/// Deletes the Task `task_id`: whole or not at all. Must run under the project's write lock
/// (the app's mutation queue holds it); the project is read again here, so the conditions are
/// those of this moment. Returns the image after the deletion.
pub fn delete_task(
    project_root: &Path,
    facts: &AgentFacts,
    task_id: &str,
) -> Result<ProjectImage, MutationError<Refused>> {
    let (image, ()) = changes::mutate(
        project_root,
        |image| plan_delete_task(image, facts, task_id).map(|c| (c, ())),
        |_| Ok(()),
    )?;
    // The files are gone; the folders that held them go too (best effort: a folder that still
    // holds something is left alone).
    remove_empty_folders(&layout::absolute(
        project_root,
        &format!("task-images/{task_id}"),
    ));
    Ok(image)
}

pub(crate) fn remove_empty_folders(folder: &Path) {
    if let Ok(entries) = fs::read_dir(folder) {
        for entry in entries.flatten() {
            if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                remove_empty_folders(&entry.path());
            }
        }
    }
    let _ = fs::remove_dir(folder);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::model::{ORDER_MACHINE, USER_ACTOR};
    use crate::project::testing::put;

    const T0: &str = "2026-11-02T07:00:00";

    fn at(second: u32) -> String {
        format!("{T0}.{second:03}Z")
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
        let refs: String = refs.iter().map(|r| format!("  - {r}\n")).collect();
        let refs = if refs.is_empty() {
            "refs: []\n".to_owned()
        } else {
            format!("refs:\n{refs}")
        };
        put(
            root,
            &format!("tasks/{id}-t.md"),
            &format!("---\ncolumn: {column}\ntechnical: false\n{refs}{extra}created: {}\n---\n# Task {id}\n", at(created)),
        );
    }

    fn drift(root: &Path, id: &str, created: u32, status: &str, refs: &[&str], extra: &str) {
        let refs: String = refs.iter().map(|r| format!("  - {r}\n")).collect();
        put(
            root,
            &format!("drifts/{id}-d.md"),
            &format!(
                "---\nstatus: {status}\nrefs:\n{refs}{extra}created: {}\n---\n# Drift {id}\n",
                at(created)
            ),
        );
    }

    fn proposal(root: &Path, id: &str, created: u32, status: &str, extra: &str) {
        put(
            root,
            &format!("proposals/{id}-p/proposal.md"),
            &format!(
                "---\nstatus: {status}\n{extra}created: {}\n---\n# Proposal {id}\n",
                at(created)
            ),
        );
    }

    fn board(dir: &tempfile::TempDir, facts: &AgentFacts) -> Board {
        compute(&ProjectImage::load(dir.path()), facts)
    }

    fn backlog_ids(board: &Board) -> Vec<&str> {
        board.backlog.iter().map(BacklogEntry::id).collect()
    }

    fn active_ids(board: &Board) -> Vec<&str> {
        board.active.iter().map(|t| t.id.as_str()).collect()
    }

    #[test]
    fn the_backlog_holds_open_proposals_and_open_drifts_in_one_creation_order() {
        let dir = project();
        let root = dir.path();
        drift(root, "dddddddd", 4, "open", &["order"], "");
        proposal(root, "pppppppp", 2, "open", "");
        drift(root, "eeeeeeee", 1, "open", &["user"], "");
        proposal(root, "qqqqqqqq", 3, "refining", "");
        drift(root, "ffffffff", 5, "fixed", &["order"], "");
        drift(root, "gggggggg", 6, "dismissed", &["order"], "");
        proposal(root, "rrrrrrrr", 7, "approved", "");
        let board = board(&dir, &AgentFacts::default());
        assert_eq!(
            backlog_ids(&board),
            ["eeeeeeee", "pppppppp", "qqqqqqqq", "dddddddd"]
        );
        assert!(matches!(
            &board.backlog[2],
            BacklogEntry::Proposal {
                state: ProposalState::Refining,
                ..
            }
        ));
    }

    #[test]
    fn a_draft_has_a_card_only_when_a_refinement_left_it_half_way() {
        let dir = project();
        let root = dir.path();
        proposal(root, "aaaaaaaa", 1, "draft", "");
        proposal(root, "bbbbbbbb", 2, "draft", "refining: true\n");
        proposal(root, "cccccccc", 3, "rejected", "");
        proposal(root, "dddddddd", 4, "discarded", "");
        // A Draft of an earlier version that Chat was writing is an ordinary Draft.
        proposal(
            root,
            "eeeeeeee",
            5,
            "draft",
            "chat: true\nchatWriting: true\n",
        );
        let board = board(&dir, &AgentFacts::default());
        assert_eq!(backlog_ids(&board), ["bbbbbbbb"]);
        assert!(matches!(
            &board.backlog[0],
            BacklogEntry::Proposal {
                state: ProposalState::Interrupted,
                ..
            }
        ));
    }

    #[test]
    fn a_proposal_is_finishing_only_while_the_job_that_wrote_it_works() {
        let dir = project();
        let root = dir.path();
        proposal(root, "aaaaaaaa", 1, "open", "job: jjjjjjjj\n");
        proposal(
            root,
            "bbbbbbbb",
            2,
            "open",
            "chat: true\nchatWriting: true\n",
        );
        let image = ProjectImage::load(root);
        let made = image.card(CardKind::Proposal, "aaaaaaaa").unwrap();
        let other = image.card(CardKind::Proposal, "bbbbbbbb").unwrap();
        let working = AgentFacts {
            working_jobs: ["jjjjjjjj".to_owned()].into(),
            ..AgentFacts::default()
        };
        assert!(working.is_finishing(made));
        assert!(!working.is_finishing(other));
        assert!(!AgentFacts::default().is_finishing(made));
    }

    #[test]
    fn a_proposal_attached_to_a_drift_is_never_shown_on_its_own() {
        let dir = project();
        let root = dir.path();
        proposal(root, "pppppppp", 1, "open", "");
        proposal(root, "qqqqqqqq", 2, "open", "");
        drift(
            root,
            "dddddddd",
            3,
            "open",
            &["order"],
            "proposal: pppppppp\n",
        );
        let board = board(&dir, &AgentFacts::default());
        assert_eq!(backlog_ids(&board), ["qqqqqqqq", "dddddddd"]);
    }

    #[test]
    fn a_resolution_gathers_its_drifts_into_one_card_at_its_own_place() {
        let dir = project();
        let root = dir.path();
        drift(root, "dddddddd", 1, "open", &["order"], "");
        drift(root, "eeeeeeee", 2, "open", &["user"], "");
        drift(root, "ffffffff", 4, "open", &["order/Paid"], "");
        proposal(
            root,
            "rrrrrrrr",
            3,
            "open",
            "kind: resolution\ndrifts:\n  - drift: dddddddd\n    decision: fix-code\n    option: 2\n    note: Do it\n  - drift: eeeeeeee\n    decision: dismissed\n",
        );
        let board = board(&dir, &AgentFacts::default());
        assert_eq!(
            backlog_ids(&board),
            ["rrrrrrrr", "ffffffff"],
            "a resolution counts once"
        );
        let BacklogEntry::Resolution { rows, .. } = &board.backlog[0] else {
            panic!("a resolution")
        };
        assert_eq!(
            rows,
            &[
                ResolutionRow {
                    drift: "dddddddd".into(),
                    decision: "fix-code".into(),
                    option: Some(2),
                    note: Some("Do it".into())
                },
                ResolutionRow {
                    drift: "eeeeeeee".into(),
                    decision: "dismissed".into(),
                    option: None,
                    note: None
                },
            ]
        );
    }

    #[test]
    fn a_resolution_still_being_written_leaves_its_drifts_on_the_board() {
        let dir = project();
        let root = dir.path();
        drift(root, "dddddddd", 1, "open", &["order"], "");
        proposal(
            root,
            "rrrrrrrr",
            2,
            "draft",
            "kind: resolution\ndrifts:\n  - drift: dddddddd\n    decision: dismissed\n",
        );
        let board = board(&dir, &AgentFacts::default());
        assert_eq!(backlog_ids(&board), ["dddddddd"]);
    }

    #[test]
    fn active_holds_todo_and_doing_tasks_oldest_first_blocked_ones_included() {
        let dir = project();
        let root = dir.path();
        task(root, "cccccccc", 3, "todo", &["order"], "");
        task(
            root,
            "aaaaaaaa",
            1,
            "doing",
            &["user"],
            "holder: jjjjjjjj\n",
        );
        task(
            root,
            "bbbbbbbb",
            2,
            "done",
            &["order"],
            "finished: 2026-11-02T07:01:00.000Z\n",
        );
        task(root, "dddddddd", 2, "todo", &["order"], "");
        drift(root, "eeeeeeee", 9, "open", &["order"], "");
        let board = board(&dir, &AgentFacts::default());
        assert_eq!(active_ids(&board), ["aaaaaaaa", "dddddddd", "cccccccc"]);
        assert!(board.active[0].doing && board.active[0].blocked_by.is_empty());
        assert_eq!(board.active[1].blocked_by, ["eeeeeeee"]);
    }

    #[test]
    fn an_open_drift_blocks_the_tasks_that_concern_the_same_thing_and_its_origin() {
        let dir = project();
        let root = dir.path();
        task(root, "aaaaaaaa", 1, "todo", &["shop"], "");
        task(root, "bbbbbbbb", 2, "todo", &["user.CANCEL"], "");
        task(root, "cccccccc", 3, "todo", &["user.CANCEL"], "");
        task(root, "dddddddd", 4, "todo", &[], "");
        drift(
            root,
            "eeeeeeee",
            5,
            "open",
            &["order/Paid"],
            "origin: bbbbbbbb\n",
        );
        drift(root, "ffffffff", 6, "open", &["order"], "");
        drift(root, "gggggggg", 7, "dismissed", &["user"], "");
        let board = board(&dir, &AgentFacts::default());
        let blocked: Vec<_> = board
            .active
            .iter()
            .map(|t| (t.id.as_str(), t.blocked_by.clone()))
            .collect();
        assert_eq!(
            blocked,
            [
                (
                    "aaaaaaaa",
                    vec!["eeeeeeee".to_owned(), "ffffffff".to_owned()]
                ),
                ("bbbbbbbb", vec!["eeeeeeee".to_owned()]),
                ("cccccccc", vec![]),
                ("dddddddd", vec![]),
            ]
        );
    }

    #[test]
    fn the_origin_of_a_drift_whose_code_is_to_be_fixed_waits_for_the_fix_task_until_it_is_done() {
        let dir = project();
        let root = dir.path();
        task(root, "aaaaaaaa", 1, "todo", &["order"], "");
        task(root, "bbbbbbbb", 2, "todo", &["order"], "drift: eeeeeeee\n");
        drift(
            root,
            "eeeeeeee",
            3,
            "code-to-be-fixed",
            &["order"],
            "origin: aaaaaaaa\ntask: bbbbbbbb\n",
        );
        let waiting = board(&dir, &AgentFacts::default());
        assert_eq!(waiting.active[0].waiting_for, ["bbbbbbbb"]);
        assert!(
            waiting.active[1].blocked_by.is_empty() && waiting.active[1].waiting_for.is_empty(),
            "a decided Drift blocks nothing else"
        );
        task(
            root,
            "bbbbbbbb",
            2,
            "done",
            &["order"],
            "drift: eeeeeeee\nfinished: 2026-11-02T07:01:00.000Z\n",
        );
        let done = board(&dir, &AgentFacts::default());
        assert!(done.active[0].waiting_for.is_empty());
    }

    #[test]
    fn run_all_lists_every_reason_in_order() {
        let dir = project();
        let root = dir.path();
        let working = AgentFacts {
            run_in_progress: true,
            resolve_working: true,
            ..AgentFacts::default()
        };
        assert_eq!(
            board(&dir, &working).run_all_reasons,
            [
                REASON_DRIFTS_RESOLVING,
                REASON_RUN_IN_PROGRESS,
                REASON_NO_TODO
            ]
        );
        assert_eq!(
            board(&dir, &AgentFacts::default()).run_all_reasons,
            [REASON_NO_TODO]
        );

        task(root, "aaaaaaaa", 1, "todo", &["order"], "");
        drift(root, "dddddddd", 2, "open", &["order"], "");
        assert_eq!(
            board(&dir, &AgentFacts::default()).run_all_reasons,
            [REASON_ALL_BLOCKED]
        );
        assert_eq!(
            board(
                &dir,
                &AgentFacts {
                    run_in_progress: true,
                    ..AgentFacts::default()
                }
            )
            .run_all_reasons,
            [REASON_RUN_IN_PROGRESS, REASON_ALL_BLOCKED]
        );

        drift(root, "dddddddd", 2, "dismissed", &["order"], "");
        assert!(board(&dir, &AgentFacts::default())
            .run_all_reasons
            .is_empty());
    }

    #[test]
    fn a_task_in_progress_is_not_counted_as_todo_for_run_all() {
        let dir = project();
        task(
            dir.path(),
            "aaaaaaaa",
            1,
            "doing",
            &["order"],
            "holder: jjjjjjjj\n",
        );
        assert_eq!(
            board(&dir, &AgentFacts::default()).run_all_reasons,
            [REASON_NO_TODO]
        );
    }

    #[test]
    fn a_resolution_draft_counts_as_drifts_being_resolved() {
        let dir = project();
        let root = dir.path();
        task(root, "aaaaaaaa", 1, "todo", &["order"], "");
        proposal(root, "rrrrrrrr", 2, "draft", "kind: resolution\n");
        assert_eq!(
            board(&dir, &AgentFacts::default()).run_all_reasons,
            [REASON_DRIFTS_RESOLVING]
        );
    }

    // --- Delete ---

    fn working(job: &str) -> AgentFacts {
        AgentFacts {
            working_jobs: BTreeSet::from([job.to_owned()]),
            ..AgentFacts::default()
        }
    }

    fn snapshot(root: &Path) -> Vec<(String, String)> {
        let mut files = Vec::new();
        fn walk(dir: &Path, out: &mut Vec<(String, String)>) {
            for entry in fs::read_dir(dir).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, out);
                } else {
                    out.push((
                        path.display().to_string(),
                        fs::read_to_string(&path).unwrap_or_default(),
                    ));
                }
            }
        }
        walk(root, &mut files);
        files.sort();
        files
    }

    #[test]
    fn deleting_a_task_removes_its_file_and_its_images_and_frees_its_number() {
        let dir = project();
        let root = dir.path();
        task(root, "aaaaaaaa", 1, "todo", &["order"], "");
        task(root, "bbbbbbbb", 2, "todo", &["order"], "");
        put(root, "task-images/aaaaaaaa/shot.png", "png");
        put(root, "task-images/aaaaaaaa/sub/more.png", "png");
        put(root, "task-images/bbbbbbbb/keep.png", "png");
        let image = delete_task(root, &AgentFacts::default(), "aaaaaaaa").unwrap();
        assert!(image.card(CardKind::Task, "aaaaaaaa").is_none());
        assert!(image.card(CardKind::Task, "bbbbbbbb").is_some());
        assert!(!layout::absolute(root, "tasks/aaaaaaaa-t.md").exists());
        assert!(!layout::absolute(root, "task-images/aaaaaaaa/shot.png").exists());
        assert!(!layout::absolute(root, "task-images/aaaaaaaa/sub/more.png").exists());
        assert!(layout::absolute(root, "task-images/bbbbbbbb/keep.png").exists());
        assert_eq!(
            image
                .short_numbers()
                .label_of(CardKind::Task, "bbbbbbbb")
                .as_deref(),
            Some("T1")
        );
    }

    #[test]
    fn a_refused_delete_says_why_and_writes_nothing() {
        let dir = project();
        let root = dir.path();
        task(root, "aaaaaaaa", 1, "todo", &["order"], "job: jjjjjjjj\n");
        let before = snapshot(root);
        let refused = delete_task(root, &working("jjjjjjjj"), "aaaaaaaa").unwrap_err();
        let MutationError::Refused(refused) = refused else {
            panic!("a refusal")
        };
        assert_eq!(
            refused.notice,
            "Could not delete T1: the agent that filed it is still working."
        );
        assert_eq!(
            snapshot(root),
            before,
            "nothing was written, not even the short number cache"
        );
    }

    #[test]
    fn delete_checks_its_conditions_when_applied_not_when_it_was_asked() {
        let dir = project();
        let root = dir.path();
        task(root, "aaaaaaaa", 1, "todo", &["order"], "");
        task(root, "bbbbbbbb", 2, "todo", &["user"], "");
        // The button was enabled; then the agent filed an open Drift whose origin is the Task.
        drift(
            root,
            "dddddddd",
            3,
            "open",
            &["order"],
            "origin: aaaaaaaa\n",
        );
        let before = snapshot(root);
        let MutationError::Refused(refused) =
            delete_task(root, &AgentFacts::default(), "aaaaaaaa").unwrap_err()
        else {
            panic!("a refusal")
        };
        assert_eq!(
            refused.notice,
            "Could not delete T1: it is the origin of open Drift D1."
        );
        assert_eq!(snapshot(root), before);
    }

    #[test]
    fn delete_lists_every_blocker_in_the_order_of_the_control() {
        let dir = project();
        let root = dir.path();
        task(root, "aaaaaaaa", 1, "todo", &["order"], "job: jjjjjjjj\n");
        drift(root, "dddddddd", 3, "open", &["user"], "origin: aaaaaaaa\n");
        drift(
            root,
            "eeeeeeee",
            4,
            "code-to-be-fixed",
            &["user"],
            "task: aaaaaaaa\n",
        );
        let image = ProjectImage::load(root);
        let numbers = image.short_numbers();
        let label = |id: &str| numbers.label_of(CardKind::Drift, id).unwrap();
        let task = image.card(CardKind::Task, "aaaaaaaa").unwrap();
        let blockers = delete_blockers(&image, &working("jjjjjjjj"), task);
        assert_eq!(
            blockers
                .iter()
                .map(|b| b.control_text(label))
                .collect::<Vec<_>>(),
            [
                "The agent is still finishing",
                "Origin of open Drift D1",
                "Fix Task of Drift D2, whose code is to be fixed",
            ]
        );
        assert!(delete_blockers(&image, &AgentFacts::default(), task).len() == 2);
    }

    #[test]
    fn only_a_task_in_todo_can_be_deleted() {
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
            "done",
            &["order"],
            "finished: 2026-11-02T07:01:00.000Z\n",
        );
        for (id, notice) in [
            ("aaaaaaaa", "Could not delete T1: it is no longer in To do."),
            (
                "zzzzzzzz",
                "Could not delete zzzzzzzz: the Task no longer exists.",
            ),
        ] {
            let MutationError::Refused(refused) =
                delete_task(root, &AgentFacts::default(), id).unwrap_err()
            else {
                panic!("a refusal")
            };
            assert_eq!(refused.notice, notice);
        }
        let MutationError::Refused(refused) =
            delete_task(root, &AgentFacts::default(), "bbbbbbbb").unwrap_err()
        else {
            panic!("a refusal")
        };
        assert!(refused.notice.ends_with("it is no longer in To do."));
    }
}
