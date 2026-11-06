//! What an Ask more conversation is opened about, read from the files (and, for a question, from
//! the round in memory) at the moment of the request (`requirements/tech#ask-conversation`).
//!
//! The UI sends the kind and the identity of the chosen thing, never its text. [`build`] reads the
//! thing with `weaver-core` and returns the name of the conversation and its instruction, or
//! `None` when the thing no longer exists: a card or a History entry that is gone, or a question
//! of a round that closed or is not the one presented. The Requirements always open: the overlay
//! Proposal of the view is named only while it is still open, and no Requirement text and no skill
//! is read (the agent loads the `weaver-req-*` Requirement skills by itself). The same function
//! builds the instruction again when a lost session is replaced.

use std::path::Path;

use serde::Deserialize;
use ts_rs::TS;
use weaver_core::jobs::ask::{
    self, Chosen, DriftPart, HistoryEntryPart, HistoryTaskCopy, HistoryTaskPart, Overlay,
    ProposalPart, QuestionOrigin, QuestionPart, RequirementsPart, ResolvedDrift, TaskPart, Titled,
};
use weaver_core::project::board::{self, AgentFacts, BacklogEntry};
use weaver_core::project::cards::CardKind;
use weaver_core::project::drift;
use weaver_core::project::history;
use weaver_core::project::image::ProjectImage;
use weaver_core::project::proposal::data::{self as proposal_data, Status};
use weaver_core::project::shortnum::{self, ShortNumbers};
use weaver_core::project::tasks::{self, Column};
use weaver_core::project::view;
use weaver_core::question::Origin;

use crate::general::wire::AskKind;
use crate::rounds::Rounds;

/// What the developer asks about, as the UI names it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum AskTarget {
    /// A Proposal of the Board, by its permanent identity.
    Proposal { id: String },
    /// A Task of the Board.
    Task { id: String },
    /// A Drift of the Board.
    Drift { id: String },
    /// A History entry with the Tasks it cites, by the entry's permanent identity.
    History { entry: String },
    /// The Requirements as a whole, and the Proposal drawn over the view, when one is. Never a
    /// section, an Area or a focus.
    Requirements { overlay: Option<String> },
    /// The question in view of the presented round of the window.
    Question {
        #[ts(type = "number")]
        round: u64,
        #[ts(type = "number")]
        index: usize,
    },
}

/// What opens a conversation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Built {
    /// `Ask T3`, `Ask Requirements`, `Ask question`.
    pub name: String,
    pub kind: AskKind,
    /// The question round it was opened from.
    pub round: Option<u64>,
    pub instruction: String,
}

/// Where the thing is read from.
pub struct Sources<'a> {
    pub project_id: &'a str,
    pub root: &'a Path,
    pub rounds: &'a Rounds,
    /// The language of the developer, as the Settings hold it.
    pub language: Option<&'a str>,
}

struct Card<'a> {
    image: &'a ProjectImage,
    numbers: &'a ShortNumbers,
    root: &'a Path,
}

impl Card<'_> {
    fn short(&self, kind: CardKind, id: &str) -> Option<String> {
        self.numbers.label_of(kind, id)
    }

    fn titled(&self, kind: CardKind, id: &str) -> Titled {
        Titled {
            id: id.to_owned(),
            title: self
                .image
                .card(kind, id)
                .map(|card| card.title.clone())
                .unwrap_or_default(),
        }
    }

    fn name(&self, kind: CardKind, id: &str) -> String {
        format!(
            "Ask {}",
            self.short(kind, id).unwrap_or_else(|| id.to_owned())
        )
    }
}

/// Reads the thing and builds the instruction. `None` when the thing no longer exists.
pub fn build(sources: &Sources, target: &AskTarget) -> Option<Built> {
    let language = sources.language;
    match target {
        AskTarget::Question { round, index } => question(sources, *round, *index),
        AskTarget::Requirements { overlay } => Some(requirements(sources, overlay.as_deref())),
        other => {
            let image = ProjectImage::load(sources.root);
            let numbers = shortnum::peek(sources.root, image.cards());
            let card = Card {
                image: &image,
                numbers: &numbers,
                root: sources.root,
            };
            let (name, kind, chosen) = match other {
                AskTarget::Proposal { id } => (
                    card.name(CardKind::Proposal, id),
                    AskKind::Proposal,
                    Chosen::Proposal(proposal(&card, id)?),
                ),
                AskTarget::Task { id } => (
                    card.name(CardKind::Task, id),
                    AskKind::Task,
                    Chosen::Task(task(&card, id)?),
                ),
                AskTarget::Drift { id } => (
                    card.name(CardKind::Drift, id),
                    AskKind::Drift,
                    Chosen::Drift(drift_card(&card, id)?),
                ),
                AskTarget::History { entry } => (
                    format!("Ask history {entry}"),
                    AskKind::History,
                    Chosen::History(history_entry(&card, entry)?),
                ),
                AskTarget::Question { .. } | AskTarget::Requirements { .. } => return None,
            };
            Some(Built {
                name,
                kind,
                round: None,
                instruction: ask::instruction(&chosen, language),
            })
        }
    }
}

fn task(card: &Card, id: &str) -> Option<TaskPart> {
    let task = card.image.card(CardKind::Task, id)?;
    if !matches!(tasks::column_of(task), Some(Column::Todo | Column::Doing)) {
        return None;
    }
    Some(TaskPart {
        id: id.to_owned(),
        short: card.short(CardKind::Task, id),
        title: task.title.clone(),
        body: tasks::body_after_title(task),
        refs: tasks::refs_of(task),
        images: board::task_image_files(card.root, id)
            .into_iter()
            .map(|path| format!(".weaver/{path}"))
            .collect(),
        proposal: task
            .str_key("proposal")
            .map(|proposal| card.titled(CardKind::Proposal, proposal)),
        drift: task.str_key("drift").map(str::to_owned),
    })
}

