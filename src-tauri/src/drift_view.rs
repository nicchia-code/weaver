//! What the card of a Drift and its details dialog show, worked out in the core and sent whole:
//! the two sides, the fix options, the files, the earlier Drift it repeats, the attached Proposal
//! and the reasons each control is disabled for, in the fixed order of the control
//! (`requirements/ui#drift`). The UI never decides anything: it shows these.

use serde::Serialize;
use ts_rs::TS;
use weaver_core::project::board::{
    AgentFacts, REASON_AGENT_FINISHING, REASON_BEING_REFINED, REASON_RUN_IN_PROGRESS,
};
use weaver_core::project::cards::{Card, CardKind};
use weaver_core::project::drift::places::places;
use weaver_core::project::drift::{self, prepared, reasons};
use weaver_core::project::image::ProjectImage;
use weaver_core::project::proposal::analysis;
use weaver_core::project::proposal::data::{self, Status};
use weaver_core::project::proposal::files::Folders;
use weaver_core::project::proposal::transitions::gate_of;
use weaver_core::project::shortnum::ShortNumbers;
use weaver_core::project::tasks;

use crate::board::TitledRef;
use crate::proposal_view::{
    ProposalControl, ProposalFiles, TOOLTIP_CONTINUE_ONLY, TOOLTIP_REFINE_ONLY,
};

/// The reason `Accept Proposal` lists while the attached Proposal is still a Draft.
pub const REASON_PROPOSAL_IS_DRAFT: &str = "The attached Proposal is still a Draft";

/// One way to fix the code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct FixOption {
    pub title: String,
    /// Markdown.
    pub text: String,
}

/// A file named under `Where in the code`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CodePlace {
    /// Relative to the project, or absolute.
    pub file: String,
    /// 1-based, when the text gives one.
    pub line: Option<u32>,
}

/// The Proposal attached to a Drift.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AttachedProposal {
    pub id: String,
    /// `P3`, when the Proposal is live and has a number.
    pub short_number: Option<String>,
    pub title: String,
    /// `Draft`, `Refinement interrupted` (a Draft a refinement left half-way), `Open` or
    /// `Refining`.
    pub state: String,
    /// Some file of the Proposal was written against a Model or Requirements that changed since
    /// (the `stale` badge).
    pub stale: bool,
    /// The Markdown of the Proposal after its title.
    pub body: String,
    pub files: ProposalFiles,
    /// Why `Accept Proposal` is disabled besides the agent still finishing the Drift, one per line
    /// in the fixed order; empty when the Proposal could be approved. `Being refined` while it is
    /// Refining.
    pub accept_reasons: Vec<String>,
    /// `Refine`, which replaces `Accept Proposal` when the Proposal is stale and Open (tooltip
    /// `Refine on the current Model`) or a Draft left half-way (tooltip `Continue refining`);
    /// its reasons are only what refuses a refinement. `None`: there is no `Refine`.
    pub refine: Option<ProposalControl>,
    /// `Discard`, next to `Refine` on a Draft left half-way: why it is disabled, one per line.
    /// `None`: there is no `Discard`.
    pub discard_reasons: Option<Vec<String>>,
}

/// The rest of the card of a Drift, beyond the head every card has.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DriftView {
    /// What it concerns: Model elements, Requirement sections and Specs fields.
    pub refs: Vec<String>,
    /// Markdown.
    pub happens: String,
    /// Markdown.
    pub should: String,
    /// The fix options, the recommended first; none when no change of the code can respect both
    /// the Model and the Requirements.
    pub options: Vec<FixOption>,
    /// The text of `Where in the code`, Markdown.
    pub where_in_code: String,
    /// The files that text names, one row each.
    pub places: Vec<CodePlace>,
    /// The Drift it repeats (`recurring`).
    pub previous: Option<TitledRef>,
    /// A resolve job works on it or a resolution Draft lists it (`resolving`).
    pub resolving: bool,
    /// A valid prepared question: `Resolve` asks it at once (`prepared`).
    pub prepared: bool,
    /// The Drift comes from a merge (`merge` badge, tooltip `Found by the scan after a merge`):
    /// `recommended` marks `Accept Proposal`, not the first fix option.
    pub merge: bool,
    /// Why `Resolve` is disabled, one per line in the fixed order; empty = it works.
    pub resolve_reasons: Vec<String>,
    /// Why `Dismiss Drift` is disabled, one per line in the fixed order; empty = it works.
    pub dismiss_reasons: Vec<String>,
    /// Why `Fix the code` is disabled besides the agent still finishing, one per line in the fixed
    /// order (`Being refined` while the attached Proposal is Refining); empty = it works.
    pub fix_reasons: Vec<String>,
    pub attached: Option<AttachedProposal>,
}

