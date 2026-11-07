//! Ask more conversations (`requirements/rules#ask-conversation`): the instruction of Weaver.
//!
//! An Ask more conversation is a free conversation with omp about one thing the developer is
//! reading. It is not a job. What is decided without a process lives here as pure functions:
//!
//! * [`instruction`]: the text Weaver sends with the developer's first message, built from the
//!   chosen thing ([`Chosen`]) as the files (or, for a question, the round in memory) say it, and
//!   from the language the developer chose.
//!
//! The text is fixed English carried by the app. It starts with the line `Weaver: Ask more`, then
//! the fixed rules for the agent, then the part that describes the chosen thing, then the commands
//! that read it. Unlike the one-line instruction of a job it spans several lines and holds texts of
//! cards. It names no skill, except the Requirement skills (`weaver-req-*`) of the Requirements
//! part, never copies text of the Requirements and never holds the developer's choices, written
//! text, answers or skips: the types below cannot even carry them.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::assets::tags;
use crate::question::request::{Question, RoundRequest};
use crate::settings::{ModelSettings, Permissions};

use super::general::clean_language;
use super::launch::approval_mode;

/// A card named in another card, with its title.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Titled {
    /// The permanent identity.
    pub id: String,
    pub title: String,
}

/// A Task card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskPart {
    pub id: String,
    /// The short number the developer sees (`T3`), when the card has one.
    pub short: Option<String>,
    pub title: String,
    /// The text after the title, in full.
    pub body: String,
    pub refs: Vec<String>,
    /// Project-relative paths of its images.
    pub images: Vec<String>,
    /// The Proposal whose design the Task belongs to.
    pub proposal: Option<Titled>,
    /// The Drift the Task fixes (permanent identity).
    pub drift: Option<String>,
}

/// A Drift a resolution Proposal resolves, with what was decided for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedDrift {
    pub id: String,
    pub decision: String,
}

/// A Proposal card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposalPart {
    pub id: String,
    pub short: Option<String>,
    pub title: String,
    pub state: String,
    pub body: String,
    /// Its Tasks, in card order.
    pub tasks: Vec<Titled>,
    /// For a resolution, each Drift it resolves.
    pub drifts: Vec<ResolvedDrift>,
}

/// A Drift card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriftPart {
    pub id: String,
    pub short: Option<String>,
    pub title: String,
    pub what_happens: String,
    pub what_should_happen: String,
    pub refs: Vec<String>,
    /// The Proposal attached to it.
    pub proposal: Option<Titled>,
    /// The Task it was born from (permanent identity).
    pub task: Option<String>,
    /// The Drift it repeats (permanent identity).
    pub repeats: Option<String>,
}

/// A Task cited by a History entry, as the copy in the entry (or, failing that, its file) keeps it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryTaskPart {
    /// The permanent identity.
    pub id: String,
    /// `None` when neither the copy nor the file can be read: the Task is not available.
    pub copy: Option<HistoryTaskCopy>,
}

/// What can be read of a Task cited by a History entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryTaskCopy {
    pub title: String,
    pub body: String,
    pub refs: Vec<String>,
    /// The Proposal whose design the Task belonged to (permanent identity).
    pub proposal: Option<String>,
    /// Whether that Proposal still exists: only then `weaver proposal show` is listed.
    pub proposal_exists: bool,
    pub label: Option<String>,
    pub finished: String,
}

/// A History entry with the Tasks it cites.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEntryPart {
    pub id: String,
    pub title: String,
    pub summary: String,
    /// The text after the title, in full.
    pub text: String,
    /// The cited Tasks, in the order cited.
    pub tasks: Vec<HistoryTaskPart>,
}

/// Who asked a round.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuestionOrigin {
    /// The agent of a job.
    Job { name: String, id: String },
    /// The agent of the project's Chat: the question belongs to its active card of questions.
    Chat,
    /// Weaver itself: the prepared question of a Drift (permanent identity).
    Drift { id: String },
}

/// The question in view of an open round or of the active card of questions of Chat: the questions
/// only, never the developer's side of them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestionPart<'a> {
    pub request: &'a RoundRequest,
    /// The position of the question in view.
    pub index: usize,
    pub origin: QuestionOrigin,
}

/// The Proposal shown over the Requirements view while it is still open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Overlay {
    pub id: String,
    pub short: Option<String>,
    pub title: String,
}

/// The Requirements as a whole. It carries no text of them, no Area, fragment or section and
/// nothing of the view's focus: the agent loads the Requirement skills that fit the message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequirementsPart {
    /// The overlay Proposal of the view, only when it is still open.
    pub overlay: Option<Overlay>,
}

/// The chosen thing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Chosen<'a> {
    Task(TaskPart),
    Proposal(ProposalPart),
    Drift(DriftPart),
    History(HistoryEntryPart),
    Question(QuestionPart<'a>),
    Requirements(RequirementsPart),
}

/// The instruction of the agent of an Ask more conversation. Fixed text, the part that describes
/// the chosen thing, the commands that read it, and the language sentence only when a language is
/// set (cleaned as General's is).
pub fn instruction(chosen: &Chosen<'_>, language: Option<&str>) -> String {
    let mut text = String::from(INSTRUCTION);
    text.push('\n');
    match chosen {
        Chosen::Task(task) => task_part(&mut text, task),
        Chosen::Proposal(proposal) => proposal_part(&mut text, proposal),
        Chosen::Drift(drift) => drift_part(&mut text, drift),
        Chosen::History(entry) => history_part(&mut text, entry),
        Chosen::Question(question) => question_part(&mut text, question),
        Chosen::Requirements(requirements) => requirements_part(&mut text, requirements),
    }
    if let Some(language) = clean_language(language) {
        let _ = write!(text, "\nWrite the texts for the developer in {language}.\n");
    }
    text
}

