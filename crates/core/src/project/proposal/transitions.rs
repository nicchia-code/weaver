//! The transitions of a Proposal: hand-over, approve, reject, rework, discard, refinement.
//!
//! Every `plan_*` function is pure: it reads the image and the text of the Proposal folders and
//! returns the [`ChangeSet`] of the transition, or a [`Refusal`] naming every reason. Nothing is
//! trusted from when a button was enabled: the wrappers ([`approve`], [`reject`], [`discard`])
//! run the plan under the project lock through [`changes::mutate`], which reads the project and
//! the folders again, plans, validates and writes the whole change or nothing.
//!
//! * `Draft`: `agent.SUBMIT_PROPOSAL` → `Open`, or → `Approved` for a setup or import handed
//!   over by a job that found the Model empty ([`plan_submit`]); `developer.DISCARD_PROPOSAL`
//!   ([`plan_discard`]); `developer.APPROVE_PROPOSAL` on a Draft a refinement left half-way →
//!   `Refining` ([`Approval::Continue`]).
//! * `Open`: `developer.APPROVE_PROPOSAL` → `Approved` or, when stale, → `Refining`
//!   ([`plan_approve`]); `developer.REJECT_PROPOSAL` and `developer.REWORK_PROPOSAL` →
//!   `Rejected` ([`plan_reject`]); `agent.EDIT_PROPOSAL` → `Draft` (`create::plan_edit`).
//! * `Refining`: `agent.SUBMIT_PROPOSAL` → `Approved` or `Open` ([`plan_submit`]; always `Open`
//!   when attached to a Drift); `job.ENDED` without a hand-over → `Draft`
//!   ([`plan_refinement_ended`], also when attached to a Drift).
//!
//! The same functions serve the decisions of a Drift on its attached Proposal
//! (`drift.PROPOSAL_ACCEPTED`, `CODE_TO_BE_FIXED`, `DISMISSED`): they take a [`Decider`].

use std::collections::BTreeSet;
use std::path::Path;

use crate::project::board::{AgentFacts, Refused};
use crate::project::cards::{Card, CardKind};
use crate::project::changes::{self, Change, ChangeSet, MutationError};
use crate::project::ids::{card_stem, draw_identity, slugify};
use crate::project::image::ProjectImage;
use crate::project::layout::{self, TASKS_DIR};
use crate::project::random::RandomSource;
use crate::project::shortnum::{self, ShortNumbers};
use crate::project::tasks::{self, render_task, TaskDraft};
use crate::project_files::{self, SyncOptions, SyncReport};

use super::analysis::{self, Analysis, Gate, Reason};
use super::data::{self, set, strings, Kind, Status};
use super::files::{self, Folders};
use super::resolution;

/// Why a decision is refused: every reason, in order.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{}", .0.join("; "))]
pub struct Refusal(pub Vec<String>);

fn refuse<T>(reason: impl Into<String>) -> Result<T, Refusal> {
    Err(Refusal(vec![reason.into()]))
}

/// Who decides: the developer, or the Drift the Proposal is attached to (a Proposal attached to a
/// Drift is never decided on its own).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decider {
    Developer,
    Drift,
}

/// The first two reasons, from what the app knows.
pub fn gate_of(facts: &AgentFacts, card: &Card) -> Gate {
    Gate {
        agent_finishing: facts.is_finishing(card),
        run_in_progress: facts.run_in_progress,
    }
}

fn merge(into: &mut ChangeSet, from: &ChangeSet) {
    for (rel, change) in from.iter() {
        match change {
            Change::Write(content) => into.write(rel, content.clone()),
            Change::Delete => into.delete(rel),
        }
    }
}

fn name_of(numbers: &ShortNumbers, card: &Card) -> String {
    numbers
        .label_of(CardKind::Proposal, &card.id)
        .unwrap_or_else(|| card.id.clone())
}

fn proposal_card<'a>(image: &'a ProjectImage, id: &str) -> Result<&'a Card, Refusal> {
    match image.card(CardKind::Proposal, id) {
        Some(card) => Ok(card),
        None => refuse(format!("no Proposal has the identity {id}")),
    }
}