fn name_of(numbers: &ShortNumbers) -> impl Fn(CardKind, &str) -> String + '_ {
    move |kind, id| numbers.label_of(kind, id).unwrap_or_else(|| id.to_owned())
}

fn titled(image: &ProjectImage, numbers: &ShortNumbers, kind: CardKind, id: &str) -> TitledRef {
    TitledRef {
        id: id.to_owned(),
        short_number: numbers.label_of(kind, id),
        title: image
            .card(kind, id)
            .map(|c| c.title.clone())
            .unwrap_or_default(),
    }
}

fn attached(
    image: &ProjectImage,
    folders: &Folders,
    numbers: &ShortNumbers,
    facts: &AgentFacts,
    drift: &Card,
) -> Option<AttachedProposal> {
    let proposal = drift
        .str_key("proposal")
        .and_then(|id| image.card(CardKind::Proposal, id))?;
    let status = data::status_of(proposal)?;
    let analysed = analysis::analyse(image, folders, numbers, &proposal.id)?;
    let gate = gate_of(facts, proposal);
    let interrupted = status == Status::Draft && data::is_interrupted(proposal);
    let stale = !analysed.stale.is_empty();
    let mut accept_reasons: Vec<String> = Vec::new();
    match status {
        Status::Draft | Status::Open => {
            accept_reasons.extend(
                analysed
                    .approve_reasons(gate)
                    .iter()
                    .map(|r| r.text(numbers)),
            );
            if status == Status::Draft {
                accept_reasons.push(REASON_PROPOSAL_IS_DRAFT.to_owned());
            }
        }
        Status::Refining => {
            if gate.agent_finishing {
                accept_reasons.push(REASON_AGENT_FINISHING.to_owned());
            }
            if gate.run_in_progress {
                accept_reasons.push(REASON_RUN_IN_PROGRESS.to_owned());
            }
            accept_reasons.push(REASON_BEING_REFINED.to_owned());
            accept_reasons.extend(analysed.blocking().iter().map(|r| r.text(numbers)));
        }
        _ => return None,
    }
    // `Refine` replaces `Accept Proposal` on a stale Open Proposal and on a Draft a refinement left
    // half-way; only what refuses a refinement disables it (never a Run).
    let refine = ((status == Status::Open && stale) || interrupted).then(|| ProposalControl {
        reasons: analysed
            .refinement_refusals(gate)
            .into_iter()
            .map(|r| r.text().to_owned())
            .collect(),
        tooltip: Some(
            if interrupted {
                TOOLTIP_CONTINUE_ONLY
            } else {
                TOOLTIP_REFINE_ONLY
            }
            .to_owned(),
        ),
        refines: true,
    });
    // `Discard` sits next to `Continue refining`, as on the Proposal card.
    let discard_reasons: Option<Vec<String>> = interrupted.then(|| {
        gate.agent_finishing
            .then(|| REASON_AGENT_FINISHING.to_owned())
            .into_iter()
            .collect()
    });
    Some(AttachedProposal {
        id: proposal.id.clone(),
        short_number: numbers.label_of(CardKind::Proposal, &proposal.id),
        title: proposal.title.clone(),
        state: match status {
            Status::Draft if interrupted => "Refinement interrupted",
            Status::Draft => "Draft",
            Status::Refining => "Refining",
            _ => "Open",
        }
        .to_owned(),
        stale,
        body: tasks::body_after_title(proposal),
        files: ProposalFiles {
            added: analysed.files.added.clone(),
            changed: analysed.files.changed.clone(),
            removed: analysed.files.removed.clone(),
        },
        accept_reasons,
        refine,
        discard_reasons,
    })
}

