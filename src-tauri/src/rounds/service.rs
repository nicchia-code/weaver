//! The question rounds of the open projects (Machine `question-round`): one queue per project,
//! the agents and Weaver waiting for their results, and what each change does to the jobs and the
//! window.
//!
//! The decisions are the core's ([`weaver_core::question::Queue`]); this module keeps the queues
//! and the waiters in memory, never on disk and never in a log, and carries out the effects
//! outside its lock.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard};

use weaver_core::ipc::Peer;
use weaver_core::protocol::{Request, Response};
use weaver_core::question::{
    self, code, Action, ActionError, Effect, Ending, Origin, Outcome, Position, Queue, Round,
    RoundId, RoundRequest, CANCELLED_MESSAGE, INTERRUPTED_MESSAGE,
};

use super::view::{view_of, QuestionRoundChanged, QuestionRoundView};

/// What the rounds know of a job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobFacts {
    /// The project hash.
    pub project_id: String,
    pub name: String,
    /// The job is Running or Waiting for the developer: the only states that may ask.
    pub accepting: bool,
}

/// What the rounds need from the rest of the app.
pub trait RoundsHost: Send + Sync + 'static {
    fn job(&self, job: &str) -> Option<JobFacts>;
    /// `question-round.OPENED`, received by the job.
    fn round_opened(&self, job: &str);
    /// `question-round.CLOSED`, received by the job.
    fn round_closed(&self, job: &str);
    /// The project of the live Chat whose identity (`WEAVER_GENERAL`) is `token`: only while its
    /// project is open and a turn of the Chat is in progress.
    fn chat(&self, token: &str) -> Option<String>;
    /// `question-round.OPENED`, received by the Chat of the project (only a round of the Chat).
    fn chat_round_opened(&self, project_id: &str);
    /// `question-round.CLOSED`, received by the Chat of the project (only a round of the Chat).
    fn chat_round_closed(&self, project_id: &str);
    /// The window of the project comes to the front.
    fn focus_project(&self, project_id: &str);
    /// The round on screen in a project changed.
    fn changed(&self, change: &QuestionRoundChanged);
}

#[derive(Default)]
struct Project {
    queue: Queue,
    /// Whoever waits for the end of a round: an agent's request, or Weaver for a Drift.
    waiters: HashMap<RoundId, Sender<Outcome>>,
}

/// The effects of a change, split: what reaches the waiters and what reaches the app.
struct Settled {
    deliveries: Vec<(Sender<Outcome>, Outcome)>,
    effects: Vec<Effect>,
}

impl Project {
    fn settle(&mut self, effects: Vec<Effect>) -> Settled {
        let mut settled = Settled {
            deliveries: Vec::new(),
            effects: Vec::new(),
        };
        for effect in effects {
            match effect {
                Effect::Ended { id, outcome, .. } => {
                    if let Some(waiter) = self.waiters.remove(&id) {
                        settled.deliveries.push((waiter, outcome));
                    }
                }
                other => settled.effects.push(other),
            }
        }
        settled
    }
}

/// The question in view of a presented round, as an Ask more conversation reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestionInView {
    pub request: RoundRequest,
    pub origin: Origin,
    /// The job that asked, when an agent did: its identity and its name when it is still known.
    pub job: Option<(String, Option<String>)>,
}

/// The rounds of every open project.
pub struct Rounds {
    host: Arc<dyn RoundsHost>,
    projects: Mutex<HashMap<String, Project>>,
    next: AtomicU64,
}