fn texts(reasons: &[Reason], numbers: &ShortNumbers) -> Vec<String> {
    reasons.iter().map(|r| r.text(numbers)).collect()
}

// ---------------------------------------------------------------------------------------------
// Approval
// ---------------------------------------------------------------------------------------------

/// Everything approving a Proposal does, all in one [`ChangeSet`]: its files replace the Model's,
/// its Tasks are created in To do with the same identities, a Task removes the code of each
/// element it removes, and for a resolution every listed Drift still open gets its decision.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ApprovalPlan {
    pub changes: ChangeSet,
    /// The Tasks of the Proposal, now in To do.
    pub created_tasks: Vec<String>,
    /// The code-removal Tasks.
    pub removal_tasks: Vec<String>,
    /// The fix Tasks of the Drifts a resolution sent to Code to be fixed.
    pub fix_tasks: Vec<String>,
    /// `(Drift, its new status)`.
    pub decided_drifts: Vec<(String, &'static str)>,
    /// The Requirements changed: the Requirement skills are generated again in the same step.
    pub requirements_changed: bool,
    /// The Requirement skills generated again after the approval was written, when
    /// `requirements_changed`: the report, or why they could not be written (the approval stands).
    pub skills: Option<Result<SyncReport, String>>,
    /// An approved setup: Run all starts.
    pub start_run_all: bool,
}

/// Plans the writing of an approval. The caller has checked that nothing blocks it.
fn plan_approval(
    image: &ProjectImage,
    folders: &Folders,
    analysis: &Analysis,
    card: &Card,
    now: &str,
    rng: &mut dyn RandomSource,
) -> Result<ApprovalPlan, Refusal> {
    let empty = files::Folder::default();
    let own = files::changes_of(card, folders.get(&card.id).unwrap_or(&empty));
    let after = analysis::overlay(image, folders, &card.id);
    let mut plan = ApprovalPlan::default();
    merge(&mut plan.changes, &own);
    plan.requirements_changed = own
        .iter()
        .any(|(rel, _)| rel.starts_with(&format!("{}/", layout::REQUIREMENTS_DIR)));

    let job = data::job_of(card);
    for task in data::tasks_of(image, &card.id) {
        let (path, content) = tasks::approved_task_file(task, &card.id, now, job);
        plan.changes.write(&path, content);
        plan.created_tasks.push(task.id.clone());
    }

    let mut used: BTreeSet<String> = image.used_ids(CardKind::Task);
    for removal in &analysis.removals {
        let id = draw_identity(rng, &|candidate| used.contains(candidate));
        used.insert(id.clone());
        let title = removal.title();
        let draft = TaskDraft {
            title: title.clone(),
            body: String::new(),
            refs: removal.refs(),
            technical: removal.technical(),
            proposal: Some(card.id.clone()),
            drift: None,
            job: job.map(str::to_owned),
        };
        plan.changes.write(
            &format!("{TASKS_DIR}/{}.md", card_stem(&id, &slugify(&title))),
            render_task(&draft, now),
        );
        plan.removal_tasks.push(id);
    }

    if analysis.kind == Kind::Resolution {
        let decisions = resolution::plan_decisions(image, &after, card, now, rng, &mut used)
            .map_err(|why| Refusal(vec![why]))?;
        merge(&mut plan.changes, &decisions.changes);
        plan.fix_tasks = decisions.fix_tasks;
        plan.decided_drifts = decisions.decided;
    }

    plan.changes.write(
        &card.path,
        data::rewritten(card, |front| {
            set(front, "status", Status::Approved.as_str());
            front.remove("refining");
        }),
    );
    plan.start_run_all = analysis.kind == Kind::Setup;
    Ok(plan)
}

/// What `developer.APPROVE_PROPOSAL` does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Approval {
    /// Approved: everything is in the changes. The Requirement skills are generated again when
    /// `requirements_changed`, and Run all starts when `start_run_all`.
    Approve(ApprovalPlan),
    /// A stale Proposal of any kind, attached to a Drift or not, whatever else blocks it:
    /// `Refining`. A refinement job adapts it to the current Model and Requirements without a new
    /// interview: the Jobs start it.
    Refine(ChangeSet),
    /// A Draft a refinement left half-way: `Refining` again; a new refinement job continues it.
    Continue(ChangeSet),
}