/// The view of one Drift of the Board.
pub fn view(
    image: &ProjectImage,
    folders: &Folders,
    numbers: &ShortNumbers,
    facts: &AgentFacts,
    drift: &Card,
) -> DriftView {
    let where_in_code = drift::section_text(&drift.body, drift::WHERE_IN_CODE);
    let name = name_of(numbers);
    DriftView {
        refs: drift::refs_of(drift),
        happens: drift::section_text(&drift.body, drift::HAPPENS),
        should: drift::section_text(&drift.body, drift::SHOULD),
        options: drift::drift_options(drift)
            .into_iter()
            .map(|o| FixOption {
                title: o.title,
                text: o.text,
            })
            .collect(),
        places: places(&where_in_code)
            .into_iter()
            .map(|p| CodePlace {
                file: p.file,
                line: p.line,
            })
            .collect(),
        where_in_code,
        previous: drift
            .str_key("previous")
            .map(|id| titled(image, numbers, CardKind::Drift, id)),
        resolving: reasons::is_resolving(image, facts, drift),
        prepared: prepared::check(image, drift).is_valid(),
        merge: drift::is_merge(drift),
        resolve_reasons: reasons::resolve_reasons(image, facts, &name, drift),
        dismiss_reasons: reasons::dismiss_reasons(image, facts, drift),
        fix_reasons: reasons::fix_reasons(image, drift),
        attached: attached(image, folders, numbers, facts, drift),
    }
}

