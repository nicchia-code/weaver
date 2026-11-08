//! The file `proposal.md`: its keys, its status and kind, and how it is rewritten.

use std::collections::BTreeMap;

use serde_yaml::{Mapping, Value};

use crate::project::cards::{Card, CardKind};
use crate::project::frontmatter;
use crate::project::ids::card_stem;
use crate::project::image::ProjectImage;
use crate::project::layout::PROPOSALS_DIR;

/// Keys of `proposal.md`, in the order `requirements/data#proposal` lists them.
pub const KEYS: [&str; 13] = [
    "status",
    "kind",
    "requires",
    "base",
    "delete",
    "created",
    "request",
    "reworks",
    "reworkRequested",
    "label",
    "refining",
    "job",
    "drifts",
];

/// Keys of a Proposal of an earlier version that no longer have a meaning: ignored when read and
/// dropped the next time Weaver writes that `proposal.md`.
const LEGACY_KEYS: [&str; 2] = ["chat", "chatWriting"];

/// The folder of the Proposal's Task files, inside its folder.
pub const TASKS_FOLDER: &str = "tasks";

/// The heading of the section that lists what the code already implements.
pub const ALREADY_PRESENT: &str = "Already present";

/// The `base` value of a file that did not exist.
pub const NEW_FILE: &str = "new";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Draft,
    Open,
    Refining,
    Approved,
    Rejected,
    Discarded,
}

impl Status {
    pub fn parse(text: &str) -> Option<Status> {
        Some(match text {
            "draft" => Status::Draft,
            "open" => Status::Open,
            "refining" => Status::Refining,
            "approved" => Status::Approved,
            "rejected" => Status::Rejected,
            "discarded" => Status::Discarded,
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Status::Draft => "draft",
            Status::Open => "open",
            Status::Refining => "refining",
            Status::Approved => "approved",
            Status::Rejected => "rejected",
            Status::Discarded => "discarded",
        }
    }

    /// Approved, Rejected and Discarded never change again.
    pub fn is_final(self) -> bool {
        matches!(
            self,
            Status::Approved | Status::Rejected | Status::Discarded
        )
    }

    /// The Proposal is live: it has a short number and is not decided.
    pub fn is_live(self) -> bool {
        !self.is_final()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Change,
    Setup,
    Import,
    Resolution,
}

impl Kind {
    pub fn parse(text: &str) -> Option<Kind> {
        Some(match text {
            "change" => Kind::Change,
            "setup" => Kind::Setup,
            "import" => Kind::Import,
            "resolution" => Kind::Resolution,
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Change => "change",
            Kind::Setup => "setup",
            Kind::Import => "import",
            Kind::Resolution => "resolution",
        }
    }

    /// An ordinary change and a setup carry Tasks; an import and a resolution carry none.
    pub fn carries_tasks(self) -> bool {
        matches!(self, Kind::Change | Kind::Setup)
    }
}

/// The status of a Proposal card; `None` when the file holds none that is known.
pub fn status_of(card: &Card) -> Option<Status> {
    card.str_key("status").and_then(Status::parse)
}

/// The kind of a Proposal card: `change` when the file says nothing.
pub fn kind_of(card: &Card) -> Kind {
    card.str_key("kind")
        .and_then(Kind::parse)
        .unwrap_or(Kind::Change)
}

pub fn requires_of(card: &Card) -> Vec<String> {
    card.list_key("requires").unwrap_or_default()
}

pub fn deleted_of(card: &Card) -> Vec<String> {
    card.list_key("delete").unwrap_or_default()
}

/// `base`: path → hash of the content when first registered, or `new`.
pub fn base_of(card: &Card) -> BTreeMap<String, String> {
    match card.front.get("base") {
        Some(Value::Mapping(map)) => map
            .iter()
            .filter_map(|(k, v)| Some((k.as_str()?.to_owned(), v.as_str()?.to_owned())))
            .collect(),
        _ => BTreeMap::new(),
    }
}

pub fn label_of(card: &Card) -> Option<&str> {
    card.str_key("label")
}

pub fn request_of(card: &Card) -> Option<&str> {
    card.str_key("request")
}

pub fn reworks_of(card: &Card) -> Option<&str> {
    card.str_key("reworks")
}

pub fn job_of(card: &Card) -> Option<&str> {
    card.str_key("job")
}

/// Rejected with `Rework`.
pub fn rework_requested(card: &Card) -> bool {
    card.bool_key("reworkRequested") == Some(true)
}

/// Rejected with `Rework` and the rework has not been handed over yet: no Proposal that takes its
/// place was opened. A Proposal that requires it keeps waiting, as for one still being written.
pub fn rework_pending(image: &ProjectImage, card: &Card) -> bool {
    status_of(card) == Some(Status::Rejected)
        && rework_requested(card)
        && !image.cards_of(CardKind::Proposal).any(|other| {
            reworks_of(other) == Some(card.id.as_str())
                && matches!(
                    status_of(other),
                    Some(Status::Open | Status::Refining | Status::Approved | Status::Rejected)
                )
        })
}

/// `refining: true`: a refinement started on it and has not handed it over again.
pub fn refining_flag(card: &Card) -> bool {
    card.bool_key("refining") == Some(true)
}

/// A Draft a refinement left half-way.
pub fn is_interrupted(card: &Card) -> bool {
    status_of(card) == Some(Status::Draft) && refining_flag(card)
}

/// `proposals/<id>-<slug>`, relative to `.weaver/`.
pub fn folder_of(card: &Card) -> String {
    format!("{PROPOSALS_DIR}/{}", card_stem(&card.id, &card.slug))
}

/// The path of `rel` (a path under `.weaver/`) inside the Proposal's folder.
pub fn held_path(card: &Card, rel: &str) -> String {
    format!("{}/{rel}", folder_of(card))
}

/// The Drift this Proposal is attached to: a Proposal attached to a Drift is decided only
/// through its Drift.
pub fn attached_drift<'a>(image: &'a ProjectImage, proposal_id: &str) -> Option<&'a Card> {
    image
        .cards_of(CardKind::Drift)
        .find(|d| d.str_key("proposal") == Some(proposal_id))
}

/// The Tasks a Proposal carries, in creation order of their files (by identity).
pub fn tasks_of<'a>(image: &'a ProjectImage, proposal_id: &str) -> Vec<&'a Card> {
    let mut tasks: Vec<&Card> = image
        .cards_of(CardKind::ProposalTask)
        .filter(|t| t.parent.as_deref() == Some(proposal_id))
        .collect();
    tasks.sort_by(|a, b| a.path.cmp(&b.path));
    tasks
}