impl Rounds {
    pub fn new(host: Arc<dyn RoundsHost>) -> Arc<Self> {
        Arc::new(Self {
            host,
            projects: Mutex::new(HashMap::new()),
            next: AtomicU64::new(1),
        })
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<String, Project>> {
        self.projects
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// `window.OPENED`: the queue of the project, created now and kept while the window is open.
    pub fn window_opened(&self, project_id: &str) {
        self.lock().entry(project_id.to_owned()).or_default();
    }

    /// `window.CLOSED`: every round of the project is interrupted and its queue is gone.
    pub fn window_closed(&self, project_id: &str) {
        let settled = {
            let mut projects = self.lock();
            let Some(mut project) = projects.remove(project_id) else {
                return;
            };
            let effects = project.queue.interrupt_all();
            project.settle(effects)
        };
        self.carry_out(project_id, settled, false);
    }

    /// `job.ENDED`: the rounds of the job are interrupted.
    pub fn job_ended(&self, project_id: &str, job: &str) {
        let settled = {
            let mut projects = self.lock();
            let Some(project) = projects.get_mut(project_id) else {
                return;
            };
            let effects = project.queue.interrupt_job(job);
            project.settle(effects)
        };
        self.carry_out(project_id, settled, false);
    }

    /// `agent.STOP_WAITING`: the round is abandoned, queued or presented.
    pub fn abandon(&self, project_id: &str, id: RoundId) {
        let settled = {
            let mut projects = self.lock();
            let Some(project) = projects.get_mut(project_id) else {
                return;
            };
            let effects = project.queue.finish(id, Ending::Abandoned);
            project.settle(effects)
        };
        self.carry_out(project_id, settled, false);
    }

    /// What the developer does in the presented round of `project_id`; the round on screen
    /// afterwards (none when the action ended it and nothing else waits).
    pub fn act(
        &self,
        project_id: &str,
        id: RoundId,
        action: Action,
    ) -> Result<Option<QuestionRoundView>, ActionError> {
        let settled = {
            let mut projects = self.lock();
            let Some(project) = projects.get_mut(project_id) else {
                return Err(ActionError::NoSuchRound);
            };
            let effects = project.queue.act(id, action)?;
            project.settle(effects)
        };
        self.carry_out(project_id, settled, false);
        Ok(self.view(project_id))
    }

    /// The round on screen in the window of the project.
    pub fn view(&self, project_id: &str) -> Option<QuestionRoundView> {
        let (round, waiting) = {
            let projects = self.lock();
            let project = projects.get(project_id)?;
            (project.queue.presented()?.clone(), project.queue.waiting())
        };
        let name = round
            .origin
            .job()
            .and_then(|job| self.host.job(job))
            .map(|facts| facts.name);
        Some(view_of(project_id, &round, name, waiting))
    }

    /// What an Ask more conversation reads of the question in view of the round `id` of the
    /// project: only when that round is the presented one and its position is the question
    /// `index`. The request holds the questions and never the developer's drafts, choices or
    /// answers.
    pub fn question_in_view(
        &self,
        project_id: &str,
        id: RoundId,
        index: usize,
    ) -> Option<QuestionInView> {
        let (request, origin) = {
            let projects = self.lock();
            let round = projects.get(project_id)?.queue.presented()?;
            if round.id != id || round.position != (Position::Question { index }) {
                return None;
            }
            if index >= round.request.questions.len() {
                return None;
            }
            (round.request.clone(), round.origin.clone())
        };
        let job = origin
            .job()
            .map(|job| (job.to_owned(), self.host.job(job).map(|facts| facts.name)));
        Some(QuestionInView {
            request,
            origin,
            job,
        })
    }

    /// Puts a round in the queue of the project; the effects are carried out, and a change is
    /// announced even when the round only waits (the count behind the presented one grew).
    fn submit(
        &self,
        project_id: &str,
        origin: Origin,
        request: RoundRequest,
    ) -> (RoundId, Receiver<Outcome>) {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let (waiter, outcome) = mpsc::channel();
        let settled = {
            let mut projects = self.lock();
            let project = projects.entry(project_id.to_owned()).or_default();
            project.waiters.insert(id, waiter);
            let effects = project.queue.submit(Round::new(id, origin, request));
            project.settle(effects)
        };
        self.carry_out(project_id, settled, true);
        (id, outcome)
    }

    /// `developer.RESOLVE_DRIFT`: the prepared question of a Drift, asked by Weaver with no agent.
    /// `id` is the Drift's permanent identity, which the round keeps; `drift` is how the developer
    /// names the Drift (its short number, `D3`) and shows in the header, `Question about D3 ·
    /// <title>`. The receiver gets what the round came to; a Drift question that was skipped
    /// leaves the Drift open, which the caller reads from the answers.
    pub fn present_prepared(
        &self,
        project_id: &str,
        id: &str,
        drift: &str,
        title: &str,
        request: RoundRequest,
    ) -> Receiver<Outcome> {
        self.submit(
            project_id,
            Origin::Drift {
                id: id.to_owned(),
                drift: drift.to_owned(),
                title: title.to_owned(),
            },
            request,
        )
        .1
    }

    /// `agent.ASK_QUESTIONS`: answers a request of kind `question.new` when the round has a
    /// result. It blocks, with no time limit, and runs on the blocking pool of the socket.
    pub fn handle(self: &Arc<Self>, request: Request, peer: Peer) -> Response {
        let round = match question::parse(&request.payload) {
            Ok(round) => round,
            Err(problems) => {
                return Response::failure(code::INVALID_INPUT, question::describe(&problems))
            }
        };
        let unavailable = || Response::failure(code::UNAVAILABLE, question::UNAVAILABLE_REASON);
        let (project_id, origin) = if let Some(job) = request.job {
            // The job identity decides: a Chat identity next to it is never looked at.
            let Some(facts) = self.host.job(&job).filter(|facts| facts.accepting) else {
                return unavailable();
            };
            (facts.project_id, Origin::Agent { job })
        } else if let Some(project) = request.general.as_deref().and_then(|t| self.host.chat(t)) {
            (project.clone(), Origin::Chat { project })
        } else {
            return unavailable();
        };

        let (id, outcome) = self.submit(&project_id, origin, round.clone());
        // The connection closing is `agent.STOP_WAITING`.
        let (rounds, project) = (self.clone(), project_id);
        peer.on_close(move || rounds.abandon(&project, id));
        match outcome.recv() {
            Ok(Outcome::Answered(answers)) => {
                Response::ok(question::answered_json(&round, &answers))
            }
            Ok(Outcome::Interrupted) => Response::failure(code::INTERRUPTED, INTERRUPTED_MESSAGE),
            Ok(Outcome::Abandoned) | Err(_) => {
                Response::failure(code::CANCELLED, CANCELLED_MESSAGE)
            }
        }
    }

    /// Delivers the results, then does what the effects ask of the jobs and the window, outside
    /// the lock.
    fn carry_out(&self, project_id: &str, settled: Settled, force: bool) {
        let Settled {
            deliveries,
            effects,
        } = settled;
        for (waiter, outcome) in deliveries {
            // The waiter may be gone (its connection closed): nobody is left to tell.
            let _ = waiter.send(outcome);
        }
        let changed = force || !effects.is_empty();
        for effect in effects {
            match effect {
                Effect::Closed { origin, .. } => match &origin {
                    Origin::Agent { job } => self.host.round_closed(job),
                    Origin::Chat { project } => self.host.chat_round_closed(project),
                    Origin::Drift { .. } => {}
                },
                Effect::Opened { origin, .. } => {
                    match &origin {
                        Origin::Agent { job } => self.host.round_opened(job),
                        Origin::Chat { project } => self.host.chat_round_opened(project),
                        Origin::Drift { .. } => {}
                    }
                    self.host.focus_project(project_id);
                }
                Effect::Ended { .. } => {}
            }
        }
        if changed {
            self.host.changed(&QuestionRoundChanged {
                project_id: project_id.to_owned(),
                round: self.view(project_id),
            });
        }
    }
}

#[cfg(test)]
mod tests;
