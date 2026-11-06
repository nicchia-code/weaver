//! The app side of the Proposal: the decisions of the developer on its card (`Approve`, `Reject`,
//! `Rework with feedback…`, `Discard`, the model control) and what a decision starts.
//!
//! The rules live in `weaver_core::project::proposal`. A decision runs through
//! [`crate::decisions::decide`]: in the project's queue, under the project lock, with the facts of
//! that moment, checked again when applied, and written whole or not at all. What it starts (a
//! refinement job, a rework job, Run all after a setup) goes through [`ProposalJobs`], which the
//! jobs of the agent implement and install in [`ProposalJobsSlot`]; until they do, a decision that
//! needs a job is refused with a notice and writes nothing.
//!
//! Contract with the jobs:
//!
//! * `Approve` on a stale Proposal of any kind, or on a Draft a refinement left half-way, writes `Refining` and
//!   then calls [`ProposalJobs::start_refinement`]; the job's agent rewrites the Proposal on the
//!   current Model (`weaver proposal edit <id> {rebase: true, …}`) and hands it over again. When the
//!   job ends without handing it over, the jobs call
//!   `proposal::transitions::plan_refinement_ended`;
//! * `Rework with feedback…` writes `Rejected` (with `reworkRequested`) and then calls
//!   [`ProposalJobs::start_rework`]; the agent writes the new Proposals with
//!   `weaver proposal new {reworks: <id>}`;
//! * a refined Proposal handed over while a Run works stays Refining without a job (the hand-over
//!   answers `waits for the Run`): when the Run ends the jobs hand over again every Refining
//!   Proposal that has no working job, with `transitions::submit`;
//! * an approved setup (at the hand-over of a job that started on an empty Model, or by the
//!   developer) calls [`ProposalJobs::run_all_after_setup`].

use std::path::Path;
use std::sync::{Arc, Mutex, RwLock};

use serde_json::json;
use tauri::{AppHandle, State};
use weaver_cli::spec::{Call, Failure};
use weaver_core::command::{CommandReply, CommandRequest};
use weaver_core::project::board::Refused;
use weaver_core::project::cards::CardKind;
use weaver_core::project::changes::MutationError;
use weaver_core::project::clock::{now_instant, SystemClock};
use weaver_core::project::image::ProjectImage;
use weaver_core::project::proposal::data::{self, Kind, Status};
use weaver_core::project::proposal::models::ProposalModels;
use weaver_core::project::proposal::transitions::{self, Approval, Decider};
use weaver_core::project::random::OsRandom;
use weaver_core::project::shortnum;

use crate::decisions;
use crate::error::{AppError, ErrorCode};
use crate::events::{self, Notice};
use crate::pictures::Revision;
use crate::state::AppState;

/// What a decision on a Proposal starts. The methods are synchronous: an implementation spawns
/// its work and returns.
pub trait ProposalJobs: Send + Sync {
    /// Whether a refinement or a rework job can start in the project now; `Err` says why not, in
    /// words for a notice. Asked before anything is written.
    fn can_start(&self, project_id: &str) -> Result<(), String>;
    /// A refinement job adapts the Proposal (now Refining) to the current Model.
    fn start_refinement(&self, project_id: &str, proposal_id: &str);
    /// A rework job writes the Proposal again from the developer's feedback (now Rejected).
    fn start_rework(&self, project_id: &str, proposal_id: &str);
    /// A setup was approved: Run all starts.
    fn run_all_after_setup(&self, project_id: &str);
}

/// No jobs exist: nothing can start.
pub struct NoProposalJobs;

impl ProposalJobs for NoProposalJobs {
    fn can_start(&self, _project_id: &str) -> Result<(), String> {
        Err("no agent job can start yet".to_owned())
    }

    fn start_refinement(&self, _project_id: &str, _proposal_id: &str) {}

    fn start_rework(&self, _project_id: &str, _proposal_id: &str) {}

    fn run_all_after_setup(&self, _project_id: &str) {}
}

/// The installed [`ProposalJobs`], replaceable at runtime by whoever owns the jobs.
pub struct ProposalJobsSlot {
    current: RwLock<Arc<dyn ProposalJobs>>,
}

impl Default for ProposalJobsSlot {
    fn default() -> Self {
        Self {
            current: RwLock::new(Arc::new(NoProposalJobs)),
        }
    }
}

impl ProposalJobsSlot {
    pub fn get(&self) -> Arc<dyn ProposalJobs> {
        self.current
            .read()
            .expect("proposal jobs slot poisoned")
            .clone()
    }

    pub fn set(&self, jobs: Arc<dyn ProposalJobs>) {
        *self.current.write().expect("proposal jobs slot poisoned") = jobs;
    }
}

fn refused(notice: impl Into<String>) -> MutationError<Refused> {
    MutationError::Refused(Refused {
        notice: notice.into(),
    })
}

fn now() -> Result<String, MutationError<Refused>> {
    now_instant(&SystemClock).map_err(|e| refused(format!("the clock gives no valid time: {e}")))
}

/// Starts what an approval asks for, once it is written.
fn after_approval(
    jobs: &dyn ProposalJobs,
    project_id: &str,
    proposal_id: &str,
    approval: &Approval,
) {
    match approval {
        Approval::Refine(_) | Approval::Continue(_) => {
            jobs.start_refinement(project_id, proposal_id)
        }
        Approval::Approve(plan) if plan.start_run_all => jobs.run_all_after_setup(project_id),
        Approval::Approve(_) => {}
    }
}

