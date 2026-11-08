//! The sub-agents of one owner (a job, or General): who they are, how they stand and what they
//! said (`requirements/tech#job`, "Sub-agents").
//!
//! The direct sub-agents are the ones the owner's `task` rows list. Each of them has a
//! conversation built from its transcript, and the `task` rows of that conversation list the ones
//! it started, at any depth. The state of a sub-agent lives in the row that lists it (updated by
//! the `task` and `wait` calls of omp and, as a fallback, by the `session_exit` record of its
//! transcript); nothing here keeps a second copy.
//!
//! [`Agents`] holds no lock and does no I/O. The owner keeps it next to its conversation, under
//! its own lock, and drives it in three steps (see [`super::poller`]): [`Agents::plan`] says which
//! transcripts to read, the poller reads them outside the lock, and [`Agents::apply`] takes the
//! result in. [`Agents::collect`] tells the UI what changed.

use std::path::PathBuf;

use weaver_core::jobs::subagents::SubAgentState;

use super::reader::{Done, Location, ReadJob};
use super::records;
use super::wire::{
    AgentNode, Outgoing, Owner, Step, SubAgentConversation, SubAgentEntries, SubAgentsEvent,
};
use crate::jobs::conversation::{Conversation, ToolStatus};

/// The instant a sub-agent is first seen.
fn now() -> String {
    weaver_core::project::clock::now_instant(&weaver_core::project::clock::SystemClock)
        .unwrap_or_default()
}

/// What to read now.
#[derive(Debug)]
pub struct Plan {
    pub location: Location,
    /// The main session file of the owner, when found already.
    pub main: Option<PathBuf>,
    pub jobs: Vec<ReadJob>,
    /// The generation of the tree this was planned for: see [`Agents::apply`].
    pub generation: u64,
}

#[derive(Debug, Default)]
struct Node {
    parent: Option<usize>,
    /// The `task` call that launched it.
    call: String,
    id: String,
    name: String,
    /// When Weaver first saw it, until its transcript says better.
    started: String,
    conversation: Conversation,
    file: Option<PathBuf>,
    offset: u64,
    /// Why the transcript cannot be read now.
    error: Option<String>,
    /// A malformed record: the transcript is not read any more.
    broken: bool,
    /// Its transcript was read to the end after the sub-agent ended.
    settled: bool,
    /// The error the UI was told last.
    told_error: Option<String>,
}

/// The sub-agents of an owner.
#[derive(Debug, Default)]
pub struct Agents {
    generation: u64,
    location: Option<Location>,
    main: Option<PathBuf>,
    nodes: Vec<Node>,
    /// The owner's list of sub-agents is on screen: every transcript is read, to find the nested ones.
    list_shown: bool,
    /// The conversation on screen.
    shown: Option<Vec<Step>>,
    /// The owner's `agents_rev` when the list was last built.
    seen_rev: u64,
    dirty: bool,
    last_list: Option<Vec<AgentNode>>,
    /// The shown conversation was rebuilt: the UI fetches it again.
    reset_shown: bool,
}

impl Agents {
    pub fn new() -> Self {
        Self::default()
    }

    /// Where the transcripts are: set when the session of the owner is open.
    pub fn locate(&mut self, location: Location) {
        self.location = Some(location);
        self.main = None;
    }

    /// The history of a reloaded session was replayed: the sub-agents are rebuilt from it and the
    /// transcripts are read again from the start.
    pub fn reset(&mut self) {
        self.generation += 1;
        self.nodes.clear();
        self.main = None;
        self.seen_rev = 0;
        // The list told last stays: the next one is told if it differs.
        self.dirty = true;
        self.reset_shown = self.shown.is_some();
    }

    /// What the UI shows now: whether the list of sub-agents is on screen, and the conversation of
    /// which sub-agent, if any.
    pub fn interest(&mut self, list_shown: bool, shown: Option<Vec<Step>>) {
        self.list_shown = list_shown;
        self.shown = shown;
    }

