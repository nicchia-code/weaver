//! The Proposals the Chat shows under the line of the job that handed them over
//! (`requirements/ui#general-chat`, **Proposal card**).
//!
//! The Board lists only the Proposals that wait for the developer; the Chat keeps showing a
//! Proposal after it was decided, for as long as it exists. So the Chat asks for its own list: the
//! Proposals whose `job:` is one of the jobs its conversation lists, in creation order, each with
//! what the card of the Board shows (worked out by [`crate::proposal_view`]) and, once decided,
//! what the one-line footer needs. The decisions go through the commands of the Board
//! (`approve_proposal`, `reject_proposal`, `discard_proposal`), which refuse them with their
//! reasons; nothing is decided here.

use std::path::Path;
use std::sync::Arc;

use serde::Serialize;
use tauri::State;
use ts_rs::TS;
use weaver_core::jobs::kinds::JobKind;
use weaver_core::project::board::AgentFacts;
use weaver_core::project::cards::{Card, CardKind};
use weaver_core::project::image::ProjectImage;
use weaver_core::project::proposal::data::{self, Status};
use weaver_core::project::proposal::files;
use weaver_core::project::proposal::models::ProposalModels;
use weaver_core::project::shortnum;

use crate::blocking;
use crate::board::{self, CardHead};
use crate::error::AppError;
use crate::jobs::Jobs;
use crate::proposal_view::{self, ProposalView};
use crate::state::AppState;

/// Where a Proposal stands, for its pill.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum ChatProposalStatus {
    /// A Draft a refinement left half-way (a Draft the agent is still writing has no card).
    Draft,
    Open,
    Refining,
    Approved,
    Rejected,
    Discarded,
}

impl ChatProposalStatus {
    fn of(status: Status) -> Self {
        match status {
            Status::Draft => Self::Draft,
            Status::Open => Self::Open,
            Status::Refining => Self::Refining,
            Status::Approved => Self::Approved,
            Status::Rejected => Self::Rejected,
            Status::Discarded => Self::Discarded,
        }
    }
}

/// One Proposal a job handed over.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ChatProposal {
    /// The identity of the job that handed it over.
    pub job: String,
    pub head: CardHead,
    pub status: ChatProposalStatus,
    /// What the card of the Board shows beyond its head: badges, files, Tasks and the reasons
    /// each control is disabled for.
    pub view: ProposalView,
    /// Approved: how many of the Tasks it created are on the Board.
    pub tasks_on_board: u32,
    /// Rejected with `Rework with feedback…`: the rework job, while the Jobs still hold it.
    pub rework_job: Option<String>,
    /// Rejected with `Rework with feedback…`.
    pub reworked: bool,
}

/// The Proposals of the jobs the Chat of a project started, at one revision of the picture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ChatProposals {
    pub project_id: String,
    /// The revision of the picture this was worked out from; an answer older than one already
    /// shown is dropped.
    #[ts(type = "number")]
    pub revision: u64,
    /// In creation order, oldest first.
    pub proposals: Vec<ChatProposal>,
}

/// Whether the Chat shows `card`: it was handed over (a Draft is shown only when a refinement left
/// it half-way) and no Drift holds it (a Proposal attached to a Drift is decided through the Drift).
fn shown(image: &ProjectImage, card: &Card) -> bool {
    let handed_over = match data::status_of(card) {
        Some(Status::Draft) => data::refining_flag(card),
        Some(_) => true,
        None => false,
    };
    handed_over && data::attached_drift(image, &card.id).is_none()
}

/// The Tasks of the Proposal that are in the Tasks folder (on the Board) now.
fn tasks_on_board(image: &ProjectImage, id: &str) -> u32 {
    let count = image
        .cards_of(CardKind::Task)
        .filter(|task| task.parent.is_none() && task.str_key("proposal") == Some(id))
        .count();
    u32::try_from(count).unwrap_or(u32::MAX)
}

/// The Proposals of `image` whose `job:` is one of `jobs`. `rework_job` tells the rework job of a
/// Proposal that was sent back.
#[allow(clippy::too_many_arguments)]
pub fn proposals_of(
    project_id: &str,
    revision: u64,
    root: &Path,
    image: &ProjectImage,
    facts: &AgentFacts,
    models: &ProposalModels,
    jobs: &[String],
    rework_job: &dyn Fn(&str) -> Option<String>,
) -> ChatProposals {
    let numbers = shortnum::peek(root, image.cards());
    let folders = files::read_folders(root, image);
    let mut cards: Vec<(&Card, &str)> = image
        .cards_of(CardKind::Proposal)
        .filter_map(|card| {
            let job = data::job_of(card)?;
            (jobs.iter().any(|j| j == job) && shown(image, card)).then_some((card, job))
        })
        .collect();
    cards.sort_by(|a, b| (&a.0.created, &a.0.id).cmp(&(&b.0.created, &b.0.id)));
    let proposals = cards
        .into_iter()
        .filter_map(|(card, job)| {
            let status = ChatProposalStatus::of(data::status_of(card)?);
            let view = proposal_view::view(image, &folders, &numbers, facts, models, &card.id)?;
            let reworked = status == ChatProposalStatus::Rejected && data::rework_requested(card);
            Some(ChatProposal {
                job: job.to_owned(),
                head: board::head(&numbers, facts, card),
                status,
                view,
                tasks_on_board: if status == ChatProposalStatus::Approved {
                    tasks_on_board(image, &card.id)
                } else {
                    0
                },
                rework_job: if reworked { rework_job(&card.id) } else { None },
                reworked,
            })
        })
        .collect();
    ChatProposals {
        project_id: project_id.to_owned(),
        revision,
        proposals,
    }
}