const INSTRUCTION: &str = "\
Weaver: Ask more

This is an Ask more conversation: the developer is reading one thing in a project that Weaver manages and asks you about it. The project keeps its Model, Requirements, Proposals, Tasks and Drifts in `.weaver/`. The part below describes the chosen thing as the project's files say it now.

- Read with the commands listed after that part whatever you need to know.
- Answer the developer's message and explain.
- Change a Task, a Drift or a Proposal only when the developer asks for it, and only with the `weaver` commands that allow it.
- Never change the Model or the Requirements: they change only through a Proposal the developer approves, written by Weaver's design job from a design request that the developer writes with `New Request` in the Backlog of the Board and sends.
- Never write by hand in a file under `.weaver/`.
- Start a job of Weaver with `weaver job new` only when the developer explicitly asks for that job in this conversation:
  - `weaver job new --input '{\"kind\":\"scan\"}'` scans the whole System;
  - `weaver job new --input '{\"kind\":\"run\",\"task\":\"<task>\"}'` runs one Task, and without `task` runs every runnable Task;
  - `weaver job new --input '{\"kind\":\"resolve\",\"drift\":\"<drift>\"}'` resolves one Drift;
  - `weaver job new --input '{\"kind\":\"design\",\"request\":\"<request>\"}'` sends a design request the developer already wrote and did not send.
- Weaver applies its own refusals to the start of a job. When one is refused, tell the developer the reason from the output of the command.
- Ask the developer only in your replies, in plain text, and wait for the answer. A form or a question tool is declined here.
";

// ---------------------------------------------------------------------------------------------
// The parts
// ---------------------------------------------------------------------------------------------

/// `Short number (as the developer sees it): T3`, when the card has one.
fn short_line(out: &mut String, short: Option<&str>) {
    if let Some(short) = short {
        let _ = writeln!(out, "Short number (as the developer sees it): {short}");
    }
}

/// `Label:` and the text under it, whole.
fn block(out: &mut String, label: &str, text: &str) {
    let _ = writeln!(out, "{label}:");
    let text = text.trim_matches('\n');
    if !text.is_empty() {
        out.push_str(text);
        out.push('\n');
    }
}

fn list(out: &mut String, label: &str, items: &[String]) {
    if items.is_empty() {
        return;
    }
    let _ = writeln!(out, "{label}:");
    for item in items {
        let _ = writeln!(out, "- {item}");
    }
}

fn titled(out: &mut String, label: &str, card: &Titled) {
    if card.title.is_empty() {
        let _ = writeln!(out, "{label}: {}", card.id);
    } else {
        let _ = writeln!(out, "{label}: {} ({})", card.id, card.title);
    }
}

fn model_show(commands: &mut Vec<String>, refs: &[String]) {
    commands.extend(refs.iter().map(|r| format!("weaver model show {r}")));
}

/// The read commands after a part.
fn commands(out: &mut String, commands: &[String]) {
    if commands.is_empty() {
        return;
    }
    out.push_str("\nCommands that read it:\n");
    for command in commands {
        let _ = writeln!(out, "- `{command}`");
    }
}

fn task_part(out: &mut String, task: &TaskPart) {
    out.push_str("Chosen thing: a Task\n");
    let _ = writeln!(out, "Permanent identity: {}", task.id);
    short_line(out, task.short.as_deref());
    let _ = writeln!(out, "Title: {}", task.title);
    list(out, "References", &task.refs);
    list(out, "Images", &task.images);
    if let Some(proposal) = &task.proposal {
        titled(out, "Proposal whose design it belongs to", proposal);
    }
    if let Some(drift) = &task.drift {
        let _ = writeln!(out, "Drift it fixes: {drift}");
    }
    block(out, "Body", &task.body);

    let mut read = vec![format!("weaver task show {}", task.id)];
    if let Some(proposal) = &task.proposal {
        read.push(format!("weaver proposal show {}", proposal.id));
    }
    if let Some(drift) = &task.drift {
        read.push(format!("weaver drift show {drift}"));
    }
    model_show(&mut read, &task.refs);
    commands(out, &read);
}

fn proposal_part(out: &mut String, proposal: &ProposalPart) {
    out.push_str("Chosen thing: a Proposal\n");
    let _ = writeln!(out, "Permanent identity: {}", proposal.id);
    short_line(out, proposal.short.as_deref());
    let _ = writeln!(out, "Title: {}", proposal.title);
    let _ = writeln!(out, "State: {}", proposal.state);
    if !proposal.tasks.is_empty() {
        out.push_str("Tasks:\n");
        for task in &proposal.tasks {
            let _ = writeln!(out, "- {} ({})", task.id, task.title);
        }
    }
    if !proposal.drifts.is_empty() {
        out.push_str("Drifts it resolves:\n");
        for drift in &proposal.drifts {
            let _ = writeln!(out, "- {}: {}", drift.id, drift.decision);
        }
    }
    block(out, "Body", &proposal.body);

    let mut read = vec![format!("weaver proposal show {}", proposal.id)];
    read.extend(
        proposal
            .tasks
            .iter()
            .map(|t| format!("weaver task show {}", t.id)),
    );
    read.extend(
        proposal
            .drifts
            .iter()
            .map(|d| format!("weaver drift show {}", d.id)),
    );
    commands(out, &read);
}

