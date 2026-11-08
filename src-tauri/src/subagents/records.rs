//! The records of a sub-agent's transcript (`<id>.jsonl`, written by omp while the sub-agent
//! works) as the entries of its conversation (`requirements/tech#job`, "Sub-agents").
//!
//! The first `message` with `role: user` is the assignment. The `content[]` of an `assistant`
//! message gives `text` (a message of the agent), `thinking` (reasoning) and `toolCall` (a tool
//! row: `id`, `name`, `arguments`, `intent`). A `toolResult` (`toolCallId`, `isError`) ends the
//! matching row, and its `details` list the sub-agents of a `task` call or report the ones a
//! `wait` call names. The `session` record says when it started, the `session_exit` record how it
//! ended. Every other record is ignored.

use serde_json::Value;
use weaver_core::jobs::subagents::{self, SubAgentState};

use crate::jobs::conversation::{Conversation, Tool, ToolKind, ToolStatus};

/// What a record says besides its entries.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Applied {
    /// The instant the session started (`session` record).
    pub started: Option<String>,
    /// How the session ended (`session_exit` record).
    pub exit: Option<SubAgentState>,
}

fn nonempty<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

/// The text of `content`: a string, or the `text` blocks of an array.
fn text_of(content: &Value) -> String {
    match content {
        Value::String(text) => text.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .filter(|block| block.get("type").and_then(Value::as_str) == Some("text"))
            .filter_map(|block| block.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n\n"),
        _ => String::new(),
    }
}

/// The row of a `toolCall` block.
fn tool_of(block: &Value) -> Option<Tool> {
    let id = nonempty(block, "id")?.to_owned();
    let name = nonempty(block, "name").unwrap_or("tool");
    let command = (name == "bash")
        .then(|| {
            block
                .get("arguments")
                .and_then(|arguments| nonempty(arguments, "command"))
        })
        .flatten()
        .map(|command| command.strip_prefix("$ ").unwrap_or(command).to_owned());
    let icon = if command.is_some() {
        ToolKind::Command
    } else {
        match name {
            "read" => ToolKind::Read,
            "grep" | "find" | "glob" | "search" | "web_search" => ToolKind::Search,
            "edit" | "write" | "ast_edit" | "delete" | "move" | "notebook" => ToolKind::Edit,
            "task" => ToolKind::SubAgent,
            _ => ToolKind::Other,
        }
    };
    Some(Tool {
        id,
        title: nonempty(block, "intent").unwrap_or(name).to_owned(),
        icon,
        command,
        status: ToolStatus::Working,
        sub_agents: Vec::new(),
    })
}

/// The instant a record says it was written (`timestamp`, RFC 3339), in milliseconds since the Unix epoch.
fn instant_of(record: &Value) -> Option<i64> {
    let text = nonempty(record, "timestamp")?;
    chrono::DateTime::parse_from_rfc3339(text).ok().map(|at| at.timestamp_millis())
}

/// Adds what `record` says to `conversation`, the entries it makes arriving when the record says it
/// was written. `from` is the name of who started the sub-agent: it labels the assignment.
pub fn apply(conversation: &mut Conversation, from: &str, record: &Value) -> Applied {
    conversation.set_clock(instant_of(record));
    let applied = apply_record(conversation, from, record);
    conversation.set_clock(None);
    applied
}

fn apply_record(conversation: &mut Conversation, from: &str, record: &Value) -> Applied {
    let mut applied = Applied::default();
    match record.get("type").and_then(Value::as_str) {
        Some("session") => {
            applied.started = nonempty(record, "timestamp").map(str::to_owned);
        }
        Some("custom") => applied.exit = subagents::exit_state_of(record),
        Some("message") => {
            let Some(message) = record.get("message") else {
                return applied;
            };
            match message.get("role").and_then(Value::as_str) {
                Some("user") if !conversation.has_assignment() => {
                    let text = text_of(message.get("content").unwrap_or(&Value::Null));
                    if !text.trim().is_empty() {
                        conversation.assignment(from, text.trim());
                    }
                }
                Some("assistant") => {
                    let Some(blocks) = message.get("content").and_then(Value::as_array) else {
                        return applied;
                    };
                    for block in blocks {
                        match block.get("type").and_then(Value::as_str) {
                            Some("text") => conversation.agent_message(
                                block.get("text").and_then(Value::as_str).unwrap_or(""),
                            ),
                            Some("thinking") => conversation.thinking_block(
                                block.get("thinking").and_then(Value::as_str).unwrap_or(""),
                            ),
                            Some("toolCall") => {
                                if let Some(tool) = tool_of(block) {
                                    conversation.tool_started(tool);
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Some("toolResult") => {
                    if let Some(id) = nonempty(message, "toolCallId") {
                        let is_error = message
                            .get("isError")
                            .and_then(Value::as_bool)
                            .unwrap_or(false);
                        conversation.tool_finished(id, is_error, message.get("details"));
                    }
                }
                _ => {}
            }
        }
        _ => {}
    }
    applied
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::jobs::conversation::Entry;

    #[test]
    fn the_entries_of_a_record_arrive_when_the_record_says_and_unstamped_ones_when_weaver_sees_them() {
        let mut c = Conversation::new();
        let message = |at: Option<&str>, text: &str| {
            let mut record = json!({ "type": "message", "message": { "role": "assistant", "content": [{ "type": "text", "text": text }] } });
            if let Some(at) = at {
                record["timestamp"] = json!(at);
            }
            record
        };
        apply(&mut c, "Run T1", &message(Some("2026-11-02T08:00:00.000Z"), "One"));
        apply(&mut c, "Run T1", &json!({ "type": "message", "timestamp": "2026-11-02T08:00:04.000Z", "message": { "role": "assistant", "content": [{ "type": "toolCall", "id": "c1", "name": "read", "arguments": { "path": "a" } }] } }));
        let before = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        apply(&mut c, "Run T1", &message(None, "Two"));
        let arrivals = c.arrivals();
        assert_eq!(arrivals[0], 1_793_606_400_000);
        assert_eq!(arrivals[1] - arrivals[0], 4000);
        assert!(arrivals[2] >= before, "a record with no timestamp arrives now");
    }

    #[test]
    fn records_become_the_entries_of_a_conversation() {
        let mut c = Conversation::new();
        let records = [
            json!({ "type": "title", "v": 1 }),
            json!({ "type": "session", "timestamp": "2026-11-02T08:00:00.000Z" }),
            json!({ "type": "message", "message": { "role": "user", "content": [{ "type": "text", "text": "Read the code." }] } }),
            json!({ "type": "message", "message": { "role": "user", "content": "A later user message is ignored." } }),
            json!({ "type": "message", "message": { "role": "assistant", "content": [
                { "type": "thinking", "thinking": "Let me look.", "thinkingSignature": "x" },
                { "type": "text", "text": "Reading." },
                { "type": "toolCall", "id": "c1", "name": "bash", "arguments": { "command": "ls -la" }, "intent": "Listing" },
                { "type": "toolCall", "id": "c2", "name": "read", "arguments": { "path": "a.rs" } },
            ] } }),
            json!({ "type": "custom", "customType": "tool_execution_start", "data": {} }),
            json!({ "type": "message", "message": { "role": "toolResult", "toolCallId": "c1", "isError": false, "content": [] } }),
            json!({ "type": "message", "message": { "role": "toolResult", "toolCallId": "c2", "isError": true, "content": [] } }),
        ];
        let applied: Vec<Applied> = records.iter().map(|r| apply(&mut c, "Run T1", r)).collect();
        assert_eq!(
            applied[1].started.as_deref(),
            Some("2026-11-02T08:00:00.000Z")
        );
        let entries = c.entries();
        assert_eq!(
            entries[0],
            Entry::Assignment {
                from: "Run T1".into(),
                text: "Read the code.".into()
            }
        );
        assert_eq!(
            entries[1],
            Entry::Thinking {
                text: "Let me look.".into()
            }
        );
        assert_eq!(
            entries[2],
            Entry::Agent {
                text: "Reading.".into(),
                complete: true
            }
        );
        let Entry::Tool(shell) = &entries[3] else {
            panic!("{:?}", entries[3])
        };
        assert_eq!(
            (
                shell.title.as_str(),
                shell.command.as_deref(),
                shell.icon,
                shell.status
            ),
            (
                "Listing",
                Some("ls -la"),
                ToolKind::Command,
                ToolStatus::Done
            )
        );
        let Entry::Tool(read) = &entries[4] else {
            panic!("{:?}", entries[4])
        };
        assert_eq!(
            (read.title.as_str(), read.icon, read.status),
            ("read", ToolKind::Read, ToolStatus::Failed)
        );
        assert_eq!(entries.len(), 5, "the other records add nothing");
    }

    #[test]
    fn the_results_of_task_and_wait_calls_list_and_settle_the_nested_sub_agents() {
        let mut c = Conversation::new();
        apply(
            &mut c,
            "x",
            &json!({ "type": "message", "message": { "role": "assistant", "content": [
            { "type": "toolCall", "id": "t1", "name": "task", "arguments": {} },
            { "type": "toolCall", "id": "w1", "name": "wait", "arguments": {} },
        ] } }),
        );
        apply(
            &mut c,
            "x",
            &json!({ "type": "message", "message": { "role": "toolResult", "toolCallId": "t1", "details": { "progress": [
            { "id": "0-A", "agent": "Reader", "status": "pending" },
            { "id": "__advisor.turn", "status": "running" },
        ] } } }),
        );
        let rows: Vec<_> = c.agent_rows().collect();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].sub_agents.len(), 1);
        assert!(rows[0].sub_agents[0].state.is_working());
        assert_eq!(c.background_agents(), [("t1".to_owned(), "0-A".to_owned())]);
        apply(
            &mut c,
            "x",
            &json!({ "type": "message", "message": { "role": "toolResult", "toolCallId": "w1", "details": { "jobs": [
            { "id": "0-A", "status": "completed" },
        ] } } }),
        );
        let rows: Vec<_> = c.agent_rows().collect();
        assert_eq!(rows[0].sub_agents[0].state, SubAgentState::Succeeded);
        assert!(c.background_agents().is_empty());
    }

    #[test]
    fn the_session_and_exit_records_say_when_it_started_and_how_it_ended() {
        let mut c = Conversation::new();
        let exit = apply(
            &mut c,
            "x",
            &json!({ "type": "custom", "customType": "session_exit", "data": { "kind": "normal" } }),
        );
        assert_eq!(exit.exit, Some(SubAgentState::Succeeded));
        assert!(c.entries().is_empty());
    }
}