    /// The nodes of the sub-agents the rows of `conversation` list, created when new.
    fn sync(&mut self, root: &Conversation) {
        let mut parent: Option<usize> = None;
        let mut next: usize = 0;
        loop {
            let wanted: Vec<(String, String, String)> = match parent {
                None => rows_of(root),
                Some(index) => rows_of(&self.nodes[index].conversation),
            };
            for (call, id, name) in wanted {
                match self.find(parent, &call, &id) {
                    Some(index) => {
                        if self.nodes[index].name != name {
                            self.nodes[index].name = name;
                            self.dirty = true;
                        }
                    }
                    None => {
                        self.nodes.push(Node {
                            parent,
                            call,
                            id,
                            name,
                            started: now(),
                            ..Node::default()
                        });
                        self.dirty = true;
                    }
                }
            }
            if next >= self.nodes.len() {
                break;
            }
            parent = Some(next);
            next += 1;
        }
    }

    fn find(&self, parent: Option<usize>, call: &str, id: &str) -> Option<usize> {
        self.nodes
            .iter()
            .position(|n| n.parent == parent && n.call == call && n.id == id)
    }

    fn resolve(&self, path: &[Step]) -> Option<usize> {
        let mut parent: Option<usize> = None;
        for step in path {
            parent = Some(self.find(parent, &step.call, &step.id)?);
        }
        parent
    }

    fn path_of(&self, mut index: usize) -> Vec<Step> {
        let mut path = Vec::new();
        loop {
            let node = &self.nodes[index];
            path.push(Step {
                call: node.call.clone(),
                id: node.id.clone(),
            });
            match node.parent {
                Some(parent) => index = parent,
                None => break,
            }
        }
        path.reverse();
        path
    }

    fn parent_conversation<'a>(&'a self, root: &'a Conversation, index: usize) -> &'a Conversation {
        match self.nodes[index].parent {
            None => root,
            Some(parent) => &self.nodes[parent].conversation,
        }
    }

    /// How the sub-agent stands, and whether its `task` call is over (so that only its session
    /// file can still tell how it ended).
    fn standing(&self, root: &Conversation, index: usize) -> (SubAgentState, bool) {
        let node = &self.nodes[index];
        self.parent_conversation(root, index)
            .agent_rows()
            .find(|row| row.id == node.call)
            .and_then(|row| {
                row.sub_agents
                    .iter()
                    .find(|a| a.id == node.id)
                    .map(|a| (a.state, row.status != ToolStatus::Working))
            })
            .unwrap_or((SubAgentState::Working, false))
    }

    // -----------------------------------------------------------------------------------------
    // The list
    // -----------------------------------------------------------------------------------------

    fn build(&self, parent: Option<usize>, conversation: &Conversation) -> Vec<AgentNode> {
        conversation
            .agent_rows()
            .flat_map(|row| row.sub_agents.iter().map(move |agent| (row, agent)))
            .filter_map(|(row, agent)| {
                let index = self.find(parent, &row.id, &agent.id)?;
                let node = &self.nodes[index];
                Some(AgentNode {
                    call: row.id.clone(),
                    id: agent.id.clone(),
                    name: agent.name.clone(),
                    state: agent.state,
                    started: Some(node.started.clone()),
                    children: self.build(Some(index), &node.conversation),
                })
            })
            .collect()
    }

    /// The direct sub-agents of the owner, in launch order, each with the ones it started. The
    /// list is taken as told: only a later difference is told again.
    pub fn list(&mut self, root: &Conversation) -> Vec<AgentNode> {
        self.sync(root);
        let list = self.build(None, root);
        self.seen_rev = root.agents_rev();
        self.dirty = false;
        self.last_list = Some(list.clone());
        list
    }

    /// The conversation of the sub-agent at `path`.
    pub fn conversation(
        &mut self,
        root: &Conversation,
        project_id: &str,
        owner: &Owner,
        path: &[Step],
    ) -> Result<SubAgentConversation, String> {
        self.sync(root);
        let index = self
            .resolve(path)
            .ok_or_else(|| "the sub-agent does not exist".to_owned())?;
        let node = &mut self.nodes[index];
        // The snapshot holds every change so far: the events that follow start after it.
        node.conversation.mark_sent();
        node.told_error.clone_from(&node.error);
        Ok(SubAgentConversation {
            project_id: project_id.to_owned(),
            owner: owner.clone(),
            path: path.to_vec(),
            seq: node.conversation.seq(),
            entries: node.conversation.entries().to_vec(),
            arrivals: node.conversation.arrivals().to_vec(),
            unavailable: node.error.clone(),
        })
    }