fn drift_part(out: &mut String, drift: &DriftPart) {
    out.push_str("Chosen thing: a Drift\n");
    let _ = writeln!(out, "Permanent identity: {}", drift.id);
    short_line(out, drift.short.as_deref());
    let _ = writeln!(out, "Title: {}", drift.title);
    list(out, "References", &drift.refs);
    if let Some(proposal) = &drift.proposal {
        titled(out, "Attached Proposal", proposal);
    }
    if let Some(task) = &drift.task {
        let _ = writeln!(out, "Task it was born from: {task}");
    }
    if let Some(repeats) = &drift.repeats {
        let _ = writeln!(out, "Drift it repeats: {repeats}");
    }
    block(out, "What happens", &drift.what_happens);
    block(out, "What should happen", &drift.what_should_happen);

    let mut read = vec![format!("weaver drift show {}", drift.id)];
    if let Some(proposal) = &drift.proposal {
        read.push(format!("weaver proposal show {}", proposal.id));
    }
    if let Some(task) = &drift.task {
        read.push(format!("weaver task show {task}"));
    }
    if let Some(repeats) = &drift.repeats {
        read.push(format!("weaver drift show {repeats}"));
    }
    model_show(&mut read, &drift.refs);
    commands(out, &read);
}

fn history_part(out: &mut String, entry: &HistoryEntryPart) {
    out.push_str("Chosen thing: a History entry with the Tasks it cites\n");
    let _ = writeln!(out, "Permanent identity: {}", entry.id);
    let _ = writeln!(out, "Title: {}", entry.title);
    block(out, "Summary", &entry.summary);
    block(out, "Text", &entry.text);
    if entry.tasks.is_empty() {
        out.push_str("\nThe entry cites no Task.\n");
    } else {
        out.push_str("\nTasks it cites, in the order cited:\n");
    }
    let count = entry.tasks.len();
    for (index, task) in entry.tasks.iter().enumerate() {
        let _ = writeln!(out, "\nTask {} of {count}:", index + 1);
        let _ = writeln!(out, "Permanent identity: {}", task.id);
        let Some(copy) = &task.copy else {
            out.push_str(
                "Not available: neither its copy in the entry nor its file can be read.\n",
            );
            continue;
        };
        let _ = writeln!(out, "Title: {}", copy.title);
        list(out, "References", &copy.refs);
        if let Some(proposal) = &copy.proposal {
            let _ = writeln!(out, "Proposal whose design it belonged to: {proposal}");
        }
        if let Some(label) = &copy.label {
            let _ = writeln!(out, "Label: {label}");
        }
        let _ = writeln!(out, "Finished: {}", copy.finished);
        block(out, "Body", &copy.body);
    }

    let mut read = vec![format!("weaver history show {}", entry.id)];
    let copies = || entry.tasks.iter().filter_map(|task| task.copy.as_ref());
    let mut proposals: Vec<&str> = Vec::new();
    for copy in copies() {
        if let Some(proposal) = copy.proposal.as_deref().filter(|_| copy.proposal_exists) {
            if !proposals.contains(&proposal) {
                proposals.push(proposal);
            }
        }
    }
    read.extend(
        proposals
            .into_iter()
            .map(|proposal| format!("weaver proposal show {proposal}")),
    );
    let mut refs: Vec<String> = Vec::new();
    for copy in copies() {
        for reference in &copy.refs {
            if !refs.contains(reference) {
                refs.push(reference.clone());
            }
        }
    }
    model_show(&mut read, &refs);
    commands(out, &read);
}

fn question_part(out: &mut String, part: &QuestionPart<'_>) {
    let card = part.origin == QuestionOrigin::Chat;
    out.push_str(if card {
        "Chosen thing: the question in view of the active card of questions of the Chat\n"
    } else {
        "Chosen thing: the question in view of a round of questions\n"
    });
    let questions = &part.request.questions;
    let Some(view) = questions.get(part.index) else {
        return;
    };
    let _ = writeln!(out, "Question {} of {}:", part.index + 1, questions.len());
    question_in_full(out, view);
    let others: Vec<(usize, &Question)> = questions.iter().enumerate().collect();
    if others.len() > 1 {
        out.push_str(if card {
            "\nThe questions of the card, in their order:\n"
        } else {
            "\nThe questions of the round, in their order:\n"
        });
        for (index, question) in others {
            if index == part.index {
                let _ = writeln!(
                    out,
                    "{}. (the question in view, described above)",
                    index + 1
                );
                continue;
            }
            let _ = writeln!(out, "{}. {}", index + 1, question.question.trim());
            for option in &question.options {
                let _ = writeln!(out, "   - {}: {}", option.label, option.description.trim());
            }
        }
    }
    out.push('\n');
    match &part.origin {
        QuestionOrigin::Job { name, id } => {
            let _ = writeln!(out, "Asked by the job: {name} ({id})");
        }
        QuestionOrigin::Chat => {
            out.push_str("Asked by: Chat\n");
        }
        QuestionOrigin::Drift { id } => {
            let _ = writeln!(out, "Prepared question of the Drift: {id}");
        }
    }
    out.push_str(if card {
        "The card is still active: the developer has not answered it. Do not answer it for the developer.\n"
    } else {
        "The round is still open: the developer has not answered it. Do not answer it for the developer.\n"
    });

    let mut read = Vec::new();
    if let QuestionOrigin::Drift { id } = &part.origin {
        read.push(format!("weaver drift show {id}"));
    }
    commands(out, &read);
}

