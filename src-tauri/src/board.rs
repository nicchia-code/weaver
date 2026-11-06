//! The Board in the app: the snapshot the UI shows and the decisions it can take on it.
//!
//! What the Board contains and in which order is worked out in `weaver_core::project::board`
//! (the UI never groups, sorts or blocks anything). This module turns that into the wire types of
//! the UI, adds the ghosts the jobs announced ([`crate::agent_work`]) and exposes the commands:
//!
//! * `board_snapshot`: the whole Board of a project at one revision;
//! * `delete_task`: the Delete decision, applied by [`crate::decisions::decide`].
//!
//! The UI fetches the snapshot when its window loads and again each time a `project-changed`
//! event names the slice `board`; the `revision` it carries lets it drop an answer older than one
//! it already shows.

use std::path::Path;

use serde::Serialize;
use tauri::{AppHandle, State};
use ts_rs::TS;
use weaver_core::project::board::{
    self, ActiveEntry, AgentFacts, BacklogEntry, DeleteBlocker, ProposalState, ResolutionRow,
};
use weaver_core::project::cards::{Card, CardKind};
use weaver_core::project::history;
use weaver_core::project::image::ProjectImage;
use weaver_core::project::proposal::files::{self, Folders};
use weaver_core::project::proposal::models::ProposalModels;
use weaver_core::project::shortnum::{self, ShortNumbers};
use weaver_core::project::tasks::{self, Blocking};

use crate::agent_work::Ghost;
use crate::blocking;
use crate::decisions;
use crate::drift_view::{self, DriftView};
use crate::error::AppError;
use crate::events;
use crate::pictures::{Pictures, SLICE_BOARD};
use crate::proposal_view::{self, ProposalView};
use crate::state::AppState;

/// A card named in another card (`Origin of open Drift D3`, `Waiting for T7`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CardRef {
    pub id: String,
    /// `T7`, when the card is live and has a number.
    pub short_number: Option<String>,
}

/// A card named in a Task card, with the title the `Blocked by` and `Waiting for` lists show.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TitledRef {
    pub id: String,
    /// `D3`, when the card is live and has a number.
    pub short_number: Option<String>,
    pub title: String,
}

/// What every card of the Board shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CardHead {
    /// The permanent identity.
    pub id: String,
    /// `P3`, `T4`, `D2`, when the card has a number.
    pub short_number: Option<String>,
    pub title: String,
    /// The Markdown of the card after its title.
    pub body: String,
    /// Creation instant (UTC ISO 8601).
    pub created: String,
    /// A job that is still working created, filed or wrote the card: every control of the card
    /// lists `The agent is still finishing` first.
    pub agent_finishing: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum ProposalStanding {
    /// Handed over, waiting for the developer.
    Open,
    /// A refinement job adapts it to the current Model.
    Refining,
    /// A Draft a refinement left half-way.
    Interrupted,
    /// A Draft the agent of Chat is writing: `Open Chat` and `Discard` only.
    ChatDraft,
}

impl From<ProposalState> for ProposalStanding {
    fn from(state: ProposalState) -> Self {
        match state {
            ProposalState::Open => ProposalStanding::Open,
            ProposalState::Refining => ProposalStanding::Refining,
            ProposalState::Interrupted => ProposalStanding::Interrupted,
            ProposalState::ChatDraft => ProposalStanding::ChatDraft,
        }
    }
}

/// A Drift of a resolution and what was decided for it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ResolutionDrift {
    pub drift: CardRef,
    /// `fix-code`, `proposal` or `dismissed`.
    pub decision: String,
    pub option: Option<u32>,
    pub note: Option<String>,
}

/// A card of the Board. The tag is the kind, which picks the renderer in the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum BoardCard {
    Proposal {
        head: CardHead,
        standing: ProposalStanding,
        /// The kind of the Proposal (`change`, `import`, ...), as its file says.
        proposal_kind: String,
        view: ProposalView,
    },
    /// A resolution Proposal: one card that gathers the Drifts it lists.
    Resolution {
        head: CardHead,
        standing: ProposalStanding,
        drifts: Vec<ResolutionDrift>,
        view: ProposalView,
    },
    Drift {
        head: CardHead,
        /// The Task whose work found it.
        origin: Option<TitledRef>,
        view: Box<DriftView>,
    },
    Task {
        head: CardHead,
        /// In progress (no Delete) rather than To do.
        doing: bool,
        /// A working job holds it (`Agent working`, the rotating border).
        held_by_agent: bool,
        /// Names no Model element.
        technical: bool,
        /// What it refers to, as written in its file.
        refs: Vec<String>,
        /// The label of the Proposal whose design it belongs to.
        label: Option<String>,
        /// Open Drifts that block it.
        blocked_by: Vec<TitledRef>,
        /// Fix Tasks it waits for.
        waiting_for: Vec<TitledRef>,
        /// Why `Run` is disabled, one reason per line in the fixed order; empty = `Run` works.
        run_reasons: Vec<String>,
        /// Why Delete is disabled, one reason per line in the fixed order; empty = Delete works.
        delete_reasons: Vec<String>,
    },
}