pub(crate) fn set(front: &mut Mapping, key: &str, value: impl Into<Value>) {
    front.insert(Value::String(key.to_owned()), value.into());
}

pub(crate) fn strings(list: &[String]) -> Value {
    Value::Sequence(list.iter().map(|s| Value::String(s.clone())).collect())
}

pub(crate) fn base_value(base: &BTreeMap<String, String>) -> Value {
    let mut map = Mapping::new();
    for (path, hash) in base {
        map.insert(Value::String(path.clone()), Value::String(hash.clone()));
    }
    Value::Mapping(map)
}

/// The text of `card` after an edit of its front matter; the body is kept. The keys a Proposal of
/// an earlier version may carry and no longer have a meaning ([`LEGACY_KEYS`]) are dropped.
pub fn rewritten(card: &Card, edit: impl FnOnce(&mut Mapping)) -> String {
    let mut front = card.front.clone();
    for key in LEGACY_KEYS {
        front.remove(key);
    }
    edit(&mut front);
    frontmatter::render(&front, &KEYS, &card.body)
}

/// The text of `card` with another status.
pub fn with_status(card: &Card, status: Status) -> String {
    rewritten(card, |front| set(front, "status", status.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::proposal::fixtures::proposal_card;

    #[test]
    fn status_and_kind_round_trip() {
        for status in [
            Status::Draft,
            Status::Open,
            Status::Refining,
            Status::Approved,
            Status::Rejected,
            Status::Discarded,
        ] {
            assert_eq!(Status::parse(status.as_str()), Some(status));
        }
        for kind in [Kind::Change, Kind::Setup, Kind::Import, Kind::Resolution] {
            assert_eq!(Kind::parse(kind.as_str()), Some(kind));
        }
        assert_eq!(Status::parse("done"), None);
        assert!(Status::Rejected.is_final() && !Status::Refining.is_final());
        assert!(
            Kind::Setup.carries_tasks()
                && !Kind::Import.carries_tasks()
                && !Kind::Resolution.carries_tasks()
        );
    }

    #[test]
    fn keys_are_read_and_the_front_matter_is_rewritten_in_order() {
        let card = proposal_card("---\nstatus: open\nkind: setup\nrequires:\n  - aaaaaaaa\nbase:\n  model/system.md: new\ncreated: 2026-11-02T07:00:00.000Z\nlabel: Shop\nextra: kept\n---\n# Title\n\nWhy.\n");
        assert_eq!(status_of(&card), Some(Status::Open));
        assert_eq!(kind_of(&card), Kind::Setup);
        assert_eq!(requires_of(&card), ["aaaaaaaa"]);
        assert_eq!(
            base_of(&card).get("model/system.md").map(String::as_str),
            Some("new")
        );
        assert_eq!(label_of(&card), Some("Shop"));
        let text = with_status(&card, Status::Approved);
        assert!(
            text.starts_with("---\nstatus: approved\nkind: setup\nrequires:"),
            "{text}"
        );
        assert!(
            text.contains("label: Shop\nextra: kept\n---\n# Title\n\nWhy.\n"),
            "{text}"
        );
    }

    #[test]
    fn the_keys_of_chat_are_dropped_the_next_time_the_file_is_written() {
        let card = proposal_card(
            "---\nstatus: draft\nchat: true\nchatWriting: true\ncreated: 2026-11-02T07:00:00.000Z\n---\n# T\n",
        );
        assert_eq!(status_of(&card), Some(Status::Draft));
        let text = with_status(&card, Status::Discarded);
        assert!(!text.contains("chat"), "{text}");
        assert!(
            text.starts_with("---\nstatus: discarded\ncreated:"),
            "{text}"
        );
    }

    #[test]
    fn a_draft_with_the_refining_flag_was_interrupted() {
        let card = proposal_card(
            "---\nstatus: draft\nrefining: true\ncreated: 2026-11-02T07:00:00.000Z\n---\n# T\n",
        );
        assert!(is_interrupted(&card));
        let other = proposal_card(
            "---\nstatus: refining\nrefining: true\ncreated: 2026-11-02T07:00:00.000Z\n---\n# T\n",
        );
        assert!(!is_interrupted(&other));
        assert_eq!(kind_of(&other), Kind::Change);
    }
}
