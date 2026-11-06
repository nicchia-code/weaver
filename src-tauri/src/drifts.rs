//! The app side of the Drift: the decisions of the developer on its card (`Resolve`, `Fix the
//! code`, `Accept Proposal`, `Refine`, `Dismiss Drift`) and the prepared question.
//!
//! The rules live in `weaver_core::project::drift`. A decision runs through
//! [`crate::decisions::decide`]: in the project's queue, under the project lock, with the facts of
//! that moment, checked again when applied, and written whole or not at all.
//!
//! `Resolve` (`developer.RESOLVE_DRIFT`):
//!
//! * with a prepared question that is still valid, Weaver asks it itself as a question round
//!   ([`Rounds::present_prepared`], the Drift named by its short number), with no agent and no
//!   omp. The answer is read by [`Drifts::answered`]: Weaver writes the resolution
//!   `Resolve: <Drift title>` and hands it over with the agent's checks; a skipped question leaves
//!   the Drift as it is; a free answer next to a choice other than a code fix, or a hand-over that
//!   is refused (its Draft is discarded), starts a resolve job with the answer already given;
//! * otherwise a resolve job asks the question ([`crate::jobs::Jobs::start_resolve`]). A resolve
//!   job without an answer is refused while a valid prepared question exists ([`PreparedGate`]):
//!   Weaver asks it itself.
//!
//! Everything here is refused, writing nothing, while a Run works, while another resolution is
//! being written or waits for a decision, while a resolve works on the Drift, or while the job that
//! filed it still works (`drift::reasons::resolve_reasons`).

use std::collections::HashSet;
use std::convert::Infallible;
use std::path::Path;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Manager, State};
use weaver_core::job_instruction::{Job, ResolveAnswer};
use weaver_core::jobs::admission::StartPlan;
use weaver_core::project::board::AgentFacts;
use weaver_core::project::cards::CardKind;
use weaver_core::project::changes::{self, MutationError};
use weaver_core::project::clock::{now_instant, SystemClock};
use weaver_core::project::drift::answer::{self, Next};
use weaver_core::project::drift::decide::{self, Decision};
use weaver_core::project::drift::prepared::{self, Preparation, Prepared};
use weaver_core::project::drift::{self, reasons};
use weaver_core::project::image::ProjectImage;
use weaver_core::project::proposal::create::read_project_file;
use weaver_core::project::proposal::files;
use weaver_core::project::proposal::transitions::{self, Decider, SubmitContext};
use weaver_core::project::random::OsRandom;
use weaver_core::project::shortnum::{self, ShortNumbers};
use weaver_core::question::{Answer, Outcome, RoundRequest};

use crate::decisions;
use crate::error::{AppError, ErrorCode};
use crate::events::{self, Notice};
use crate::jobs::host::MutationResult;
use crate::jobs::starts::{Ctx, StartGate};
use crate::jobs::Jobs;
use crate::pictures::Revision;
use crate::rounds::Rounds;
use crate::state::AppState;
use crate::windows::jobs::BoxFuture;

// ---------------------------------------------------------------------------------------------
// Decisions
// ---------------------------------------------------------------------------------------------

fn refused(notice: impl Into<String>) -> weaver_core::project::board::Refused {
    weaver_core::project::board::Refused {
        notice: notice.into(),
    }
}

fn now() -> Result<String, MutationError<weaver_core::project::board::Refused>> {
    now_instant(&SystemClock)
        .map_err(|e| MutationError::Refused(refused(format!("the clock gives no valid time: {e}"))))
}

/// `Fix the code`, `Accept Proposal` and `Dismiss Drift`, applied under the project lock.
/// Returns the new revision.
pub async fn decide_drift(
    state: &AppState,
    project_id: &str,
    drift_id: &str,
    decision: Decision,
) -> Result<Option<Revision>, AppError> {
    let drift = drift_id.to_owned();
    decisions::decide(state, project_id, move |root, facts| {
        let decided_at = now()?;
        let (image, _) =
            decide::decide(root, facts, &drift, &decision, &decided_at, &mut OsRandom)?;
        Ok(image)
    })
    .await
}

/// `Refine` on the Proposal attached to a Drift (`developer.REFINE_DRIFT_PROPOSAL`): writes
/// Refining on it under the project lock, the Drift unchanged, then starts the refinement job the
/// way `Approve` does on a stale Proposal. Refused before anything is written when no job can
/// start. Returns the new revision.
pub async fn refine_attached(
    state: &AppState,
    project_id: &str,
    drift_id: &str,
) -> Result<Option<Revision>, AppError> {
    let jobs = state.proposal_jobs.get();
    let started: Arc<Mutex<Option<String>>> = Arc::default();
    let (slot, check, project, drift) = (
        started.clone(),
        jobs.clone(),
        project_id.to_owned(),
        drift_id.to_owned(),
    );
    let revision = decisions::decide(state, project_id, move |root, facts| {
        let decided_at = now()?;
        let (image, decided) = decide::decide_with(
            root,
            facts,
            &drift,
            &Decision::RefineProposal,
            &decided_at,
            &mut OsRandom,
            &|| check.can_start(&project),
        )?;
        *slot.lock().expect("refinement slot poisoned") = decided.refine;
        Ok(image)
    })
    .await?;
    if let Some(proposal) = started.lock().expect("refinement slot poisoned").take() {
        jobs.start_refinement(project_id, &proposal);
    }
    Ok(revision)
}

// ---------------------------------------------------------------------------------------------
// What the service needs from the rest of the app
// ---------------------------------------------------------------------------------------------

/// What `Resolve` needs from the jobs and the windows, behind one trait so that the tests bring
/// their own.
pub trait DriftHost: Send + Sync + 'static {
    /// What the app knows about the agent's work in the project now.
    fn facts(&self, project_id: &str) -> AgentFacts;
    /// A resolve job on the Drift, with the answer already given when there is one. `Err` is the
    /// reason it did not start.
    fn start_resolve(
        &self,
        ctx: &Ctx,
        drift: &str,
        answer: Option<ResolveAnswer>,
    ) -> Result<String, String>;
    /// Runs `change` in the project's queue, under the project lock, and publishes the image it
    /// returns.
    fn mutate(
        &self,
        project_id: &str,
        change: Box<dyn FnOnce(&Path) -> MutationResult + Send>,
    ) -> BoxFuture<Result<(), String>>;
    /// Something the developer must read at the top of the Board.
    fn notify(&self, project_id: &str, text: String);
}