/// Forgets the models of the Proposals that are decided and have no Task to do.
fn prune_models(state: &AppState, project_id: &str) -> Result<(), AppError> {
    let image = state.pictures.get(project_id)?.image;
    let mut models = ProposalModels::load(&state.dirs, project_id);
    if models.prune(&image) {
        models.save(&state.dirs, project_id)?;
    }
    Ok(())
}

/// `Approve`: `Approved`, or `Refining` for a stale Proposal of any kind, or `Refining` again for
/// a Draft left half-way. Returns the new revision.
pub async fn approve(
    state: &AppState,
    project_id: &str,
    proposal_id: &str,
) -> Result<Option<Revision>, AppError> {
    let jobs = state.proposal_jobs.get();
    let outcome: Arc<Mutex<Option<Approval>>> = Arc::default();
    let (slot, check, project, proposal) = (
        outcome.clone(),
        jobs.clone(),
        project_id.to_owned(),
        proposal_id.to_owned(),
    );
    let revision = decisions::decide(state, project_id, move |root, facts| {
        let created = now()?;
        let (image, approval) = transitions::approve_with(
            root,
            facts,
            &proposal,
            Decider::Developer,
            &created,
            &mut OsRandom,
            &|| check.can_start(&project),
        )?;
        *slot.lock().expect("outcome poisoned") = Some(approval);
        Ok(image)
    })
    .await?;
    if let Some(approval) = outcome.lock().expect("outcome poisoned").take() {
        after_approval(jobs.as_ref(), project_id, proposal_id, &approval);
    }
    prune_models(state, project_id)?;
    Ok(revision)
}

/// `Reject`, and `Rework with feedback…` with `rework`. `feedback` is the reason, or the feedback
/// of the rework; it stays with the Proposal when it has text.
pub async fn reject(
    state: &AppState,
    project_id: &str,
    proposal_id: &str,
    feedback: Option<String>,
    rework: bool,
) -> Result<Option<Revision>, AppError> {
    let jobs = state.proposal_jobs.get();
    let (check, project, proposal) = (jobs.clone(), project_id.to_owned(), proposal_id.to_owned());
    let revision = decisions::decide(state, project_id, move |root, facts| {
        let (image, _) = transitions::reject_with(
            root,
            facts,
            &proposal,
            feedback.as_deref(),
            rework,
            Decider::Developer,
            &|| check.can_start(&project),
        )?;
        Ok(image)
    })
    .await?;
    if rework {
        jobs.start_rework(project_id, proposal_id);
    }
    prune_models(state, project_id)?;
    Ok(revision)
}

/// `Discard`: a Draft a refinement left half-way.
pub async fn discard(
    state: &AppState,
    project_id: &str,
    proposal_id: &str,
) -> Result<Option<Revision>, AppError> {
    let proposal = proposal_id.to_owned();
    let revision = decisions::decide(state, project_id, move |root, facts| {
        transitions::discard(root, facts, &proposal, Decider::Developer)
    })
    .await?;
    prune_models(state, project_id)?;
    Ok(revision)
}

/// The most characters of a model value (a selector of omp such as `anthropic/claude-sonnet-5`).
const MODEL_MAX: usize = 200;

/// Chooses the model of an Open Proposal that carries Tasks, or goes back to the Settings models
/// with `None`. It is kept in the local data, never in the project, and never sends the Proposal
/// back to Draft: no file of the project changes, only the Board is asked again.
pub async fn choose_model(
    state: &AppState,
    project_id: &str,
    proposal_id: &str,
    model: Option<String>,
) -> Result<Revision, AppError> {
    if let Some(model) = &model {
        if model.trim().is_empty()
            || model.contains(['\n', '\r'])
            || model.chars().count() > MODEL_MAX
        {
            return Err(AppError::new(
                ErrorCode::InvalidInput,
                format!("the model is one line of 1 to {MODEL_MAX} characters"),
            ));
        }
    }
    let root = state.pictures.get(project_id)?.root;
    let (dirs, hash, proposal) = (
        state.dirs.clone(),
        project_id.to_owned(),
        proposal_id.to_owned(),
    );
    state
        .mutations
        .run(project_id, move || {
            let image = ProjectImage::load(&root);
            let numbers = shortnum::peek(&root, image.cards());
            transitions::check_model_choice(&image, &proposal).map_err(|refusal| {
                let name = numbers
                    .label_of(CardKind::Proposal, &proposal)
                    .unwrap_or_else(|| proposal.clone());
                AppError::new(
                    ErrorCode::Refused,
                    format!("Could not choose the model of {name}: {refusal}."),
                )
            })?;
            let mut models = ProposalModels::load(&dirs, &hash);
            models.set(&proposal, model.as_deref());
            models.prune(&image);
            models.save(&dirs, &hash)?;
            Ok(())
        })
        .await?;
    state
        .pictures
        .touch(project_id, &[crate::pictures::SLICE_BOARD])
}

// ---------------------------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------------------------

fn announce(app: &AppHandle, revision: Option<Revision>) -> Result<(), AppError> {
    match revision {
        Some(revision) => events::emit_project_changed(app, &revision.event()),
        None => Ok(()),
    }
}