    // -----------------------------------------------------------------------------------------
    // Reading
    // -----------------------------------------------------------------------------------------

    /// The transcripts to read now: the one shown, all of them while the list is shown, and the
    /// ones whose end only the session file can tell. `None` when there is nothing to read.
    pub fn plan(&mut self, root: &Conversation) -> Option<Plan> {
        self.sync(root);
        let location = self.location.clone()?;
        let shown = self.shown.as_ref().and_then(|path| self.resolve(path));
        let mut jobs = Vec::new();
        for (index, node) in self.nodes.iter().enumerate() {
            if node.settled || node.broken {
                continue;
            }
            let (state, call_over) = self.standing(root, index);
            let awaiting_exit = state.is_working() && call_over;
            if !(self.list_shown || shown == Some(index) || awaiting_exit) {
                continue;
            }
            let parent_file = node.parent.and_then(|p| self.nodes[p].file.clone());
            jobs.push(ReadJob {
                index,
                id: node.id.clone(),
                file: node.file.clone(),
                parent_file,
                direct: node.parent.is_none(),
                offset: node.offset,
                final_pass: !state.is_working(),
            });
        }
        (!jobs.is_empty()).then(|| Plan {
            generation: self.generation,
            location,
            main: self.main.clone(),
            jobs,
        })
    }

    /// Takes in what the reading found. `generation` is the one `plan` was made in: the result of
    /// a plan older than the last [`Agents::reset`] is dropped.
    pub fn apply(&mut self, root: &mut Conversation, generation: u64, done: Vec<Done>) {
        if generation != self.generation {
            return;
        }
        for done in done {
            let Some(node) = self.nodes.get_mut(done.index) else {
                continue;
            };
            if let Some(main) = done.main {
                self.main = Some(main);
            }
            if let Some(file) = done.file {
                node.file = Some(file);
            }
            match done.outcome {
                Err(reason) => node.error = Some(reason),
                Ok(chunk) => {
                    node.error = None;
                    self.take_lines(root, done.index, &chunk.lines);
                    let node = &mut self.nodes[done.index];
                    if !node.broken {
                        node.offset = chunk.end;
                    }
                }
            }
            if done.final_pass {
                self.nodes[done.index].settled = true;
            }
            self.dirty = true;
        }
        self.sync(root);
    }

    fn take_lines(&mut self, root: &mut Conversation, index: usize, lines: &[String]) {
        let from = match self.nodes[index].parent {
            None => self
                .location
                .as_ref()
                .map(|l| l.owner_name.clone())
                .unwrap_or_default(),
            Some(parent) => self.nodes[parent].name.clone(),
        };
        let mut exit = None;
        for line in lines {
            let Ok(record) = serde_json::from_str::<serde_json::Value>(line) else {
                let node = &mut self.nodes[index];
                node.error = Some("a record of the transcript is malformed".to_owned());
                node.broken = true;
                break;
            };
            let node = &mut self.nodes[index];
            let applied = records::apply(&mut node.conversation, &from, &record);
            if let Some(started) = applied.started {
                node.started = started;
            }
            exit = applied.exit.or(exit);
        }
        if let Some(state) = exit {
            let (call, id, parent) = {
                let node = &self.nodes[index];
                (node.call.clone(), node.id.clone(), node.parent)
            };
            match parent {
                None => root.set_sub_agent(&call, &id, state),
                Some(parent) => self.nodes[parent]
                    .conversation
                    .set_sub_agent(&call, &id, state),
            }
        }
    }

    /// The owner ended, or omp exited: every sub-agent still working is interrupted, the nested
    /// ones included. The owner does its own rows; these are the ones inside the transcripts.
    pub fn interrupt(&mut self) {
        for node in &mut self.nodes {
            node.conversation.interrupt_working();
        }
        self.dirty = true;
    }

    // -----------------------------------------------------------------------------------------
    // What the UI is told
    // -----------------------------------------------------------------------------------------