/// The Task that found a Drift, with its title.
pub fn origin_of(image: &ProjectImage, numbers: &ShortNumbers, drift: &Card) -> Option<TitledRef> {
    drift
        .str_key("origin")
        .map(|task| titled(image, numbers, CardKind::Task, task))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use weaver_core::project::shortnum;

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

    /// An Open Drift with the Proposal `pppppppp` attached: `front` is its status and flags, and a
    /// stale one started from another text of `order`.
    fn project(front: &str, stale: bool) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        put(root, "model/system.md", "# Shop\n\nA shop.\n");
        put(root, "model/shop/feature.md", "# Shopping\n");
        put(root, "model/shop/order.md", ORDER);
        put(
            root,
            "drifts/dddddddd-d.md",
            "---\nstatus: open\nrefs:\n  - order/Paid\nproposal: pppppppp\ncreated: 2026-11-02T06:00:00.000Z\n---\n# It ships twice\n\n## What happens\n\nTwice.\n\n## What should happen\n\nOnce.\n\n## Where in the code\n\n`src/ship.rs`\n",
        );
        let base = if stale {
            hash("another text")
        } else {
            hash(ORDER)
        };
        put(
            root,
            "proposals/pppppppp-t/proposal.md",
            &format!(
                "---\n{front}kind: change\njob: jjjjjjjj\nbase:\n  model/shop/order.md: {base}\ncreated: 2026-11-02T07:00:00.000Z\n---\n# Already paid\n\n## Already present\n\n- `order` — src/order.rs\n"
            ),
        );
        put(
            root,
            "proposals/pppppppp-t/model/shop/order.md",
            &ORDER.replace("### Paid\n", "### Paid\nPaid.\n"),
        );
        dir
    }

    fn view_of(root: &Path, facts: &AgentFacts) -> DriftView {
        let image = ProjectImage::load(root);
        let folders = weaver_core::project::proposal::files::read_folders(root, &image);
        let numbers = shortnum::peek(root, image.cards());
        let drift = image.card(CardKind::Drift, "dddddddd").unwrap();
        view(&image, &folders, &numbers, facts, drift)
    }

    fn finishing() -> AgentFacts {
        let mut facts = AgentFacts::default();
        facts.working_jobs.insert("jjjjjjjj".into());
        facts
    }

    #[test]
    fn a_stale_open_proposal_is_refined_only_the_agent_finishing_disables_it() {
        let dir = project("status: open\n", true);
        let attached = view_of(dir.path(), &AgentFacts::default())
            .attached
            .unwrap();
        assert_eq!((attached.state.as_str(), attached.stale), ("Open", true));
        assert_eq!(
            attached.refine,
            Some(ProposalControl {
                reasons: Vec::new(),
                tooltip: Some("Refine on the current Model".to_owned()),
                refines: true,
            })
        );
        assert_eq!(attached.discard_reasons, None);

        // A Run blocks Accept Proposal, never Refine.
        let run = AgentFacts {
            run_in_progress: true,
            ..AgentFacts::default()
        };
        let attached = view_of(dir.path(), &run).attached.unwrap();
        assert!(attached
            .accept_reasons
            .contains(&"A run is in progress".to_owned()));
        assert!(attached.refine.unwrap().reasons.is_empty());

        let attached = view_of(dir.path(), &finishing()).attached.unwrap();
        assert_eq!(
            attached.refine.unwrap().reasons,
            ["The agent is still finishing"]
        );
    }

    #[test]
    fn an_open_proposal_that_is_not_stale_is_accepted_not_refined() {
        let dir = project("status: open\n", false);
        let attached = view_of(dir.path(), &AgentFacts::default())
            .attached
            .unwrap();
        assert_eq!((attached.state.as_str(), attached.stale), ("Open", false));
        assert!(attached.refine.is_none() && attached.discard_reasons.is_none());
        assert_eq!(attached.accept_reasons, Vec::<String>::new());
    }

    #[test]
    fn a_draft_left_half_way_is_continued_or_discarded_and_a_plain_draft_is_neither() {
        let dir = project("status: draft\nrefining: true\n", false);
        let attached = view_of(dir.path(), &AgentFacts::default())
            .attached
            .unwrap();
        assert_eq!(attached.state, "Refinement interrupted");
        assert_eq!(
            attached.refine.as_ref().and_then(|c| c.tooltip.as_deref()),
            Some("Continue refining")
        );
        assert_eq!(attached.discard_reasons, Some(Vec::new()));
        let attached = view_of(dir.path(), &finishing()).attached.unwrap();
        assert_eq!(
            attached.refine.unwrap().reasons,
            ["The agent is still finishing"]
        );
        assert_eq!(
            attached.discard_reasons,
            Some(vec!["The agent is still finishing".to_owned()])
        );

        let dir = project("status: draft\n", false);
        let attached = view_of(dir.path(), &AgentFacts::default())
            .attached
            .unwrap();
        assert_eq!(attached.state, "Draft");
        assert!(attached.refine.is_none() && attached.discard_reasons.is_none());
        assert!(attached
            .accept_reasons
            .contains(&REASON_PROPOSAL_IS_DRAFT.to_owned()));
    }

    #[test]
    fn a_proposal_being_refined_is_shown_and_blocks_every_decision_with_being_refined() {
        let dir = project("status: refining\nrefining: true\n", true);
        let run = AgentFacts {
            run_in_progress: true,
            ..AgentFacts::default()
        };
        let view = view_of(dir.path(), &run);
        let attached = view.attached.as_ref().unwrap();
        assert_eq!(
            (attached.state.as_str(), attached.stale),
            ("Refining", true)
        );
        assert!(attached.refine.is_none() && attached.discard_reasons.is_none());
        assert_eq!(
            attached.accept_reasons[..2],
            ["A run is in progress", "Being refined"]
        );
        assert_eq!(view.fix_reasons, ["Being refined"]);
        assert_eq!(
            view.resolve_reasons,
            ["A run is in progress", "Being refined"]
        );
        assert_eq!(view.dismiss_reasons, ["Being refined"]);
    }
}