/// **Approve** on a Proposal. Refused with `refused` (the message is the notice) when a condition
/// no longer holds at the moment of applying; nothing is written then.
#[tauri::command]
pub async fn approve_proposal(
    app: AppHandle,
    state: State<'_, AppState>,
    project_id: String,
    proposal_id: String,
) -> Result<(), AppError> {
    announce(&app, approve(&state, &project_id, &proposal_id).await?)
}

/// **Reject** and **Rework with feedback…** (`rework`). `feedback` is the optional reason or the
/// feedback; the UI confirms a rework without feedback before calling.
#[tauri::command]
pub async fn reject_proposal(
    app: AppHandle,
    state: State<'_, AppState>,
    project_id: String,
    proposal_id: String,
    feedback: Option<String>,
    rework: bool,
) -> Result<(), AppError> {
    announce(
        &app,
        reject(&state, &project_id, &proposal_id, feedback, rework).await?,
    )
}

/// **Discard** on a Draft a refinement left half-way.
#[tauri::command]
pub async fn discard_proposal(
    app: AppHandle,
    state: State<'_, AppState>,
    project_id: String,
    proposal_id: String,
) -> Result<(), AppError> {
    announce(&app, discard(&state, &project_id, &proposal_id).await?)
}

/// The model control: `model` is a selector of omp, or `None` for the Settings models.
#[tauri::command]
pub async fn choose_proposal_model(
    app: AppHandle,
    state: State<'_, AppState>,
    project_id: String,
    proposal_id: String,
    model: Option<String>,
) -> Result<(), AppError> {
    let revision = choose_model(&state, &project_id, &proposal_id, model).await?;
    events::emit_project_changed(&app, &revision.event())
}

// ---------------------------------------------------------------------------------------------
// What the command line asks of the app
// ---------------------------------------------------------------------------------------------

fn failure_of(error: MutationError<Refused>) -> Failure {
    match error {
        MutationError::Refused(refused) => Failure::refused(refused.notice),
        other => Failure::failed(other.to_string()),
    }
}

fn answer(call: &Call, image: &ProjectImage, id: &str, markdown: String) -> CommandReply {
    let numbers = shortnum::peek(call.root, image.cards());
    let card = image.card(CardKind::Proposal, id);
    let json = json!({
        "id": id,
        "number": numbers.number_of(CardKind::Proposal, id),
        "title": card.map_or("", |c| c.title.as_str()),
        "kind": card.map_or(Kind::Change, data::kind_of).as_str(),
        "status": card.and_then(data::status_of).map_or("draft", Status::as_str),
        "warnings": [],
    });
    CommandReply {
        markdown,
        json,
        exit: 0,
        note: None,
    }
}

fn label(call: &Call, id: &str) -> String {
    shortnum::peek(call.root, call.image.cards())
        .label_of(CardKind::Proposal, id)
        .unwrap_or_else(|| id.to_owned())
}

/// `weaver proposal move <id> approved` inside the app: it can start the refinement job a stale
/// Proposal of any kind needs. Runs in the project's queue; the facts are the app's.
fn approve_from_cli(
    call: &Call,
    jobs: &dyn ProposalJobs,
    project_id: &str,
    id: &str,
) -> Result<CommandReply, Failure> {
    let created = now_instant(&SystemClock).map_err(|e| Failure::failed(e.to_string()))?;
    let (image, approval) = transitions::approve_with(
        call.root,
        &call.caller.agent,
        id,
        Decider::Developer,
        &created,
        &mut OsRandom,
        &|| jobs.can_start(project_id),
    )
    .map_err(failure_of)?;
    after_approval(jobs, project_id, id, &approval);
    let name = label(call, id);
    let markdown = match &approval {
        Approval::Approve(plan) => format!(
            "# {name} approved\n\nThe changes of {name} are in the Model, the Requirements and the Specs.{}\n",
            match plan.created_tasks.len() + plan.removal_tasks.len() + plan.fix_tasks.len() {
                0 => String::new(),
                n => format!("\n\n{n} Task(s) are in To do."),
            }
        ),
        Approval::Refine(_) => format!("# {name} is being refined\n\n{name} is stale: a refinement job adapts it to the current Model, then it is handed over again.\n"),
        Approval::Continue(_) => format!("# {name} is being refined\n\nA new refinement job continues {name}.\n"),
    };
    Ok(answer(call, &image, id, markdown))
}

/// `weaver proposal move <id> rejected {rework: true}` inside the app: it starts the rework job.
fn rework_from_cli(
    call: &Call,
    jobs: &dyn ProposalJobs,
    project_id: &str,
    id: &str,
) -> Result<CommandReply, Failure> {
    let feedback = call.input.get("feedback").and_then(|v| v.as_str());
    let (image, _) = transitions::reject_with(
        call.root,
        &call.caller.agent,
        id,
        feedback,
        true,
        Decider::Developer,
        &|| jobs.can_start(project_id),
    )
    .map_err(failure_of)?;
    jobs.start_rework(project_id, id);
    let name = label(call, id);
    Ok(answer(call, &image, id, format!("# {name} rejected\n\n{name} is Rejected: a rework job writes it again from the feedback.\n")))
}