/// The latest rework job of the Proposal in the Jobs of the project.
fn rework_job_of(jobs: &Jobs, root: &Path, proposal: &str) -> Option<String> {
    jobs.list_for(root, true)
        .into_iter()
        .find(|record| record.kind == JobKind::Rework && record.target.as_deref() == Some(proposal))
        .map(|record| record.id)
}

/// The Proposals that the jobs listed in `jobs` handed over: what the Chat shows under their
/// `Started` lines.
#[tauri::command]
pub async fn general_proposals(
    state: State<'_, AppState>,
    registry: State<'_, Arc<Jobs>>,
    project_id: String,
    jobs: Vec<String>,
) -> Result<ChatProposals, AppError> {
    let picture = state.pictures.get(&project_id)?;
    let work = state.agent_work.get();
    let name = picture
        .root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned());
    let dirs = state.dirs.clone();
    let registry = registry.inner().clone();
    blocking::disk_named("chat proposals", name, move || {
        let facts = work.facts(&project_id);
        let mut models = ProposalModels::load(&dirs, &project_id);
        models.prune(&picture.image);
        proposals_of(
            &project_id,
            picture.revision,
            &picture.root,
            &picture.image,
            &facts,
            &models,
            &jobs,
            &|proposal| rework_job_of(&registry, &picture.root, proposal),
        )
    })
    .await
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    const ORDER: &str = "# Order\n\nAn order.\n\n## States\n\n### Draft (initial)\nChoosing.\n- `CONFIRM` → Paid\n\n### Paid\n\n## Events\n\n### CONFIRM\nConfirmed.\n";

    fn put(root: &Path, rel: &str, text: &str) {
        let path = root.join(".weaver").join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn hash(text: &str) -> String {
        weaver_core::project::text::sha256_hex(&weaver_core::project::text::file_content(text))
    }

    fn project() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        put(dir.path(), "model/system.md", "# Shop\n\nA shop.\n");
        put(dir.path(), "model/shop/feature.md", "# Shopping\n");
        put(dir.path(), "model/shop/order.md", ORDER);
        dir
    }

    /// A Proposal that changes the order, with the creation instant `created`.
    fn proposal(root: &Path, id: &str, front: &str, created: &str) {
        put(
            root,
            &format!("proposals/{id}-t/proposal.md"),
            &format!(
                "---\nbase:\n  model/shop/order.md: {}\n{front}created: {created}\n---\n# Pay {id}\n\nWhy.\n",
                hash(ORDER)
            ),
        );
        put(
            root,
            &format!("proposals/{id}-t/model/shop/order.md"),
            &ORDER.replace("### Paid\n", "### Paid\nPaid.\n"),
        );
    }

    fn listed(root: &Path, jobs: &[&str]) -> ChatProposals {
        let image = ProjectImage::load(root);
        let jobs: Vec<String> = jobs.iter().map(|j| (*j).to_owned()).collect();
        proposals_of(
            "0123456789abcdef",
            7,
            root,
            &image,
            &AgentFacts::default(),
            &ProposalModels::default(),
            &jobs,
            &|proposal| (proposal == "rrrrrrrr").then(|| "wwwwwwww".to_owned()),
        )
    }

    fn ids(list: &ChatProposals) -> Vec<&str> {
        list.proposals.iter().map(|p| p.head.id.as_str()).collect()
    }

    #[test]
    fn only_the_proposals_of_the_listed_jobs_show_in_creation_order() {
        let dir = project();
        proposal(
            dir.path(),
            "bbbbbbbb",
            "status: open\nkind: change\njob: jjjjjjjj\n",
            "2026-11-02T08:00:00.000Z",
        );
        proposal(
            dir.path(),
            "aaaaaaaa",
            "status: open\nkind: change\njob: jjjjjjjj\n",
            "2026-11-02T07:00:00.000Z",
        );
        proposal(
            dir.path(),
            "cccccccc",
            "status: open\nkind: change\njob: kkkkkkkk\n",
            "2026-11-02T09:00:00.000Z",
        );
        proposal(
            dir.path(),
            "dddddddd",
            "status: open\nkind: change\n",
            "2026-11-02T09:30:00.000Z",
        );
        let list = listed(dir.path(), &["jjjjjjjj"]);
        assert_eq!(ids(&list), ["aaaaaaaa", "bbbbbbbb"]);
        assert_eq!((list.revision, list.project_id.as_str()), (7, "0123456789abcdef"));
        assert!(list
            .proposals
            .iter()
            .all(|p| p.job == "jjjjjjjj" && p.status == ChatProposalStatus::Open));
        assert_eq!(list.proposals[0].view.files.changed, ["model/shop/order.md"]);
        assert_eq!(ids(&listed(dir.path(), &["jjjjjjjj", "kkkkkkkk"])).len(), 3);
        assert!(listed(dir.path(), &[]).proposals.is_empty());
    }

    #[test]
    fn a_draft_shows_only_when_a_refinement_left_it_half_way_and_one_attached_to_a_drift_never() {
        let dir = project();
        proposal(
            dir.path(),
            "aaaaaaaa",
            "status: draft\nkind: change\njob: jjjjjjjj\n",
            "2026-11-02T07:00:00.000Z",
        );
        proposal(
            dir.path(),
            "bbbbbbbb",
            "status: draft\nkind: change\nrefining: true\njob: jjjjjjjj\n",
            "2026-11-02T08:00:00.000Z",
        );
        proposal(
            dir.path(),
            "cccccccc",
            "status: open\nkind: change\njob: jjjjjjjj\n",
            "2026-11-02T09:00:00.000Z",
        );
        put(
            dir.path(),
            "drifts/dddddddd-d.md",
            "---\nstatus: open\nrefs:\n  - order/Paid\nproposal: cccccccc\ncreated: 2026-11-02T06:00:00.000Z\n---\n# The code ships twice\n\n## What happens\n\nIt ships twice.\n\n## What should happen\n\nIt ships once.\n\n## Where in the code\n\n`src/ship.rs`\n",
        );
        let list = listed(dir.path(), &["jjjjjjjj"]);
        assert_eq!(ids(&list), ["bbbbbbbb"]);
        assert_eq!(list.proposals[0].status, ChatProposalStatus::Draft);
    }

    #[test]
    fn a_decided_proposal_keeps_its_card_with_what_its_footer_says() {
        let dir = project();
        proposal(
            dir.path(),
            "pppppppp",
            "status: approved\nkind: change\njob: jjjjjjjj\n",
            "2026-11-02T07:00:00.000Z",
        );
        put(
            dir.path(),
            "tasks/tttttttt-pay.md",
            "---\ncolumn: todo\ntechnical: false\nrefs:\n  - order/Paid\nproposal: pppppppp\ncreated: 2026-11-02T07:00:01.000Z\n---\n# Pay\n\nDo it.\n",
        );
        proposal(
            dir.path(),
            "rrrrrrrr",
            "status: rejected\nkind: change\nreworkRequested: true\njob: jjjjjjjj\n",
            "2026-11-02T08:00:00.000Z",
        );
        proposal(
            dir.path(),
            "xxxxxxxx",
            "status: rejected\nkind: change\njob: jjjjjjjj\n",
            "2026-11-02T09:00:00.000Z",
        );
        proposal(
            dir.path(),
            "yyyyyyyy",
            "status: discarded\nkind: change\njob: jjjjjjjj\n",
            "2026-11-02T10:00:00.000Z",
        );
        let list = listed(dir.path(), &["jjjjjjjj"]);
        assert_eq!(
            ids(&list),
            ["pppppppp", "rrrrrrrr", "xxxxxxxx", "yyyyyyyy"]
        );
        let by = |id: &str| list.proposals.iter().find(|p| p.head.id == id).unwrap();
        assert_eq!(by("pppppppp").status, ChatProposalStatus::Approved);
        assert_eq!(by("pppppppp").tasks_on_board, 1);
        assert_eq!(by("rrrrrrrr").status, ChatProposalStatus::Rejected);
        assert!(by("rrrrrrrr").reworked);
        assert_eq!(by("rrrrrrrr").rework_job.as_deref(), Some("wwwwwwww"));
        assert!(!by("xxxxxxxx").reworked && by("xxxxxxxx").rework_job.is_none());
        assert_eq!(by("yyyyyyyy").status, ChatProposalStatus::Discarded);
    }

    #[test]
    fn the_wire_form_is_camel_case_with_kebab_case_statuses() {
        let dir = project();
        proposal(
            dir.path(),
            "aaaaaaaa",
            "status: open\nkind: change\njob: jjjjjjjj\n",
            "2026-11-02T07:00:00.000Z",
        );
        let json = serde_json::to_value(listed(dir.path(), &["jjjjjjjj"])).unwrap();
        assert_eq!(json["projectId"], "0123456789abcdef");
        let card = &json["proposals"][0];
        assert_eq!(card["status"], "open");
        assert_eq!(card["tasksOnBoard"], 0);
        assert_eq!(card["reworkJob"], serde_json::Value::Null);
        assert!(card["head"]["id"] == "aaaaaaaa" && card["view"]["files"].is_object());
    }
}
