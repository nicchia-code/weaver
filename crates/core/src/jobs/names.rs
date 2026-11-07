//! The fixed names of the jobs (`requirements/ui#job`), set at creation: `Setup`, `Design`,
//! `Import`, `Scan`, `Run T3` (short number, else permanent identity), `All tasks`,
//! `All tasks · <model>`, `Resolve D3`, `Rework P3`, `Refine P3`, `Merge conflicts`.

use crate::project::cards::CardKind;
use crate::project::shortnum::ShortNumbers;

/// `T3` for a live card with a number, else its permanent identity.
pub fn card_label(numbers: &ShortNumbers, kind: CardKind, id: &str) -> String {
    numbers.label_of(kind, id).unwrap_or_else(|| id.to_owned())
}

pub const SETUP: &str = "Setup";
pub const DESIGN: &str = "Design";
pub const IMPORT: &str = "Import";
pub const SCAN: &str = "Scan";
pub const MERGE: &str = "Merge conflicts";

/// The title of Weaver's own ghost of an import job (`requirements/ui#work`); the job's name stays
/// `Import`. The ghost of a setup or design job takes the title of the request the job received.
pub const IMPORT_GHOST_TITLE: &str = "Import from Code";

pub fn run_task(numbers: &ShortNumbers, task: &str) -> String {
    format!("Run {}", card_label(numbers, CardKind::Task, task))
}

/// The Run of all the runnable Tasks; `model` is told apart only when there are several groups.
/// A group on omp's own default shows `default model`.
pub fn run_all(model: Option<&str>) -> String {
    match model {
        None => "All tasks".to_owned(),
        Some(model) if model.trim().is_empty() => "All tasks · default model".to_owned(),
        Some(model) => format!("All tasks · {}", model.trim()),
    }
}

pub fn resolve(numbers: &ShortNumbers, drift: &str) -> String {
    format!("Resolve {}", card_label(numbers, CardKind::Drift, drift))
}

pub fn rework(numbers: &ShortNumbers, proposal: &str) -> String {
    format!(
        "Rework {}",
        card_label(numbers, CardKind::Proposal, proposal)
    )
}

pub fn refine(numbers: &ShortNumbers, proposal: &str) -> String {
    format!(
        "Refine {}",
        card_label(numbers, CardKind::Proposal, proposal)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::shortnum::{ShortCard, ShortNumbers};

    fn numbers() -> ShortNumbers {
        let card = |kind, id: &str, created: &str| ShortCard {
            kind,
            id: id.to_owned(),
            created: created.to_owned(),
            live: true,
        };
        ShortNumbers::assign(
            &[
                card(CardKind::Task, "aaaaaaaa", "2026-11-02T07:00:00.000Z"),
                card(CardKind::Drift, "dddddddd", "2026-11-02T07:00:01.000Z"),
                card(CardKind::Proposal, "pppppppp", "2026-11-02T07:00:02.000Z"),
            ],
            "",
        )
    }

    #[test]
    fn a_name_shows_the_short_number_and_else_the_permanent_identity() {
        let numbers = numbers();
        assert_eq!(run_task(&numbers, "aaaaaaaa"), "Run T1");
        assert_eq!(run_task(&numbers, "zzzzzzzz"), "Run zzzzzzzz");
        assert_eq!(resolve(&numbers, "dddddddd"), "Resolve D1");
        assert_eq!(rework(&numbers, "pppppppp"), "Rework P1");
        assert_eq!(refine(&numbers, "pppppppp"), "Refine P1");
    }

    #[test]
    fn run_all_names_the_model_only_when_groups_must_be_told_apart() {
        assert_eq!(run_all(None), "All tasks");
        assert_eq!(
            run_all(Some(" openai/gpt-5.2:high ")),
            "All tasks · openai/gpt-5.2:high"
        );
        assert_eq!(run_all(Some("")), "All tasks · default model");
    }
}
