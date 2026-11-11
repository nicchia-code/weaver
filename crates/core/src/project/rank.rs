//! The order of the two columns of the Board (`requirements/rules#work`): one function ranks both
//! at every snapshot, from the same facts as the cards' disabled reasons.
//!
//! In each column what the developer can act on comes first, then the ghosts of the jobs, then
//! what he cannot; inside the actionable and the not actionable cards, those that unblock other
//! cards come first; in Active the Tasks In progress follow the Tasks in To do that can run.
//! Inside each group the cards keep creation order. The result also says where the ghosts go.
//!
//! * **Actionable** is decided by the card's own reasons only; what stops every card at once (the
//!   agent still finishing, a Run, a merge, Drifts being resolved) never counts, so such a state
//!   moves nothing.
//! * **Unblocks**: another card waits on it now.
//! * Run all, the slots, the groups and `runnable` never use this order: they stay oldest first
//!   ([`super::board::compute`] gives the cards in creation order).

use std::collections::BTreeSet;

use super::board::{place_of, ActiveEntry, AgentFacts, BacklogEntry, Board, ProposalState};
use super::cards::{Card, CardKind};
use super::drift::reasons;
use super::image::ProjectImage;
use super::order::compare;
use super::proposal::analysis::{self, Gate};
use super::proposal::data::{self, Status};
use super::proposal::files::Folders;
use super::shortnum::ShortNumbers;

/// A request being written, as the order of the Backlog needs it. Requests are not part of the
/// image: the app reads them from the request store and passes them here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftFact {
    /// The id of the request, the local time of its creation.
    pub id: String,
    /// Nothing is left of its text after removing whitespace: `Send` is disabled.
    pub empty: bool,
}

/// A card of the Backlog in the Board order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RankedBacklog {
    Card(BacklogEntry),
    /// The draft of a request, by its id.
    Draft(String),
}

/// Both columns in the order the Board shows them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ranked {
    pub backlog: Vec<RankedBacklog>,
    /// How many cards of Backlog come before its ghosts.
    pub backlog_ghosts_at: usize,
    pub active: Vec<ActiveEntry>,
    /// How many cards of Active come before its ghosts.
    pub active_ghosts_at: usize,
}

/// The groups of Backlog, top to bottom; the ghosts go after [`BACKLOG_LAST_BEFORE_GHOSTS`].
const BACKLOG_ACTIONABLE_UNBLOCKS: u8 = 0;
const BACKLOG_ACTIONABLE: u8 = 1;
const BACKLOG_DRAFT: u8 = 2;
const BACKLOG_LAST_BEFORE_GHOSTS: u8 = BACKLOG_DRAFT;
const BACKLOG_BLOCKED_UNBLOCKS: u8 = 3;
const BACKLOG_BLOCKED: u8 = 4;

/// The groups of Active, top to bottom; the ghosts go after [`ACTIVE_LAST_BEFORE_GHOSTS`].
const ACTIVE_RUNNABLE_UNBLOCKS: u8 = 0;
const ACTIVE_RUNNABLE: u8 = 1;
const ACTIVE_DOING: u8 = 2;
const ACTIVE_LAST_BEFORE_GHOSTS: u8 = ACTIVE_DOING;
const ACTIVE_BLOCKED_UNBLOCKS: u8 = 3;
const ACTIVE_BLOCKED: u8 = 4;

/// A card with the group it is in and its place inside it.
struct Slot<T> {
    group: u8,
    /// `created` (a draft's request id) and identity.
    place: (String, String),
    item: T,
}

fn in_order<T>(mut slots: Vec<Slot<T>>) -> Vec<Slot<T>> {
    slots.sort_by(|a, b| {
        a.group.cmp(&b.group).then_with(|| {
            compare(&a.place.0, &a.place.1, &b.place.0, &b.place.1)
        })
    });
    slots
}

fn unblocking_group(unblocks: bool, actionable: bool, groups: [u8; 4]) -> u8 {
    match (actionable, unblocks) {
        (true, true) => groups[0],
        (true, false) => groups[1],
        (false, true) => groups[2],
        (false, false) => groups[3],
    }
}