impl Approval {
    pub fn changes(&self) -> &ChangeSet {
        match self {
            Approval::Approve(plan) => &plan.changes,
            Approval::Refine(changes) | Approval::Continue(changes) => changes,
        }
    }

    /// A refinement job must start.
    pub fn needs_refinement_job(&self) -> bool {
        !matches!(self, Approval::Approve(_))
    }
}

fn refining_text(card: &Card) -> String {
    data::rewritten(card, |front| {
        set(front, "status", Status::Refining.as_str());
        set(front, "refining", true);
    })
}

/// `developer.APPROVE_PROPOSAL` (and, with `Decider::Drift`, the decisions of the Drift on its
/// attached Proposal: `drift.PROPOSAL_ACCEPTED`, `developer.REFINE_DRIFT_PROPOSAL`).
///
/// * Open, nothing blocks it, the job that handed it over has ended and no Run works: `Approved`.
/// * Open and stale, of any kind, whatever else blocks it and also while a Run works: `Refining`
///   ([`Approval::Refine`]). Refused only while the agent that handed it over is still finishing
///   and, for a resolution, when none of its Drifts is still open.
/// * A Draft with `refining: true`: `Refining` again ([`Approval::Continue`]), with the same
///   refusals.
///
/// A Proposal attached to a Drift is decided through its Drift: the developer's own approval is
/// refused for it, the Board's `Refine` included. Everything is refused before anything is
/// written, naming every reason in the order of the control.
#[allow(clippy::too_many_arguments)]
pub fn plan_approve(
    image: &ProjectImage,
    folders: &Folders,
    numbers: &ShortNumbers,
    id: &str,
    gate: Gate,
    now: &str,
    rng: &mut dyn RandomSource,
    by: Decider,
) -> Result<Approval, Refusal> {
    let card = proposal_card(image, id)?;
    let status = data::status_of(card);
    if by == Decider::Developer && data::attached_drift(image, id).is_some() {
        return refuse("it is attached to a Drift: it is decided through its Drift");
    }
    match status {
        Some(Status::Draft) if data::refining_flag(card) => {
            let analysis = analysis::analyse(image, folders, numbers, id)
                .ok_or_else(|| Refusal(vec![format!("no Proposal has the identity {id}")]))?;
            refinement_refusal(&analysis, gate)?;
            Ok(Approval::Continue(single(&card.path, refining_text(card))))
        }
        Some(Status::Draft) => refuse("the agent is still writing it: nobody can approve a Draft"),
        Some(Status::Refining) => refuse("Being refined"),
        Some(Status::Open) => {
            let analysis = analysis::analyse(image, folders, numbers, id)
                .ok_or_else(|| Refusal(vec![format!("no Proposal has the identity {id}")]))?;
            if !analysis.stale.is_empty() {
                refinement_refusal(&analysis, gate)?;
                return Ok(Approval::Refine(single(&card.path, refining_text(card))));
            }
            let reasons = analysis.approve_reasons(gate);
            if reasons.is_empty() {
                return plan_approval(image, folders, &analysis, card, now, rng)
                    .map(Approval::Approve);
            }
            Err(Refusal(texts(&reasons, numbers)))
        }
        Some(done) => refuse(format!("the Proposal is {}", done.as_str())),
        None => refuse("the Proposal has no valid status"),
    }
}

/// Refuses a refinement (or the continuation of one) for the reasons of
/// [`Analysis::refinement_refusals`], every one of them, before anything is written.
fn refinement_refusal(analysis: &Analysis, gate: Gate) -> Result<(), Refusal> {
    let refusals = analysis.refinement_refusals(gate);
    if refusals.is_empty() {
        Ok(())
    } else {
        Err(Refusal(
            refusals.into_iter().map(|r| r.text().to_owned()).collect(),
        ))
    }
}

fn single(path: &str, content: String) -> ChangeSet {
    let mut changes = ChangeSet::new();
    changes.write(path, content);
    changes
}

// ---------------------------------------------------------------------------------------------
// Hand-over
// ---------------------------------------------------------------------------------------------