/// The host of the running app.
struct TauriDriftHost {
    app: AppHandle,
    jobs: Arc<Jobs>,
    notices: Mutex<u64>,
}

impl DriftHost for TauriDriftHost {
    fn facts(&self, project_id: &str) -> AgentFacts {
        self.jobs.facts_of(project_id)
    }

    fn start_resolve(
        &self,
        ctx: &Ctx,
        drift: &str,
        answer: Option<ResolveAnswer>,
    ) -> Result<String, String> {
        self.jobs
            .start_resolve(ctx, drift, answer)
            .map_err(|refusal| refusal.0)
    }

    fn mutate(
        &self,
        project_id: &str,
        change: Box<dyn FnOnce(&Path) -> MutationResult + Send>,
    ) -> BoxFuture<Result<(), String>> {
        self.jobs.host().mutate(project_id, change)
    }

    fn notify(&self, project_id: &str, text: String) {
        let mut count = self
            .notices
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *count += 1;
        let notice = Notice {
            project_id: project_id.to_owned(),
            id: format!("drift-{count}"),
            text,
        };
        let _ = events::emit_notice(&self.app, &notice);
    }
}

// ---------------------------------------------------------------------------------------------
// Resolve
// ---------------------------------------------------------------------------------------------

/// The prepared question of a Drift, being asked.
pub struct Asked {
    /// What the round came to.
    pub outcome: Receiver<Outcome>,
    pub prepared: Prepared,
}

/// What pressing `Resolve` did.
pub enum Pressed {
    /// A resolve job was started: it asks the developer.
    Job,
    /// Weaver asks the prepared question itself.
    Asked(Asked),
}

/// The Drifts whose prepared question is being asked, by project and Drift.
#[derive(Default)]
struct Asking(Mutex<HashSet<(String, String)>>);

impl Asking {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashSet<(String, String)>> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// The service of the Drifts: `Resolve` and what the answer to a prepared question becomes.
pub struct Drifts {
    host: Arc<dyn DriftHost>,
    rounds: Arc<Rounds>,
    asking: Asking,
}

fn label_of(numbers: &ShortNumbers, id: &str) -> String {
    numbers
        .label_of(CardKind::Drift, id)
        .unwrap_or_else(|| id.to_owned())
}

/// The answer to a question as the instruction of a resolve job carries it.
fn given(answer: &Answer) -> Option<ResolveAnswer> {
    match answer {
        Answer::Answered { selected, text } => Some(ResolveAnswer {
            selected: selected.clone(),
            text: text.clone(),
        }),
        Answer::Skipped => None,
    }
}

impl Drifts {
    pub fn new(host: Arc<dyn DriftHost>, rounds: Arc<Rounds>) -> Arc<Self> {
        Arc::new(Self {
            host,
            rounds,
            asking: Asking::default(),
        })
    }

    /// `developer.RESOLVE_DRIFT`, checked again now: refused with every reason, writing nothing
    /// and starting nothing, or a resolve job (no valid prepared question) or the round of the
    /// prepared question.
    pub fn press(
        &self,
        root: &Path,
        project_id: &str,
        image: &ProjectImage,
        facts: &AgentFacts,
        drift_id: &str,
    ) -> Result<Pressed, String> {
        let numbers = shortnum::peek(root, image.cards());
        let label = label_of(&numbers, drift_id);
        let Some(drift) = image.card(CardKind::Drift, drift_id) else {
            return Err(format!("Could not resolve {label}: it no longer exists."));
        };
        if !drift::is_open(drift) {
            return Err(format!("Could not resolve {label}: it is no longer open."));
        }
        let name =
            |kind: CardKind, id: &str| numbers.label_of(kind, id).unwrap_or_else(|| id.to_owned());
        let mut reasons = reasons::resolve_reasons(image, facts, &name, drift);
        if self
            .asking
            .lock()
            .contains(&(project_id.to_owned(), drift_id.to_owned()))
            && !reasons.iter().any(|r| r == reasons::REASON_BEING_RESOLVED)
        {
            let at = reasons
                .iter()
                .position(|r| {
                    r == reasons::REASON_ANOTHER_BEING_WRITTEN || r.starts_with("Proposal ")
                })
                .unwrap_or(reasons.len());
            reasons.insert(at, reasons::REASON_BEING_RESOLVED.to_owned());
        }
        if !reasons.is_empty() {
            return Err(format!(
                "Could not resolve {label}: {}.",
                reasons.join("; ")
            ));
        }
        match prepared::check(image, drift) {
            Preparation::Valid(prepared) => {
                self.asking
                    .lock()
                    .insert((project_id.to_owned(), drift_id.to_owned()));
                let request = RoundRequest {
                    questions: vec![prepared.question.clone()],
                };
                let outcome = self.rounds.present_prepared(
                    project_id,
                    drift_id,
                    &label,
                    &drift.title,
                    request,
                );
                Ok(Pressed::Asked(Asked {
                    outcome,
                    prepared: *prepared,
                }))
            }
            _ => self
                .host
                .start_resolve(
                    &Ctx {
                        project_id,
                        root,
                        image,
                    },
                    drift_id,
                    None,
                )
                .map(|_| Pressed::Job)
                .map_err(|why| format!("Could not resolve {label}: {why}.")),
        }
    }

    /// `developer.RESOLVE_DRIFT`, in the project's queue. When Weaver asks the prepared question
    /// itself, the answer is read in the background.
    pub async fn resolve(self: &Arc<Self>, project_id: &str, drift_id: &str) -> Result<(), String> {
        let slot: Arc<Mutex<Option<Asked>>> = Arc::default();
        let (this, project, drift, out) = (
            self.clone(),
            project_id.to_owned(),
            drift_id.to_owned(),
            slot.clone(),
        );
        self.host
            .mutate(
                project_id,
                Box::new(move |root| {
                    let image = ProjectImage::load(root);
                    let facts = this.host.facts(&project);
                    if let Pressed::Asked(asked) =
                        this.press(root, &project, &image, &facts, &drift)?
                    {
                        *out.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(asked);
                    }
                    Ok(None)
                }),
            )
            .await?;
        let asked = slot
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();
        if let Some(asked) = asked {
            self.wait_for(project_id, drift_id, asked);
        }
        Ok(())
    }

