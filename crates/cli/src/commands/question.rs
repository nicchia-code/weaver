//! `weaver question new`: a round of questions for the developer, or a card of questions in Chat.
//! The registry holds the whole input and its checks; running it is [`crate::round`], which in a
//! job waits with no time limit and so never goes through the handlers of the registry.

use serde_json::{Map, Value};

use super::{app_only, in_app, question_fields};
use crate::spec::*;

pub fn commands() -> Vec<Command> {
    vec![Command::new("question", "new", "Asks the developer a group of questions: a round in a job, a card of questions in Chat.", app_only)
        .notes(&[
            "Prints only JSON on standard output, always. The input is validated whole before Weaver is contacted. In a job the command then waits with no time limit until a result exists: run it with no time limit and wait until it ends. In Chat it returns as soon as Weaver has published the card of questions or refused it.",
            "Write every text so that it stands alone: the developer sees only the questions, not the conversation. Texts are Markdown except labels. Use no first person and no \"Other\" option: a free answer is always offered.",
            "In Chat, use it for every question with distinct options, in one card per turn: a second card in the same turn is refused whole. After `published`, end the turn now, do not repeat the questions in the reply and do not wait. The answers come in a later message of the developer, in a block that starts with `Answers to the card of questions <card>`: each question is `answered`, `skipped` (use the recommendation and list it as a decision to verify) or `not-answered` (still open: use no recommendation and do not decide it). Open questions without options stay in the reply.",
            "Results: `{\"status\":\"answered\",\"answers\":{<id>:{\"status\":\"answered\",\"selected\":[...],\"text\":...}|{\"status\":\"skipped\"}}}` (exit 0, in a job); `{\"status\":\"published\",\"card\":<card>,\"message\":...}` (exit 0, in Chat); `{\"status\":\"unavailable\",\"reason\":...}` (exit 0) when the caller is neither in a working Weaver job nor in the project's Chat during a turn, or no Weaver answers before the questions are submitted: stop and tell the developer; `{\"error\":{\"code\":\"invalid-input\"|\"interrupted\"|\"cancelled\"|\"card-already-published\",\"message\":...}}`, exit 2 for `invalid-input` and 1 for the others.",
        ])
        .access(Access::JobOrUnavailable)
        .json_only()
        .input(vec![req("questions", list_min(object(question_fields()), 1), "The questions, at least one.")])
        .output(vec![
            req("status", one_of(&["answered", "published", "unavailable"]), "The result."),
            opt("answers", Ty::Any, "One answer per question id: `{status: \"answered\", selected, text?}` or `{status: \"skipped\"}`."),
            opt("card", line(), "In Chat: the identity of the card that was published."),
            opt("message", line(), "In Chat: what to do next, which is to end the turn without repeating the questions."),
            opt("reason", line(), "Why no question could be shown."),
        ])
        .place(in_app)
        .check(check)
        .example("weaver question new --input -")]
}

/// The rules of a round that types do not say: ids are unique, labels are unique within a
/// question and `recommended` is one of the options.
fn check(input: &Map<String, Value>) -> Vec<Problem> {
    let mut problems = Vec::new();
    let Some(questions) = input.get("questions").and_then(Value::as_array) else {
        return problems;
    };
    let mut seen: Vec<&str> = Vec::new();
    for (index, question) in questions.iter().enumerate() {
        if let Some(id) = question.get("id").and_then(Value::as_str) {
            if seen.contains(&id) {
                problems.push(Problem::new(
                    format!("questions[{index}].id"),
                    format!("`{id}` is used by an earlier question: ids are unique"),
                ));
            }
            seen.push(id);
        }
        let options = question
            .get("options")
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        let mut labels: Vec<&str> = Vec::new();
        for (at, option) in question
            .get("options")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .enumerate()
        {
            if let Some(label) = option
                .get("label")
                .and_then(Value::as_str)
                .filter(|label| !label.trim().is_empty())
            {
                if labels.contains(&label) {
                    problems.push(Problem::new(
                        format!("questions[{index}].options[{at}].label"),
                        format!("`{label}` is used by an earlier option: labels are unique within a question"),
                    ));
                }
                labels.push(label);
            }
        }
        if let Some(recommended) = question.get("recommended").and_then(Value::as_i64) {
            if recommended >= 0 && recommended as usize >= options && options > 0 {
                problems.push(Problem::new(
                    format!("questions[{index}].recommended"),
                    format!(
                        "must be the index of an option, 0 to {} (found {recommended})",
                        options - 1
                    ),
                ));
            }
        }
    }
    problems
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn problems(value: Value) -> Vec<(String, String)> {
        check(value.as_object().unwrap())
            .into_iter()
            .map(|p| (p.path, p.message))
            .collect()
    }

    #[test]
    fn ids_are_unique_and_recommended_is_an_option() {
        let (a, b) = (
            json!({"label": "a", "description": "d"}),
            json!({"label": "b", "description": "d"}),
        );
        let found = problems(json!({ "questions": [
            {"id": "x", "options": [a, b], "recommended": 1},
            {"id": "x", "options": [a, b], "recommended": 2},
        ]}));
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].0, "questions[1].id");
        assert_eq!(found[1].0, "questions[1].recommended");
        assert!(found[1].1.contains("0 to 1"));
    }

    #[test]
    fn labels_are_unique_within_a_question_only() {
        let option = |label: &str| json!({"label": label, "description": "d"});
        let found = problems(json!({ "questions": [
            {"id": "x", "options": [option("a"), option("b"), option("a")], "recommended": 0},
            {"id": "y", "options": [option("a"), option("b")], "recommended": 0},
        ]}));
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].0, "questions[0].options[2].label");
    }

    #[test]
    fn the_registry_and_the_core_agree_on_what_a_round_is() {
        let option = |label: &str| json!({"label": label, "description": "d"});
        let rounds = [
            json!({"questions": [{"id": "a", "question": "Q", "options": [option("x"), option("y")], "recommended": 1, "multi": true}]}),
            json!({"questions": [{"id": "a", "question": "Q", "options": [option("x"), option("x")], "recommended": 0, "multi": false}]}),
            json!({"questions": [{"id": "a", "question": "Q", "options": [option("x"), option("y")], "recommended": 2, "multi": false}]}),
            json!({"questions": [{"id": "a", "question": "Q", "options": [option("x"), option("y")], "recommended": 0, "multi": false},
                                 {"id": "a", "question": " ", "options": [option("x"), option("y")], "recommended": 0, "multi": false}]}),
            json!({"questions": []}),
        ];
        for round in rounds {
            let input = round.as_object().unwrap();
            let by_registry = crate::exec::check_input(registry_command(), input).is_ok();
            let by_core = weaver_core::question::problems(&round).is_empty();
            assert_eq!(by_registry, by_core, "{round}");
        }
    }

    fn registry_command() -> &'static Command {
        crate::registry::find("question", "new").unwrap()
    }
}