/// The Board of one project at one revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct BoardSnapshot {
    pub project_id: String,
    /// The revision of the picture this was worked out from; a snapshot older than one already
    /// shown is dropped.
    #[ts(type = "number")]
    pub revision: u64,
    /// Open Proposals, resolutions and Drifts in one creation order, oldest first.
    pub backlog: Vec<BoardCard>,
    /// Tasks in To do and In progress, blocked ones included, oldest first.
    pub active: Vec<BoardCard>,
    /// Cards the jobs announced and have not filed, in announcement order.
    pub ghosts: Vec<Ghost>,
    /// The Drifts a resolution gathers: not cards of the Board (they are not counted and have no
    /// place in Backlog), only what the details dialog of a row of the resolution shows.
    pub gathered: Vec<BoardCard>,
    /// Why `Run all` is disabled, in the fixed order; empty when it can start.
    pub run_all_reasons: Vec<String>,
    /// Tasks finished since the last Scan System started.
    pub done_since_last_scan: u32,
    /// Cited Tasks that stay because their copy in History could not be made, one text each.
    pub warnings: Vec<String>,
}

fn short_of(numbers: &ShortNumbers, kind: CardKind, id: &str) -> Option<String> {
    numbers.label_of(kind, id)
}

fn card_ref(numbers: &ShortNumbers, kind: CardKind, id: &str) -> CardRef {
    CardRef {
        id: id.to_owned(),
        short_number: short_of(numbers, kind, id),
    }
}

/// The body without its first H1, which the card shows as its title.
pub(crate) fn body_after_title(card: &Card) -> String {
    match card.body.trim_start().strip_prefix("# ") {
        Some(rest) => rest
            .split_once('\n')
            .map_or("", |(_, after)| after)
            .trim_start_matches('\n')
            .to_owned(),
        None => card.body.clone(),
    }
}

fn head(numbers: &ShortNumbers, facts: &AgentFacts, card: &Card) -> CardHead {
    CardHead {
        id: card.id.clone(),
        short_number: short_of(numbers, card.kind, &card.id),
        title: card.title.clone(),
        body: body_after_title(card),
        created: card.created.clone().unwrap_or_default(),
        agent_finishing: facts.is_finishing(card),
    }
}

fn resolution_drift(numbers: &ShortNumbers, row: &ResolutionRow) -> ResolutionDrift {
    ResolutionDrift {
        drift: card_ref(numbers, CardKind::Drift, &row.drift),
        decision: row.decision.clone(),
        option: row.option,
        note: row.note.clone(),
    }
}

fn backlog_card(
    image: &ProjectImage,
    numbers: &ShortNumbers,
    facts: &AgentFacts,
    folders: &Folders,
    models: &ProposalModels,
    entry: &BacklogEntry,
) -> Option<BoardCard> {
    Some(match entry {
        BacklogEntry::Proposal { id, state, kind } => BoardCard::Proposal {
            head: head(numbers, facts, image.card(CardKind::Proposal, id)?),
            standing: (*state).into(),
            proposal_kind: kind.clone(),
            view: proposal_view::view(image, folders, numbers, facts, models, id)?,
        },
        BacklogEntry::Resolution { id, state, rows } => BoardCard::Resolution {
            head: head(numbers, facts, image.card(CardKind::Proposal, id)?),
            standing: (*state).into(),
            drifts: rows
                .iter()
                .map(|row| resolution_drift(numbers, row))
                .collect(),
            view: proposal_view::view(image, folders, numbers, facts, models, id)?,
        },
        BacklogEntry::Drift { id } => drift_card(image, numbers, facts, folders, id)?,
    })
}

/// The card of the Drift `id`: its head, the Task that found it and its view.
fn drift_card(
    image: &ProjectImage,
    numbers: &ShortNumbers,
    facts: &AgentFacts,
    folders: &Folders,
    id: &str,
) -> Option<BoardCard> {
    let card = image.card(CardKind::Drift, id)?;
    Some(BoardCard::Drift {
        head: head(numbers, facts, card),
        origin: drift_view::origin_of(image, numbers, card),
        view: Box::new(drift_view::view(image, folders, numbers, facts, card)),
    })
}