fn question_in_full(out: &mut String, question: &Question) {
    block(out, "Question", &question.question);
    if let Some(context) = &question.context {
        block(out, "Context", context);
    }
    out.push_str("Options:\n");
    for (index, option) in question.options.iter().enumerate() {
        let recommended = if index == question.recommended {
            " (recommended)"
        } else {
            ""
        };
        let _ = writeln!(out, "- {}{recommended}", option.label);
        let _ = writeln!(out, "  Description: {}", option.description.trim());
        if let Some(details) = &option.details {
            let _ = writeln!(out, "  Details: {}", details.trim());
        }
    }
    if question.multi {
        out.push_str("The developer can choose several options.\n");
    } else {
        out.push_str("The developer can choose one option.\n");
    }
}

fn requirements_part(out: &mut String, requirements: &RequirementsPart) {
    out.push_str("Chosen thing: the Requirements of the project\n");
    out.push_str(
        "The developer asks about the Requirements of the project as a whole, not about one part of them.\n",
    );
    out.push_str(
        "The Requirements are in the Requirement skills `weaver-req-*` installed in the project, which hold the current Requirements. Load by yourself the ones that fit the developer's message, as many as you need.\n",
    );
    let mut read = Vec::new();
    if let Some(overlay) = &requirements.overlay {
        out.push_str("The view shows the Requirements with the changes of a Proposal applied:\n");
        let _ = writeln!(out, "  Permanent identity: {}", overlay.id);
        if let Some(short) = &overlay.short {
            let _ = writeln!(out, "  Short number (as the developer sees it): {short}");
        }
        let _ = writeln!(out, "  Title: {}", overlay.title);
        out.push_str(
            "The skills hold the current Requirements, without that Proposal: read its changes with the command below, not in the skills.\n",
        );
        read.push(format!("weaver proposal show {}", overlay.id));
    }
    commands(out, &read);
}

// ---------------------------------------------------------------------------------------------
// How omp is started and what the first prompt holds
// ---------------------------------------------------------------------------------------------

/// The arguments after the program: `[--model <analysis>] --approval-mode <mode> acp`. The model
/// is the analysis model of the Settings, never a Proposal's chosen one; no
/// `--append-system-prompt`, `--advisor`, `--slow` or overlay.
pub fn args(models: &ModelSettings, permissions: Permissions) -> Vec<String> {
    let mut args = Vec::new();
    let model = models.analysis.trim();
    if !model.is_empty() {
        args.push("--model".to_owned());
        args.push(model.to_owned());
    }
    args.push("--approval-mode".to_owned());
    args.push(approval_mode(permissions).to_owned());
    args.push("acp".to_owned());
    args
}

/// The variables Weaver adds to the environment of omp: `WEAVER_ASK` (the identity of the
/// conversation) and `WEAVER_ENDPOINT`, and never `WEAVER_JOB` or `WEAVER_GENERAL`, so that
/// `weaver` refuses the commands tied to a job.
pub fn env(identity: &str, endpoint: &str) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("WEAVER_ASK".to_owned(), identity.to_owned()),
        ("WEAVER_ENDPOINT".to_owned(), endpoint.to_owned()),
    ])
}

const PROMPT_HEAD: &str = "Instructions from Weaver (not written by the developer):";
const MESSAGE_HEAD: &str = "Developer's message:";

/// The one text block of the first `session/prompt`: the instruction, then the developer's
/// message trimmed as every message is.
pub fn first_prompt(instruction: &str, message: &str) -> String {
    format!(
        "{PROMPT_HEAD}\n{}\n\n{MESSAGE_HEAD}\n{}",
        instruction.trim_end(),
        message.trim()
    )
}

