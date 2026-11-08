//! The conversation of a job: what the Jobs view shows (`requirements/ui#job`).
//!
//! It is built from the `session/update` notifications of omp and from what Weaver adds itself:
//! the instruction, the messages of the developer, the notes, the permission cards and the end
//! line. Nothing of it is stored by Weaver: omp keeps the text in its session files and a resumed
//! job rebuilds the conversation from the history that `session/load` replays.
//!
//! The conversation is a list of entries. A change touches the last entry or appends one, so the UI
//! gets `(index, entry)` pairs, in batches ([`Conversation::take_changes`]); `seq` counts the
//! batches, so that the UI can tell a snapshot from the events that follow it.

use std::collections::{BTreeSet, HashMap};

use serde::Serialize;
use serde_json::Value;
use ts_rs::TS;
use weaver_core::assets::tags;
use weaver_core::jobs::ask;
use weaver_core::jobs::machine::{MessageId, RequestId, RESUME_PROMPT};
use weaver_core::jobs::subagents::{self, SubAgent, SubAgentState};

/// What happened to a message of the developer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "status", rename_all = "kebab-case")]
pub enum Delivery {
    /// Not delivered yet: a round or a permission waits, or omp has not taken it.
    Queued,
    Delivered,
    /// Refused or never sent; the reason is shown as `Not delivered: <reason>.`.
    NotDelivered {
        reason: String,
    },
}

/// The icon of a tool row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum ToolKind {
    Read,
    Search,
    Command,
    Edit,
    SubAgent,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum ToolStatus {
    Working,
    Done,
    Failed,
}

/// One compact row for a tool call. Rows never show results or output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Tool {
    pub id: String,
    /// omp's title of the call.
    pub title: String,
    /// The icon of the row.
    pub icon: ToolKind,
    /// The full command of a command call, without a leading `$ `.
    pub command: Option<String>,
    pub status: ToolStatus,
    /// The sub-agents of a `task` call, in omp's order.
    pub sub_agents: Vec<SubAgent>,
}

/// An option of a permission request, with omp's own name and order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PermissionOption {
    pub id: String,
    pub name: String,
    /// Whether the option rejects (`reject_once`, `reject_always`): it shows as `outline`.
    pub rejects: bool,
}

/// How a permission card stands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "status", rename_all = "kebab-case")]
pub enum PermissionState {
    Asking,
    /// The developer chose this option (its name).
    Chosen {
        name: String,
    },
    /// The turn ended: `No longer needed.`.
    Void,
    /// Stop or close: `Cancelled.`.
    Cancelled,
}

/// How the job ended, for the end line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
pub enum EndLine {
    Finished,
    Stopped,
    /// `Failed: <reason>.`; `details` are omp's last error lines after an exit.
    Failed {
        reason: String,
        details: Option<String>,
    },
}

/// An entry of the conversation, top to bottom.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum Entry {
    /// Weaver's one-line instruction, shown as a message labelled `Weaver`.
    Instruction {
        text: String,
    },
    /// A sub-agent: the assignment it received, shown as a message labelled with the name of who
    /// started it (the job, or the sub-agent that launched it).
    Assignment {
        from: String,
        text: String,
    },
    Developer {
        #[ts(type = "number")]
        message: MessageId,
        text: String,
        delivery: Delivery,
    },
    /// An agent message in Markdown; `complete` once the next entry started or the turn ended.
    Agent {
        text: String,
        complete: bool,
    },
    /// Reasoning, shown in a collapsed `Thinking` block.
    Thinking {
        text: String,
    },
    Tool(Tool),
    Note {
        text: String,
        at: String,
    },
    Permission {
        #[ts(type = "number")]
        request: RequestId,
        tool: Option<Tool>,
        options: Vec<PermissionOption>,
        state: PermissionState,
    },
    /// `Session reloaded; the agent continues its work.`
    Reloaded,
    /// General: a job the agent started on the developer's request (`Started <name>`).
    JobStarted {
        job: String,
        name: String,
    },
    /// General: the start of a job was refused (`Weaver did not start <job kind>: <reason>.`).
    JobRefused {
        job_kind: String,
        reason: String,
    },
    /// General: a notice of Weaver, for example that the kept conversation could not be reloaded.
    Notice {
        text: String,
    },
    /// General: the turn failed and omp stays.
    TurnFailed {
        reason: String,
    },
    /// General: omp stopped (`The agent stopped: <reason>.`); the next message starts it again.
    OmpStopped {
        reason: String,
        details: Option<String>,
    },
    End(EndLine),
}

/// The entries that changed since the last batch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Changed {
    #[ts(type = "number")]
    pub index: usize,
    pub entry: Entry,
    /// The instant the entry arrived, in milliseconds since the Unix epoch.
    #[ts(type = "number")]
    pub at: i64,
}

/// The instant now, in milliseconds since the Unix epoch.
fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
        })
}

#[derive(Debug, Default)]
pub struct Conversation {
    entries: Vec<Entry>,
    /// The instant each entry arrived, as `entries` is indexed.
    arrived: Vec<i64>,
    /// The instant the entries now being made were written, when a transcript says it.
    clock: Option<i64>,
    seq: u64,
    dirty: BTreeSet<usize>,
    tools: HashMap<String, usize>,
    /// The agent message or the reasoning still being written.
    open_agent: Option<usize>,
    open_thinking: Option<usize>,
    permissions: HashMap<RequestId, usize>,
    messages: HashMap<MessageId, usize>,
    /// A General or an Ask more conversation: no instruction entry.
    general: bool,
    /// An Ask more conversation: the first prompt of its history holds Weaver's instruction, of
    /// which only the developer's message is shown.
    ask: bool,
    /// Counts the changes of the sub-agents the rows list.
    agents_rev: u64,
}