/// What the running app does for `weaver proposal move`, instead of the handler of the registry:
///
/// * `approved` and `rejected` with `rework`: they start jobs, which only the app can do;
/// * `open`: the handler hands the Proposal over; an approved setup then starts Run all.
///
/// `None` for any other command, which its handler runs. Called by the authority of the app, in
/// the project's queue.
pub fn answer_cli(call: &Call, jobs: &dyn ProposalJobs) -> Option<Result<CommandReply, Failure>> {
    if call.command.object != "proposal" || call.command.verb != "move" {
        return None;
    }
    let id = call.arg?;
    let project_id = weaver_core::paths::project_hash_of_canonical(call.root);
    match call.state? {
        "approved" => Some(approve_from_cli(call, jobs, &project_id, id)),
        "rejected" if call.input.get("rework").and_then(|v| v.as_bool()) == Some(true) => {
            Some(rework_from_cli(call, jobs, &project_id, id))
        }
        "open" => {
            // `proposal.SUBMITTED`: read before the hand-over, which deletes the request's files.
            let request = call
                .image
                .card(CardKind::Proposal, id)
                .and_then(data::request_of)
                .map(str::to_owned);
            let result = (call.command.run)(call);
            if let (Ok(_), Some(request)) = (&result, &request) {
                crate::requests::proposal_submitted(call.root, request);
            }
            if let Ok(reply) = &result {
                let setup = reply.json["kind"] == "setup" && reply.json["status"] == "approved";
                if setup {
                    jobs.run_all_after_setup(&project_id);
                }
            }
            Some(result)
        }
        _ => None,
    }
}