/// What the hand-over knows beyond the files.
#[derive(Debug, Clone, Copy)]
pub struct SubmitContext<'a> {
    /// The instant of the hand-over (the creation instant of the Tasks if it is approved).
    pub now: &'a str,
    /// The job whose agent hands the Proposal over found the Model with no Feature and no Actor
    /// when it reached Running. Only a setup or import it hands over is approved at once.
    pub job_found_empty_model: bool,
    /// A Run works in the project.
    pub run_in_progress: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Submitted {
    /// Handed over: the developer decides.
    Open,
    /// Approved at the hand-over: a setup or import of a job that started on an empty Model, or a
    /// refined Proposal that nothing blocks.
    Approved(ApprovalPlan),
    /// Refined, not attached to a Drift, and only a Run works: it stays Refining. Hand it over
    /// again when the Run ends.
    WaitsForRun,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Submission {
    pub outcome: Submitted,
    pub changes: ChangeSet,
    /// Reasons the Proposal could not be approved yet (required Proposals, staleness, a Run).
    pub warnings: Vec<String>,
    /// The design request it consumes: the Proposal bound to it was handed over.
    pub consumed_request: Option<String>,
}

/// `agent.SUBMIT_PROPOSAL`. Refused, naming every problem, for reasons 5–7 of Approve and for
/// the checks of the hand-over; never for staleness or required Proposals.
pub fn plan_submit(
    image: &ProjectImage,
    folders: &Folders,
    numbers: &ShortNumbers,
    id: &str,
    ctx: &SubmitContext,
    rng: &mut dyn RandomSource,
) -> Result<Submission, Refusal> {
    let card = proposal_card(image, id)?;
    let status = data::status_of(card);
    match status {
        Some(Status::Draft | Status::Refining) => {}
        Some(Status::Open) => return refuse("it is already handed over"),
        Some(done) => {
            return refuse(format!(
                "the Proposal is {}: it cannot be handed over",
                done.as_str()
            ))
        }
        None => return refuse("the Proposal has no valid status"),
    }
    let analysis = analysis::analyse(image, folders, numbers, id)
        .ok_or_else(|| Refusal(vec![format!("no Proposal has the identity {id}")]))?;
    let refusals = analysis.handover_refusals(numbers);
    if !refusals.is_empty() {
        return Err(Refusal(refusals));
    }

    // Reason 1 does not count: the job that hands the Proposal over is still working.
    let gate = Gate {
        agent_finishing: false,
        run_in_progress: ctx.run_in_progress,
    };
    let refined = status == Some(Status::Refining);
    let automatic = !refined
        && matches!(analysis.kind, Kind::Setup | Kind::Import)
        && ctx.job_found_empty_model;
    let reasons = analysis.approve_reasons(gate);
    let blocking = analysis.blocking();
    let mut changes = ChangeSet::new();
    // Attached to a Drift, a refined Proposal is always handed over as Open: the developer
    // accepts it again from the Drift.
    let to_drift = refined && analysis.attached;

    let outcome = if !to_drift && (automatic || refined) && reasons.is_empty() {
        let plan = plan_approval(image, folders, &analysis, card, ctx.now, rng)?;
        merge(&mut changes, &plan.changes);
        Submitted::Approved(plan)
    } else if !to_drift && refined && blocking.is_empty() {
        return Ok(Submission {
            outcome: Submitted::WaitsForRun,
            changes,
            warnings: texts(&reasons, numbers),
            consumed_request: None,
        });
    } else {
        changes.write(
            &card.path,
            data::rewritten(card, |front| {
                set(front, "status", Status::Open.as_str());
                front.remove("refining");
            }),
        );
        Submitted::Open
    };

    // The first Proposal opened with `reworks` takes the place of the rejected one in the
    // requirements of the Proposals that waited for it.
    if let Some(old) = data::reworks_of(card) {
        for other in image.cards_of(CardKind::Proposal) {
            if other.id == card.id || !data::status_of(other).is_some_and(Status::is_live) {
                continue;
            }
            let requires = data::requires_of(other);
            if requires.iter().any(|r| r == old) {
                let mut replaced: Vec<String> = Vec::new();
                for required in requires {
                    let required = if required == old {
                        card.id.clone()
                    } else {
                        required
                    };
                    if !replaced.contains(&required) {
                        replaced.push(required);
                    }
                }
                changes.write(
                    &other.path,
                    data::rewritten(other, |front| set(front, "requires", strings(&replaced))),
                );
            }
        }
    }

    let warnings = match outcome {
        Submitted::Approved(_) => Vec::new(),
        _ => {
            let mut warnings = texts(&reasons, numbers);
            let after = analysis::overlay(image, folders, id);
            warnings.extend(project_files::sync::mode_switch_warning(
                image.areas(),
                after.areas(),
            ));
            warnings
        }
    };
    let consumed_request = if refined {
        None
    } else {
        data::request_of(card).map(str::to_owned)
    };
    Ok(Submission {
        outcome,
        changes,
        warnings,
        consumed_request,
    })
}

/// `job.ENDED` of a refinement job that did not hand the Proposal over: `Refining` → `Draft`, with
/// `refining: true`, of any kind and also when attached to a Drift; it stays half-way until the
/// developer refines it again (from its Drift when it is attached to one) or discards it.
pub fn plan_refinement_ended(image: &ProjectImage, id: &str) -> ChangeSet {
    let mut changes = ChangeSet::new();
    if let Some(card) = image.card(CardKind::Proposal, id) {
        if data::status_of(card) == Some(Status::Refining) {
            changes.write(
                &card.path,
                data::rewritten(card, |front| {
                    set(front, "status", Status::Draft.as_str());
                    set(front, "refining", true);
                }),
            );
        }
    }
    changes
}

// ---------------------------------------------------------------------------------------------
// Reject, rework, discard
// ---------------------------------------------------------------------------------------------

/// What rejecting does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rejection {
    pub changes: ChangeSet,
    /// A rework job writes it again.
    pub rework: bool,
}

