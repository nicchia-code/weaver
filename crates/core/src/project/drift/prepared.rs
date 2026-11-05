//! The prepared question of a Drift (`requirements/rules#drift`, `requirements/data#drift`).
//!
//! Before its job ends, the agent that filed a Drift records the question with which the developer
//! will decide it. It lives in the Drift's file, in `## Prepared question`, as exactly one fenced
//! `json` block `{question, decisions, fingerprint, prepared}`:
//!
//! * `question` is one single-choice question of `weaver question new`, its `id` the identity of the
//!   Drift;
//! * `decisions` has one `{decision, option?}` per option of the question, in the same order: every
//!   fix option once as `fix-code`, then one `proposal` or `dismissed` for the behaviour the code
//!   has (a Drift without fix options has exactly `proposal`, the recommended one, and
//!   `dismissed`);
//! * `fingerprint` is the text fingerprint of the Drift's references when it was prepared;
//! * `prepared` is the instant.
//!
//! It is *valid* while the Drift is Open, the block reads, the fingerprint equals the current one,
//! every `fix-code` names a fix option that exists and a `proposal` decision has a Proposal in
//! Draft or Open attached. A block that does not read counts as no question and is reported.

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::project::cards::{Card, CardKind};
use crate::project::image::ProjectImage;
use crate::project::proposal::data::{self, Status};
use crate::question::{self, Question};

use super::fingerprint;
use super::{
    drift_options, is_merge, refs_of, section_text, status_of, FixOption, PREPARED_QUESTION,
};

pub const FIX_CODE: &str = "fix-code";
pub const PROPOSAL: &str = "proposal";
pub const DISMISSED: &str = "dismissed";

/// What one option of the question decides.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreparedDecision {
    /// `fix-code`, `proposal` or `dismissed`.
    pub decision: String,
    /// The fix option, from 1; only with `fix-code`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub option: Option<u32>,
}

/// The block of `## Prepared question`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Prepared {
    pub question: Question,
    pub decisions: Vec<PreparedDecision>,
    /// `sha256:<hex>` of the text fingerprint of the Drift's references at preparation.
    pub fingerprint: String,
    /// The instant of the preparation.
    pub prepared: String,
}

/// What the Drift holds and whether it may be asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Preparation {
    /// No block.
    Missing,
    /// A block that does not read: it counts as no prepared question.
    Unreadable(String),
    /// A block that reads but no longer holds (the Model or the Requirements changed, a fix option
    /// or an attached Proposal is gone, the Drift is no longer open); never changed by itself.
    Invalid(Box<Prepared>, String),
    Valid(Box<Prepared>),
}

impl Preparation {
    pub fn is_valid(&self) -> bool {
        matches!(self, Preparation::Valid(_))
    }

    /// The question, when it is valid.
    pub fn valid(&self) -> Option<&Prepared> {
        match self {
            Preparation::Valid(prepared) => Some(prepared),
            _ => None,
        }
    }