/// The open Drifts the resolutions of the Backlog gather, for the details of their rows.
fn gathered_cards(
    image: &ProjectImage,
    numbers: &ShortNumbers,
    facts: &AgentFacts,
    folders: &Folders,
    backlog: &[BacklogEntry],
) -> Vec<BoardCard> {
    let mut seen = std::collections::BTreeSet::new();
    backlog
        .iter()
        .filter_map(|entry| match entry {
            BacklogEntry::Resolution { rows, .. } => Some(rows),
            _ => None,
        })
        .flatten()
        .filter(|row| seen.insert(row.drift.clone()))
        .filter(|row| {
            image
                .card(CardKind::Drift, &row.drift)
                .is_some_and(|d| d.str_key("status") == Some("open"))
        })
        .filter_map(|row| drift_card(image, numbers, facts, folders, &row.drift))
        .collect()
}

fn active_card(
    image: &ProjectImage,
    numbers: &ShortNumbers,
    facts: &AgentFacts,
    entry: &ActiveEntry,
) -> Option<BoardCard> {
    let task = image.card(CardKind::Task, &entry.id)?;
    let drift_label = |id: &str| {
        numbers
            .label_of(CardKind::Drift, id)
            .unwrap_or_else(|| id.to_owned())
    };
    // Delete exists in To do only; a Task in progress shows none.
    let delete_reasons = if entry.doing {
        Vec::new()
    } else {
        board::delete_blockers(image, facts, task)
            .iter()
            .map(|b: &DeleteBlocker| b.control_text(drift_label))
            .collect()
    };
    let blocking = Blocking {
        drifts: entry.blocked_by.clone(),
        fix_tasks: entry.waiting_for.clone(),
    };
    let name =
        |kind: CardKind, id: &str| numbers.label_of(kind, id).unwrap_or_else(|| id.to_owned());
    let titled = |kind: CardKind, id: &String| TitledRef {
        id: id.clone(),
        short_number: short_of(numbers, kind, id),
        title: image
            .card(kind, id)
            .map(|c| c.title.clone())
            .unwrap_or_default(),
    };
    Some(BoardCard::Task {
        head: head(numbers, facts, task),
        doing: entry.doing,
        held_by_agent: facts.holds(task),
        technical: tasks::is_technical(task),
        refs: tasks::refs_of(task),
        label: tasks::proposal_label(image, task),
        blocked_by: entry
            .blocked_by
            .iter()
            .map(|id| titled(CardKind::Drift, id))
            .collect(),
        waiting_for: entry
            .waiting_for
            .iter()
            .map(|id| titled(CardKind::Task, id))
            .collect(),
        run_reasons: tasks::run_reasons(image, facts, task, &blocking, &name),
        delete_reasons,
    })
}

/// The Board of `image`, worked out whole. `root` is where the short-number cache is read (it is
/// never written here).
pub fn snapshot(
    project_id: &str,
    revision: u64,
    root: &Path,
    image: &ProjectImage,
    facts: &AgentFacts,
    ghosts: Vec<Ghost>,
    models: &ProposalModels,
) -> BoardSnapshot {
    let numbers = shortnum::peek(root, image.cards());
    let computed = board::compute(image, facts);
    let folders = files::read_folders(root, image);
    BoardSnapshot {
        project_id: project_id.to_owned(),
        revision,
        backlog: computed
            .backlog
            .iter()
            .filter_map(|e| backlog_card(image, &numbers, facts, &folders, models, e))
            .collect(),
        active: computed
            .active
            .iter()
            .filter_map(|e| active_card(image, &numbers, facts, e))
            .collect(),
        ghosts,
        gathered: gathered_cards(image, &numbers, facts, &folders, &computed.backlog),
        run_all_reasons: computed
            .run_all_reasons
            .iter()
            .map(|r| (*r).to_owned())
            .collect(),
        done_since_last_scan: facts.done_since_last_scan,
        warnings: history_warnings(image, &numbers),
    }
}

/// `Could not record T3 in History: …` for every cited Task that is still a file.
fn history_warnings(image: &ProjectImage, numbers: &ShortNumbers) -> Vec<String> {
    history::stuck(image)
        .into_iter()
        .map(|stuck| {
            let name = numbers.label_of(CardKind::Task, &stuck.task).unwrap_or(stuck.task);
            format!("Could not record {name} in History: {}. The Task stays until its copy can be made.", stuck.reason)
        })
        .collect()
}