/// An update never takes a sub-agent that ended back to working: `new` keeps the state of the
/// same sub-agent in `old` when it ended and `new` says it works.
fn keep_ended(old: &[SubAgent], new: &mut [SubAgent]) {
    for agent in new.iter_mut().filter(|a| a.state.is_working()) {
        if let Some(ended) = old
            .iter()
            .find(|o| o.id == agent.id && !o.state.is_working())
        {
            agent.state = ended.state;
        }
    }
}

fn text_of(content: &Value) -> &str {
    content
        .get("text")
        .and_then(Value::as_str)
        .unwrap_or_default()
}

fn tool_kind(update: &Value, command: &Option<String>) -> ToolKind {
    if command.is_some() {
        return ToolKind::Command;
    }
    match update.get("kind").and_then(Value::as_str) {
        Some("read") => ToolKind::Read,
        Some("search") => ToolKind::Search,
        Some("execute") => ToolKind::Command,
        Some("edit" | "delete" | "move") => ToolKind::Edit,
        _ => ToolKind::Other,
    }
}

/// The full command of a command call: `rawInput.command`, else the title without its `$ `.
fn command_of(update: &Value) -> Option<String> {
    let command = update
        .get("rawInput")
        .and_then(|input| input.get("command"))
        .and_then(Value::as_str);
    match (update.get("kind").and_then(Value::as_str), command) {
        (Some("execute"), Some(command)) => Some(command.trim().to_owned()),
        (Some("execute"), None) => update
            .get("title")
            .and_then(Value::as_str)
            .map(|t| t.trim().strip_prefix("$ ").unwrap_or(t.trim()).to_owned()),
        _ => None,
    }
}

pub(crate) fn looks_like_task_call(update: &Value) -> bool {
    let title = update
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let input = update.get("rawInput");
    title.starts_with("task")
        || input.is_some_and(|i| {
            i.get("tasks").is_some_and(Value::is_array)
                || (i.get("agent").is_some() && i.get("task").is_some())
        })
}

fn status_of(text: Option<&str>, previous: ToolStatus) -> ToolStatus {
    match text {
        Some("completed") => ToolStatus::Done,
        Some("failed") => ToolStatus::Failed,
        Some("pending" | "in_progress") => ToolStatus::Working,
        _ => previous,
    }
}

/// The row of a tool call of `session/update` or of the `toolCall` of a permission request.
pub fn tool_of(update: &Value) -> Option<Tool> {
    let id = update.get("toolCallId").and_then(Value::as_str)?.to_owned();
    let title = update
        .get("title")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .unwrap_or("Tool call")
        .to_owned();
    let command = command_of(update);
    let mut kind = tool_kind(update, &command);
    let sub_agents = update
        .get("rawOutput")
        .and_then(subagents::from_raw_output)
        .unwrap_or_default();
    if !sub_agents.is_empty() || looks_like_task_call(update) {
        kind = ToolKind::SubAgent;
    }
    Some(Tool {
        id,
        title,
        icon: kind,
        command,
        status: status_of(
            update.get("status").and_then(Value::as_str),
            ToolStatus::Working,
        ),
        sub_agents,
    })
}

impl Conversation {
    pub fn new() -> Self {
        Self::default()
    }

    /// The conversation of a General: it has no instruction, so the first user message of a
    /// replayed history is a message of the developer.
    pub fn general() -> Self {
        Self {
            general: true,
            ..Self::default()
        }
    }