    /// Waits, with no time limit, for what the round came to, then acts on it.
    fn wait_for(self: &Arc<Self>, project_id: &str, drift_id: &str, asked: Asked) {
        let (this, project, drift) = (self.clone(), project_id.to_owned(), drift_id.to_owned());
        tauri::async_runtime::spawn(async move {
            let Asked { outcome, prepared } = asked;
            let result = tauri::async_runtime::spawn_blocking(move || outcome.recv()).await;
            this.asking.lock().remove(&(project.clone(), drift.clone()));
            if let Ok(Ok(outcome)) = result {
                this.answered(&project, &drift, &prepared, outcome).await;
            }
        });
    }

    /// `question-round.ANSWERED` for the prepared question of a Drift. A skipped question, a
    /// window that closed and an answer that asks for nothing leave the Drift as it is.
    pub async fn answered(
        self: &Arc<Self>,
        project_id: &str,
        drift_id: &str,
        prepared: &Prepared,
        outcome: Outcome,
    ) {
        let Outcome::Answered(answers) = outcome else {
            return;
        };
        let Some(answer) = answers.first() else {
            return;
        };
        let (next, resolve_answer) = (answer::next_of(drift_id, prepared, answer), given(answer));
        let (this, project, drift) = (self.clone(), project_id.to_owned(), drift_id.to_owned());
        let result = match next {
            Next::Nothing => return,
            Next::Job(answer) => {
                self.host
                    .mutate(
                        project_id,
                        Box::new(move |root| {
                            let image = ProjectImage::load(root);
                            this.start_job(root, &project, &image, &drift, answer)?;
                            Ok(None)
                        }),
                    )
                    .await
            }
            Next::Resolution(row) => {
                self.host
                    .mutate(
                        project_id,
                        Box::new(move |root| {
                            let Some(answer) = resolve_answer else {
                                return Ok(None);
                            };
                            this.write_resolution(root, &project, &drift, &row, answer)
                        }),
                    )
                    .await
            }
        };
        if let Err(text) = result {
            self.host.notify(project_id, text);
        }
    }

    /// A resolve job with the answer already given.
    fn start_job(
        &self,
        root: &Path,
        project_id: &str,
        image: &ProjectImage,
        drift_id: &str,
        answer: ResolveAnswer,
    ) -> Result<(), String> {
        let numbers = shortnum::peek(root, image.cards());
        let label = label_of(&numbers, drift_id);
        let open = image
            .card(CardKind::Drift, drift_id)
            .is_some_and(drift::is_open);
        if !open {
            return Err(format!(
                "Could not resolve {label}: it is no longer open. The answer was not used."
            ));
        }
        self.host
            .start_resolve(
                &Ctx {
                    project_id,
                    root,
                    image,
                },
                drift_id,
                Some(answer),
            )
            .map(|_| ())
            .map_err(|why| format!("Could not resolve {label}: {why}. The answer was not used."))
    }

    /// Weaver writes the resolution of the answer and hands it over with the agent's checks. When
    /// it cannot be written, or the hand-over is refused (the Draft is discarded), a resolve job
    /// starts with the answer already given. Returns the image to publish.
    fn write_resolution(
        &self,
        root: &Path,
        project_id: &str,
        drift_id: &str,
        row: &weaver_core::project::board::ResolutionRow,
        answer: ResolveAnswer,
    ) -> MutationResult {
        let image = ProjectImage::load(root);
        let facts = self.host.facts(project_id);
        let numbers = shortnum::peek(root, image.cards());
        let label = label_of(&numbers, drift_id);
        let Some(drift) = image
            .card(CardKind::Drift, drift_id)
            .filter(|d| drift::is_open(d))
        else {
            return Err(format!(
                "Could not resolve {label}: it is no longer open. The answer was not used."
            ));
        };
        let created =
            now_instant(&SystemClock).map_err(|e| format!("the clock gives no valid time: {e}"))?;
        let folders = files::read_folders(root, &image);
        let planned = answer::plan_resolution(
            &image,
            &folders,
            &numbers,
            drift,
            row,
            &created,
            &|path| read_project_file(root, path),
            &mut OsRandom,
        );
        let written = match planned {
            Ok(written) => written,
            Err(_) => {
                return self
                    .job_instead(root, project_id, drift_id, &image, answer)
                    .map(|()| None)
            }
        };
        changes::apply(root, &written.changes, |_| Ok::<(), Infallible>(()))
            .map_err(|error| format!("Could not resolve {label}: the resolution could not be written ({error}). Nothing changed."))?;
        let ctx = SubmitContext {
            now: &created,
            job_found_empty_model: false,
            run_in_progress: facts.run_in_progress,
        };
        match transitions::submit(root, &written.id, &ctx, &mut OsRandom) {
            Ok((image, _)) => Ok(Some(image)),
            Err(MutationError::Refused(_)) => {
                // The hand-over is refused: its Draft is discarded and a job asks again, with the answer.
                let image = transitions::discard(root, &facts, &written.id, Decider::Developer)
                    .map_err(|error| format!("Could not resolve {label}: the resolution was refused and its Draft could not be discarded ({error:?})."))?;
                match self.job_instead(root, project_id, drift_id, &image, answer) {
                    Ok(()) => Ok(Some(image)),
                    Err(text) => {
                        self.host.notify(project_id, text);
                        Ok(Some(image))
                    }
                }
            }
            Err(other) => Err(format!("Could not resolve {label}: the resolution could not be handed over ({other}). Nothing was decided.")),
        }
    }

