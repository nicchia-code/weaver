//! Drifts (Machine `drift`): a difference between the code and the Model or the Requirements that
//! only the developer decides.
//!
//! The file is `drifts/<id>-<slug>.md` (`requirements/data#drift`). This module holds the rules as
//! pure functions over the project image: nothing here reads a clock, draws a random number or
//! touches the disk unless it is handed the means to.
//!
//! | Module | Responsibility |
//! |--------|----------------|
//! | [`fingerprint`] | The text fingerprint of a Drift's references: one function for dismissals and prepared questions |
//! | [`prepared`] | The prepared question: its block, its shape, whether it is still valid |
//! | [`create`] | `drift new` and `drift edit` |
//! | [`decide`] | The decisions of the developer: fix the code, accept the Proposal, dismiss |
//! | [`reasons`] | Why `Resolve` and `Dismiss Drift` are disabled; which Drifts a list shows |
//! | [`answer`] | What the answer to a prepared question becomes: the resolution Weaver writes |
//! | [`places`] | The files a Drift names under `## Where in the code` |

pub mod answer;
pub mod create;
pub mod decide;
pub mod fingerprint;
pub mod places;
pub mod prepared;
pub mod reasons;
#[cfg(test)]
mod tests;

use super::cards::Card;
pub use super::proposal::resolution::{drift_options, section_text, FixOption};

/// Keys of a Drift file, in the order `requirements/data#drift` lists them. `fingerprint` follows
/// them: a dismissal records the text fingerprint of the Drift's references there.
pub const DRIFT_KEYS: [&str; 13] = [
    "status",
    "refs",
    "origin",
    "previous",
    "proposal",
    "task",
    "option",
    "note",
    "job",
    "merge",
    "created",
    "decided",
    "fingerprint",
];

pub const HAPPENS: &str = "What happens";
pub const SHOULD: &str = "What should happen";
pub const FIX_OPTIONS: &str = "Fix options";
pub const WHERE_IN_CODE: &str = "Where in the code";
pub const PREPARED_QUESTION: &str = "Prepared question";

/// The `status` of a Drift.
pub fn status_of(drift: &Card) -> Option<&str> {
    drift.str_key("status")
}

/// The Drift is Open: it waits for the developer.
pub fn is_open(drift: &Card) -> bool {
    status_of(drift) == Some("open")
}

/// The Drift comes from a merge (`merge: true`): written only by `drift new`, never changed.
pub fn is_merge(drift: &Card) -> bool {
    drift.bool_key("merge") == Some(true)
}

/// What the Drift concerns, as written in its file.
pub fn refs_of(drift: &Card) -> Vec<String> {
    drift.list_key("refs").unwrap_or_default()
}

/// The body of the Drift after its title.
pub fn body_after_title(drift: &Card) -> String {
    super::tasks::body_after_title(drift)
}

/// The fix option the developer chose (from 1), once the code is to be fixed.
pub fn option_of(drift: &Card) -> Option<u32> {
    drift
        .front
        .get("option")
        .and_then(serde_yaml::Value::as_u64)
        .and_then(|n| u32::try_from(n).ok())
}