    /// What changed since the last call: the whole list when it differs from the one told last,
    /// and the batch of changes of the conversation on screen.
    pub fn collect(
        &mut self,
        root: &Conversation,
        project_id: &str,
        owner: &Owner,
    ) -> Vec<Outgoing> {
        let mut out = Vec::new();
        if self.dirty || root.agents_rev() != self.seen_rev {
            self.seen_rev = root.agents_rev();
            self.dirty = false;
            self.sync(root);
            let list = self.build(None, root);
            let told = self.last_list.as_deref().unwrap_or_default();
            if told != list.as_slice() {
                out.push(Outgoing::List(SubAgentsEvent {
                    project_id: project_id.to_owned(),
                    owner: owner.clone(),
                    nodes: list.clone(),
                }));
            }
            self.last_list = Some(list);
        }
        let shown = self.shown.as_ref().and_then(|path| self.resolve(path));
        let path = shown.map(|index| self.path_of(index));
        let reset = shown.is_some() && std::mem::take(&mut self.reset_shown);
        for (index, node) in self.nodes.iter_mut().enumerate() {
            let Some(path) = path.as_ref().filter(|_| Some(index) == shown) else {
                // Nobody looks: the snapshot taken when somebody does holds these changes.
                node.conversation.mark_sent();
                continue;
            };
            let changes = node.conversation.take_changes();
            if changes.is_empty() && node.told_error == node.error && !reset {
                continue;
            }
            node.told_error.clone_from(&node.error);
            out.push(Outgoing::Entries(SubAgentEntries {
                project_id: project_id.to_owned(),
                owner: owner.clone(),
                path: path.clone(),
                seq: node.conversation.seq(),
                reset,
                unavailable: node.error.clone(),
                changes,
            }));
        }
        out
    }
}