/// Whether the primary control of the Proposal has none of its own reasons: `Approve` (reasons 3,
/// 5, 6 and 7), `Refine` on a stale Open one, `Refine` on a Draft a refinement left half-way;
/// never one that is being refined.
fn proposal_is_actionable(
    image: &ProjectImage,
    folders: &Folders,
    numbers: &ShortNumbers,
    state: ProposalState,
    id: &str,
) -> bool {
    match state {
        ProposalState::Refining => false,
        ProposalState::Interrupted => true,
        ProposalState::Open => analysis::analyse(image, folders, numbers, id).is_some_and(|a| {
            if a.stale.is_empty() {
                a.blocking().is_empty()
            } else {
                a.refinement_refusals(Gate::default()).is_empty()
            }
        }),
    }
}

/// Ranks `board`, whose cards are in creation order, into the order the Board shows.
///
/// `folders` are the files of the live Proposals, `drafts` the requests being written.
pub fn rank(
    board: &Board,
    image: &ProjectImage,
    facts: &AgentFacts,
    folders: &Folders,
    drafts: &[DraftFact],
) -> Ranked {
    let numbers = ShortNumbers::default();

    // What waits on what, now.
    let blocking_drifts: BTreeSet<&str> = board
        .active
        .iter()
        .flat_map(|entry| entry.blocked_by.iter().map(String::as_str))
        .collect();
    let awaited_fix_tasks: BTreeSet<&str> = board
        .active
        .iter()
        .flat_map(|entry| entry.waiting_for.iter().map(String::as_str))
        .collect();
    let required: BTreeSet<String> = image
        .cards_of(CardKind::Proposal)
        .filter(|p| {
            matches!(
                data::status_of(p),
                Some(Status::Draft | Status::Open | Status::Refining)
            )
        })
        .flat_map(|p| {
            data::requires_of(p)
                .into_iter()
                .filter(move |id| *id != p.id)
        })
        .collect();
    let open_drifts: Vec<&Card> = image
        .cards_of(CardKind::Drift)
        .filter(|d| d.str_key("status") == Some("open"))
        .collect();

    // --- Backlog ---
    let mut backlog: Vec<Slot<RankedBacklog>> = Vec::new();
    for entry in &board.backlog {
        let (card, actionable, unblocks) = match entry {
            BacklogEntry::Drift { id } => {
                let Some(card) = image.card(CardKind::Drift, id) else {
                    continue;
                };
                (
                    card,
                    reasons::resolve_is_actionable(image, facts, card),
                    blocking_drifts.contains(id.as_str()),
                )
            }
            BacklogEntry::Proposal { id, state, .. } => {
                let Some(card) = image.card(CardKind::Proposal, id) else {
                    continue;
                };
                (
                    card,
                    proposal_is_actionable(image, folders, &numbers, *state, id),
                    required.contains(id),
                )
            }
            BacklogEntry::Resolution { id, state, rows } => {
                let Some(card) = image.card(CardKind::Proposal, id) else {
                    continue;
                };
                // A resolution waiting for a decision stops `Resolve` of every open Drift it does
                // not list.
                let holds_back = *state != ProposalState::Refining
                    && open_drifts
                        .iter()
                        .any(|d| !rows.iter().any(|row| row.drift == d.id));
                (
                    card,
                    proposal_is_actionable(image, folders, &numbers, *state, id),
                    required.contains(id) || holds_back,
                )
            }
        };
        backlog.push(Slot {
            group: unblocking_group(
                unblocks,
                actionable,
                [
                    BACKLOG_ACTIONABLE_UNBLOCKS,
                    BACKLOG_ACTIONABLE,
                    BACKLOG_BLOCKED_UNBLOCKS,
                    BACKLOG_BLOCKED,
                ],
            ),
            place: place_of(image, card),
            item: RankedBacklog::Card(entry.clone()),
        });
    }
    for draft in drafts {
        backlog.push(Slot {
            group: if draft.empty {
                BACKLOG_BLOCKED
            } else {
                BACKLOG_DRAFT
            },
            place: (draft.id.clone(), draft.id.clone()),
            item: RankedBacklog::Draft(draft.id.clone()),
        });
    }
    let backlog = in_order(backlog);
    let backlog_ghosts_at = backlog
        .iter()
        .take_while(|slot| slot.group <= BACKLOG_LAST_BEFORE_GHOSTS)
        .count();

    // --- Active ---
    let active: Vec<Slot<ActiveEntry>> = board
        .active
        .iter()
        .map(|entry| {
            let created = image
                .card(CardKind::Task, &entry.id)
                .and_then(|task| task.created.clone())
                .unwrap_or_default();
            let group = if entry.doing {
                ACTIVE_DOING
            } else {
                unblocking_group(
                    awaited_fix_tasks.contains(entry.id.as_str()),
                    entry.runnable(),
                    [
                        ACTIVE_RUNNABLE_UNBLOCKS,
                        ACTIVE_RUNNABLE,
                        ACTIVE_BLOCKED_UNBLOCKS,
                        ACTIVE_BLOCKED,
                    ],
                )
            };
            Slot {
                group,
                place: (created, entry.id.clone()),
                item: entry.clone(),
            }
        })
        .collect();
    let active = in_order(active);
    let active_ghosts_at = active
        .iter()
        .take_while(|slot| slot.group <= ACTIVE_LAST_BEFORE_GHOSTS)
        .count();

    Ranked {
        backlog: backlog.into_iter().map(|slot| slot.item).collect(),
        backlog_ghosts_at,
        active: active.into_iter().map(|slot| slot.item).collect(),
        active_ghosts_at,
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::project::board::compute;
    use crate::project::model::{ORDER_MACHINE, USER_ACTOR};
    use crate::project::proposal::files::read_folders;
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

    fn ranked(dir: &tempfile::TempDir, facts: &AgentFacts, drafts: &[DraftFact]) -> Ranked {
        let image = ProjectImage::load(dir.path());
        let folders = read_folders(dir.path(), &image);
        rank(&compute(&image, facts), &image, facts, &folders, drafts)
    }

    fn backlog_ids(ranked: &Ranked) -> Vec<&str> {
        ranked
            .backlog
            .iter()
            .map(|item| match item {
                RankedBacklog::Card(entry) => entry.id(),
                RankedBacklog::Draft(id) => id.as_str(),
            })
            .collect()
    }

    fn active_ids(ranked: &Ranked) -> Vec<&str> {
        ranked.active.iter().map(|e| e.id.as_str()).collect()
    }

    fn draft(id: &str, empty: bool) -> DraftFact {
        DraftFact {
            id: id.to_owned(),
            empty,
        }
    }

    #[test]
    fn active_orders_the_runnable_tasks_that_unblock_then_the_runnable_then_the_doing_then_the_rest() {
        let dir = project();
        let root = dir.path();
        // An open Drift on `order/Paid` names bbbbbbbb as its origin and so blocks it; the fix
        // Task ffffffff is waited for by gggggggg, the origin of Drift hhhhhhhh.
        drift(root, "dddddddd", 9, "open", &["order/Paid"], "origin: bbbbbbbb\n");
        task(root, "aaaaaaaa", 1, "todo", &["user"], "");
        task(root, "bbbbbbbb", 2, "todo", &["order"], "");
        task(root, "cccccccc", 3, "doing", &["user"], "holder: jjjjjjjj\n");
        task(root, "eeeeeeee", 5, "todo", &["user"], "");
        task(root, "ffffffff", 6, "todo", &["user"], "drift: hhhhhhhh\n");
        drift(
            root,
            "hhhhhhhh",
            7,
            "code-to-be-fixed",
            &["user"],
            "origin: gggggggg\ntask: ffffffff\n",
        );
        task(root, "gggggggg", 8, "todo", &["user"], "");
        let ranked = ranked(&dir, &AgentFacts::default(), &[]);
        // ffffffff runs and unblocks gggggggg; aaaaaaaa and eeeeeeee run; cccccccc is doing; the
        // Drift blocks bbbbbbbb, and gggggggg waits for the fix Task.
        assert_eq!(
            active_ids(&ranked),
            ["ffffffff", "aaaaaaaa", "eeeeeeee", "cccccccc", "bbbbbbbb", "gggggggg"]
        );
        assert_eq!(ranked.active_ghosts_at, 4, "the ghosts go after the Tasks In progress");
    }

    #[test]
    fn a_state_that_stops_every_card_moves_none() {
        let dir = project();
        let root = dir.path();
        task(root, "aaaaaaaa", 1, "todo", &["user"], "job: jjjjjjjj\n");
        task(root, "bbbbbbbb", 2, "todo", &["order"], "");
        drift(root, "dddddddd", 3, "open", &["order"], "");
        let calm = ranked(&dir, &AgentFacts::default(), &[]);
        let busy = ranked(
            &dir,
            &AgentFacts {
                run_in_progress: true,
                merge_in_progress: true,
                working_jobs: ["jjjjjjjj".to_owned()].into(),
                ..AgentFacts::default()
            },
            &[],
        );
        assert_eq!(busy, calm);
    }

    #[test]
    fn backlog_puts_the_drafts_between_the_actionable_cards_and_the_ghosts() {
        let dir = project();
        let root = dir.path();
        proposal(root, "pppppppp", 1, "open", "");
        drift(root, "dddddddd", 2, "open", &["order"], "");
        proposal(root, "qqqqqqqq", 3, "refining", "");
        let drafts = [draft("2026-11-02T08:00:00", true), draft("2026-11-02T09:00:00", false)];
        let ranked = ranked(&dir, &AgentFacts::default(), &drafts);
        // Actionable: pppppppp, dddddddd; then the draft with text; the ghosts; then what cannot
        // be acted on: the Proposal being refined and the empty draft (in creation order).
        assert_eq!(
            backlog_ids(&ranked),
            [
                "pppppppp",
                "dddddddd",
                "2026-11-02T09:00:00",
                "qqqqqqqq",
                "2026-11-02T08:00:00"
            ]
        );
        assert_eq!(ranked.backlog_ghosts_at, 3);
    }

    #[test]
    fn a_resolution_awaiting_a_decision_unblocks_the_drifts_it_holds_back() {
        let dir = project();
        let root = dir.path();
        drift(root, "aaaaaaaa", 1, "open", &["order"], "");
        proposal(root, "pppppppp", 2, "open", "");
        drift(root, "dddddddd", 3, "open", &["user"], "");
        proposal(
            root,
            "rrrrrrrr",
            4,
            "open",
            "kind: resolution\ndrifts:\n  - drift: dddddddd\n    decision: dismissed\n",
        );
        let ranked = ranked(&dir, &AgentFacts::default(), &[]);
        // `aaaaaaaa` cannot be resolved while the resolution waits; the resolution unblocks it
        // and is actionable, so it comes first; the Proposal is actionable too.
        let ids = backlog_ids(&ranked);
        assert_eq!(ids, ["rrrrrrrr", "pppppppp", "aaaaaaaa"]);
        assert_eq!(ranked.backlog_ghosts_at, 2);
    }

    #[test]
    fn a_required_proposal_unblocks_and_a_proposal_being_refined_is_never_actionable() {
        let dir = project();
        let root = dir.path();
        proposal(root, "aaaaaaaa", 1, "open", "");
        proposal(root, "bbbbbbbb", 2, "open", "");
        proposal(root, "cccccccc", 3, "refining", "requires:\n  - bbbbbbbb\n");
        let ranked = ranked(&dir, &AgentFacts::default(), &[]);
        // bbbbbbbb is required by a Proposal being refined; aaaaaaaa is not required by anyone.
        let ids = backlog_ids(&ranked);
        assert_eq!(ids.last(), Some(&"cccccccc"));
        let position = |id: &str| ids.iter().position(|i| *i == id).unwrap();
        assert!(position("bbbbbbbb") < position("aaaaaaaa"));
    }

    #[test]
    fn a_proposal_that_waits_for_another_one_is_not_actionable_and_the_one_it_waits_for_goes_first() {
        let dir = project();
        let root = dir.path();
        proposal(root, "aaaaaaaa", 1, "open", "requires:\n  - bbbbbbbb\n");
        proposal(root, "bbbbbbbb", 2, "open", "");
        let ranked = ranked(&dir, &AgentFacts::default(), &[]);
        assert_eq!(backlog_ids(&ranked), ["bbbbbbbb", "aaaaaaaa"]);
        assert_eq!(ranked.backlog_ghosts_at, 1);
    }

    #[test]
    fn a_drift_whose_attached_proposal_is_being_refined_is_not_actionable() {
        let dir = project();
        let root = dir.path();
        proposal(root, "pppppppp", 1, "refining", "");
        drift(root, "aaaaaaaa", 2, "open", &["order"], "proposal: pppppppp\n");
        drift(root, "bbbbbbbb", 3, "open", &["user"], "");
        let ranked = ranked(&dir, &AgentFacts::default(), &[]);
        assert_eq!(backlog_ids(&ranked), ["bbbbbbbb", "aaaaaaaa"]);
        assert_eq!(ranked.backlog_ghosts_at, 1);
    }

    #[test]
    fn a_rework_takes_the_place_of_the_rejected_proposal_inside_its_group() {
        let dir = project();
        let root = dir.path();
        proposal(root, "aaaaaaaa", 2, "open", "");
        proposal(root, "bbbbbbbb", 1, "rejected", "");
        proposal(root, "cccccccc", 5, "open", "reworks: bbbbbbbb\n");
        let ranked = ranked(&dir, &AgentFacts::default(), &[]);
        assert_eq!(backlog_ids(&ranked), ["cccccccc", "aaaaaaaa"]);
    }
}