/// What the conversation shows of a user message of a replayed history: for the first prompt only
/// the text after `Developer's message:`, for any other message the message itself; in both
/// without the block Weaver appended for the assets the message tags.
pub fn shown_message(prompt: &str) -> &str {
    let message = if prompt.starts_with(PROMPT_HEAD) {
        let marker = format!("\n\n{MESSAGE_HEAD}\n");
        match prompt.find(&marker) {
            Some(at) => &prompt[at + marker.len()..],
            None => prompt,
        }
    } else {
        prompt
    };
    // What Weaver appended for the tagged assets is not the developer's.
    tags::without_block(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::question::request::QuestionOption;

    fn titled(id: &str, title: &str) -> Titled {
        Titled {
            id: id.to_owned(),
            title: title.to_owned(),
        }
    }

    fn task_full() -> TaskPart {
        TaskPart {
            id: "abcd1234".into(),
            short: Some("T3".into()),
            title: "Build it".into(),
            body: "First line\n\nSecond line".into(),
            refs: vec!["requirements/ui#app".into(), "features/board".into()],
            images: vec![".weaver/tasks/abcd1234/a.png".into()],
            proposal: Some(titled("prop0001", "A design")),
            drift: Some("drft0001".into()),
        }
    }

    fn task_bare() -> TaskPart {
        TaskPart {
            id: "abcd1234".into(),
            short: None,
            title: "Build it".into(),
            body: String::new(),
            refs: vec![],
            images: vec![],
            proposal: None,
            drift: None,
        }
    }

    fn option(label: &str, details: Option<&str>) -> QuestionOption {
        QuestionOption {
            label: label.into(),
            description: format!("{label} description"),
            details: details.map(str::to_owned),
        }
    }

    fn round() -> RoundRequest {
        RoundRequest {
            questions: vec![
                Question {
                    id: "q1".into(),
                    question: "First?".into(),
                    context: Some("Because.".into()),
                    options: vec![option("Yes", Some("Many details")), option("No", None)],
                    recommended: 1,
                    multi: true,
                },
                Question {
                    id: "q2".into(),
                    question: "Second?".into(),
                    context: None,
                    options: vec![option("Left", None), option("Right", None)],
                    recommended: 0,
                    multi: false,
                },
            ],
        }
    }

    fn overlay() -> Overlay {
        Overlay {
            id: "prop0001".into(),
            short: Some("P2".into()),
            title: "A design".into(),
        }
    }

    #[test]
    fn every_text_starts_with_the_line_and_holds_the_fixed_rules() {
        let text = instruction(&Chosen::Task(task_bare()), None);
        assert!(text.starts_with("Weaver: Ask more\n"));
        for rule in [
            "Read with the commands listed after that part",
            "Answer the developer's message and explain.",
            "only when the developer asks for it",
            "Never change the Model or the Requirements",
            "Never write by hand in a file under `.weaver/`",
            "only when the developer explicitly asks for that job",
            "Ask the developer only in your replies",
        ] {
            assert!(text.contains(rule), "missing: {rule}");
        }
        assert!(!text.to_lowercase().contains("skill"));
        // The control of the Board that opens a request is named as the Board names it.
        assert!(text.contains("with `New Request` in the Backlog of the Board"));
        assert!(!text.to_lowercase().contains("work item"));
    }

    #[test]
    fn a_task_part_has_all_its_fields() {
        let text = instruction(&Chosen::Task(task_full()), None);
        for line in [
            "Chosen thing: a Task\n",
            "Permanent identity: abcd1234\n",
            "Short number (as the developer sees it): T3\n",
            "Title: Build it\n",
            "References:\n- requirements/ui#app\n- features/board\n",
            "Images:\n- .weaver/tasks/abcd1234/a.png\n",
            "Proposal whose design it belongs to: prop0001 (A design)\n",
            "Drift it fixes: drft0001\n",
            "Body:\nFirst line\n\nSecond line\n",
            "- `weaver task show abcd1234`\n",
            "- `weaver proposal show prop0001`\n",
            "- `weaver drift show drft0001`\n",
            "- `weaver model show requirements/ui#app`\n",
            "- `weaver model show features/board`\n",
        ] {
            assert!(text.contains(line), "missing: {line}");
        }
    }

    #[test]
    fn a_task_part_without_its_optional_fields_names_none_of_them() {
        let text = instruction(&Chosen::Task(task_bare()), None);
        for absent in [
            "Short number",
            "References:",
            "Images:",
            "Proposal whose design",
            "Drift it fixes",
            "weaver proposal show",
            "weaver drift show",
            "weaver model show",
        ] {
            assert!(!text.contains(absent), "unexpected: {absent}");
        }
        assert!(text.contains("- `weaver task show abcd1234`\n"));
    }

    #[test]
    fn a_proposal_part_has_all_its_fields() {
        let proposal = ProposalPart {
            id: "prop0001".into(),
            short: Some("P2".into()),
            title: "Resolve them".into(),
            state: "open".into(),
            body: "The body.\n\nMore.".into(),
            tasks: vec![titled("task0001", "First"), titled("task0002", "Second")],
            drifts: vec![ResolvedDrift {
                id: "drft0001".into(),
                decision: "fix the code".into(),
            }],
        };
        let text = instruction(&Chosen::Proposal(proposal), None);
        for line in [
            "Chosen thing: a Proposal\n",
            "Permanent identity: prop0001\n",
            "Short number (as the developer sees it): P2\n",
            "Title: Resolve them\n",
            "State: open\n",
            "Tasks:\n- task0001 (First)\n- task0002 (Second)\n",
            "Drifts it resolves:\n- drft0001: fix the code\n",
            "Body:\nThe body.\n\nMore.\n",
            "- `weaver proposal show prop0001`\n",
            "- `weaver task show task0001`\n",
            "- `weaver task show task0002`\n",
            "- `weaver drift show drft0001`\n",
        ] {
            assert!(text.contains(line), "missing: {line}");
        }
    }

    #[test]
    fn a_proposal_part_without_tasks_or_drifts_lists_neither() {
        let proposal = ProposalPart {
            id: "prop0001".into(),
            short: None,
            title: "Plain".into(),
            state: "draft".into(),
            body: String::new(),
            tasks: vec![],
            drifts: vec![],
        };
        let text = instruction(&Chosen::Proposal(proposal), None);
        assert!(!text.contains("Short number"));
        assert!(!text.contains("Tasks:"));
        assert!(!text.contains("Drifts it resolves"));
        assert!(!text.contains("weaver task show"));
        assert!(!text.contains("weaver drift show"));
        assert!(text.contains("- `weaver proposal show prop0001`\n"));
    }

    #[test]
    fn a_drift_part_has_all_its_fields() {
        let drift = DriftPart {
            id: "drft0001".into(),
            short: Some("D4".into()),
            title: "It drifts".into(),
            what_happens: "It does A.\nTwice.".into(),
            what_should_happen: "It does B.".into(),
            refs: vec!["requirements/rules#job".into()],
            proposal: Some(titled("prop0001", "A fix")),
            task: Some("task0001".into()),
            repeats: Some("drft0000".into()),
        };
        let text = instruction(&Chosen::Drift(drift), None);
        for line in [
            "Chosen thing: a Drift\n",
            "Permanent identity: drft0001\n",
            "Short number (as the developer sees it): D4\n",
            "Title: It drifts\n",
            "References:\n- requirements/rules#job\n",
            "Attached Proposal: prop0001 (A fix)\n",
            "Task it was born from: task0001\n",
            "Drift it repeats: drft0000\n",
            "What happens:\nIt does A.\nTwice.\n",
            "What should happen:\nIt does B.\n",
            "- `weaver drift show drft0001`\n",
            "- `weaver proposal show prop0001`\n",
            "- `weaver task show task0001`\n",
            "- `weaver drift show drft0000`\n",
            "- `weaver model show requirements/rules#job`\n",
        ] {
            assert!(text.contains(line), "missing: {line}");
        }
    }

    #[test]
    fn a_drift_part_without_its_optional_fields_names_none_of_them() {
        let drift = DriftPart {
            id: "drft0001".into(),
            short: None,
            title: "It drifts".into(),
            what_happens: "A".into(),
            what_should_happen: "B".into(),
            refs: vec![],
            proposal: None,
            task: None,
            repeats: None,
        };
        let text = instruction(&Chosen::Drift(drift), None);
        for absent in [
            "Short number",
            "References:",
            "Attached Proposal",
            "Task it was born from",
            "Drift it repeats",
            "weaver proposal show",
            "weaver task show",
            "weaver model show",
        ] {
            assert!(!text.contains(absent), "unexpected: {absent}");
        }
        assert_eq!(text.matches("weaver drift show").count(), 1);
    }

    fn cited(id: &str, proposal_exists: bool) -> HistoryTaskPart {
        HistoryTaskPart {
            id: id.into(),
            copy: Some(HistoryTaskCopy {
                title: format!("Title of {id}"),
                body: format!("What {id} did."),
                refs: vec!["features/board".into(), format!("features/{id}")],
                proposal: Some("prop0001".into()),
                proposal_exists,
                label: Some("Board".into()),
                finished: "2026-11-03T10:00:00Z".into(),
            }),
        }
    }

    fn entry(tasks: Vec<HistoryTaskPart>) -> HistoryEntryPart {
        HistoryEntryPart {
            id: "hist0001".into(),
            title: "Entry".into(),
            summary: "A summary.".into(),
            text: "The whole text.\n\nSecond paragraph.".into(),
            tasks,
        }
    }

    #[test]
    fn a_history_part_has_the_entry_in_full_and_each_cited_task_in_order() {
        let text = instruction(
            &Chosen::History(entry(vec![
                cited("task0001", true),
                cited("task0002", true),
            ])),
            None,
        );
        for line in [
            "Chosen thing: a History entry with the Tasks it cites\nPermanent identity: hist0001\nTitle: Entry\nSummary:\nA summary.\nText:\nThe whole text.\n\nSecond paragraph.\n",
            "\nTask 1 of 2:\nPermanent identity: task0001\nTitle: Title of task0001\nReferences:\n- features/board\n- features/task0001\nProposal whose design it belonged to: prop0001\nLabel: Board\nFinished: 2026-11-03T10:00:00Z\nBody:\nWhat task0001 did.\n",
            "\nTask 2 of 2:\nPermanent identity: task0002\nTitle: Title of task0002\n",
            "Body:\nWhat task0002 did.\n",
        ] {
            assert!(text.contains(line), "missing: {line}");
        }
        assert!(text.find("Task 1 of 2").unwrap() < text.find("Task 2 of 2").unwrap());
        assert!(!text.contains("Short number"));
        assert!(!text.contains("Not available"));
    }

    #[test]
    fn a_history_part_reads_the_entry_then_each_proposal_and_reference_once() {
        let text = instruction(
            &Chosen::History(entry(vec![
                cited("task0001", true),
                cited("task0002", true),
            ])),
            None,
        );
        let commands: Vec<&str> = text
            .split_once("Commands that read it:\n")
            .expect("commands are listed")
            .1
            .lines()
            .collect();
        assert_eq!(
            commands,
            [
                "- `weaver history show hist0001`",
                "- `weaver proposal show prop0001`",
                "- `weaver model show features/board`",
                "- `weaver model show features/task0001`",
                "- `weaver model show features/task0002`",
            ]
        );
    }

    #[test]
    fn a_history_part_lists_a_proposal_only_while_it_exists() {
        let text = instruction(
            &Chosen::History(entry(vec![cited("task0001", false)])),
            None,
        );
        assert!(text.contains("Proposal whose design it belonged to: prop0001\n"));
        assert!(!text.contains("weaver proposal show"));

        let mut bare = cited("task0001", true);
        let copy = bare.copy.as_mut().unwrap();
        copy.proposal = None;
        copy.label = None;
        copy.refs.clear();
        let text = instruction(&Chosen::History(entry(vec![bare])), None);
        assert!(!text.contains("Proposal whose design"));
        assert!(!text.contains("Label:"));
        assert!(!text.contains("References:"));
        assert!(!text.contains("weaver proposal show"));
        assert!(!text.contains("weaver model show"));
    }

    #[test]
    fn a_history_part_lists_an_unreadable_task_by_its_identity_alone() {
        let missing = HistoryTaskPart {
            id: "task0009".into(),
            copy: None,
        };
        let text = instruction(
            &Chosen::History(entry(vec![missing, cited("task0001", true)])),
            None,
        );
        assert!(text.contains(
            "\nTask 1 of 2:\nPermanent identity: task0009\nNot available: neither its copy in the entry nor its file can be read.\n\nTask 2 of 2:\n"
        ));
        assert!(text.contains("- `weaver history show hist0001`\n"));
        assert!(text.contains("- `weaver proposal show prop0001`\n"));
    }

    #[test]
    fn a_history_part_without_tasks_says_so() {
        let text = instruction(&Chosen::History(entry(vec![])), None);
        assert!(text.contains("\nThe entry cites no Task.\n"));
        assert!(!text.contains("Task 1"));
        assert!(text.ends_with("Commands that read it:\n- `weaver history show hist0001`\n"));
    }

    #[test]
    fn a_question_part_has_the_question_in_full_and_the_others() {
        let request = round();
        let text = instruction(
            &Chosen::Question(QuestionPart {
                request: &request,
                index: 0,
                origin: QuestionOrigin::Job {
                    name: "Design P2".into(),
                    id: "job00001".into(),
                },
            }),
            None,
        );
        for line in [
            "Chosen thing: the question in view of a round of questions\n",
            "Question 1 of 2:\n",
            "Question:\nFirst?\n",
            "Context:\nBecause.\n",
            "- Yes\n  Description: Yes description\n  Details: Many details\n",
            "- No (recommended)\n  Description: No description\n",
            "The developer can choose several options.\n",
            "1. (the question in view, described above)\n",
            "2. Second?\n   - Left: Left description\n   - Right: Right description\n",
            "Asked by the job: Design P2 (job00001)\n",
            "The round is still open",
            "Do not answer it for the developer.",
        ] {
            assert!(text.contains(line), "missing: {line}");
        }
        assert!(!text.contains("weaver drift show"));
        assert!(!text.contains("Commands that read it"));
    }

    #[test]
    fn a_question_part_of_the_card_of_the_chat_names_the_chat_and_says_the_card_is_active() {
        let request = round();
        let text = instruction(
            &Chosen::Question(QuestionPart {
                request: &request,
                index: 1,
                origin: QuestionOrigin::Chat,
            }),
            None,
        );
        for line in [
            "Chosen thing: the question in view of the active card of questions of the Chat\n",
            "Question 2 of 2:\n",
            "The questions of the card, in their order:\n",
            "1. First?\n   - Yes: Yes description\n   - No: No description\n",
            "2. (the question in view, described above)\n",
            "Asked by: Chat\n",
            "The card is still active: the developer has not answered it. Do not answer it for the developer.\n",
        ] {
            assert!(text.contains(line), "missing: {line}");
        }
        assert!(!text.contains("round"));
        assert!(!text.contains("weaver drift show"));
        assert!(!text.contains("Commands that read it"));
    }

    #[test]
    fn a_question_part_of_a_lone_question_with_no_context_or_details() {
        let mut request = round();
        request.questions.remove(0);
        let text = instruction(
            &Chosen::Question(QuestionPart {
                request: &request,
                index: 0,
                origin: QuestionOrigin::Drift {
                    id: "drft0001".into(),
                },
            }),
            None,
        );
        assert!(text.contains("Question 1 of 1:\n"));
        assert!(text.contains("Question:\nSecond?\n"));
        assert!(!text.contains("Context:"));
        assert!(!text.contains("Details:"));
        assert!(!text.contains("The questions of the round"));
        assert!(text.contains("- Left (recommended)\n"));
        assert!(text.contains("The developer can choose one option.\n"));
        assert!(text.contains("Prepared question of the Drift: drft0001\n"));
        assert!(text.contains("- `weaver drift show drft0001`\n"));
    }

    #[test]
    fn a_question_part_marks_the_question_in_view_among_the_others() {
        let request = round();
        let text = instruction(
            &Chosen::Question(QuestionPart {
                request: &request,
                index: 1,
                origin: QuestionOrigin::Job {
                    name: "Scan".into(),
                    id: "job00002".into(),
                },
            }),
            None,
        );
        assert!(text.contains("Question 2 of 2:\n"));
        assert!(text.contains("1. First?\n   - Yes: Yes description\n   - No: No description\n"));
        assert!(text.contains("2. (the question in view, described above)\n"));
    }

    #[test]
    fn a_requirements_part_without_an_overlay_lists_no_command() {
        let text = instruction(
            &Chosen::Requirements(RequirementsPart { overlay: None }),
            None,
        );
        // The fixed rules name `weaver job new` and Proposals; look only at the part.
        let part = text.split("Chosen thing:").nth(1).unwrap();
        for line in [
            "the Requirements of the project\n",
            "The developer asks about the Requirements of the project as a whole",
            "Requirement skills `weaver-req-*` installed in the project, which hold the current Requirements.",
            "Load by yourself the ones that fit the developer's message, as many as you need.\n",
        ] {
            assert!(part.contains(line), "missing: {line}");
        }
        for absent in [
            "Commands that read it",
            "weaver ",
            "Proposal",
            "overlay",
            "Area",
            "Fragment",
            "Reference:",
        ] {
            assert!(!part.contains(absent), "unexpected: {absent}");
        }
    }

    #[test]
    fn a_requirements_part_with_an_overlay_names_it_and_lists_only_proposal_show() {
        let text = instruction(
            &Chosen::Requirements(RequirementsPart {
                overlay: Some(overlay()),
            }),
            None,
        );
        let part = text.split("Chosen thing:").nth(1).unwrap();
        for line in [
            "The view shows the Requirements with the changes of a Proposal applied:\n",
            "  Permanent identity: prop0001\n",
            "  Short number (as the developer sees it): P2\n",
            "  Title: A design\n",
            "read its changes with the command below, not in the skills.\n",
            "Commands that read it:\n- `weaver proposal show prop0001`\n",
        ] {
            assert!(part.contains(line), "missing: {line}");
        }
        assert_eq!(part.matches("weaver ").count(), 1);
        assert!(!part.contains("weaver model show"));
    }

    #[test]
    fn the_overlay_without_a_short_number_names_only_its_identity() {
        let mut overlay = overlay();
        overlay.short = None;
        let text = instruction(
            &Chosen::Requirements(RequirementsPart {
                overlay: Some(overlay),
            }),
            None,
        );
        assert!(text.contains("  Permanent identity: prop0001\n"));
        assert!(!text.contains("Short number"));
    }

    #[test]
    fn only_the_requirements_part_names_the_requirement_skills() {
        for text in [
            instruction(&Chosen::Task(task_bare()), None),
            instruction(&Chosen::Task(task_full()), None),
        ] {
            assert!(!text.to_lowercase().contains("skill"));
            assert!(!text.contains("weaver-req"));
        }
        let text = instruction(
            &Chosen::Requirements(RequirementsPart { overlay: None }),
            None,
        );
        assert!(text.contains("weaver-req-*"));
    }

    #[test]
    fn the_language_sentence_is_the_only_addition_and_only_with_a_language() {
        let chosen = Chosen::Task(task_full());
        let without = instruction(&chosen, None);
        let with = instruction(&chosen, Some("  Italian "));
        assert_eq!(
            with,
            format!("{without}\nWrite the texts for the developer in Italian.\n")
        );
        assert_eq!(instruction(&chosen, Some("   ")), without);
        assert_eq!(instruction(&chosen, Some("Ita\nlian")), without);
        assert!(!without.contains("Write the texts for the developer"));
    }

    #[test]
    fn none_of_the_sentences_of_a_job_instruction_is_there() {
        let request = round();
        let texts = [
            instruction(&Chosen::Task(task_full()), Some("Italian")),
            instruction(
                &Chosen::Question(QuestionPart {
                    request: &request,
                    index: 0,
                    origin: QuestionOrigin::Drift { id: "d".into() },
                }),
                Some("Italian"),
            ),
        ];
        for text in texts {
            for phrase in [
                "sub-agent",
                "Sub-agent",
                "context sharing",
                "weaver question new",
                "at most",
            ] {
                assert!(!text.contains(phrase), "unexpected: {phrase}");
            }
            assert!(text.lines().count() > 10);
        }
    }

    fn models(analysis: &str) -> ModelSettings {
        ModelSettings {
            analysis: analysis.into(),
            scan: "s/m".into(),
            execution: "e/m".into(),
        }
    }

    #[test]
    fn omp_starts_on_the_analysis_model_with_no_instruction_file() {
        assert_eq!(
            args(&models("  a/m "), Permissions::Yolo),
            ["--model", "a/m", "--approval-mode", "yolo", "acp"]
        );
        assert_eq!(
            args(&models(""), Permissions::Ask),
            ["--approval-mode", "always-ask", "acp"]
        );
        let line = args(&models("a/m"), Permissions::Yolo).join(" ");
        for forbidden in ["--append-system-prompt", "--advisor", "--slow", "--config"] {
            assert!(!line.contains(forbidden), "{forbidden}");
        }
    }

    #[test]
    fn the_environment_names_the_conversation_and_never_a_job_or_general() {
        let env = env("aaaaaaaa", "/run/weaver.sock");
        assert_eq!(
            env.keys().map(String::as_str).collect::<Vec<_>>(),
            ["WEAVER_ASK", "WEAVER_ENDPOINT"]
        );
        assert_eq!(env["WEAVER_ASK"], "aaaaaaaa");
    }

    #[test]
    fn the_first_prompt_is_one_text_with_the_instruction_then_the_message() {
        let prompt = first_prompt("Weaver: Ask more\n\nrules\n", "  What is this?\n");
        assert_eq!(
            prompt,
            "Instructions from Weaver (not written by the developer):\nWeaver: Ask more\n\nrules\n\nDeveloper's message:\nWhat is this?"
        );
    }

    #[test]
    fn a_replayed_first_prompt_shows_only_the_message() {
        let prompt = first_prompt(
            &instruction(&Chosen::Task(task_bare()), None),
            "Why?\n\nPlease.",
        );
        assert_eq!(shown_message(&prompt), "Why?\n\nPlease.");
        // Any other message is shown as it is.
        assert_eq!(shown_message("And then?"), "And then?");
        assert_eq!(
            shown_message("Developer's message:\nnot a first prompt"),
            "Developer's message:\nnot a first prompt"
        );
    }

    #[test]
    fn the_block_of_tagged_assets_is_never_shown_in_a_first_or_a_later_message() {
        let tagged = vec!["logo.png".to_owned(), "my notes.txt".to_owned()];
        let message = tags::with_block("Look at @logo.png and @my notes.txt", &tagged);
        let prompt = first_prompt(&instruction(&Chosen::Task(task_bare()), None), &message);
        assert!(prompt.ends_with("Read the file `.weaver/assets/my notes.txt`."));
        assert_eq!(
            shown_message(&prompt),
            "Look at @logo.png and @my notes.txt"
        );
        assert_eq!(
            shown_message(&message),
            "Look at @logo.png and @my notes.txt"
        );
    }
}