/// The text of `card` rejected, with the feedback appended under `## Feedback` only when it has
/// non-blank text.
fn rejected_text(card: &Card, feedback: Option<&str>, rework: bool) -> String {
    let feedback = feedback.map(str::trim).filter(|f| !f.is_empty());
    let mut text = data::rewritten(card, |front| {
        set(front, "status", Status::Rejected.as_str());
        front.remove("refining");
        if rework {
            set(front, "reworkRequested", true);
        }
    });
    if let Some(feedback) = feedback {
        if !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(&format!("\n## Feedback\n\n{feedback}\n"));
    }
    text
}

/// `developer.REJECT_PROPOSAL` and, with `rework`, `developer.REWORK_PROPOSAL`;
/// `drift.CODE_TO_BE_FIXED` with `Decider::Drift`.
///
/// An Open Proposal is rejected, or reworked (once: a Proposal that is itself a rework is not
/// reworked again). A Refining one is rejected when the developer chose to remove a change that
/// is already in the Model. Both are refused while the agent that handed the Proposal over is
/// still finishing. `feedback` (the reason of a rejection, the feedback of a rework) stays with
/// the Proposal under `## Feedback` when it has text.
pub fn plan_reject(
    image: &ProjectImage,
    numbers: &ShortNumbers,
    id: &str,
    feedback: Option<&str>,
    rework: bool,
    gate: Gate,
    by: Decider,
) -> Result<Rejection, Refusal> {
    let card = proposal_card(image, id)?;
    if by == Decider::Developer && data::attached_drift(image, id).is_some() {
        return refuse("it is attached to a Drift: it is decided through its Drift");
    }
    let status = data::status_of(card);
    match status {
        Some(Status::Open) => {
            if gate.agent_finishing {
                return refuse(Reason::AgentFinishing.text(numbers));
            }
        }
        Some(Status::Refining) if !rework => {}
        Some(Status::Refining) => return refuse("Being refined"),
        Some(Status::Draft) if by == Decider::Drift => {}
        Some(Status::Draft) => {
            return refuse("the agent is still writing it: nobody can reject a Draft; discard it")
        }
        Some(done) => return refuse(format!("the Proposal is {}", done.as_str())),
        None => return refuse("the Proposal has no valid status"),
    }
    if rework {
        if by == Decider::Drift {
            return refuse("a Drift never asks for a rework");
        }
        if data::reworks_of(card).is_some() {
            return refuse("it is already a rework: a Proposal is reworked at most once");
        }
    }
    Ok(Rejection {
        changes: single(&card.path, rejected_text(card, feedback, rework)),
        rework,
    })
}