/// Tells the UI that what the Board shows changed without any file changing (a job started or
/// ended, a ghost came or went): a new revision of the picture, announced for the `board` slice.
/// The jobs call it after every such change.
pub fn publish_agent_change(
    app: &AppHandle,
    pictures: &Pictures,
    project_id: &str,
) -> Result<(), AppError> {
    let revision = pictures.touch(project_id, &[SLICE_BOARD])?;
    events::emit_project_changed(app, &revision.event())
}

/// The whole Board of an open project.
#[tauri::command]
pub async fn board_snapshot(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<BoardSnapshot, AppError> {
    let picture = state.pictures.get(&project_id)?;
    let work = state.agent_work.get();
    let name = picture
        .root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned());
    let dirs = state.dirs.clone();
    blocking::disk_named("board snapshot", name, move || {
        let facts = work.facts(&project_id);
        let ghosts = work.ghosts(&project_id);
        let mut models = ProposalModels::load(&dirs, &project_id);
        models.prune(&picture.image);
        snapshot(
            &project_id,
            picture.revision,
            &picture.root,
            &picture.image,
            &facts,
            ghosts,
            &models,
        )
    })
    .await
}

/// **Delete** on a Task in To do. Refused with `refused` (the message is the notice) when a
/// condition no longer holds at the moment of applying; nothing is written then.
#[tauri::command]
pub async fn delete_task(
    app: AppHandle,
    state: State<'_, AppState>,
    project_id: String,
    task_id: String,
) -> Result<(), AppError> {
    decisions::decide_and_announce(&app, &state, &project_id, move |root, facts| {
        board::delete_task(root, facts, &task_id)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_work::{GhostKind, GhostPhase};

    const TASK: &str = "---\ncolumn: todo\ntechnical: false\nrefs: []\njob: jjjjjjjj\ncreated: 2026-11-02T07:00:01.000Z\n---\n# First task\n\nDo the thing.\n";
    const DOING: &str = "---\ncolumn: doing\ntechnical: false\nrefs: []\nholder: jjjjjjjj\ncreated: 2026-11-02T07:00:02.000Z\n---\n# Second task\n";
    const DRIFT: &str = "---\nstatus: open\nrefs: []\norigin: aaaaaaaa\ncreated: 2026-11-02T07:00:03.000Z\n---\n# A drift\n";

    fn write(root: &Path, rel: &str, text: &str) {
        let path = root.join(".weaver").join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn finishing() -> AgentFacts {
        let mut facts = AgentFacts::default();
        facts.working_jobs.insert("jjjjjjjj".into());
        facts.done_since_last_scan = 2;
        facts
    }

    #[test]
    fn the_snapshot_carries_cards_blocking_delete_reasons_ghosts_and_counts() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "tasks/aaaaaaaa-first-task.md", TASK);
        write(dir.path(), "tasks/bbbbbbbb-second-task.md", DOING);
        write(dir.path(), "drifts/dddddddd-a-drift.md", DRIFT);
        let image = ProjectImage::load(dir.path());
        let ghost = Ghost {
            id: "gggggggg".into(),
            job_id: "jjjjjjjj".into(),
            kind: GhostKind::Task,
            title: "Coming".into(),
            phase: GhostPhase::Writing,
            note: Some("Reading the Model".into()),
        };

        let board = snapshot(
            "0123456789abcdef",
            7,
            dir.path(),
            &image,
            &finishing(),
            vec![ghost.clone()],
            &ProposalModels::default(),
        );
        assert_eq!((board.revision, board.done_since_last_scan), (7, 2));
        assert_eq!(board.ghosts, vec![ghost]);

        let [BoardCard::Drift { head, origin, view }] = board.backlog.as_slice() else {
            panic!("{:?}", board.backlog)
        };
        assert_eq!(view.refs, Vec::<String>::new());
        assert!(!view.prepared && view.resolve_reasons.is_empty() && view.attached.is_none());
        assert_eq!(
            (head.short_number.as_deref(), head.title.as_str()),
            (Some("D1"), "A drift")
        );
        assert_eq!(
            origin.as_ref().and_then(|o| o.short_number.as_deref()),
            Some("T1")
        );

        let [BoardCard::Task {
            head: first,
            blocked_by,
            delete_reasons,
            doing: false,
            ..
        }, BoardCard::Task {
            head: second,
            doing: true,
            delete_reasons: none,
            ..
        }] = board.active.as_slice()
        else {
            panic!("{:?}", board.active)
        };
        assert_eq!(
            (first.short_number.as_deref(), first.body.trim()),
            (Some("T1"), "Do the thing.")
        );
        assert!(first.agent_finishing && !second.agent_finishing);
        assert_eq!(
            blocked_by
                .iter()
                .map(|d| d.short_number.as_deref())
                .collect::<Vec<_>>(),
            [Some("D1")]
        );
        assert_eq!(
            delete_reasons,
            &[
                "The agent is still finishing".to_owned(),
                "Origin of open Drift D1".to_owned()
            ]
        );
        assert!(none.is_empty(), "a Task in progress has no Delete");
        assert_eq!(
            board.run_all_reasons,
            ["All the Tasks in To do are blocked"]
        );
    }

    #[test]
    fn the_wire_form_is_camel_case_tagged_by_kind() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "tasks/aaaaaaaa-first-task.md", TASK);
        let image = ProjectImage::load(dir.path());
        let board = snapshot(
            "0123456789abcdef",
            1,
            dir.path(),
            &image,
            &AgentFacts::default(),
            Vec::new(),
            &ProposalModels::default(),
        );
        let json = serde_json::to_value(&board).unwrap();
        let task = &json["active"][0];
        assert_eq!(task["kind"], "task");
        assert_eq!(task["head"]["shortNumber"], "T1");
        assert_eq!(task["head"]["agentFinishing"], false);
        assert_eq!(task["blockedBy"], serde_json::json!([]));
        assert_eq!(json["runAllReasons"], serde_json::json!([]));
        assert_eq!(json["doneSinceLastScan"], 0);
    }

    #[test]
    fn a_cited_task_that_stays_is_a_warning_of_the_board() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "tasks/aaaaaaaa-broken.md", "---\ncolumn: done\ntechnical: true\nrefs: []\ncreated: 2026-11-02T07:00:01.000Z\n---\n# Broken\n");
        write(
            dir.path(),
            "history/hhhhhhhh-entry.md",
            "---\ncreated: 2026-11-02T09:00:00.000Z\nupdated: 2026-11-02T09:00:00.000Z\nsummary: s\ntasks:\n  - aaaaaaaa\n---\n# Entry\n",
        );
        let image = ProjectImage::load(dir.path());
        let board = snapshot(
            "0123456789abcdef",
            1,
            dir.path(),
            &image,
            &AgentFacts::default(),
            Vec::new(),
            &ProposalModels::default(),
        );
        assert_eq!(board.warnings.len(), 1);
        assert!(
            board.warnings[0]
                .starts_with("Could not record aaaaaaaa in History: it has no `finished` instant."),
            "{}",
            board.warnings[0]
        );
        assert!(board.active.is_empty(), "a Done Task is never on the Board");
    }

    #[test]
    fn a_draft_the_chat_writes_is_a_card_that_is_finishing_while_a_turn_of_chat_works() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "proposals/pppppppp-idea/proposal.md",
            "---\nstatus: draft\nkind: change\nchat: true\nchatWriting: true\ncreated: 2026-11-02T07:00:00.000Z\n---\n# Idea\n\nWhy.\n",
        );
        write(
            dir.path(),
            "proposals/qqqqqqqq-other/proposal.md",
            "---\nstatus: draft\nkind: change\ncreated: 2026-11-02T07:00:01.000Z\n---\n# Other\n",
        );
        let image = ProjectImage::load(dir.path());
        let at = |facts: &AgentFacts| {
            snapshot(
                "0123456789abcdef",
                1,
                dir.path(),
                &image,
                facts,
                Vec::new(),
                &ProposalModels::default(),
            )
        };

        let idle = at(&AgentFacts::default());
        let [BoardCard::Proposal { head, standing, view, .. }] = idle.backlog.as_slice() else {
            panic!("{:?}", idle.backlog)
        };
        assert_eq!(head.id, "pppppppp");
        assert_eq!(*standing, ProposalStanding::ChatDraft);
        assert!(!head.agent_finishing && view.reject_reasons.is_empty());
        assert_eq!(
            serde_json::to_value(&idle).unwrap()["backlog"][0]["standing"],
            "chat-draft"
        );

        let turn = at(&AgentFacts {
            chat_working: true,
            ..AgentFacts::default()
        });
        let [BoardCard::Proposal { head, view, .. }] = turn.backlog.as_slice() else {
            panic!("{:?}", turn.backlog)
        };
        assert!(head.agent_finishing);
        assert_eq!(view.reject_reasons, ["The agent is still finishing"]);
    }
}