    /// Why a block that exists cannot be asked: a block that does not read, or one that no longer
    /// holds.
    pub fn problem(&self) -> Option<&str> {
        match self {
            Preparation::Unreadable(why) | Preparation::Invalid(_, why) => Some(why),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The block in the file
// ---------------------------------------------------------------------------------------------

/// The text of the fenced blocks of `section`, with their info strings.
fn fenced_blocks(section: &str) -> Vec<(String, String)> {
    let mut blocks = Vec::new();
    let mut current: Option<(String, String)> = None;
    for line in section.lines() {
        let trimmed = line.trim_start();
        match &mut current {
            None => {
                if let Some(info) = trimmed.strip_prefix("```") {
                    current = Some((info.trim().to_owned(), String::new()));
                }
            }
            Some((_, content)) => {
                if trimmed.trim_end() == "```" {
                    blocks.push(current.take().expect("a block is open"));
                } else {
                    content.push_str(line);
                    content.push('\n');
                }
            }
        }
    }
    blocks
}

/// Reads the block of the Drift: `Ok(None)` when it has none, `Err` with the reason when the
/// section is there but does not hold exactly one block that reads.
pub fn read_block(drift: &Card) -> Result<Option<Prepared>, String> {
    let has_section = crate::project::md::outline(&drift.body)
        .headings
        .iter()
        .any(|h| h.level == 2 && h.text == PREPARED_QUESTION);
    if !has_section {
        return Ok(None);
    }
    let section = section_text(&drift.body, PREPARED_QUESTION);
    let blocks: Vec<(String, String)> = fenced_blocks(&section)
        .into_iter()
        .filter(|(info, _)| info == "json")
        .collect();
    match blocks.as_slice() {
        [] => Err("`## Prepared question` holds no `json` block".to_owned()),
        [(_, content)] => serde_json::from_str::<Prepared>(content)
            .map(Some)
            .map_err(|e| format!("the `json` block of `## Prepared question` does not read: {e}")),
        _ => Err("`## Prepared question` holds more than one `json` block".to_owned()),
    }
}

/// The text of the section for `prepared`, heading included, ending with a newline.
pub fn render_section(prepared: &Prepared) -> String {
    let json =
        serde_json::to_string_pretty(prepared).expect("a prepared question always serialises");
    format!("## {PREPARED_QUESTION}\n\n```json\n{json}\n```\n")
}

/// The body of a Drift with its prepared question replaced by `prepared` (or added at the end).
pub fn body_with(body: &str, prepared: &Prepared) -> String {
    let headings = crate::project::md::outline(body).headings;
    let mut text = body.to_owned();
    if let Some(start) = headings
        .iter()
        .position(|h| h.level == 2 && h.text == PREPARED_QUESTION)
    {
        let from = headings[start].range.start;
        let to = headings[start + 1..]
            .iter()
            .find(|h| h.level <= 2)
            .map_or(body.len(), |h| h.range.start);
        text.replace_range(from..to, "");
    }
    let mut text = text.trim_end().to_owned();
    text.push_str("\n\n");
    text.push_str(&render_section(prepared));
    text
}

// ---------------------------------------------------------------------------------------------
// Shape
// ---------------------------------------------------------------------------------------------

/// What a question and its decisions must look like for a Drift with `options` fix options.
/// `merge` is a Drift from a merge: its question always has the `proposal` decision for the
/// behaviour the code has, and recommends it (`requirements/rules#drift`). Every problem is listed
/// with the path of the input it comes from.
pub fn shape_problems(
    drift_id: &str,
    options: usize,
    merge: bool,
    question: &Question,
    decisions: &[PreparedDecision],
) -> Vec<(String, String)> {
    let mut problems: Vec<(String, String)> = Vec::new();
    let mut problem = |path: String, message: String| problems.push((path, message));
    if question.id != drift_id {
        problem(
            "prepared.question.id".to_owned(),
            format!(
                "must be the identity of the Drift, `{drift_id}` (found `{}`)",
                question.id
            ),
        );
    }
    if question.multi {
        problem(
            "prepared.question.multi".to_owned(),
            "must be false: the question is single-choice".to_owned(),
        );
    }
    let expected = if options == 0 { 2 } else { options + 1 };
    if question.options.len() != expected {
        problem(
            "prepared.question.options".to_owned(),
            format!(
                "needs {expected} options ({}), found {}",
                if options == 0 {
                    "adopting the attached Proposal, and closing without changing anything"
                        .to_owned()
                } else {
                    format!("one for each of the {options} fix options, in order, and one for the behaviour the code has")
                },
                question.options.len()
            ),
        );
    }
    if decisions.len() != question.options.len() {
        problem(
            "prepared.decisions".to_owned(),
            format!(
                "needs one decision for each option of the question ({}), found {}",
                question.options.len(),
                decisions.len()
            ),
        );
        return problems;
    }
    for (index, decision) in decisions.iter().enumerate() {
        let at = format!("prepared.decisions[{index}]");
        let want =
            |what: &str| format!("must be {what} (the option {} of the question)", index + 1);
        if options == 0 {
            let (name, recommended) = if index == 0 {
                (PROPOSAL, true)
            } else {
                (DISMISSED, false)
            };
            if decision.decision != name || decision.option.is_some() {
                problem(at, want(&format!("`{{\"decision\":\"{name}\"}}`: a Drift without fix options has `proposal` first and `dismissed` second")));
            } else if recommended && question.recommended != 0 {
                problem("prepared.question.recommended".to_owned(), "must be 0: with no fix option, adopting the attached Proposal is the recommended option".to_owned());
            }
        } else if index < options {
            let number = u32::try_from(index + 1).unwrap_or(u32::MAX);
            if decision.decision != FIX_CODE || decision.option != Some(number) {
                problem(
                    at,
                    want(&format!(
                        "`{{\"decision\":\"fix-code\",\"option\":{number}}}`"
                    )),
                );
            }
        } else if !matches!(decision.decision.as_str(), PROPOSAL | DISMISSED)
            || decision.option.is_some()
        {
            problem(at, want("`{\"decision\":\"proposal\"}` or `{\"decision\":\"dismissed\"}`: the behaviour the code has"));
        } else if merge {
            if decision.decision != PROPOSAL {
                problem(at, want("`{\"decision\":\"proposal\"}`: a Drift from a merge always offers keeping the code as it is and adapting the Model"));
            } else if question.recommended != index {
                problem("prepared.question.recommended".to_owned(), format!("must be {index}: a Drift from a merge recommends keeping the code as it is and adapting the Model"));
            }
        }
    }
    problems
}

/// Reads and checks what `drift edit` was given: the question as a round of one question, and the
/// decisions. `attached` says whether the Drift has a Proposal in Draft or Open (the Proposal the
/// command attaches counts).
pub fn validate_input(
    drift_id: &str,
    options: usize,
    merge: bool,
    attached: bool,
    question: &serde_json::Value,
    decisions: &[PreparedDecision],
) -> Result<Question, Vec<(String, String)>> {
    let round = json!({ "questions": [question] });
    let found = question::problems(&round);
    if !found.is_empty() {
        return Err(found
            .into_iter()
            .map(|p| {
                (
                    p.path.replacen("questions[0]", "prepared.question", 1),
                    p.message,
                )
            })
            .collect());
    }
    let question: Question = serde_json::from_value(question.clone())
        .map_err(|e| vec![("prepared.question".to_owned(), e.to_string())])?;
    let mut problems = shape_problems(drift_id, options, merge, &question, decisions);
    if !attached && decisions.iter().any(|d| d.decision == PROPOSAL) {
        problems.push((
            "prepared.decisions".to_owned(),
            "a `proposal` decision needs a Proposal in Draft or Open attached to the Drift: write one and pass its identity in `proposal`".to_owned(),
        ));
    }
    if problems.is_empty() {
        Ok(question)
    } else {
        Err(problems)
    }
}

/// The Proposal attached to `drift`, when it is in Draft or Open.
pub fn live_attached<'a>(image: &'a ProjectImage, drift: &Card) -> Option<&'a Card> {
    let proposal = image.card(CardKind::Proposal, drift.str_key("proposal")?)?;
    matches!(
        data::status_of(proposal),
        Some(Status::Draft | Status::Open)
    )
    .then_some(proposal)
}

// ---------------------------------------------------------------------------------------------
// Validity
// ---------------------------------------------------------------------------------------------

/// The prepared question of `drift` and whether it may be asked now.
pub fn check(image: &ProjectImage, drift: &Card) -> Preparation {
    let prepared = match read_block(drift) {
        Ok(None) => return Preparation::Missing,
        Ok(Some(prepared)) => prepared,
        Err(why) => return Preparation::Unreadable(why),
    };
    let invalid = |why: String| Preparation::Invalid(Box::new(prepared.clone()), why);
    if status_of(drift) != Some("open") {
        return invalid("the Drift is no longer open".to_owned());
    }
    if prepared.fingerprint != fingerprint::stored_of_refs(image, &refs_of(drift)) {
        return invalid("the Model or the Requirements the Drift concerns changed since the question was prepared".to_owned());
    }
    let options: Vec<FixOption> = drift_options(drift);
    if prepared.decisions.len() != prepared.question.options.len() {
        return invalid("the question and its decisions do not match".to_owned());
    }
    for decision in &prepared.decisions {
        match decision.decision.as_str() {
            FIX_CODE => {
                let exists = decision
                    .option
                    .is_some_and(|n| n >= 1 && (n as usize) <= options.len())
                    || (decision.option.is_none() && options.is_empty());
                if !exists {
                    return invalid(
                        "a decision names a fix option the Drift does not offer".to_owned(),
                    );
                }
            }
            PROPOSAL => {
                if live_attached(image, drift).is_none() {
                    return invalid("a decision adopts the attached Proposal, which is no longer in Draft or Open".to_owned());
                }
            }
            DISMISSED => {}
            other => return invalid(format!("`{other}` is not a decision")),
        }
    }
    if is_merge(drift)
        && !prepared
            .decisions
            .get(prepared.question.recommended)
            .is_some_and(|decision| decision.decision == PROPOSAL)
    {
        return invalid("a Drift from a merge recommends keeping the code as it is and adapting the Model, with an attached Proposal".to_owned());
    }
    Preparation::Valid(Box::new(prepared))
}