/// `developer.DISCARD_PROPOSAL`: a Draft, only when no working job is writing it (a Draft attached
/// to a Drift too: the Drift stays open). `drift.DISMISSED` and `drift.CODE_TO_BE_FIXED` discard
/// the Draft or Open Proposal attached to the Drift (`Decider::Drift`).
pub fn plan_discard(
    image: &ProjectImage,
    numbers: &ShortNumbers,
    id: &str,
    gate: Gate,
    by: Decider,
) -> Result<ChangeSet, Refusal> {
    let card = proposal_card(image, id)?;
    let status = data::status_of(card);
    match (status, by) {
        (Some(Status::Draft), _) => {
            if gate.agent_finishing {
                return refuse(Reason::AgentFinishing.text(numbers));
            }
        }
        (Some(Status::Open), Decider::Drift) => {}
        (Some(Status::Open), Decider::Developer) => {
            return refuse("an Open Proposal is rejected, not discarded")
        }
        (Some(Status::Refining), _) => return refuse("Being refined"),
        (Some(done), _) => return refuse(format!("the Proposal is {}", done.as_str())),
        (None, _) => return refuse("the Proposal has no valid status"),
    }
    Ok(single(
        &card.path,
        data::with_status(card, Status::Discarded),
    ))
}

/// `developer.CHOOSE_PROPOSAL_MODEL` is only for an Open Proposal that carries Tasks.
pub fn check_model_choice(image: &ProjectImage, id: &str) -> Result<(), Refusal> {
    let card = proposal_card(image, id)?;
    if data::status_of(card) != Some(Status::Open) {
        return refuse("the model is chosen for an Open Proposal");
    }
    if data::tasks_of(image, id).is_empty() {
        return refuse("the Proposal carries no Task: there is no model to choose");
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Under the project lock
// ---------------------------------------------------------------------------------------------

fn refused(verb: &str, name: &str, refusal: &Refusal) -> Refused {
    Refused {
        notice: format!("Could not {verb} {name}: {}.", refusal.0.join("; ")),
    }
}

fn name_at(image: &ProjectImage, numbers: &ShortNumbers, id: &str) -> String {
    image
        .card(CardKind::Proposal, id)
        .map_or_else(|| id.to_owned(), |c| name_of(numbers, c))
}

/// Whether the agent can start a job now: `Err` says why not. A decision that starts a job is
/// refused, writing nothing, when it cannot.
pub type JobCheck<'a> = &'a dyn Fn() -> Result<(), String>;

/// `Approve`, under the project lock: reads the project and the folders again, plans, writes the
/// whole change or nothing. Returns the image after it and what the approval did.
pub fn approve(
    project_root: &Path,
    facts: &AgentFacts,
    id: &str,
    by: Decider,
    now: &str,
    rng: &mut dyn RandomSource,
) -> Result<(ProjectImage, Approval), MutationError<Refused>> {
    approve_with(project_root, facts, id, by, now, rng, &|| Ok(()))
}

/// [`approve`] for the app: an approval that starts a refinement job (`Refining`) first asks
/// `jobs` whether one can start, and is refused, writing nothing, when it cannot.
pub fn approve_with(
    project_root: &Path,
    facts: &AgentFacts,
    id: &str,
    by: Decider,
    now: &str,
    rng: &mut dyn RandomSource,
    jobs: JobCheck,
) -> Result<(ProjectImage, Approval), MutationError<Refused>> {
    let (image, mut approval) = changes::mutate(
        project_root,
        |image| {
            let folders = files::read_folders(project_root, image);
            let numbers = shortnum::peek(project_root, image.cards());
            let name = name_at(image, &numbers, id);
            let gate = image
                .card(CardKind::Proposal, id)
                .map(|c| gate_of(facts, c))
                .unwrap_or_default();
            let approval = plan_approve(image, &folders, &numbers, id, gate, now, rng, by)
                .map_err(|refusal| refused("approve", &name, &refusal))?;
            if approval.needs_refinement_job() {
                jobs().map_err(|why| refused("approve", &name, &Refusal(vec![why])))?;
            }
            Ok((approval.changes().clone(), approval))
        },
        |_| Ok(()),
    )?;
    if let Approval::Approve(plan) = &mut approval {
        plan.skills = regenerate_skills(project_root, &image, plan.requirements_changed);
    }
    Ok((image, approval))
}

/// The Requirement skills generated again from the Requirements an approval just wrote.
pub(crate) fn regenerate_skills(
    project_root: &Path,
    image: &ProjectImage,
    requirements_changed: bool,
) -> Option<Result<SyncReport, String>> {
    requirements_changed.then(|| {
        project_files::sync::sync_image(project_root, image, SyncOptions::requirement_skills())
            .map_err(|error| error.to_string())
    })
}

/// `Reject` and `Rework with feedback…`, under the project lock.
pub fn reject(
    project_root: &Path,
    facts: &AgentFacts,
    id: &str,
    feedback: Option<&str>,
    rework: bool,
    by: Decider,
) -> Result<(ProjectImage, Rejection), MutationError<Refused>> {
    reject_with(project_root, facts, id, feedback, rework, by, &|| Ok(()))
}

/// [`reject`] for the app: a rework first asks `jobs` whether a job can start, and is refused,
/// writing nothing, when it cannot.
pub fn reject_with(
    project_root: &Path,
    facts: &AgentFacts,
    id: &str,
    feedback: Option<&str>,
    rework: bool,
    by: Decider,
    jobs: JobCheck,
) -> Result<(ProjectImage, Rejection), MutationError<Refused>> {
    changes::mutate(
        project_root,
        |image| {
            let numbers = shortnum::peek(project_root, image.cards());
            let name = name_at(image, &numbers, id);
            let verb = if rework { "rework" } else { "reject" };
            let gate = image
                .card(CardKind::Proposal, id)
                .map(|c| gate_of(facts, c))
                .unwrap_or_default();
            let rejection = plan_reject(image, &numbers, id, feedback, rework, gate, by)
                .map_err(|refusal| refused(verb, &name, &refusal))?;
            if rejection.rework {
                jobs().map_err(|why| refused(verb, &name, &Refusal(vec![why])))?;
            }
            Ok((rejection.changes.clone(), rejection))
        },
        |_| Ok(()),
    )
}

/// `Discard`, under the project lock.
pub fn discard(
    project_root: &Path,
    facts: &AgentFacts,
    id: &str,
    by: Decider,
) -> Result<ProjectImage, MutationError<Refused>> {
    changes::mutate(
        project_root,
        |image| {
            let numbers = shortnum::peek(project_root, image.cards());
            let name = name_at(image, &numbers, id);
            let gate = image
                .card(CardKind::Proposal, id)
                .map(|c| gate_of(facts, c))
                .unwrap_or_default();
            let changes = plan_discard(image, &numbers, id, gate, by)
                .map_err(|refusal| refused("discard", &name, &refusal))?;
            Ok((changes, ()))
        },
        |_| Ok(()),
    )
    .map(|(image, ())| image)
}

/// The hand-over, under the project lock.
pub fn submit(
    project_root: &Path,
    id: &str,
    ctx: &SubmitContext,
    rng: &mut dyn RandomSource,
) -> Result<(ProjectImage, Submission), MutationError<Refusal>> {
    let (image, mut submission) = changes::mutate(
        project_root,
        |image| {
            let folders = files::read_folders(project_root, image);
            let numbers = shortnum::peek(project_root, image.cards());
            let submission = plan_submit(image, &folders, &numbers, id, ctx, rng)?;
            Ok((submission.changes.clone(), submission))
        },
        |_| Ok(()),
    )?;
    if let Submitted::Approved(plan) = &mut submission.outcome {
        plan.skills = regenerate_skills(project_root, &image, plan.requirements_changed);
    }
    Ok((image, submission))
}