    /// The conversation of an Ask more conversation: like General's, it has no instruction entry,
    /// and the first user message of a replayed history shows only the text after
    /// `Developer's message:`.
    pub fn ask() -> Self {
        Self {
            general: true,
            ask: true,
            ..Self::default()
        }
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// The instant each entry arrived, in milliseconds since the Unix epoch, as `entries` is
    /// indexed.
    pub fn arrivals(&self) -> &[i64] {
        &self.arrived
    }

    /// The number of batches taken so far.
    pub fn seq(&self) -> u64 {
        self.seq
    }

    /// The instant a transcript record says it was written, for the entries it makes; `None` goes
    /// back to the instant Weaver sees them.
    pub fn set_clock(&mut self, at: Option<i64>) {
        self.clock = at;
    }

    fn push(&mut self, entry: Entry) -> usize {
        let at = self.clock.unwrap_or_else(now_ms);
        self.push_at(entry, at)
    }

    fn push_at(&mut self, entry: Entry, at: i64) -> usize {
        self.close_open();
        self.entries.push(entry);
        self.arrived.push(at);
        let index = self.entries.len() - 1;
        self.dirty.insert(index);
        index
    }

    /// The next entry starts: the agent message being written is complete.
    fn close_open(&mut self) {
        if let Some(index) = self.open_agent.take() {
            if let Some(Entry::Agent { complete, .. }) = self.entries.get_mut(index) {
                *complete = true;
                self.dirty.insert(index);
            }
        }
        self.open_thinking = None;
    }

    /// The turn ended: the agent message being written is complete, and the next turn starts a
    /// message of its own.
    pub fn end_turn(&mut self) {
        self.close_open();
    }

    pub fn instruction(&mut self, text: &str) {
        self.push(Entry::Instruction {
            text: text.to_owned(),
        });
    }

    pub fn developer_message(&mut self, message: MessageId, text: &str, delivery: Delivery) {
        let index = self.push(Entry::Developer {
            message,
            text: text.to_owned(),
            delivery,
        });
        self.messages.insert(message, index);
    }

    pub fn set_delivery(&mut self, message: MessageId, to: Delivery) {
        let Some(&index) = self.messages.get(&message) else {
            return;
        };
        if let Some(Entry::Developer { delivery, .. }) = self.entries.get_mut(index) {
            if *delivery != to {
                *delivery = to;
                self.dirty.insert(index);
            }
        }
    }

    pub fn note(&mut self, text: &str, at: &str) {
        self.push(Entry::Note {
            text: text.to_owned(),
            at: at.to_owned(),
        });
    }

    pub fn reloaded(&mut self) {
        self.push(Entry::Reloaded);
    }

    /// General: adds an entry a conversation already held (the messages of the developer and the
    /// lines of Weaver that came before the kept session was reloaded), with the instant it
    /// arrived.
    pub fn append(&mut self, entry: &Entry, at: i64) {
        let index = self.push_at(entry.clone(), at);
        if let Entry::Developer { message, .. } = entry {
            self.messages.insert(*message, index);
        }
    }

    /// General: a job the agent started on the developer's request.
    pub fn job_started(&mut self, job: &str, name: &str) {
        self.push(Entry::JobStarted {
            job: job.to_owned(),
            name: name.to_owned(),
        });
    }

    /// General: a job the Chat started in an earlier run of Weaver, known from its record, with
    /// the instant it was created.
    pub fn job_started_at(&mut self, job: &str, name: &str, at: i64) {
        self.push_at(
            Entry::JobStarted {
                job: job.to_owned(),
                name: name.to_owned(),
            },
            at,
        );
    }

    /// General: the start of a job was refused.
    pub fn job_refused(&mut self, kind: &str, reason: &str) {
        self.push(Entry::JobRefused {
            job_kind: kind.to_owned(),
            reason: reason.to_owned(),
        });
    }

    /// General: something the developer reads that omp did not say.
    pub fn notice(&mut self, text: &str) {
        self.push(Entry::Notice {
            text: text.to_owned(),
        });
    }

    /// General: the turn failed; omp stays.
    pub fn turn_failed(&mut self, reason: &str) {
        self.push(Entry::TurnFailed {
            reason: reason.to_owned(),
        });
    }

    /// General: omp stopped; `details` are its last error lines.
    pub fn omp_stopped(&mut self, reason: &str, details: Option<String>) {
        self.push(Entry::OmpStopped {
            reason: reason.to_owned(),
            details,
        });
    }

    /// A permission request of omp, with its `toolCall` and `options`.
    pub fn permission(&mut self, request: RequestId, tool_call: &Value, options: &Value) {
        let options = options
            .as_array()
            .map(|list| {
                list.iter()
                    .filter_map(|option| {
                        Some(PermissionOption {
                            id: option.get("optionId")?.as_str()?.to_owned(),
                            name: option
                                .get("name")
                                .and_then(Value::as_str)
                                .unwrap_or("Choose")
                                .to_owned(),
                            rejects: option
                                .get("kind")
                                .and_then(Value::as_str)
                                .is_some_and(|k| k.starts_with("reject")),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let index = self.push(Entry::Permission {
            request,
            tool: tool_of(tool_call),
            options,
            state: PermissionState::Asking,
        });
        self.permissions.insert(request, index);
    }

    /// Closes a permission card, unless it was closed already.
    pub fn close_permission(&mut self, request: RequestId, to: PermissionState) {
        let Some(&index) = self.permissions.get(&request) else {
            return;
        };
        if let Some(Entry::Permission { state, .. }) = self.entries.get_mut(index) {
            if *state == PermissionState::Asking {
                *state = to;
                self.dirty.insert(index);
            }
        }
    }

    /// The name of the option `option` of the request, for the card that shows the choice.
    pub fn option_name(&self, request: RequestId, option: &str) -> Option<String> {
        let &index = self.permissions.get(&request)?;
        match self.entries.get(index)? {
            Entry::Permission { options, .. } => options
                .iter()
                .find(|o| o.id == option)
                .map(|o| o.name.clone()),
            _ => None,
        }
    }

    /// The permission requests that still ask.
    pub fn asking(&self) -> Vec<RequestId> {
        let mut asking: Vec<RequestId> = self
            .permissions
            .iter()
            .filter(|(_, &index)| {
                matches!(
                    self.entries.get(index),
                    Some(Entry::Permission {
                        state: PermissionState::Asking,
                        ..
                    })
                )
            })
            .map(|(&request, _)| request)
            .collect();
        asking.sort_unstable();
        asking
    }

    /// Applies the `update` of a `session/update` notification.
    pub fn apply_update(&mut self, update: &Value) {
        match update.get("sessionUpdate").and_then(Value::as_str) {
            Some("agent_message_chunk") => {
                self.append_agent(text_of(update.get("content").unwrap_or(&Value::Null)))
            }
            Some("agent_thought_chunk") => {
                self.append_thinking(text_of(update.get("content").unwrap_or(&Value::Null)))
            }
            Some("user_message_chunk") => {
                self.replayed_user(text_of(update.get("content").unwrap_or(&Value::Null)))
            }
            Some("tool_call") => self.tool_call(update),
            Some("tool_call_update") => self.tool_update(update),
            _ => {}
        }
    }

    fn append_agent(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        self.open_thinking = None;
        if let Some(index) = self.open_agent {
            if let Some(Entry::Agent { text: existing, .. }) = self.entries.get_mut(index) {
                existing.push_str(text);
                self.dirty.insert(index);
                return;
            }
        }
        let index = self.push(Entry::Agent {
            text: text.to_owned(),
            complete: false,
        });
        self.open_agent = Some(index);
    }

    fn append_thinking(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        if let Some(index) = self.open_thinking {
            if let Some(Entry::Thinking { text: existing }) = self.entries.get_mut(index) {
                existing.push_str(text);
                self.dirty.insert(index);
                return;
            }
        }
        let index = self.push(Entry::Thinking {
            text: text.to_owned(),
        });
        self.open_thinking = Some(index);
    }

    /// A user message of the replayed history: the first one is the instruction of Weaver (a
    /// General conversation has none), the prompt that tells the agent its session was reloaded
    /// is the reload line, the others are messages of the developer.
    fn replayed_user(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        if text.trim() == RESUME_PROMPT {
            self.reloaded();
        } else if !self.general
            && !self
                .entries
                .iter()
                .any(|e| matches!(e, Entry::Instruction { .. }))
        {
            self.instruction(text);
        } else {
            // What Weaver appended for the tagged assets is never shown, live or replayed.
            let text = if self.ask {
                ask::shown_message(text)
            } else {
                tags::without_block(text)
            };
            self.developer_message(0, text, Delivery::Delivered);
        }
    }

    fn tool_call(&mut self, update: &Value) {
        let Some(tool) = tool_of(update) else { return };
        if let Some(&index) = self.tools.get(&tool.id) {
            self.replace_tool(index, tool);
        } else {
            let id = tool.id.clone();
            if !tool.sub_agents.is_empty() {
                self.agents_rev += 1;
            }
            let index = self.push(Entry::Tool(tool));
            self.tools.insert(id, index);
        }
        self.waited(update);
    }

    /// Replaces the row at `index`; a sub-agent that ended stays ended whatever the new row says
    /// of it (an update never takes a sub-agent back to working).
    fn replace_tool(&mut self, index: usize, mut tool: Tool) {
        if let Some(Entry::Tool(existing)) = self.entries.get_mut(index) {
            keep_ended(&existing.sub_agents, &mut tool.sub_agents);
            if *existing != tool {
                if existing.sub_agents != tool.sub_agents {
                    self.agents_rev += 1;
                }
                *existing = tool;
                self.dirty.insert(index);
            }
        }
    }

    fn tool_update(&mut self, update: &Value) {
        let Some(id) = update.get("toolCallId").and_then(Value::as_str) else {
            return;
        };
        let Some(&index) = self.tools.get(id) else {
            // An update of a call this conversation never saw: show it as a row of its own.
            self.tool_call(update);
            return;
        };
        let Some(Entry::Tool(current)) = self.entries.get(index) else {
            return;
        };
        let mut tool = current.clone();
        if let Some(title) = update
            .get("title")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|t| !t.is_empty())
        {
            title.clone_into(&mut tool.title);
        }
        if update.get("kind").is_some() || update.get("rawInput").is_some() {
            if let Some(command) = command_of(update) {
                tool.command = Some(command);
                tool.icon = ToolKind::Command;
            }
        }
        tool.status = status_of(update.get("status").and_then(Value::as_str), tool.status);
        if let Some(agents) = update.get("rawOutput").and_then(subagents::from_raw_output) {
            if !agents.is_empty() {
                tool.icon = ToolKind::SubAgent;
                tool.sub_agents = agents;
            }
        }
        if looks_like_task_call(update) {
            tool.icon = ToolKind::SubAgent;
        }
        self.replace_tool(index, tool);
        self.waited(update);
    }

    /// The statuses a `wait` call reports (`rawOutput.details.jobs[]`) reach the sub-agents it
    /// names, wherever their `task` call is.
    fn waited(&mut self, update: &Value) {
        let Some(raw_output) = update.get("rawOutput") else {
            return;
        };
        for (agent, state) in subagents::wait_states(raw_output) {
            self.report_state(&agent, state);
        }
    }

    /// omp reports `state` for the sub-agent `agent` (a `wait` call): every row that lists it
    /// follows, unless the report would take an ended sub-agent back to working.
    pub fn report_state(&mut self, agent: &str, state: SubAgentState) {
        for (index, entry) in self.entries.iter_mut().enumerate() {
            let Entry::Tool(tool) = entry else { continue };
            for found in tool.sub_agents.iter_mut().filter(|a| a.id == agent) {
                if found.state == state || (state.is_working() && !found.state.is_working()) {
                    continue;
                }
                found.state = state;
                self.dirty.insert(index);
                self.agents_rev += 1;
            }
        }
    }

    /// The sub-agents whose `task` call completed while they still work: the ones whose end only
    /// their session file can tell. Pairs of `(tool call id, sub-agent id)`.
    pub fn background_agents(&self) -> Vec<(String, String)> {
        self.entries
            .iter()
            .filter_map(|entry| match entry {
                Entry::Tool(tool)
                    if tool.icon == ToolKind::SubAgent && tool.status != ToolStatus::Working =>
                {
                    Some(tool)
                }
                _ => None,
            })
            .flat_map(|tool| {
                tool.sub_agents
                    .iter()
                    .filter(|a| a.state.is_working())
                    .map(|a| (tool.id.clone(), a.id.clone()))
            })
            .collect()
    }

    /// The `session_exit` record of a sub-agent says how it ended: the fallback of the other
    /// sources, so it only settles a sub-agent still working.
    pub fn set_sub_agent(&mut self, tool: &str, agent: &str, state: SubAgentState) {
        let Some(&index) = self.tools.get(tool) else {
            return;
        };
        if let Some(Entry::Tool(row)) = self.entries.get_mut(index) {
            if let Some(found) = row
                .sub_agents
                .iter_mut()
                .find(|a| a.id == agent && a.state.is_working())
            {
                if found.state != state {
                    found.state = state;
                    self.dirty.insert(index);
                    self.agents_rev += 1;
                }
            }
        }
    }

    /// Counts the changes of the sub-agents of the rows (new ones, new states): what the list of
    /// sub-agents is built from.
    pub fn agents_rev(&self) -> u64 {
        self.agents_rev
    }

    /// The rows that list sub-agents, in the order of the conversation.
    pub fn agent_rows(&self) -> impl Iterator<Item = &Tool> {
        self.entries.iter().filter_map(|entry| match entry {
            Entry::Tool(tool) if !tool.sub_agents.is_empty() => Some(tool),
            _ => None,
        })
    }

    /// Every sub-agent still working is interrupted, every tool call still working stops
    /// spinning, and the agent message being written is complete.
    pub fn interrupt_working(&mut self) {
        self.close_open();
        for (index, entry) in self.entries.iter_mut().enumerate() {
            if let Entry::Tool(tool) = entry {
                let mut changed = false;
                for agent in tool.sub_agents.iter_mut().filter(|a| a.state.is_working()) {
                    agent.state = SubAgentState::Interrupted;
                    changed = true;
                    self.agents_rev += 1;
                }
                if tool.status == ToolStatus::Working {
                    tool.status = ToolStatus::Failed;
                    changed = true;
                }
                if changed {
                    self.dirty.insert(index);
                }
            }
        }
    }

    /// The job ended: the agent message is complete, every sub-agent still working is interrupted,
    /// every tool call still working stops spinning, and the end line closes the conversation.
    pub fn finish(&mut self, end: EndLine) {
        self.interrupt_working();
        self.push(Entry::End(end));
    }

    // -----------------------------------------------------------------------------------------
    // The transcript of a sub-agent: whole messages, not chunks
    // -----------------------------------------------------------------------------------------

    /// Whether the assignment of a sub-agent is already the first entry.
    pub fn has_assignment(&self) -> bool {
        self.entries
            .iter()
            .any(|entry| matches!(entry, Entry::Assignment { .. }))
    }

    /// The assignment a sub-agent received, labelled with the name of who started it.
    pub fn assignment(&mut self, from: &str, text: &str) {
        self.push(Entry::Assignment {
            from: from.to_owned(),
            text: text.to_owned(),
        });
    }

    /// A complete message of the agent.
    pub fn agent_message(&mut self, text: &str) {
        if !text.trim().is_empty() {
            self.push(Entry::Agent {
                text: text.to_owned(),
                complete: true,
            });
        }
    }

    /// A complete block of reasoning.
    pub fn thinking_block(&mut self, text: &str) {
        if !text.trim().is_empty() {
            self.push(Entry::Thinking {
                text: text.to_owned(),
            });
        }
    }

    /// The row of a tool call the transcript says was made; a call seen already is not added again.
    pub fn tool_started(&mut self, tool: Tool) {
        if self.tools.contains_key(&tool.id) {
            return;
        }
        let id = tool.id.clone();
        let index = self.push(Entry::Tool(tool));
        self.tools.insert(id, index);
    }

    /// The result of a tool call ends its row; the `details` of a `task` call list its
    /// sub-agents, and the ones of a `wait` call report how they stand.
    pub fn tool_finished(&mut self, id: &str, is_error: bool, details: Option<&Value>) {
        let Some(&index) = self.tools.get(id) else {
            return;
        };
        if let Some(Entry::Tool(tool)) = self.entries.get_mut(index) {
            let status = if is_error {
                ToolStatus::Failed
            } else {
                ToolStatus::Done
            };
            let mut changed = tool.status != status;
            tool.status = status;
            if let Some(mut agents) = details.and_then(subagents::from_details) {
                if !agents.is_empty() {
                    keep_ended(&tool.sub_agents, &mut agents);
                    if tool.sub_agents != agents {
                        tool.sub_agents = agents;
                        tool.icon = ToolKind::SubAgent;
                        self.agents_rev += 1;
                        changed = true;
                    }
                }
            }
            if changed {
                self.dirty.insert(index);
            }
        }
        for (agent, state) in details
            .map(subagents::wait_states_of_details)
            .unwrap_or_default()
        {
            self.report_state(&agent, state);
        }
    }

    /// The entries that changed since the last call, in index order; empty when none did.
    pub fn take_changes(&mut self) -> Vec<Changed> {
        if self.dirty.is_empty() {
            return Vec::new();
        }
        self.seq += 1;
        std::mem::take(&mut self.dirty)
            .into_iter()
            .filter_map(|index| {
                self.entries.get(index).map(|entry| Changed {
                    index,
                    entry: entry.clone(),
                    at: self.arrived.get(index).copied().unwrap_or_default(),
                })
            })
            .collect()
    }

    /// Forgets the unsent changes (a snapshot was sent that holds them).
    pub fn mark_sent(&mut self) {
        self.dirty.clear();
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn chunk(kind: &str, text: &str) -> Value {
        json!({ "sessionUpdate": kind, "content": { "type": "text", "text": text } })
    }

    #[test]
    fn agent_chunks_join_into_one_message_until_another_entry_starts() {
        let mut c = Conversation::new();
        c.instruction("Use the weaver-scan skill on the whole System.");
        c.apply_update(&chunk("agent_message_chunk", "Reading "));
        c.apply_update(&chunk("agent_message_chunk", "the code."));
        assert_eq!(c.entries().len(), 2);
        assert_eq!(
            c.entries()[1],
            Entry::Agent {
                text: "Reading the code.".into(),
                complete: false
            }
        );
        c.apply_update(&json!({ "sessionUpdate": "tool_call", "toolCallId": "t1", "title": "Read a.rs", "kind": "read", "status": "pending" }));
        assert_eq!(
            c.entries()[1],
            Entry::Agent {
                text: "Reading the code.".into(),
                complete: true
            },
            "the next entry completes it"
        );
        c.apply_update(&chunk("agent_message_chunk", "Done."));
        assert_eq!(c.entries().len(), 4);
    }

    #[test]
    fn reasoning_is_its_own_block_and_empty_chunks_add_nothing() {
        let mut c = Conversation::new();
        c.apply_update(&chunk("agent_thought_chunk", "Let me "));
        c.apply_update(&chunk("agent_thought_chunk", "think"));
        c.apply_update(&chunk("agent_message_chunk", ""));
        c.apply_update(&chunk("agent_thought_chunk", ""));
        assert_eq!(
            c.entries(),
            [Entry::Thinking {
                text: "Let me think".into()
            }]
        );
        c.apply_update(&chunk("agent_message_chunk", "Hello"));
        c.apply_update(&chunk("agent_thought_chunk", "more"));
        assert_eq!(
            c.entries().len(),
            3,
            "a thought after a message is a new block"
        );
    }

    #[test]
    fn a_tool_call_is_one_row_that_its_updates_change_in_place() {
        let mut c = Conversation::new();
        c.apply_update(&json!({ "sessionUpdate": "tool_call", "toolCallId": "t1", "title": "$ cargo test --workspace", "kind": "execute", "status": "in_progress", "rawInput": { "command": "cargo test --workspace" } }));
        let Entry::Tool(tool) = &c.entries()[0] else {
            panic!()
        };
        assert_eq!(
            (tool.icon, tool.status, tool.command.as_deref()),
            (
                ToolKind::Command,
                ToolStatus::Working,
                Some("cargo test --workspace")
            )
        );
        c.apply_update(&json!({ "sessionUpdate": "tool_call_update", "toolCallId": "t1", "status": "completed", "rawOutput": "ignored output" }));
        c.apply_update(&json!({ "sessionUpdate": "tool_call_update", "toolCallId": "t2", "status": "failed", "title": "Search x" }));
        assert_eq!(c.entries().len(), 2);
        let Entry::Tool(done) = &c.entries()[0] else {
            panic!()
        };
        assert_eq!(done.status, ToolStatus::Done);
        assert_eq!(done.title, "$ cargo test --workspace");
        let Entry::Tool(unseen) = &c.entries()[1] else {
            panic!()
        };
        assert_eq!(
            (unseen.title.as_str(), unseen.status),
            ("Search x", ToolStatus::Failed)
        );
    }

    #[test]
    fn the_icon_follows_the_kind_of_the_call() {
        let kind = |value: Value| tool_of(&value).unwrap().icon;
        assert_eq!(
            kind(json!({ "toolCallId": "a", "kind": "read" })),
            ToolKind::Read
        );
        assert_eq!(
            kind(json!({ "toolCallId": "a", "kind": "search" })),
            ToolKind::Search
        );
        assert_eq!(
            kind(json!({ "toolCallId": "a", "kind": "edit" })),
            ToolKind::Edit
        );
        assert_eq!(
            kind(json!({ "toolCallId": "a", "kind": "delete" })),
            ToolKind::Edit
        );
        assert_eq!(
            kind(json!({ "toolCallId": "a", "kind": "think" })),
            ToolKind::Other
        );
        assert_eq!(
            kind(json!({ "toolCallId": "a", "kind": "execute", "title": "$ ls" })),
            ToolKind::Command
        );
        assert_eq!(
            kind(json!({ "toolCallId": "a", "title": "Task: 2 agents" })),
            ToolKind::SubAgent
        );
        assert!(tool_of(&json!({ "title": "no id" })).is_none());
    }

    #[test]
    fn the_sub_agents_of_a_task_call_are_listed_under_its_row_and_interrupted_when_the_job_ends() {
        let mut c = Conversation::new();
        c.apply_update(&json!({ "sessionUpdate": "tool_call", "toolCallId": "t1", "title": "Task", "status": "in_progress" }));
        c.apply_update(&json!({ "sessionUpdate": "tool_call_update", "toolCallId": "t1", "rawOutput": { "details": { "progress": [
            { "id": "0-A", "name": "Reader", "status": "running" },
            { "id": "1-B", "name": "Writer", "status": "completed" },
        ] } } }));
        let Entry::Tool(tool) = &c.entries()[0] else {
            panic!()
        };
        assert_eq!(tool.icon, ToolKind::SubAgent);
        assert_eq!(
            tool.sub_agents
                .iter()
                .map(|a| (a.name.as_str(), a.state))
                .collect::<Vec<_>>(),
            [
                ("Reader", SubAgentState::Working),
                ("Writer", SubAgentState::Succeeded)
            ]
        );
        // The call completed while Reader still works: it runs in the background.
        c.apply_update(&json!({ "sessionUpdate": "tool_call_update", "toolCallId": "t1", "status": "completed" }));
        assert_eq!(c.background_agents(), [("t1".to_owned(), "0-A".to_owned())]);
        c.set_sub_agent("t1", "0-A", SubAgentState::Failed);
        assert!(c.background_agents().is_empty());

        let mut ended = Conversation::new();
        ended.apply_update(&json!({ "sessionUpdate": "tool_call", "toolCallId": "t", "title": "Task", "rawOutput": { "details": { "progress": [{ "id": "x", "status": "pending" }] } } }));
        ended.finish(EndLine::Stopped);
        let Entry::Tool(tool) = &ended.entries()[0] else {
            panic!()
        };
        assert_eq!(tool.sub_agents[0].state, SubAgentState::Interrupted);
        assert_eq!(
            tool.status,
            ToolStatus::Failed,
            "no row spins after the end"
        );
        assert_eq!(ended.entries()[1], Entry::End(EndLine::Stopped));
    }

    #[test]
    fn messages_of_the_developer_show_their_delivery_until_it_is_settled() {
        let mut c = Conversation::new();
        c.developer_message(7, "Use the old parser", Delivery::Queued);
        c.set_delivery(7, Delivery::Delivered);
        c.developer_message(8, "Stop", Delivery::Queued);
        c.set_delivery(
            8,
            Delivery::NotDelivered {
                reason: "the job ended".into(),
            },
        );
        c.set_delivery(9, Delivery::Delivered);
        assert_eq!(
            c.entries()[0],
            Entry::Developer {
                message: 7,
                text: "Use the old parser".into(),
                delivery: Delivery::Delivered
            }
        );
        assert_eq!(
            c.entries()[1],
            Entry::Developer {
                message: 8,
                text: "Stop".into(),
                delivery: Delivery::NotDelivered {
                    reason: "the job ended".into()
                }
            }
        );
    }

    #[test]
    fn a_permission_card_keeps_omps_options_and_ends_as_chosen_void_or_cancelled() {
        let mut c = Conversation::new();
        let options = json!([
            { "optionId": "allow", "name": "Allow once", "kind": "allow_once" },
            { "optionId": "always", "name": "Always allow", "kind": "allow_always" },
            { "optionId": "no", "name": "Reject", "kind": "reject_once" },
        ]);
        c.permission(1, &json!({ "toolCallId": "t9", "title": "$ rm -rf build", "kind": "execute", "rawInput": { "command": "rm -rf build" } }), &options);
        c.permission(
            2,
            &json!({ "toolCallId": "t10", "title": "Edit a.rs" }),
            &options,
        );
        c.permission(
            3,
            &json!({ "toolCallId": "t11", "title": "Edit b.rs" }),
            &options,
        );
        let Entry::Permission {
            tool,
            options,
            state,
            ..
        } = &c.entries()[0]
        else {
            panic!()
        };
        assert_eq!(
            tool.as_ref().unwrap().command.as_deref(),
            Some("rm -rf build")
        );
        assert_eq!(
            options
                .iter()
                .map(|o| (o.name.as_str(), o.rejects))
                .collect::<Vec<_>>(),
            [
                ("Allow once", false),
                ("Always allow", false),
                ("Reject", true)
            ]
        );
        assert_eq!(state, &PermissionState::Asking);
        assert_eq!(c.asking(), [1, 2, 3]);
        assert_eq!(c.option_name(1, "always").as_deref(), Some("Always allow"));
        c.close_permission(
            1,
            PermissionState::Chosen {
                name: "Allow once".into(),
            },
        );
        c.close_permission(2, PermissionState::Void);
        c.close_permission(3, PermissionState::Cancelled);
        c.close_permission(3, PermissionState::Void);
        assert!(c.asking().is_empty());
        let states: Vec<&PermissionState> = c
            .entries()
            .iter()
            .filter_map(|e| {
                if let Entry::Permission { state, .. } = e {
                    Some(state)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(
            states,
            [
                &PermissionState::Chosen {
                    name: "Allow once".into()
                },
                &PermissionState::Void,
                &PermissionState::Cancelled
            ]
        );
    }

    #[test]
    fn the_replayed_history_rebuilds_the_instruction_the_messages_and_the_reload_line() {
        let mut c = Conversation::new();
        c.apply_update(&chunk(
            "user_message_chunk",
            "Use the weaver-scan skill on the whole System.",
        ));
        c.apply_update(&chunk("agent_message_chunk", "On it."));
        c.apply_update(&chunk("user_message_chunk", "Skip the tests"));
        c.apply_update(&chunk("user_message_chunk", RESUME_PROMPT));
        assert!(
            matches!(&c.entries()[0], Entry::Instruction { text } if text.starts_with("Use the weaver-scan"))
        );
        assert!(
            matches!(&c.entries()[2], Entry::Developer { text, delivery: Delivery::Delivered, .. } if text == "Skip the tests")
        );
        assert_eq!(c.entries()[3], Entry::Reloaded);
    }

    #[test]
    fn the_replayed_history_of_an_ask_conversation_shows_only_the_developers_message() {
        let mut c = Conversation::ask();
        c.apply_update(&chunk(
            "user_message_chunk",
            &ask::first_prompt("Weaver: Ask more\n\nrules\n", "Why is it like this?"),
        ));
        c.apply_update(&chunk("agent_message_chunk", "Because."));
        c.apply_update(&chunk("user_message_chunk", "And then?"));
        assert!(matches!(
            &c.entries()[0],
            Entry::Developer { text, delivery: Delivery::Delivered, .. } if text == "Why is it like this?"
        ));
        assert!(matches!(
            &c.entries()[2],
            Entry::Developer { text, .. } if text == "And then?"
        ));
        assert!(
            !c.entries()
                .iter()
                .any(|e| matches!(e, Entry::Instruction { .. })),
            "the instruction is never an entry"
        );
    }

    #[test]
    fn the_replayed_history_never_shows_the_block_of_tagged_assets() {
        let sent = |text: &str| tags::with_block(text, &["logo.png".to_owned()]);
        let mut job = Conversation::new();
        job.apply_update(&chunk("user_message_chunk", "Use the weaver-scan skill."));
        job.apply_update(&chunk("user_message_chunk", &sent("Look at @logo.png")));
        assert!(matches!(
            &job.entries()[1],
            Entry::Developer { text, .. } if text == "Look at @logo.png"
        ));
        let mut chat = Conversation::general();
        chat.apply_update(&chunk("user_message_chunk", &sent("Look at @logo.png")));
        assert!(matches!(
            &chat.entries()[0],
            Entry::Developer { text, .. } if text == "Look at @logo.png"
        ));
        let mut ask = Conversation::ask();
        ask.apply_update(&chunk(
            "user_message_chunk",
            &ask::first_prompt("Weaver: Ask more\n\nrules\n", &sent("Why @logo.png?")),
        ));
        assert!(matches!(
            &ask.entries()[0],
            Entry::Developer { text, .. } if text == "Why @logo.png?"
        ));
    }

    #[test]
    fn changes_come_in_batches_in_index_order_and_the_sequence_counts_them() {
        let mut c = Conversation::new();
        assert!(c.take_changes().is_empty());
        assert_eq!(c.seq(), 0);
        c.instruction("x");
        c.apply_update(&chunk("agent_message_chunk", "a"));
        let batch = c.take_changes();
        assert_eq!(batch.iter().map(|c| c.index).collect::<Vec<_>>(), [0, 1]);
        assert_eq!(c.seq(), 1);
        c.apply_update(&chunk("agent_message_chunk", "b"));
        let batch = c.take_changes();
        assert_eq!(batch.len(), 1);
        assert_eq!(
            batch[0].entry,
            Entry::Agent {
                text: "ab".into(),
                complete: false
            }
        );
        assert_eq!(c.seq(), 2);
        assert!(c.take_changes().is_empty());
        assert_eq!(c.seq(), 2, "an empty batch is not a batch");
    }

    #[test]
    fn the_end_completes_the_message_and_closes_with_the_line() {
        let mut c = Conversation::new();
        c.apply_update(&chunk("agent_message_chunk", "All done"));
        c.finish(EndLine::Failed {
            reason: "the agent exited with code 3".into(),
            details: Some("boom".into()),
        });
        assert_eq!(
            c.entries()[0],
            Entry::Agent {
                text: "All done".into(),
                complete: true
            }
        );
        assert!(
            matches!(&c.entries()[1], Entry::End(EndLine::Failed { reason, .. }) if reason == "the agent exited with code 3")
        );
    }

    #[test]
    fn the_wire_form_is_tagged_and_camel_case() {
        let json = serde_json::to_value(Entry::Developer {
            message: 3,
            text: "hi".into(),
            delivery: Delivery::NotDelivered {
                reason: "busy".into(),
            },
        })
        .unwrap();
        assert_eq!(
            json,
            json!({ "kind": "developer", "message": 3, "text": "hi", "delivery": { "status": "not-delivered", "reason": "busy" } })
        );
        let json = serde_json::to_value(Entry::Tool(Tool {
            id: "t".into(),
            title: "x".into(),
            icon: ToolKind::SubAgent,
            command: None,
            status: ToolStatus::Done,
            sub_agents: vec![],
        }))
        .unwrap();
        assert_eq!(json["kind"], "tool");
        assert_eq!(json["subAgents"], json!([]));
        assert_eq!(json["status"], "done");
    }

    fn task_rows(c: &Conversation) -> Vec<(String, SubAgentState)> {
        c.agent_rows()
            .flat_map(|row| row.sub_agents.iter().map(|a| (a.id.clone(), a.state)))
            .collect()
    }

    #[test]
    fn a_wait_call_settles_the_background_sub_agents_and_nothing_takes_an_ended_one_back_to_working(
    ) {
        let mut c = Conversation::new();
        c.apply_update(&json!({ "sessionUpdate": "tool_call", "toolCallId": "t1", "title": "Task", "status": "in_progress" }));
        c.apply_update(&json!({ "sessionUpdate": "tool_call_update", "toolCallId": "t1", "status": "completed", "rawOutput": { "details": { "progress": [
            { "id": "0-A", "name": "A", "status": "pending" },
            { "id": "1-B", "name": "B", "status": "pending" },
            { "id": "__advisor.turn", "status": "running" },
        ] } } }));
        assert_eq!(
            task_rows(&c),
            [
                ("0-A".to_owned(), SubAgentState::Working),
                ("1-B".to_owned(), SubAgentState::Working)
            ]
        );
        assert_eq!(c.background_agents().len(), 2);
        let revision = c.agents_rev();

        // The wait names one of them, and a background command that is no sub-agent.
        c.apply_update(&json!({ "sessionUpdate": "tool_call", "toolCallId": "w1", "title": "Wait", "status": "completed", "rawOutput": { "details": { "jobs": [
            { "id": "0-A", "status": "completed" },
            { "id": "bg_1", "status": "failed" },
        ] } } }));
        assert_eq!(
            task_rows(&c),
            [
                ("0-A".to_owned(), SubAgentState::Succeeded),
                ("1-B".to_owned(), SubAgentState::Working)
            ]
        );
        assert!(c.agents_rev() > revision);

        // A later update of the task call says it still works: it stays ended. A later wait that
        // says working changes nothing either; one that says failed does.
        c.apply_update(&json!({ "sessionUpdate": "tool_call_update", "toolCallId": "t1", "rawOutput": { "details": { "progress": [
            { "id": "0-A", "name": "A", "status": "running" },
            { "id": "1-B", "name": "B", "status": "running" },
        ] } } }));
        c.apply_update(&json!({ "sessionUpdate": "tool_call_update", "toolCallId": "w1", "rawOutput": { "details": { "jobs": [
            { "id": "0-A", "status": "running" },
            { "id": "1-B", "status": "failed" },
        ] } } }));
        assert_eq!(
            task_rows(&c),
            [
                ("0-A".to_owned(), SubAgentState::Succeeded),
                ("1-B".to_owned(), SubAgentState::Failed)
            ]
        );
        assert!(c.background_agents().is_empty());
    }

    #[test]
    fn the_exit_record_only_settles_a_sub_agent_still_working() {
        let mut c = Conversation::new();
        c.apply_update(&json!({ "sessionUpdate": "tool_call", "toolCallId": "t1", "title": "Task", "status": "completed", "rawOutput": { "details": { "progress": [
            { "id": "0-A", "status": "pending" },
            { "id": "1-B", "status": "completed" },
        ] } } }));
        c.set_sub_agent("t1", "0-A", SubAgentState::Failed);
        c.set_sub_agent("t1", "1-B", SubAgentState::Interrupted);
        assert_eq!(
            task_rows(&c),
            [
                ("0-A".to_owned(), SubAgentState::Failed),
                ("1-B".to_owned(), SubAgentState::Succeeded)
            ]
        );
    }
}