/// `(call, id, name)` of every sub-agent the rows of `conversation` list, in order.
fn rows_of(conversation: &Conversation) -> Vec<(String, String, String)> {
    conversation
        .agent_rows()
        .flat_map(|row| {
            row.sub_agents
                .iter()
                .map(move |agent| (row.id.clone(), agent.id.clone(), agent.name.clone()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use serde_json::{json, Value};

    use super::*;
    use crate::jobs::conversation::Entry;
    use crate::subagents::reader;

    struct Sessions {
        _dir: tempfile::TempDir,
        root: PathBuf,
        main: PathBuf,
    }

    impl Sessions {
        fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path().join("sessions");
            let folder = root.join("--project--");
            std::fs::create_dir_all(&folder).unwrap();
            let main = folder.join("2026-11-02T07-00-00-000Z_sess-1.jsonl");
            std::fs::write(&main, "{}\n").unwrap();
            Sessions {
                _dir: dir,
                root,
                main,
            }
        }

        /// The folder of the transcripts of the sub-agents of `parent` (the main file, or a transcript).
        fn folder(parent: &std::path::Path) -> PathBuf {
            let name = parent.file_name().unwrap().to_str().unwrap();
            parent.with_file_name(name.strip_suffix(".jsonl").unwrap())
        }

        /// Appends `text` to the transcript `<folder of parent>/<id>.jsonl`.
        fn append(&self, parent: &std::path::Path, id: &str, text: &str) -> PathBuf {
            let folder = Self::folder(parent);
            std::fs::create_dir_all(&folder).unwrap();
            let file = folder.join(format!("{id}.jsonl"));
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&file)
                .unwrap()
                .write_all(text.as_bytes())
                .unwrap();
            file
        }

        fn location(&self) -> Location {
            Location {
                root: self.root.clone(),
                session: "sess-1".into(),
                owner_name: "Run T1".into(),
            }
        }
    }

    fn line(value: &Value) -> String {
        format!("{value}\n")
    }

    fn user(text: &str) -> String {
        line(
            &json!({ "type": "message", "message": { "role": "user", "content": [{ "type": "text", "text": text }] } }),
        )
    }

    fn assistant(blocks: Value) -> String {
        line(&json!({ "type": "message", "message": { "role": "assistant", "content": blocks } }))
    }

    fn exit(kind: &str) -> String {
        line(&json!({ "type": "custom", "customType": "session_exit", "data": { "kind": kind } }))
    }

    /// A `task` call of the owner with `progress` as its output.
    fn task_call(root: &mut Conversation, call: &str, status: &str, progress: Value) {
        root.apply_update(&json!({ "sessionUpdate": "tool_call", "toolCallId": call, "title": "Task", "status": "in_progress" }));
        root.apply_update(&json!({ "sessionUpdate": "tool_call_update", "toolCallId": call, "status": status, "rawOutput": { "details": { "progress": progress } } }));
    }

    /// One turn of the poller: plan, read, apply. `false` when there was nothing to read.
    fn pump(agents: &mut Agents, root: &mut Conversation) -> bool {
        let Some(plan) = agents.plan(root) else {
            return false;
        };
        let done = plan
            .jobs
            .iter()
            .map(|job| reader::read(&plan.location, plan.main.clone(), job))
            .collect();
        agents.apply(root, plan.generation, done);
        true
    }

    fn owner() -> Owner {
        Owner::Job { id: "job1".into() }
    }

    fn step(call: &str, id: &str) -> Step {
        Step {
            call: call.into(),
            id: id.into(),
        }
    }

    fn shown(agents: &mut Agents, root: &Conversation, path: &[Step]) -> SubAgentConversation {
        agents.conversation(root, "p", &owner(), path).unwrap()
    }

    #[test]
    fn the_shown_transcript_is_read_incrementally_and_a_half_written_line_waits() {
        let sessions = Sessions::new();
        let mut root = Conversation::new();
        task_call(
            &mut root,
            "t1",
            "in_progress",
            json!([{ "id": "0-Reader", "name": "Reader", "status": "running" }]),
        );
        let mut agents = Agents::new();
        agents.locate(sessions.location());
        let path = vec![step("t1", "0-Reader")];

        // Nothing is on screen, and the task call still runs: nothing is read.
        assert!(!pump(&mut agents, &mut root));
        agents.interest(false, Some(path.clone()));
        assert!(pump(&mut agents, &mut root));
        let view = shown(&mut agents, &root, &path);
        assert_eq!(
            view.unavailable.as_deref(),
            Some("the transcript file was not found")
        );
        assert!(view.entries.is_empty());

        let file = sessions.append(
            &sessions.main,
            "0-Reader",
            &line(&json!({ "type": "session", "timestamp": "2026-11-02T08:00:00.000Z" })),
        );
        sessions.append(&sessions.main, "0-Reader", &user("Read the code."));
        let half = assistant(json!([{ "type": "text", "text": "Reading." }]));
        let (first, second) = half.split_at(20);
        sessions.append(&sessions.main, "0-Reader", first);
        assert!(pump(&mut agents, &mut root));
        let view = shown(&mut agents, &root, &path);
        assert_eq!(view.unavailable, None);
        assert_eq!(
            view.entries,
            [Entry::Assignment {
                from: "Run T1".into(),
                text: "Read the code.".into()
            }],
            "the half line is not read"
        );
        let offset = std::fs::metadata(&file).unwrap().len() - first.len() as u64;
        assert_eq!(agents.nodes[0].offset, offset);

        sessions.append(&sessions.main, "0-Reader", second);
        assert!(pump(&mut agents, &mut root));
        let out = agents.collect(&root, "p", &owner());
        let entries: Vec<_> = out
            .iter()
            .filter_map(|o| match o {
                Outgoing::Entries(e) => Some(e),
                Outgoing::List(_) => None,
            })
            .collect();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, path);
        assert_eq!(entries[0].changes.len(), 1);
        assert_eq!(
            entries[0].changes[0].entry,
            Entry::Agent {
                text: "Reading.".into(),
                complete: true
            }
        );
        assert_eq!(agents.nodes[0].started, "2026-11-02T08:00:00.000Z");
        let list = agents.list(&root);
        assert_eq!(list.len(), 1);
        assert_eq!(
            (list[0].name.as_str(), list[0].state),
            ("Reader", SubAgentState::Working)
        );
    }

    #[test]
    fn nested_sub_agents_come_from_the_parent_transcript_and_advisors_never_show() {
        let sessions = Sessions::new();
        let mut root = Conversation::new();
        task_call(
            &mut root,
            "t1",
            "in_progress",
            json!([
                { "id": "0-Lead", "agent": "Lead", "status": "running" },
                { "id": "__advisor.turn", "status": "running" },
            ]),
        );
        let mut agents = Agents::new();
        agents.locate(sessions.location());
        agents.interest(true, None);

        let lead = sessions.append(&sessions.main, "0-Lead", &user("Lead the work."));
        sessions.append(
            &sessions.main,
            "0-Lead",
            &assistant(
                json!([{ "type": "toolCall", "id": "c9", "name": "task", "arguments": {} }]),
            ),
        );
        sessions.append(&sessions.main, "__advisor.turn", &user("advise"));
        sessions.append(&lead, "1-Child", &user("Do a part."));
        sessions.append(&lead, "1-Child", &exit("normal"));
        sessions.append(&lead, "__advisor.final", &user("advise"));
        sessions.append(
            &sessions.main,
            "0-Lead",
            &line(&json!({ "type": "message", "message": { "role": "toolResult", "toolCallId": "c9", "details": { "progress": [
                { "id": "1-Child", "agent": "Worker", "status": "pending" },
                { "id": "__advisor.final", "status": "running" },
            ] } } })),
        );
        while pump(&mut agents, &mut root) && agents.nodes.len() < 2 {}
        pump(&mut agents, &mut root);

        let list = agents.list(&root);
        assert_eq!(list.len(), 1, "{list:?}");
        assert_eq!(
            (list[0].id.as_str(), list[0].name.as_str()),
            ("0-Lead", "Lead")
        );
        assert_eq!(list[0].children.len(), 1, "{list:?}");
        let child = &list[0].children[0];
        assert_eq!(
            (child.call.as_str(), child.id.as_str(), child.name.as_str()),
            ("c9", "1-Child", "Worker")
        );

        // The transcript of the child is read too (the list is shown), and the assignment is
        // labelled with the name of its parent; its exit record settles it.
        assert_eq!(list[0].children[0].state, SubAgentState::Succeeded);
        let path = vec![step("t1", "0-Lead"), step("c9", "1-Child")];
        let view = shown(&mut agents, &root, &path);
        assert_eq!(
            view.entries,
            [Entry::Assignment {
                from: "Lead".into(),
                text: "Do a part.".into()
            }]
        );
        let missing = agents.conversation(&root, "p", &owner(), &[step("t1", "__advisor.turn")]);
        assert!(missing.is_err());
    }

    #[test]
    fn a_background_sub_agent_waits_on_its_exit_record_only_until_a_wait_says_better() {
        let sessions = Sessions::new();
        let mut root = Conversation::new();
        task_call(
            &mut root,
            "t1",
            "completed",
            json!([
                { "id": "0-A", "name": "A", "status": "pending" },
                { "id": "1-B", "name": "B", "status": "pending" },
                { "id": "2-C", "name": "C", "status": "pending" },
            ]),
        );
        let mut agents = Agents::new();
        agents.locate(sessions.location());
        sessions.append(&sessions.main, "0-A", &user("a"));
        sessions.append(&sessions.main, "0-A", &exit("normal"));
        sessions.append(&sessions.main, "1-B", &user("b"));
        sessions.append(&sessions.main, "2-C", &user("c"));

        // A `wait` names B as failed: it never needs its file.
        root.apply_update(&json!({ "sessionUpdate": "tool_call", "toolCallId": "w1", "title": "Wait", "status": "completed",
            "rawOutput": { "details": { "jobs": [{ "id": "1-B", "status": "failed" }, { "id": "bg_1", "status": "completed" }] } } }));
        let plan = agents.plan(&root).unwrap();
        let mut ids: Vec<_> = plan.jobs.iter().map(|j| j.id.as_str()).collect();
        ids.sort_unstable();
        assert_eq!(
            ids,
            ["0-A", "2-C"],
            "nothing is on screen: only the ones whose end is not known"
        );
        assert!(pump(&mut agents, &mut root));

        let states: Vec<_> = agents
            .list(&root)
            .iter()
            .map(|n| (n.id.clone(), n.state))
            .collect();
        assert_eq!(
            states,
            [
                ("0-A".to_owned(), SubAgentState::Succeeded),
                ("1-B".to_owned(), SubAgentState::Failed),
                ("2-C".to_owned(), SubAgentState::Working),
            ]
        );
        // C is read again while it works; the ones that ended are not.
        let plan = agents.plan(&root).unwrap();
        assert_eq!(
            plan.jobs.iter().map(|j| j.id.as_str()).collect::<Vec<_>>(),
            ["2-C"]
        );
        sessions.append(&sessions.main, "2-C", &exit("signal"));
        assert!(pump(&mut agents, &mut root));
        assert_eq!(agents.list(&root)[2].state, SubAgentState::Interrupted);
        assert!(
            agents.plan(&root).is_none(),
            "everything ended: nothing to read"
        );
    }

    #[test]
    fn a_malformed_record_makes_the_conversation_unavailable_and_keeps_the_state() {
        let sessions = Sessions::new();
        let mut root = Conversation::new();
        task_call(
            &mut root,
            "t1",
            "completed",
            json!([{ "id": "0-A", "name": "A", "status": "pending" }]),
        );
        let mut agents = Agents::new();
        agents.locate(sessions.location());
        agents.interest(true, Some(vec![step("t1", "0-A")]));
        sessions.append(&sessions.main, "0-A", &user("a"));
        sessions.append(&sessions.main, "0-A", "this is not json\n");
        assert!(pump(&mut agents, &mut root));
        let view = shown(&mut agents, &root, &[step("t1", "0-A")]);
        assert_eq!(
            view.unavailable.as_deref(),
            Some("a record of the transcript is malformed")
        );
        assert_eq!(
            agents.list(&root)[0].state,
            SubAgentState::Working,
            "the state comes from the other sources"
        );
        assert!(
            agents.plan(&root).is_none(),
            "a malformed transcript is not read again"
        );
    }

    #[test]
    fn the_end_interrupts_the_nested_ones_and_a_reload_reads_everything_again() {
        let sessions = Sessions::new();
        let mut root = Conversation::new();
        task_call(
            &mut root,
            "t1",
            "in_progress",
            json!([{ "id": "0-Lead", "name": "Lead", "status": "running" }]),
        );
        let mut agents = Agents::new();
        agents.locate(sessions.location());
        agents.interest(true, None);
        let lead = sessions.append(&sessions.main, "0-Lead", &user("Lead."));
        sessions.append(
            &sessions.main,
            "0-Lead",
            &assistant(
                json!([{ "type": "toolCall", "id": "c9", "name": "task", "arguments": {} }]),
            ),
        );
        sessions.append(
            &sessions.main,
            "0-Lead",
            &line(&json!({ "type": "message", "message": { "role": "toolResult", "toolCallId": "c9", "details": { "progress": [
                { "id": "1-Child", "name": "Child", "status": "running" },
            ] } } })),
        );
        sessions.append(&lead, "1-Child", &user("Part."));
        while pump(&mut agents, &mut root) && agents.nodes.len() < 2 {}
        assert_eq!(
            agents.list(&root)[0].children[0].state,
            SubAgentState::Working
        );

        root.finish(crate::jobs::conversation::EndLine::Finished);
        agents.interrupt();
        let list = agents.list(&root);
        assert_eq!(list[0].state, SubAgentState::Interrupted);
        assert_eq!(list[0].children[0].state, SubAgentState::Interrupted);

        // The history is replayed: the tree is rebuilt and read from the start.
        let mut replayed = Conversation::new();
        task_call(
            &mut replayed,
            "t1",
            "completed",
            json!([{ "id": "0-Lead", "name": "Lead", "status": "completed" }]),
        );
        agents.reset();
        assert!(agents.nodes.is_empty());
        assert!(pump(&mut agents, &mut replayed));
        assert!(agents.nodes[0].offset > 0);
        let view = shown(&mut agents, &replayed, &[step("t1", "0-Lead")]);
        assert!(view
            .entries
            .iter()
            .any(|e| matches!(e, Entry::Assignment { text, .. } if text == "Lead.")));
    }
}