    fn job_instead(
        &self,
        root: &Path,
        project_id: &str,
        drift_id: &str,
        image: &ProjectImage,
        answer: ResolveAnswer,
    ) -> Result<(), String> {
        self.start_job(root, project_id, image, drift_id, answer)
    }
}

/// Refuses a resolve job without an answer while the Drift has a valid prepared question: Weaver
/// asks it itself, with no agent.
pub struct PreparedGate;

impl StartGate for PreparedGate {
    fn refuse(&self, image: &ProjectImage, plan: &StartPlan) -> Option<String> {
        let Job::Resolve {
            drift,
            answer: None,
        } = &plan.job
        else {
            return None;
        };
        let card = image.card(CardKind::Drift, drift)?;
        prepared::check(image, card)
            .is_valid()
            .then(|| "Resolve asks its prepared question itself, with no agent".to_owned())
    }
}

/// Plugs the service into the app: manages it, and installs the gate in the jobs. Call from the
/// setup of the app, after the jobs are installed (the question rounds are installed with them).
pub fn install(app: &AppHandle, jobs: &Arc<Jobs>) {
    let Some(rounds) = app
        .try_state::<Arc<Rounds>>()
        .map(|rounds| rounds.inner().clone())
    else {
        return;
    };
    let host = Arc::new(TauriDriftHost {
        app: app.clone(),
        jobs: jobs.clone(),
        notices: Mutex::new(0),
    });
    app.manage(Drifts::new(host, rounds));
    jobs.add_gate(Arc::new(PreparedGate));
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

/// **Resolve** on a Drift. Refused with `refused` (the message is the notice) when a condition no
/// longer holds at the moment of applying; nothing is written and nothing starts then.
#[tauri::command]
pub async fn resolve_drift(
    drifts: State<'_, Arc<Drifts>>,
    project_id: String,
    drift_id: String,
) -> Result<(), AppError> {
    let drifts = drifts.inner().clone();
    drifts
        .resolve(&project_id, &drift_id)
        .await
        .map_err(|notice| AppError::new(ErrorCode::Refused, notice))
}

/// **Fix the code**: the fix option (required when the Drift offers some) and the developer's note.
#[tauri::command]
pub async fn fix_drift_code(
    app: AppHandle,
    state: State<'_, AppState>,
    project_id: String,
    drift_id: String,
    option: Option<u32>,
    note: Option<String>,
) -> Result<(), AppError> {
    let note = note.map(|n| n.trim().to_owned()).filter(|n| !n.is_empty());
    announce(
        &app,
        decide_drift(
            &state,
            &project_id,
            &drift_id,
            Decision::FixCode { option, note },
        )
        .await?,
    )
}

/// **Accept Proposal**: approves the Proposal attached to the Drift.
#[tauri::command]
pub async fn accept_drift_proposal(
    app: AppHandle,
    state: State<'_, AppState>,
    project_id: String,
    drift_id: String,
) -> Result<(), AppError> {
    announce(
        &app,
        decide_drift(&state, &project_id, &drift_id, Decision::AcceptProposal).await?,
    )
}

/// **Refine**: adapts the stale Proposal attached to the Drift to the current Model and
/// Requirements, or continues a refinement of it left half-way. The Drift is not decided.
#[tauri::command]
pub async fn refine_drift_proposal(
    app: AppHandle,
    state: State<'_, AppState>,
    project_id: String,
    drift_id: String,
) -> Result<(), AppError> {
    announce(&app, refine_attached(&state, &project_id, &drift_id).await?)
}

/// **Discard**, once confirmed: throws away the attached Draft a refinement left half-way. The
/// Drift is not decided.
#[tauri::command]
pub async fn discard_drift_proposal(
    app: AppHandle,
    state: State<'_, AppState>,
    project_id: String,
    drift_id: String,
) -> Result<(), AppError> {
    announce(
        &app,
        decide_drift(&state, &project_id, &drift_id, Decision::DiscardProposal).await?,
    )
}

/// **Dismiss Drift**, once confirmed.
#[tauri::command]
pub async fn dismiss_drift(
    app: AppHandle,
    state: State<'_, AppState>,
    project_id: String,
    drift_id: String,
) -> Result<(), AppError> {
    announce(
        &app,
        decide_drift(&state, &project_id, &drift_id, Decision::Dismiss).await?,
    )
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::{AtomicBool, Ordering};

    use serde_json::json;
    use weaver_core::paths::AppDirs;
    use weaver_core::project::drift::create::{self, EditDrift, NewDrift, PreparedInput};
    use weaver_core::project::drift::prepared::PreparedDecision;
    use weaver_core::project::drift::FixOption;
    use weaver_core::project::proposal::data::{self as proposal_data, Kind, Status};
    use weaver_core::question::{Action, Position};

    use super::*;
    use crate::agent_work::{AgentWork, Ghost};
    use crate::rounds::service::{JobFacts, RoundsHost};
    use crate::rounds::view::{OriginView, QuestionRoundChanged};

    const NOW: &str = "2026-11-02T09:00:00.000Z";
    const ORDER: &str = "# Order\n\nAn order.\n\n## States\n\n### Draft (initial)\nChoosing.\n- `CONFIRM` → Paid\n\n### Paid\n\n## Events\n\n### CONFIRM\nConfirmed.\n";

    #[derive(Default)]
    struct FakeHost {
        root: std::path::PathBuf,
        facts: Mutex<AgentFacts>,
        started: Mutex<Vec<(String, Option<ResolveAnswer>)>>,
        cannot_start: Mutex<Option<String>>,
        notices: Mutex<Vec<String>>,
    }

    impl DriftHost for FakeHost {
        fn facts(&self, _: &str) -> AgentFacts {
            self.facts.lock().unwrap().clone()
        }

        fn start_resolve(
            &self,
            _: &Ctx,
            drift: &str,
            answer: Option<ResolveAnswer>,
        ) -> Result<String, String> {
            if let Some(why) = self.cannot_start.lock().unwrap().clone() {
                return Err(why);
            }
            self.started
                .lock()
                .unwrap()
                .push((drift.to_owned(), answer));
            Ok("job".to_owned())
        }

        fn mutate(
            &self,
            _: &str,
            change: Box<dyn FnOnce(&Path) -> MutationResult + Send>,
        ) -> BoxFuture<Result<(), String>> {
            let root = self.root.clone();
            Box::pin(async move { change(&root).map(|_| ()) })
        }

        fn notify(&self, _: &str, text: String) {
            self.notices.lock().unwrap().push(text);
        }
    }

    /// The windows and jobs the rounds talk to: nothing happens but the log.
    struct QuietRounds;

    impl RoundsHost for QuietRounds {
        fn job(&self, _: &str) -> Option<JobFacts> {
            None
        }
        fn round_opened(&self, _: &str) {}
        fn round_closed(&self, _: &str) {}
        fn focus_project(&self, _: &str) {}
        fn chat(&self, _: &str) -> Option<String> {
            None
        }
        fn chat_round_opened(&self, _: &str) {}
        fn chat_round_closed(&self, _: &str) {}
        fn changed(&self, _: &QuestionRoundChanged) {}
    }

    struct Fixture {
        project: tempfile::TempDir,
        host: Arc<FakeHost>,
        rounds: Arc<Rounds>,
        drifts: Arc<Drifts>,
        id: String,
    }

    fn put(root: &Path, rel: &str, text: &str) {
        let path = root.join(".weaver").join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn apply(root: &Path, changes: &weaver_core::project::changes::ChangeSet) {
        weaver_core::project::changes::apply(root, changes, |_| Ok::<(), Infallible>(())).unwrap();
    }

    /// A project with one Open Drift that has two fix options and a valid prepared question
    /// (`Deduplicate`, `Lock`, `Keep as it is` → dismissed).
    fn fixture_with(refs: &[&str]) -> Fixture {
        let project = tempfile::tempdir().unwrap();
        let root = project.path();
        put(root, "model/system.md", "# Shop\n\nA shop.\n");
        put(root, "model/shop/feature.md", "# Shopping\n");
        put(root, "model/shop/order.md", ORDER);
        let image = ProjectImage::load(root);
        let mut rng = OsRandom;
        let new = NewDrift {
            title: "The code ships twice".into(),
            refs: refs.iter().map(|r| (*r).to_owned()).collect(),
            happens: "It ships twice.".into(),
            should: "It ships once.".into(),
            where_in_code: "`src/ship.rs`".into(),
            options: vec![
                FixOption {
                    title: "Deduplicate".into(),
                    text: "Keep one shipment.".into(),
                },
                FixOption {
                    title: "Lock".into(),
                    text: "Lock the order.".into(),
                },
            ],
            ..NewDrift::default()
        };
        // References that are gone cannot be filed: write the file with the existing ones, then edit its text.
        let filed = create::plan_new(
            &image,
            &NewDrift {
                refs: vec!["order".into()],
                ..new.clone()
            },
            NOW,
            &mut rng,
        )
        .unwrap();
        let text = match filed.changes.get(&filed.path) {
            Some(weaver_core::project::changes::Change::Write(text)) => text.replace(
                "- order\n",
                &refs.iter().map(|r| format!("- {r}\n")).collect::<String>(),
            ),
            _ => unreachable!(),
        };
        let mut changes = weaver_core::project::changes::ChangeSet::new();
        changes.write(&filed.path, text);
        apply(root, &changes);
        let image = ProjectImage::load(root);
        let prepared = PreparedInput {
            question: json!({
                "id": filed.id,
                "question": "What now?",
                "options": [
                    {"label": "Deduplicate", "description": "d"},
                    {"label": "Lock", "description": "d"},
                    {"label": "Keep as it is", "description": "d"},
                ],
                "recommended": 0,
                "multi": false,
            }),
            decisions: vec![
                PreparedDecision {
                    decision: "fix-code".into(),
                    option: Some(1),
                },
                PreparedDecision {
                    decision: "fix-code".into(),
                    option: Some(2),
                },
                PreparedDecision {
                    decision: "dismissed".into(),
                    option: None,
                },
            ],
        };
        let edited = create::plan_edit(
            &image,
            &filed.id,
            &EditDrift {
                prepared: Some(prepared),
                proposal: None,
            },
            NOW,
        )
        .unwrap();
        apply(root, &edited.changes);
        let host = Arc::new(FakeHost {
            root: root.to_path_buf(),
            ..FakeHost::default()
        });
        let rounds = Rounds::new(Arc::new(QuietRounds));
        let drifts = Drifts::new(host.clone(), rounds.clone());
        Fixture {
            project,
            host,
            rounds,
            drifts,
            id: filed.id,
        }
    }

    fn fixture() -> Fixture {
        fixture_with(&["order/Paid"])
    }

    fn press(f: &Fixture) -> Result<Pressed, String> {
        let image = ProjectImage::load(f.project.path());
        let facts = f.host.facts("p1");
        f.drifts
            .press(f.project.path(), "p1", &image, &facts, &f.id)
    }

    /// The developer chooses `option` (and writes `text`) in the round on screen and sends it.
    fn answer(f: &Fixture, option: Option<usize>, text: &str) {
        let view = f.rounds.view("p1").expect("a round is on screen");
        f.rounds
            .act("p1", view.id, Action::Go(Position::Question { index: 0 }))
            .unwrap();
        f.rounds
            .act(
                "p1",
                view.id,
                Action::Draft {
                    index: 0,
                    selected: option.into_iter().collect(),
                    text: text.into(),
                },
            )
            .unwrap();
        f.rounds
            .act("p1", view.id, Action::Confirm { index: 0 })
            .unwrap();
        f.rounds.act("p1", view.id, Action::Send).unwrap();
    }

    fn resolutions(f: &Fixture) -> Vec<(String, Status)> {
        ProjectImage::load(f.project.path())
            .cards_of(CardKind::Proposal)
            .filter(|p| proposal_data::kind_of(p) == Kind::Resolution)
            .map(|p| (p.title.clone(), proposal_data::status_of(p).unwrap()))
            .collect()
    }

    fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
        tauri::async_runtime::block_on(future)
    }

    #[test]
    fn resolve_with_a_valid_prepared_question_asks_it_itself_and_starts_no_job() {
        let f = fixture();
        let Ok(Pressed::Asked(asked)) = press(&f) else {
            panic!("Weaver asks the prepared question")
        };
        assert!(
            f.host.started.lock().unwrap().is_empty(),
            "no omp is started"
        );
        let view = f.rounds.view("p1").expect("the round is on screen");
        assert_eq!(
            view.origin,
            OriginView::Drift {
                drift: "D1".into(),
                title: "The code ships twice".into()
            },
            "the Drift is named by its short number"
        );
        assert_eq!(view.questions.len(), 1);

        // A second press while the question is on screen is refused.
        let again = press(&f).err().expect("refused");
        assert_eq!(again, "Could not resolve D1: This Drift is being resolved.");

        answer(&f, Some(1), "Mind the lock");
        let outcome = asked.outcome.recv().unwrap();
        block_on(f.drifts.answered("p1", &f.id, &asked.prepared, outcome));
        assert_eq!(
            resolutions(&f),
            [("Resolve: The code ships twice".to_owned(), Status::Open)],
            "delivered, waiting for the developer"
        );
        let image = ProjectImage::load(f.project.path());
        let resolution = image.cards_of(CardKind::Proposal).next().unwrap();
        let rows = weaver_core::project::board::resolution_rows(resolution);
        assert_eq!(
            (
                rows[0].decision.as_str(),
                rows[0].option,
                rows[0].note.as_deref()
            ),
            ("fix-code", Some(2), Some("Mind the lock"))
        );
        assert!(
            f.host.started.lock().unwrap().is_empty() && f.host.notices.lock().unwrap().is_empty()
        );
        // Nothing was decided: the Drift is still open, now in a resolution waiting for the developer.
        assert!(weaver_core::project::drift::is_open(
            image.card(CardKind::Drift, &f.id).unwrap()
        ));
    }

    #[test]
    fn a_dismissal_or_a_code_fix_without_text_is_written_without_an_agent() {
        let f = fixture();
        let Ok(Pressed::Asked(asked)) = press(&f) else {
            panic!()
        };
        answer(&f, Some(2), "");
        block_on(
            f.drifts
                .answered("p1", &f.id, &asked.prepared, asked.outcome.recv().unwrap()),
        );
        let image = ProjectImage::load(f.project.path());
        let resolution = image.cards_of(CardKind::Proposal).next().unwrap();
        assert_eq!(proposal_data::status_of(resolution), Some(Status::Open));
        assert_eq!(
            weaver_core::project::board::resolution_rows(resolution)[0].decision,
            "dismissed"
        );
        assert!(f.host.started.lock().unwrap().is_empty());
    }

    #[test]
    fn a_free_answer_next_to_a_choice_other_than_a_code_fix_starts_a_resolve_job_with_the_answer() {
        let f = fixture();
        let Ok(Pressed::Asked(asked)) = press(&f) else {
            panic!()
        };
        answer(&f, Some(2), "Only for guests");
        block_on(
            f.drifts
                .answered("p1", &f.id, &asked.prepared, asked.outcome.recv().unwrap()),
        );
        assert!(resolutions(&f).is_empty(), "no resolution is written");
        let started = f.host.started.lock().unwrap().clone();
        assert_eq!(
            started,
            [(
                f.id.clone(),
                Some(ResolveAnswer {
                    selected: vec!["Keep as it is".into()],
                    text: Some("Only for guests".into())
                })
            )]
        );
    }

    #[test]
    fn a_skipped_question_leaves_the_drift_as_it_is() {
        let f = fixture();
        let Ok(Pressed::Asked(asked)) = press(&f) else {
            panic!()
        };
        let view = f.rounds.view("p1").unwrap();
        f.rounds.act("p1", view.id, Action::SkipAll).unwrap();
        block_on(
            f.drifts
                .answered("p1", &f.id, &asked.prepared, asked.outcome.recv().unwrap()),
        );
        assert!(resolutions(&f).is_empty());
        assert!(
            f.host.started.lock().unwrap().is_empty() && f.host.notices.lock().unwrap().is_empty()
        );
        assert!(f.rounds.view("p1").is_none());
    }

    #[test]
    fn a_refused_hand_over_discards_the_draft_and_starts_a_resolve_job_with_the_answer() {
        // The Drift concerns only a State that is gone: a code fix is refused at the hand-over.
        let f = fixture_with(&["ghost/Nowhere"]);
        let Ok(Pressed::Asked(asked)) = press(&f) else {
            panic!("the prepared question holds: the reference was missing at preparation too")
        };
        answer(&f, Some(0), "");
        block_on(
            f.drifts
                .answered("p1", &f.id, &asked.prepared, asked.outcome.recv().unwrap()),
        );
        assert_eq!(
            resolutions(&f),
            [(
                "Resolve: The code ships twice".to_owned(),
                Status::Discarded
            )],
            "the Draft is discarded"
        );
        let started = f.host.started.lock().unwrap().clone();
        assert_eq!(
            started,
            [(
                f.id.clone(),
                Some(ResolveAnswer {
                    selected: vec!["Deduplicate".into()],
                    text: None
                })
            )]
        );
    }

    #[test]
    fn a_job_that_cannot_start_after_the_answer_says_so() {
        let f = fixture();
        let Ok(Pressed::Asked(asked)) = press(&f) else {
            panic!()
        };
        answer(&f, Some(2), "Only for guests");
        *f.host.cannot_start.lock().unwrap() = Some("A run is in progress".to_owned());
        block_on(
            f.drifts
                .answered("p1", &f.id, &asked.prepared, asked.outcome.recv().unwrap()),
        );
        assert_eq!(
            f.host.notices.lock().unwrap().as_slice(),
            ["Could not resolve D1: A run is in progress. The answer was not used."]
        );
    }

    #[test]
    fn a_drift_that_is_no_longer_open_ignores_the_answer() {
        let f = fixture();
        let Ok(Pressed::Asked(asked)) = press(&f) else {
            panic!()
        };
        answer(&f, Some(0), "");
        let path = ProjectImage::load(f.project.path())
            .card(CardKind::Drift, &f.id)
            .unwrap()
            .path
            .clone();
        let text = fs::read_to_string(f.project.path().join(".weaver").join(&path)).unwrap();
        fs::write(
            f.project.path().join(".weaver").join(&path),
            text.replace("status: open", "status: dismissed"),
        )
        .unwrap();
        block_on(
            f.drifts
                .answered("p1", &f.id, &asked.prepared, asked.outcome.recv().unwrap()),
        );
        assert!(resolutions(&f).is_empty());
        assert_eq!(f.host.notices.lock().unwrap().len(), 1);
    }

    #[test]
    fn without_a_valid_question_a_resolve_job_asks_and_every_reason_refuses_resolve_before_anything_starts(
    ) {
        let f = fixture();
        // The Model the Drift concerns changes: the prepared question no longer holds.
        put(
            f.project.path(),
            "model/shop/order.md",
            &ORDER.replace("### Paid\n", "### Paid\nPaid for.\n"),
        );
        assert!(matches!(press(&f), Ok(Pressed::Job)));
        assert_eq!(
            f.host.started.lock().unwrap().as_slice(),
            [(f.id.clone(), None)]
        );
        assert!(f.rounds.view("p1").is_none(), "no round is asked by Weaver");

        f.host.started.lock().unwrap().clear();
        f.host.facts.lock().unwrap().run_in_progress = true;
        assert_eq!(
            press(&f).err().unwrap(),
            "Could not resolve D1: A run is in progress."
        );
        *f.host.facts.lock().unwrap() = AgentFacts {
            resolving_drifts: [f.id.clone()].into(),
            ..AgentFacts::default()
        };
        assert_eq!(
            press(&f).err().unwrap(),
            "Could not resolve D1: This Drift is being resolved."
        );
        *f.host.cannot_start.lock().unwrap() = Some("omp is not ready".to_owned());
        *f.host.facts.lock().unwrap() = AgentFacts::default();
        assert_eq!(
            press(&f).err().unwrap(),
            "Could not resolve D1: omp is not ready."
        );
        assert!(f.host.started.lock().unwrap().is_empty());
        assert!(
            !f.project.path().join(".weaver/proposals").exists(),
            "a refusal writes nothing"
        );
    }

    #[test]
    fn resolve_is_refused_while_a_merge_is_in_progress_even_with_a_valid_prepared_question() {
        let f = fixture();
        f.host.facts.lock().unwrap().merge_in_progress = true;
        assert_eq!(
            press(&f).err().unwrap(),
            "Could not resolve D1: A merge is in progress."
        );
        assert!(f.rounds.view("p1").is_none(), "no round is asked by Weaver");
        assert!(f.host.started.lock().unwrap().is_empty());
        assert!(
            !f.project.path().join(".weaver/proposals").exists(),
            "a refusal writes nothing"
        );
        f.host.facts.lock().unwrap().merge_in_progress = false;
        assert!(matches!(press(&f), Ok(Pressed::Asked(_))));
    }

    #[test]
    fn resolve_is_refused_while_the_job_that_filed_the_drift_works() {
        let f = fixture();
        let path = ProjectImage::load(f.project.path())
            .card(CardKind::Drift, &f.id)
            .unwrap()
            .path
            .clone();
        let text = fs::read_to_string(f.project.path().join(".weaver").join(&path)).unwrap();
        fs::write(
            f.project.path().join(".weaver").join(&path),
            text.replacen("created:", "job: jjjjjjjj\ncreated:", 1),
        )
        .unwrap();
        f.host
            .facts
            .lock()
            .unwrap()
            .working_jobs
            .insert("jjjjjjjj".to_owned());
        assert_eq!(
            press(&f).err().unwrap(),
            "Could not resolve D1: The agent is still finishing."
        );
        assert!(f.rounds.view("p1").is_none());
    }

    #[test]
    fn a_resolve_job_without_an_answer_is_refused_while_a_valid_question_exists() {
        use weaver_core::jobs::{names, JobKind};
        let f = fixture();
        let image = ProjectImage::load(f.project.path());
        let plan = |answer: Option<ResolveAnswer>| StartPlan {
            kind: JobKind::Resolve,
            name: names::resolve(&image.short_numbers(), &f.id),
            job: Job::Resolve {
                drift: f.id.clone(),
                answer,
            },
            target: Some(f.id.clone()),
            task: None,
            proposal_model: None,
            max_agents: 1,
            merged: None,
        };
        assert!(PreparedGate
            .refuse(&image, &plan(None))
            .is_some_and(|why| why.contains("prepared question")));
        assert_eq!(
            PreparedGate.refuse(
                &image,
                &plan(Some(ResolveAnswer {
                    selected: vec![],
                    text: Some("x".into())
                }))
            ),
            None,
            "an answer already given starts the job"
        );
        put(
            f.project.path(),
            "model/shop/order.md",
            &ORDER.replace("### Paid\n", "### Paid\nPaid for.\n"),
        );
        let stale = ProjectImage::load(f.project.path());
        assert_eq!(
            PreparedGate.refuse(&stale, &plan(None)),
            None,
            "no valid question: the job asks"
        );
    }

    // --- the decisions ---

    struct Working(AtomicBool);

    impl AgentWork for Working {
        fn ghosts(&self, _: &str) -> Vec<Ghost> {
            Vec::new()
        }
        fn facts(&self, _: &str) -> AgentFacts {
            let mut facts = AgentFacts::default();
            if self.0.load(Ordering::SeqCst) {
                facts.working_jobs.insert("jjjjjjjj".to_owned());
            }
            facts
        }
    }

    async fn open(f: &Fixture, base: &Path) -> (AppState, String) {
        let state = AppState::new(AppDirs::from_bases(base, base, None));
        let id = state
            .pictures
            .open(f.project.path().to_path_buf())
            .await
            .unwrap();
        (state, id)
    }

    #[tokio::test]
    async fn fixing_the_code_is_applied_in_the_queue_and_publishes_the_picture() {
        let f = fixture();
        let base = tempfile::tempdir().unwrap();
        let (state, id) = open(&f, base.path()).await;
        state
            .agent_work
            .set(Arc::new(Working(AtomicBool::new(false))));
        let revision = decide_drift(
            &state,
            &id,
            &f.id,
            Decision::FixCode {
                option: Some(2),
                note: Some("Mind the lock".into()),
            },
        )
        .await
        .unwrap();
        assert_eq!(revision.map(|r| r.revision), Some(2));
        let image = state.pictures.get(&id).unwrap().image;
        let drift = image.card(CardKind::Drift, &f.id).unwrap();
        assert_eq!(drift.str_key("status"), Some("code-to-be-fixed"));
        assert_eq!(image.cards_of(CardKind::Task).count(), 1, "the fix Task");
    }

    #[tokio::test]
    async fn a_decision_refused_when_applied_names_the_drift_and_writes_nothing() {
        let f = fixture();
        let base = tempfile::tempdir().unwrap();
        let (state, id) = open(&f, base.path()).await;
        let path = f.project.path().join(".weaver").join(
            &state
                .pictures
                .get(&id)
                .unwrap()
                .image
                .card(CardKind::Drift, &f.id)
                .unwrap()
                .path,
        );
        let text = fs::read_to_string(&path).unwrap();
        fs::write(
            &path,
            text.replacen("created:", "job: jjjjjjjj\ncreated:", 1),
        )
        .unwrap();
        state
            .pictures
            .rebuild(&id, crate::pictures::Rebuild::All)
            .await
            .unwrap();
        // The button was enabled when the developer saw the Board; the agent works now.
        state
            .agent_work
            .set(Arc::new(Working(AtomicBool::new(true))));
        for decision in [
            Decision::Dismiss,
            Decision::FixCode {
                option: Some(1),
                note: None,
            },
            Decision::AcceptProposal,
        ] {
            let error = decide_drift(&state, &id, &f.id, decision)
                .await
                .unwrap_err();
            assert_eq!(error.code, ErrorCode::Refused);
            assert!(
                error.message.starts_with("Could not ")
                    && error.message.ends_with("D1: The agent is still finishing."),
                "{}",
                error.message
            );
        }
        assert!(!f.project.path().join(".weaver/tasks").exists());
    }

    #[tokio::test]
    async fn dismissing_closes_the_drift_and_a_drift_in_a_resolution_is_refused() {
        let f = fixture();
        let base = tempfile::tempdir().unwrap();
        let (state, id) = open(&f, base.path()).await;
        state
            .agent_work
            .set(Arc::new(Working(AtomicBool::new(false))));
        put(
            f.project.path(),
            "proposals/rrrrrrrr-r/proposal.md",
            &format!("---\nstatus: open\nkind: resolution\ndrifts:\n  - drift: {}\n    decision: dismissed\ncreated: 2026-11-02T08:00:00.000Z\n---\n# R\n", f.id),
        );
        state
            .pictures
            .rebuild(&id, crate::pictures::Rebuild::All)
            .await
            .unwrap();
        let error = decide_drift(&state, &id, &f.id, Decision::Dismiss)
            .await
            .unwrap_err();
        assert_eq!(
            error.message,
            "Could not dismiss D1: This Drift is in a resolution."
        );
        fs::remove_file(
            f.project
                .path()
                .join(".weaver/proposals/rrrrrrrr-r/proposal.md"),
        )
        .unwrap();
        state
            .pictures
            .rebuild(&id, crate::pictures::Rebuild::All)
            .await
            .unwrap();
        decide_drift(&state, &id, &f.id, Decision::Dismiss)
            .await
            .unwrap();
        let image = state.pictures.get(&id).unwrap().image;
        let drift = image.card(CardKind::Drift, &f.id).unwrap();
        assert_eq!(
            (
                drift.str_key("status"),
                drift
                    .str_key("fingerprint")
                    .map(|f| f.starts_with("sha256:"))
            ),
            (Some("dismissed"), Some(true))
        );
    }

    /// Records the refinement jobs it is asked to start.
    #[derive(Default)]
    struct RecordingJobs {
        started: Mutex<Vec<String>>,
        cannot_start: Mutex<Option<String>>,
    }

    impl crate::proposals::ProposalJobs for RecordingJobs {
        fn can_start(&self, _: &str) -> Result<(), String> {
            self.cannot_start
                .lock()
                .unwrap()
                .clone()
                .map_or(Ok(()), Err)
        }

        fn start_refinement(&self, _: &str, proposal: &str) {
            self.started.lock().unwrap().push(proposal.to_owned());
        }

        fn start_rework(&self, _: &str, _: &str) {}

        fn run_all_after_setup(&self, _: &str) {}
    }

    /// Attaches a Proposal `pppppppp` to the Drift of the fixture: a stale Open one, or `status`.
    async fn attach_stale_proposal(f: &Fixture, state: &AppState, id: &str, status: &str) {
        let hash = |text: &str| {
            weaver_core::project::text::sha256_hex(&weaver_core::project::text::file_content(text))
        };
        let path = f.project.path().join(".weaver").join(
            &state
                .pictures
                .get(id)
                .unwrap()
                .image
                .card(CardKind::Drift, &f.id)
                .unwrap()
                .path,
        );
        let text = fs::read_to_string(&path).unwrap();
        fs::write(
            &path,
            text.replacen("created:", "proposal: pppppppp\ncreated:", 1),
        )
        .unwrap();
        put(
            f.project.path(),
            "proposals/pppppppp-p/proposal.md",
            &format!(
                "---\nstatus: {status}\nkind: change\nbase:\n  model/shop/order.md: {}\ncreated: 2026-11-02T07:00:00.000Z\n---\n# Already paid\n",
                hash("another text")
            ),
        );
        put(
            f.project.path(),
            "proposals/pppppppp-p/model/shop/order.md",
            &ORDER.replace("### Paid\n", "### Paid\nPaid.\n"),
        );
        state
            .pictures
            .rebuild(id, crate::pictures::Rebuild::All)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn refining_writes_refining_on_the_attached_proposal_leaves_the_drift_open_and_starts_the_job(
    ) {
        let f = fixture();
        let base = tempfile::tempdir().unwrap();
        let (state, id) = open(&f, base.path()).await;
        state
            .agent_work
            .set(Arc::new(Working(AtomicBool::new(false))));
        let jobs = Arc::new(RecordingJobs::default());
        state.proposal_jobs.set(jobs.clone());
        attach_stale_proposal(&f, &state, &id, "open").await;

        let revision = refine_attached(&state, &id, &f.id).await.unwrap();
        assert!(revision.is_some());
        let image = state.pictures.get(&id).unwrap().image;
        let proposal = image.card(CardKind::Proposal, "pppppppp").unwrap();
        assert_eq!(proposal_data::status_of(proposal), Some(Status::Refining));
        assert!(proposal_data::refining_flag(proposal));
        assert_eq!(
            image
                .card(CardKind::Drift, &f.id)
                .unwrap()
                .str_key("status"),
            Some("open"),
            "the Drift is not decided"
        );
        assert_eq!(*jobs.started.lock().unwrap(), ["pppppppp"]);

        // Being refined, every decision is refused.
        let error = refine_attached(&state, &id, &f.id).await.unwrap_err();
        assert_eq!(
            error.message,
            "Could not refine the Proposal of D1: Being refined."
        );
        let error = decide_drift(&state, &id, &f.id, Decision::AcceptProposal)
            .await
            .unwrap_err();
        assert_eq!(
            error.message,
            "Could not accept the Proposal of D1: Being refined."
        );
        assert_eq!(jobs.started.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn refining_writes_nothing_and_starts_nothing_when_no_job_can_start() {
        let f = fixture();
        let base = tempfile::tempdir().unwrap();
        let (state, id) = open(&f, base.path()).await;
        state
            .agent_work
            .set(Arc::new(Working(AtomicBool::new(false))));
        let jobs = Arc::new(RecordingJobs::default());
        *jobs.cannot_start.lock().unwrap() = Some("no agent job can start yet".to_owned());
        state.proposal_jobs.set(jobs.clone());
        attach_stale_proposal(&f, &state, &id, "open").await;

        let error = refine_attached(&state, &id, &f.id).await.unwrap_err();
        assert_eq!(error.code, ErrorCode::Refused);
        assert_eq!(
            error.message,
            "Could not refine the Proposal of D1: no agent job can start yet."
        );
        let image = state.pictures.get(&id).unwrap().image;
        assert_eq!(
            proposal_data::status_of(image.card(CardKind::Proposal, "pppppppp").unwrap()),
            Some(Status::Open)
        );
        assert!(jobs.started.lock().unwrap().is_empty());
    }
}
