//! Why the controls of a Drift are disabled (`requirements/ui#drift`), what "resolving" means,
//! and which Drifts a listing shows.
//!
//! Reasons come from the project and from the facts about the agent's work at the moment they are
//! asked, are listed all, in the fixed order of the control, and are checked again when the
//! decision is applied.

use crate::project::board::{
    is_being_written, resolution_rows, AgentFacts, REASON_AGENT_FINISHING, REASON_BEING_REFINED,
    REASON_MERGE_IN_PROGRESS, REASON_RUN_IN_PROGRESS,
};
use crate::project::cards::{Card, CardKind};
use crate::project::concerns::{key_lists_concern, key_of};
use crate::project::image::ProjectImage;
use crate::project::proposal::data::{self, Kind, Status};
use crate::project::refs::parse_reference;

use super::prepared::{self, Preparation};
use super::{fingerprint, is_open, refs_of, status_of};

pub const REASON_BEING_RESOLVED: &str = "This Drift is being resolved";
pub const REASON_ANOTHER_BEING_WRITTEN: &str = "Another resolution is being written";
pub const REASON_IN_RESOLUTION: &str = "This Drift is in a resolution";

/// The resolutions that list `drift`: being written or refined, or waiting for a decision.
pub fn resolutions_listing<'a>(image: &'a ProjectImage, drift: &str) -> Vec<&'a Card> {
    resolutions(image)
        .filter(|resolution| {
            resolution_rows(resolution)
                .iter()
                .any(|row| row.drift == drift)
        })
        .collect()
}

/// Every resolution being written (a Draft no refinement left half-way, or Refining) or waiting
/// for a decision (Open, or a Draft a refinement left half-way).
fn resolutions(image: &ProjectImage) -> impl Iterator<Item = &Card> {
    image.cards_of(CardKind::Proposal).filter(|p| {
        data::kind_of(p) == Kind::Resolution
            && matches!(
                data::status_of(p),
                Some(Status::Draft | Status::Open | Status::Refining)
            )
    })
}

/// The Drift is in a resolution being written, being refined or waiting for a decision.
pub fn in_resolution(image: &ProjectImage, drift: &str) -> bool {
    !resolutions_listing(image, drift).is_empty()
}

/// The `resolving` badge: a resolve job works on the Drift, or a resolution being written or
/// refined lists it.
pub fn is_resolving(image: &ProjectImage, facts: &AgentFacts, drift: &Card) -> bool {
    facts.resolving_drifts.contains(&drift.id)
        || resolutions_listing(image, &drift.id)
            .iter()
            .any(|r| is_being_written(r))
}

/// The Proposal attached to `drift` is being refined (Refining).
pub fn attached_being_refined(image: &ProjectImage, drift: &Card) -> bool {
    drift
        .str_key("proposal")
        .and_then(|id| image.card(CardKind::Proposal, id))
        .is_some_and(|p| data::status_of(p) == Some(Status::Refining))
}

/// Why `Resolve` is disabled, in order: `The agent is still finishing`, `A run is in progress`,
/// `A merge is in progress`, `Being refined` (its attached Proposal is Refining), `This Drift is being resolved`,
/// `Another resolution is being written`, and one `Proposal <id> resolving Drifts awaits a
/// decision` for each resolution waiting for a decision. `name` shows a Proposal (`P3`, or its
/// permanent id).
pub fn resolve_reasons(
    image: &ProjectImage,
    facts: &AgentFacts,
    name: &dyn Fn(CardKind, &str) -> String,
    drift: &Card,
) -> Vec<String> {
    let mut reasons = Vec::new();
    if facts.is_finishing(drift) {
        reasons.push(REASON_AGENT_FINISHING.to_owned());
    }
    if facts.run_in_progress {
        reasons.push(REASON_RUN_IN_PROGRESS.to_owned());
    }
    if facts.merge_in_progress {
        reasons.push(REASON_MERGE_IN_PROGRESS.to_owned());
    }
    if attached_being_refined(image, drift) {
        reasons.push(REASON_BEING_REFINED.to_owned());
    }
    if is_resolving(image, facts, drift) {
        reasons.push(REASON_BEING_RESOLVED.to_owned());
    }
    let listing: Vec<&str> = resolutions_listing(image, &drift.id)
        .iter()
        .map(|r| r.id.as_str())
        .collect();
    if resolutions(image).any(|r| is_being_written(r) && !listing.contains(&r.id.as_str())) {
        reasons.push(REASON_ANOTHER_BEING_WRITTEN.to_owned());
    }
    for resolution in resolutions(image).filter(|r| !is_being_written(r)) {
        reasons.push(format!(
            "Proposal {} resolving Drifts awaits a decision",
            name(CardKind::Proposal, &resolution.id)
        ));
    }
    reasons
}

/// Why `Dismiss Drift` is disabled, in order: `The agent is still finishing`, `Being refined` (its
/// attached Proposal is Refining), `This Drift is in a resolution`.
pub fn dismiss_reasons(image: &ProjectImage, facts: &AgentFacts, drift: &Card) -> Vec<String> {
    let mut reasons = Vec::new();
    if facts.is_finishing(drift) {
        reasons.push(REASON_AGENT_FINISHING.to_owned());
    }
    if attached_being_refined(image, drift) {
        reasons.push(REASON_BEING_REFINED.to_owned());
    }
    if in_resolution(image, &drift.id) {
        reasons.push(REASON_IN_RESOLUTION.to_owned());
    }
    reasons
}

/// Why `Fix the code` is disabled besides the agent still finishing: `Being refined` (its attached
/// Proposal is Refining).
pub fn fix_reasons(image: &ProjectImage, drift: &Card) -> Vec<String> {
    let mut reasons = Vec::new();
    if attached_being_refined(image, drift) {
        reasons.push(REASON_BEING_REFINED.to_owned());
    }
    reasons
}

/// `weaver drift list {unprepared}`: the open Drifts with no valid prepared question that are in no
/// resolution being written or open.
pub fn is_unprepared(image: &ProjectImage, drift: &Card) -> bool {
    is_open(drift)
        && !matches!(prepared::check(image, drift), Preparation::Valid(_))
        && !in_resolution(image, &drift.id)
}

/// `weaver drift list {refs}`: whether `drift` concerns the same thing as any of `refs`, the way a
/// Drift blocks a Task.
pub fn concerns(image: &ProjectImage, drift: &Card, refs: &[String]) -> bool {
    let wanted: Vec<_> = refs
        .iter()
        .filter_map(|text| parse_reference(text).ok())
        .filter_map(|r| key_of(&r))
        .collect();
    let own: Vec<_> = refs_of(drift)
        .iter()
        .filter_map(|text| parse_reference(text).ok())
        .filter_map(|r| key_of(&r))
        .collect();
    key_lists_concern(image.model(), &wanted, &own)
}

/// A Dismissed Drift whose fingerprint is still the current one: the difference it recorded is
/// still there, decided, and the agent never files it again.
pub fn dismissal_stands(image: &ProjectImage, drift: &Card) -> bool {
    status_of(drift) == Some("dismissed")
        && drift
            .str_key("fingerprint")
            .is_some_and(|recorded| recorded == fingerprint::stored_of(image, drift))
}