fn proposal(card: &Card, id: &str) -> Option<ProposalPart> {
    let proposal = card.image.card(CardKind::Proposal, id)?;
    let status = proposal_data::status_of(proposal).filter(|status| status.is_live())?;
    let tasks = proposal_data::tasks_of(card.image, id)
        .into_iter()
        .map(|task| Titled {
            id: task.id.clone(),
            title: task.title.clone(),
        })
        .collect();
    // The Drifts a resolution gathers, with what was decided for each.
    let computed = board::compute(card.image, &AgentFacts::default());
    let drifts = computed
        .backlog
        .iter()
        .find_map(|entry| match entry {
            BacklogEntry::Resolution {
                id: found, rows, ..
            } if found == id => Some(rows),
            _ => None,
        })
        .map(|rows| {
            rows.iter()
                .map(|row| ResolvedDrift {
                    id: row.drift.clone(),
                    decision: decision_text(row),
                })
                .collect()
        })
        .unwrap_or_default();
    Some(ProposalPart {
        id: id.to_owned(),
        short: card.short(CardKind::Proposal, id),
        title: proposal.title.clone(),
        state: state_text(status).to_owned(),
        body: tasks::body_after_title(proposal),
        tasks,
        drifts,
    })
}

fn state_text(status: Status) -> &'static str {
    status.as_str()
}

/// `fix-code (fix option 2): the note`, `proposal`, `dismissed`.
fn decision_text(row: &board::ResolutionRow) -> String {
    let mut text = row.decision.clone();
    if let Some(option) = row.option {
        text.push_str(&format!(" (fix option {option})"));
    }
    if let Some(note) = row.note.as_deref().filter(|note| !note.trim().is_empty()) {
        text.push_str(&format!(": {}", note.trim()));
    }
    text
}

fn drift_card(card: &Card, id: &str) -> Option<DriftPart> {
    let found = card.image.card(CardKind::Drift, id)?;
    if !drift::is_open(found) {
        return None;
    }
    Some(DriftPart {
        id: id.to_owned(),
        short: card.short(CardKind::Drift, id),
        title: found.title.clone(),
        what_happens: drift::section_text(&found.body, drift::HAPPENS),
        what_should_happen: drift::section_text(&found.body, drift::SHOULD),
        refs: drift::refs_of(found),
        proposal: found
            .str_key("proposal")
            .filter(|proposal| card.image.card(CardKind::Proposal, proposal).is_some())
            .map(|proposal| card.titled(CardKind::Proposal, proposal)),
        task: found.str_key("origin").map(str::to_owned),
        repeats: found.str_key("previous").map(str::to_owned),
    })
}

fn history_entry(card: &Card, entry_id: &str) -> Option<HistoryEntryPart> {
    let entry = history::entries(card.image)
        .into_iter()
        .find(|entry| entry.id == entry_id)?;
    // A cited Task still a file (its copy could not be made yet) is read from the file, as the
    // History view does; one that cannot be read is kept, without a copy.
    let tasks = entry
        .tasks
        .iter()
        .map(|id| HistoryTaskPart {
            id: id.clone(),
            copy: history::shown_copy(card.image, &entry, id).map(|copy| {
                let proposal_exists = copy.proposal.as_deref().is_some_and(|proposal| {
                    card.image.card(CardKind::Proposal, proposal).is_some()
                });
                HistoryTaskCopy {
                    title: copy.title,
                    body: copy.body,
                    refs: copy.refs,
                    proposal: copy.proposal,
                    proposal_exists,
                    label: copy.label,
                    finished: copy.finished,
                }
            }),
        })
        .collect();
    Some(HistoryEntryPart {
        id: entry.id,
        title: entry.title,
        summary: entry.summary,
        text: entry.text,
        tasks,
    })
}

fn question(sources: &Sources, round: u64, index: usize) -> Option<Built> {
    let seen = sources
        .rounds
        .question_in_view(sources.project_id, round, index)?;
    let origin = match &seen.origin {
        Origin::Agent { job } => {
            let name = seen
                .job
                .as_ref()
                .and_then(|(_, name)| name.clone())
                .unwrap_or_else(|| job.clone());
            QuestionOrigin::Job {
                name,
                id: job.clone(),
            }
        }
        Origin::Chat { .. } => QuestionOrigin::Chat,
        Origin::Drift { id, .. } => QuestionOrigin::Drift { id: id.clone() },
    };
    let chosen = Chosen::Question(QuestionPart {
        request: &seen.request,
        index,
        origin,
    });
    Some(Built {
        name: "Ask question".to_owned(),
        kind: AskKind::Question,
        round: Some(round),
        instruction: ask::instruction(&chosen, sources.language),
    })
}

/// The Requirements as a whole. Only the overlay Proposal is read, and only to see that it is
/// still one the overlay picker offers (open and not attached to a Drift); when it is not, it is
/// left out and the conversation opens all the same. No Requirement text and no skill is read.
fn requirements(sources: &Sources, overlay: Option<&str>) -> Built {
    let overlay = overlay.and_then(|id| {
        let image = ProjectImage::load_cards(sources.root);
        let numbers = shortnum::peek(sources.root, image.cards());
        let option = view::eligible_overlays(&image, &numbers)
            .into_iter()
            .find(|option| option.id == id)?;
        Some(Overlay {
            id: option.id,
            short: option.short_number,
            title: option.title,
        })
    });
    let chosen = Chosen::Requirements(RequirementsPart { overlay });
    Built {
        name: "Ask Requirements".to_owned(),
        kind: AskKind::Requirements,
        round: None,
        instruction: ask::instruction(&chosen, sources.language),
    }
}