/// The notice for a Proposal the agent sent back to Draft while the developer could be deciding
/// it: `proposal edit` of an Open Proposal. `None` for any other command or outcome.
pub fn notice_of_command(request: &CommandRequest, reply: &CommandReply) -> Option<Notice> {
    if request.object != "proposal"
        || request.verb != "edit"
        || reply.json["sentBackToDraft"] != true
    {
        return None;
    }
    let id = reply.json["id"].as_str()?;
    let name = reply.json["number"]
        .as_u64()
        .map_or_else(|| id.to_owned(), |n| format!("P{n}"));
    Some(Notice {
        project_id: weaver_core::paths::project_hash_of_canonical(Path::new(&request.root)),
        id: format!("sent-back-{id}"),
        text: format!("{name} went back to Draft: the agent is editing it. It returns to the Board when the agent hands it over again."),
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use serde_json::{json, Map, Value};
    use weaver_cli::spec::Caller;
    use weaver_core::paths::{project_hash_of_canonical, AppDirs};
    use weaver_core::project::text::{file_content, sha256_hex};

    use super::*;

    const ORDER: &str = "# Order\n\nAn order.\n\n## States\n\n### Draft (initial)\nChoosing.\n- `CONFIRM` → Paid\n\n### Paid\n\n## Events\n\n### CONFIRM\nConfirmed.\n";
    const FEATURE: &str = "# Shopping\n\nBuying things.\n";

    /// What the jobs would be asked, and whether they can start.
    #[derive(Default)]
    struct FakeJobs {
        cannot: Mutex<Option<String>>,
        calls: Mutex<Vec<String>>,
    }

    impl FakeJobs {
        fn calls(&self) -> Vec<String> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl ProposalJobs for FakeJobs {
        fn can_start(&self, _project_id: &str) -> Result<(), String> {
            self.cannot.lock().unwrap().clone().map_or(Ok(()), Err)
        }
        fn start_refinement(&self, _project_id: &str, proposal_id: &str) {
            self.calls
                .lock()
                .unwrap()
                .push(format!("refine {proposal_id}"));
        }
        fn start_rework(&self, _project_id: &str, proposal_id: &str) {
            self.calls
                .lock()
                .unwrap()
                .push(format!("rework {proposal_id}"));
        }
        fn run_all_after_setup(&self, _project_id: &str) {
            self.calls.lock().unwrap().push("run all".to_owned());
        }
    }

    fn put(root: &Path, rel: &str, text: &str) {
        let path = root.join(".weaver").join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn hash(text: &str) -> String {
        sha256_hex(&file_content(text))
    }

    fn project() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        put(dir.path(), "model/system.md", "# Shop\n\nA shop.\n");
        put(dir.path(), "model/shop/feature.md", FEATURE);
        put(dir.path(), "model/shop/order.md", ORDER);
        dir
    }

    fn task(refs: &str) -> String {
        format!("---\ntechnical: false\nrefs:\n  - {refs}\n---\n# Pay\n\nDo it.\n")
    }

    /// An Open change of the Machine `order` with a Task, as `status` says.
    fn change(root: &Path, id: &str, status: &str) {
        let base = hash(ORDER);
        put(
            root,
            &format!("proposals/{id}-t/proposal.md"),
            &format!("---\nstatus: {status}\nkind: change\nbase:\n  model/shop/order.md: {base}\nlabel: Shop\ncreated: 2026-11-02T07:00:00.000Z\n---\n# Pay\n\nWhy.\n"),
        );
        put(
            root,
            &format!("proposals/{id}-t/model/shop/order.md"),
            &ORDER.replace("### Paid\n", "### Paid\nPaid.\n"),
        );
        put(
            root,
            &format!("proposals/{id}-t/tasks/tttttttt-pay.md"),
            &task("order"),
        );
    }

    struct Fixture {
        base: tempfile::TempDir,
        project: tempfile::TempDir,
        state: AppState,
        id: String,
        jobs: Arc<FakeJobs>,
    }

    async fn fixture() -> Fixture {
        let (base, project) = (tempfile::tempdir().unwrap(), project());
        change(project.path(), "pppppppp", "open");
        let state = AppState::new(AppDirs::from_bases(base.path(), base.path(), None));
        let jobs = Arc::new(FakeJobs::default());
        state.proposal_jobs.set(jobs.clone());
        let id = state
            .pictures
            .open(project.path().to_path_buf())
            .await
            .unwrap();
        Fixture {
            base,
            project,
            state,
            id,
            jobs,
        }
    }

    fn status(f: &Fixture, id: &str) -> String {
        let text = fs::read_to_string(
            f.project
                .path()
                .join(format!(".weaver/proposals/{id}-t/proposal.md")),
        )
        .unwrap();
        text.lines()
            .find_map(|l| l.strip_prefix("status: "))
            .unwrap()
            .to_owned()
    }

    #[tokio::test]
    async fn approve_writes_the_whole_change_and_starts_nothing() {
        let f = fixture().await;
        let revision = approve(&f.state, &f.id, "pppppppp").await.unwrap();
        assert!(revision.is_some());
        assert_eq!(status(&f, "pppppppp"), "approved");
        assert!(
            fs::read_to_string(f.project.path().join(".weaver/model/shop/order.md"))
                .unwrap()
                .contains("Paid.")
        );
        assert!(f
            .project
            .path()
            .join(".weaver/tasks/tttttttt-pay.md")
            .is_file());
        assert!(f.jobs.calls().is_empty());
    }

    fn proposal_file(f: &Fixture) -> std::path::PathBuf {
        f.project
            .path()
            .join(".weaver/proposals/pppppppp-t/proposal.md")
    }

    /// Turns the Open change `pppppppp` of the fixture into a Proposal of `kind`.
    fn into_kind(f: &Fixture, kind: &str) {
        let mut text = fs::read_to_string(proposal_file(f))
            .unwrap()
            .replace("kind: change", &format!("kind: {kind}"));
        if kind == "import" || kind == "resolution" {
            let task = ".weaver/proposals/pppppppp-t/tasks/tttttttt-pay.md";
            fs::remove_file(f.project.path().join(task)).unwrap();
        }
        if kind == "resolution" {
            text = text.replacen(
                "base:",
                "drifts:\n  - drift: dddddddd\n    decision: dismissed\nbase:",
                1,
            );
        }
        fs::write(proposal_file(f), text).unwrap();
    }

    fn drift(f: &Fixture, status: &str) {
        let text = format!("---\nstatus: {status}\nrefs:\n  - order/Paid\ncreated: 2026-11-02T06:00:00.000Z\n---\n# The code ships twice\n\n## What happens\n\nIt ships twice.\n\n## What should happen\n\nIt ships once.\n\n## Where in the code\n\n`src/ship.rs`\n");
        put(f.project.path(), "drifts/dddddddd-d.md", &text);
    }

    fn make_stale(f: &Fixture) {
        put(
            f.project.path(),
            "model/shop/order.md",
            &ORDER.replace("Choosing.", "Browsing."),
        );
    }

    #[tokio::test]
    async fn approving_a_stale_proposal_of_any_kind_writes_refining_and_starts_the_refinement() {
        for kind in ["change", "setup", "import", "resolution"] {
            let f = fixture().await;
            into_kind(&f, kind);
            drift(&f, "open");
            make_stale(&f);
            // Whatever else blocks it (here a missing required Proposal), it is refined.
            let text = fs::read_to_string(proposal_file(&f)).unwrap();
            let text = text.replacen("---\n", "---\nrequires:\n  - zzzzzzzz\n", 1);
            fs::write(proposal_file(&f), text).unwrap();
            approve(&f.state, &f.id, "pppppppp").await.unwrap();
            assert_eq!(status(&f, "pppppppp"), "refining", "{kind}");
            assert_eq!(f.jobs.calls(), ["refine pppppppp"], "{kind}");
            assert!(
                !f.project.path().join(".weaver/tasks").exists(),
                "nothing is approved yet"
            );
            // Being refined: Approve is refused now.
            let error = approve(&f.state, &f.id, "pppppppp").await.unwrap_err();
            assert_eq!(error.code, ErrorCode::Refused);
            assert_eq!(error.message, "Could not approve P1: Being refined.");
        }
    }

    #[tokio::test]
    async fn a_stale_resolution_with_no_open_drift_is_refused_before_anything_is_written() {
        let f = fixture().await;
        into_kind(&f, "resolution");
        drift(&f, "dismissed");
        make_stale(&f);
        let before = fs::read_to_string(proposal_file(&f)).unwrap();
        let error = approve(&f.state, &f.id, "pppppppp").await.unwrap_err();
        assert_eq!(
            (error.code, error.message.as_str()),
            (
                ErrorCode::Refused,
                "Could not approve P1: None of its Drifts is still open."
            )
        );
        assert_eq!(status(&f, "pppppppp"), "open");
        assert_eq!(fs::read_to_string(proposal_file(&f)).unwrap(), before);
        assert!(f.jobs.calls().is_empty());
    }

    #[tokio::test]
    async fn a_refinement_that_cannot_start_refuses_the_approval_and_writes_nothing() {
        let f = fixture().await;
        put(
            f.project.path(),
            "model/shop/order.md",
            &ORDER.replace("Choosing.", "Browsing."),
        );
        *f.jobs.cannot.lock().unwrap() = Some("omp is not ready".to_owned());
        let error = approve(&f.state, &f.id, "pppppppp").await.unwrap_err();
        assert_eq!(
            (error.code, error.message.as_str()),
            (
                ErrorCode::Refused,
                "Could not approve P1: omp is not ready."
            )
        );
        assert_eq!(status(&f, "pppppppp"), "open");
        assert!(f.jobs.calls().is_empty());
    }

    #[tokio::test]
    async fn the_agent_still_finishing_refuses_every_decision() {
        let f = fixture().await;
        let text = fs::read_to_string(
            f.project
                .path()
                .join(".weaver/proposals/pppppppp-t/proposal.md"),
        )
        .unwrap();
        put(
            f.project.path(),
            "proposals/pppppppp-t/proposal.md",
            &text.replacen("---\n", "---\njob: jjjjjjjj\n", 1),
        );
        struct Working;
        impl crate::agent_work::AgentWork for Working {
            fn ghosts(&self, _: &str) -> Vec<crate::agent_work::Ghost> {
                Vec::new()
            }
            fn facts(&self, _: &str) -> weaver_core::project::board::AgentFacts {
                let mut facts = weaver_core::project::board::AgentFacts::default();
                facts.working_jobs.insert("jjjjjjjj".to_owned());
                facts
            }
        }
        f.state.agent_work.set(Arc::new(Working));
        for error in [
            approve(&f.state, &f.id, "pppppppp").await.unwrap_err(),
            reject(&f.state, &f.id, "pppppppp", None, false)
                .await
                .unwrap_err(),
            reject(&f.state, &f.id, "pppppppp", Some("x".into()), true)
                .await
                .unwrap_err(),
        ] {
            assert_eq!(error.code, ErrorCode::Refused);
            assert!(
                error.message.ends_with("The agent is still finishing."),
                "{}",
                error.message
            );
        }
        assert_eq!(status(&f, "pppppppp"), "open");
    }

    #[tokio::test]
    async fn reject_keeps_the_reason_and_a_rework_starts_its_job() {
        let f = fixture().await;
        reject(&f.state, &f.id, "pppppppp", Some("Too big".into()), false)
            .await
            .unwrap();
        assert_eq!(status(&f, "pppppppp"), "rejected");
        assert!(fs::read_to_string(
            f.project
                .path()
                .join(".weaver/proposals/pppppppp-t/proposal.md")
        )
        .unwrap()
        .ends_with("## Feedback\n\nToo big\n"));
        assert!(f.jobs.calls().is_empty(), "a rejection starts no job");

        change(f.project.path(), "qqqqqqqq", "open");
        let revision = reject(&f.state, &f.id, "qqqqqqqq", Some("Split it".into()), true)
            .await
            .unwrap();
        assert!(revision.is_some());
        assert_eq!(status(&f, "qqqqqqqq"), "rejected");
        assert_eq!(f.jobs.calls(), ["rework qqqqqqqq"]);
        let text = fs::read_to_string(
            f.project
                .path()
                .join(".weaver/proposals/qqqqqqqq-t/proposal.md"),
        )
        .unwrap();
        assert!(text.contains("reworkRequested: true"), "{text}");

        change(f.project.path(), "rrrrrrrr", "open");
        *f.jobs.cannot.lock().unwrap() = Some("omp is not ready".to_owned());
        let error = reject(&f.state, &f.id, "rrrrrrrr", Some("Again".into()), true)
            .await
            .unwrap_err();
        assert_eq!(error.message, "Could not rework P1: omp is not ready.");
        assert_eq!(status(&f, "rrrrrrrr"), "open");
        // A plain rejection never needs a job.
        reject(&f.state, &f.id, "rrrrrrrr", None, false)
            .await
            .unwrap();
        assert_eq!(status(&f, "rrrrrrrr"), "rejected");
    }

    #[tokio::test]
    async fn an_approved_setup_starts_run_all() {
        let (base, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        put(project.path(), "model/system.md", "# Shop\n\nA shop.\n");
        put(
            project.path(),
            "proposals/pppppppp-t/proposal.md",
            "---\nstatus: open\nkind: setup\nbase:\n  model/shop/feature.md: new\n  model/shop/order.md: new\nlabel: Shop\ncreated: 2026-11-02T07:00:00.000Z\n---\n# Shop\n",
        );
        put(
            project.path(),
            "proposals/pppppppp-t/model/shop/feature.md",
            FEATURE,
        );
        put(
            project.path(),
            "proposals/pppppppp-t/model/shop/order.md",
            ORDER,
        );
        put(
            project.path(),
            "proposals/pppppppp-t/tasks/tttttttt-pay.md",
            &task("shop"),
        );
        let state = AppState::new(AppDirs::from_bases(base.path(), base.path(), None));
        let jobs = Arc::new(FakeJobs::default());
        state.proposal_jobs.set(jobs.clone());
        let id = state
            .pictures
            .open(project.path().to_path_buf())
            .await
            .unwrap();
        approve(&state, &id, "pppppppp").await.unwrap();
        assert_eq!(jobs.calls(), ["run all"]);
        assert!(project
            .path()
            .join(".weaver/tasks/tttttttt-pay.md")
            .is_file());
    }

    #[tokio::test]
    async fn discard_is_for_a_draft_and_forgets_the_model() {
        let f = fixture().await;
        change(f.project.path(), "qqqqqqqq", "draft");
        discard(&f.state, &f.id, "qqqqqqqq").await.unwrap();
        assert_eq!(status(&f, "qqqqqqqq"), "discarded");
        let error = discard(&f.state, &f.id, "pppppppp").await.unwrap_err();
        assert_eq!(
            error.message,
            "Could not discard P1: an Open Proposal is rejected, not discarded."
        );
    }

    #[tokio::test]
    async fn a_draft_of_an_earlier_version_that_chat_wrote_is_discarded_as_any_draft() {
        let f = fixture().await;
        change(f.project.path(), "qqqqqqqq", "draft");
        let text = fs::read_to_string(
            f.project
                .path()
                .join(".weaver/proposals/qqqqqqqq-t/proposal.md"),
        )
        .unwrap();
        put(
            f.project.path(),
            "proposals/qqqqqqqq-t/proposal.md",
            &text.replacen("---\n", "---\nchat: true\nchatWriting: true\n", 1),
        );
        discard(&f.state, &f.id, "qqqqqqqq").await.unwrap();
        assert_eq!(status(&f, "qqqqqqqq"), "discarded");
        let after = fs::read_to_string(
            f.project
                .path()
                .join(".weaver/proposals/qqqqqqqq-t/proposal.md"),
        )
        .unwrap();
        assert!(!after.contains("chat"), "{after}");
    }

    #[tokio::test]
    async fn the_chosen_model_stays_in_the_local_data_and_changes_no_file_of_the_project() {
        let f = fixture().await;
        let before = fs::read(
            f.project
                .path()
                .join(".weaver/proposals/pppppppp-t/proposal.md"),
        )
        .unwrap();
        let revision = choose_model(
            &f.state,
            &f.id,
            "pppppppp",
            Some("anthropic/claude-opus".into()),
        )
        .await
        .unwrap();
        assert_eq!(revision.slices, ["board"]);
        assert_eq!(
            ProposalModels::load(&f.state.dirs, &f.id).get("pppppppp"),
            Some("anthropic/claude-opus")
        );
        assert_eq!(
            fs::read(
                f.project
                    .path()
                    .join(".weaver/proposals/pppppppp-t/proposal.md")
            )
            .unwrap(),
            before
        );
        assert_eq!(
            status(&f, "pppppppp"),
            "open",
            "choosing a model never sends it back to Draft"
        );
        let everything = weaver_core::project::proposal::files::read_folder(
            f.project.path(),
            ProjectImage::load(f.project.path())
                .card(CardKind::Proposal, "pppppppp")
                .unwrap(),
        );
        assert!(everything
            .files
            .values()
            .all(|t| !t.contains("claude-opus")));
        assert!(
            weaver_core::project::proposal::models::models_file(&f.state.dirs, &f.id).is_file()
        );
        assert!(
            f.state.dirs.local_data_dir().starts_with(f.base.path()),
            "the local data of the app, not the project"
        );

        // Back to the Settings models.
        choose_model(&f.state, &f.id, "pppppppp", None)
            .await
            .unwrap();
        assert_eq!(
            ProposalModels::load(&f.state.dirs, &f.id).get("pppppppp"),
            None
        );

        let invalid = choose_model(&f.state, &f.id, "pppppppp", Some("two\nlines".into()))
            .await
            .unwrap_err();
        assert_eq!(invalid.code, ErrorCode::InvalidInput);
        change(f.project.path(), "qqqqqqqq", "draft");
        let refused = choose_model(&f.state, &f.id, "qqqqqqqq", Some("m".into()))
            .await
            .unwrap_err();
        assert_eq!(refused.code, ErrorCode::Refused);
        assert!(
            refused
                .message
                .contains("the model is chosen for an Open Proposal"),
            "{}",
            refused.message
        );
    }

    fn cli_call<'a>(
        command: &'a weaver_cli::spec::Command,
        root: &'a Path,
        image: &'a ProjectImage,
        arg: &'a str,
        state: &'a str,
        input: &'a Map<String, Value>,
        caller: &'a Caller,
    ) -> Call<'a> {
        Call {
            command,
            root,
            image,
            arg: Some(arg),
            state: Some(state),
            input,
            caller,
        }
    }

    fn move_command() -> &'static weaver_cli::spec::Command {
        weaver_cli::registry::find("proposal", "move").expect("the command")
    }

    #[test]
    fn the_command_line_decisions_that_start_jobs_are_answered_by_the_app() {
        let (base, project) = (tempfile::tempdir().unwrap(), project());
        let _ = base;
        change(project.path(), "pppppppp", "open");
        put(
            project.path(),
            "model/shop/order.md",
            &ORDER.replace("Choosing.", "Browsing."),
        );
        let root = weaver_core::paths::canonical_root(project.path()).unwrap();
        let jobs = FakeJobs::default();
        let command = move_command();
        let image = ProjectImage::load(&root);
        let caller = Caller::default();
        let input = Map::new();
        let reply = answer_cli(
            &cli_call(
                command, &root, &image, "pppppppp", "approved", &input, &caller,
            ),
            &jobs,
        )
        .unwrap()
        .unwrap();
        assert_eq!(reply.json["status"], "refining");
        assert!(
            reply.markdown.contains("is being refined"),
            "{}",
            reply.markdown
        );
        assert_eq!(jobs.calls(), ["refine pppppppp"]);

        let jobs = FakeJobs::default();
        change(&root, "qqqqqqqq", "open");
        let image = ProjectImage::load(&root);
        let input: Map<String, Value> = json!({ "rework": true, "feedback": "Split it" })
            .as_object()
            .unwrap()
            .clone();
        let reply = answer_cli(
            &cli_call(
                command, &root, &image, "qqqqqqqq", "rejected", &input, &caller,
            ),
            &jobs,
        )
        .unwrap()
        .unwrap();
        assert_eq!(reply.json["status"], "rejected");
        assert_eq!(jobs.calls(), ["rework qqqqqqqq"]);

        // A job that cannot start refuses the command and writes nothing.
        let jobs = FakeJobs::default();
        *jobs.cannot.lock().unwrap() = Some("omp is not ready".into());
        change(&root, "rrrrrrrr", "open");
        let image = ProjectImage::load(&root);
        let failure = answer_cli(
            &cli_call(
                command, &root, &image, "rrrrrrrr", "rejected", &input, &caller,
            ),
            &jobs,
        )
        .unwrap()
        .unwrap_err();
        assert_eq!(failure.message, "Could not rework P2: omp is not ready.");
        assert!(jobs.calls().is_empty());

        // Other commands are left to their handlers.
        let none = Map::new();
        assert!(answer_cli(
            &cli_call(
                command,
                &root,
                &image,
                "rrrrrrrr",
                "discarded",
                &none,
                &caller
            ),
            &jobs
        )
        .is_none());
        let show = weaver_cli::registry::find("proposal", "show").unwrap();
        assert!(answer_cli(
            &cli_call(show, &root, &image, "rrrrrrrr", "x", &none, &caller),
            &jobs
        )
        .is_none());
        assert_eq!(project_hash_of_canonical(&root).len(), 16);
    }

    #[test]
    fn handing_over_a_proposal_bound_to_a_sent_request_consumes_the_request_for_good() {
        let project = project();
        change(project.path(), "pppppppp", "draft");
        let proposal = project
            .path()
            .join(".weaver/proposals/pppppppp-t/proposal.md");
        let text = fs::read_to_string(&proposal).unwrap();
        let request = "2026-11-02T09-30-00";
        fs::write(
            &proposal,
            text.replace(
                "label: Shop\n",
                &format!("label: Shop\nrequest: {request}\n"),
            ),
        )
        .unwrap();
        let store = weaver_core::project::requests::store::RequestStore::new(project.path());
        let time =
            weaver_core::project::requests::id::LocalTime::parse("2026-11-02T09:30:00").unwrap();
        assert_eq!(store.create(&time).unwrap(), request);
        store.save(request, "Add a cart").unwrap();
        store.record_sent(request, "jjjjjjjj").unwrap();

        let root = weaver_core::paths::canonical_root(project.path()).unwrap();
        let image = ProjectImage::load(&root);
        let caller = Caller::default();
        let input = Map::new();
        let jobs = FakeJobs::default();
        let reply = answer_cli(
            &cli_call(
                move_command(),
                &root,
                &image,
                "pppppppp",
                "open",
                &input,
                &caller,
            ),
            &jobs,
        )
        .unwrap()
        .unwrap();
        assert_eq!(reply.json["status"], "open");

        let store = weaver_core::project::requests::store::RequestStore::new(&root);
        assert!(
            !root.join(format!(".weaver/requests/{request}.md")).exists(),
            "the hand-over deletes the file"
        );
        assert_eq!(
            store.state_of(request).unwrap(),
            weaver_core::project::requests::machine::RequestState::Consumed
        );
        // It never comes back, even if an editor writes the file again.
        fs::write(
            root.join(format!(".weaver/requests/{request}.md")),
            "Again\n",
        )
        .unwrap();
        assert!(store.list().unwrap().is_empty());
    }

    #[test]
    fn an_open_proposal_sent_back_to_draft_is_a_notice() {
        let request = |verb: &str| CommandRequest {
            object: "proposal".into(),
            verb: verb.into(),
            root: "/p".into(),
            card: Some("p1".into()),
            state: None,
            input: Map::new(),
            general: None,
            ask: None,
        };
        let reply = |sent: bool| CommandReply {
            markdown: String::new(),
            json: json!({ "id": "pppppppp", "number": 3, "sentBackToDraft": sent }),
            exit: 0,
            note: None,
        };
        let notice = notice_of_command(&request("edit"), &reply(true)).unwrap();
        assert_eq!(notice.id, "sent-back-pppppppp");
        assert!(
            notice.text.starts_with("P3 went back to Draft"),
            "{}",
            notice.text
        );
        assert!(notice_of_command(&request("edit"), &reply(false)).is_none());
        assert!(notice_of_command(&request("new"), &reply(true)).is_none());
    }
}
