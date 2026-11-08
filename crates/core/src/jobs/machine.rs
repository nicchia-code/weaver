//! The Machine `job`: events in, effects out, no I/O, no clock and no process.
//!
//! The app feeds it what happens (the developer's actions, the ACP session of omp, the window)
//! and executes the effects it answers with. Everything the Machine decides is here, so that every
//! transition is tested without omp:
//!
//! ```text
//! Queued ──SESSION_STARTED──▶ Running ◀─round closed / permission answered─▶ Waiting
//!   │                           │   ▲                                           │
//!   │ START_FAILED              │   └───── SESSION_RESUMED ── Suspended ◀─ window closed
//!   ▼                           ▼
//! Failed / Stopped / Done  (final: ENDED, holdings released)
//! ```
//!
//! A job ends only on a definite fact (the turn ended or failed, omp exited, the developer
//! stopped it, the session could not be resumed); no event here is a timeout.
//!
//! Messages of the developer: while Running one goes out at once as a steer; while Waiting it is
//! held and goes out when the wait ends; one that was not yet delivered when the turn ended starts
//! a new turn.

use std::collections::BTreeSet;

use super::kinds::JobState;

/// The identity of a message of the developer, given by the app.
pub type MessageId = u64;

/// The identity of a permission request of omp, given by the app.
pub type RequestId = u64;

/// What `_session/steering` answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SteeringOutcome {
    /// The message joined the running turn.
    Injected,
    /// No turn runs: the message must go as `session/prompt`.
    PromptRequired,
    /// `failed` or an error, with the reason.
    Failed(String),
}

/// What happened to the job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// `omp.SESSION_STARTED`: `session/new` returned and the instruction went out.
    SessionStarted,
    /// `omp.START_FAILED`.
    StartFailed(String),
    /// `developer.STOP_AGENT`.
    StopAgent,
    /// `window.CLOSED`.
    WindowClosed,
    /// `window.OPENED`.
    WindowOpened,
    /// `omp.SESSION_RESUMED`.
    SessionResumed,
    /// `omp.SESSION_NOT_RESUMED`.
    SessionNotResumed(String),
    /// `developer.SEND_MESSAGE`.
    SendMessage(MessageId),
    /// The reply of omp to the steer of a message.
    SteeringReplied(MessageId, SteeringOutcome),
    /// `question-round.OPENED` (a round of this job).
    RoundOpened,
    /// `question-round.CLOSED`.
    RoundClosed,
    /// `omp.PERMISSION_REQUESTED`.
    PermissionRequested(RequestId),
    /// `developer.ANSWER_PERMISSION`.
    PermissionAnswered(RequestId),
    /// `omp.TURN_ENDED`.
    TurnEnded,
    /// `omp.TURN_FAILED`.
    TurnFailed(String),
    /// `omp.EXITED`, with omp's last lines about the error.
    Exited(String),
}

/// How a permission request that is still pending is closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionEnd {
    /// Stop or close: omp is answered `cancelled`; the card says `Cancelled.`.
    Cancelled,
    /// The turn ended: nobody answers; the card says `No longer needed.`.
    Void,
}

/// What the app must do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// Spawn omp, open its session and send the instruction.
    StartOmp,
    /// Send the message as `_session/steering` with no `_meta`.
    Steer(MessageId),
    /// Send the held message as `_session/steering` with `_meta: {steering: {queue: "followUp"}}`.
    FollowUp(MessageId),
    /// Start a new turn: `session/prompt` with these messages.
    Prompt(Vec<MessageId>),
    /// The message reached the agent.
    Delivered(MessageId),
    /// The message did not reach the agent, with the reason shown under it.
    NotDelivered(MessageId, String),
    /// Close the pending permission requests of the job.
    ClosePermissions(PermissionEnd),
    /// End the omp process in order: `session/cancel` when a turn runs, `session/close` (2 s),
    /// SIGTERM to the group, SIGKILL after 3 s.
    ShutDown,
    /// Spawn omp again with the same arguments and environment, and reload the session.
    Resume,
    /// Tell the agent that Weaver stopped and reloaded its session.
    ResumePrompt,
    /// `job.WAITING`.
    Waiting,
    /// `job.ENDED`: the job reached Done, Failed or Stopped; its holdings are released.
    Ended,
}

/// The reason shown under a message that was never delivered.
pub const NOT_DELIVERED_ENDED: &str = "the job ended";
pub const NOT_DELIVERED_SUSPENDED: &str = "Weaver stopped before it was delivered";

/// The prompt that tells the agent its session was reloaded.
pub const RESUME_PROMPT: &str =
    "Weaver stopped and reloaded your session. Continue your work from where it stopped.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobMachine {
    state: JobState,
    reason: Option<String>,
    /// A turn of the agent is in flight (a `session/prompt` has not answered yet).
    turn_running: bool,
    rounds: u32,
    permissions: BTreeSet<RequestId>,
    /// Held while Waiting.
    held: Vec<MessageId>,
    /// Steered and not answered yet.
    in_flight: BTreeSet<MessageId>,
    resuming: bool,
}

impl Default for JobMachine {
    fn default() -> Self {
        Self::new()
    }
}

impl JobMachine {
    /// A job just created: Queued.
    pub fn new() -> Self {
        Self::at(JobState::Queued)
    }

    /// A job restored from its record at `state` (a Suspended one waits for the window).
    pub fn at(state: JobState) -> Self {
        Self {
            state,
            reason: None,
            turn_running: false,
            rounds: 0,
            permissions: BTreeSet::new(),
            held: Vec::new(),
            in_flight: BTreeSet::new(),
            resuming: false,
        }
    }

    pub fn state(&self) -> JobState {
        self.state
    }

    /// Why a Failed job failed.
    pub fn reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }

    /// Whether `developer.SEND_MESSAGE` is accepted now: the agent works or waits.
    pub fn accepts_messages(&self) -> bool {
        self.state.is_working()
    }

    /// Whether the agent may declare notes and announce cards now.
    pub fn accepts_agent_calls(&self) -> bool {
        self.state.is_working()
    }

    /// Whether a turn is in flight.
    pub fn turn_running(&self) -> bool {
        self.turn_running
    }

    /// Messages held for the end of a wait.
    pub fn held(&self) -> &[MessageId] {
        &self.held
    }

    fn waiting(&self) -> bool {
        self.rounds > 0 || !self.permissions.is_empty()
    }

    /// Applies `event`; returns what the app must do, in order.
    pub fn handle(&mut self, event: Event) -> Vec<Effect> {
        match self.state {
            JobState::Queued => self.queued(event),
            JobState::Running | JobState::Waiting => self.working(event),
            JobState::Suspended => self.suspended(event),
            JobState::Done | JobState::Failed | JobState::Stopped => Vec::new(),
        }
    }

    fn queued(&mut self, event: Event) -> Vec<Effect> {
        match event {
            Event::SessionStarted => {
                self.state = JobState::Running;
                self.turn_running = true;
                Vec::new()
            }
            Event::StartFailed(reason) => self.finish(JobState::Failed, Some(reason), Vec::new()),
            Event::StopAgent | Event::WindowClosed => {
                self.finish(JobState::Stopped, None, Vec::new())
            }
            _ => Vec::new(),
        }
    }

    fn working(&mut self, event: Event) -> Vec<Effect> {
        match event {
            Event::SendMessage(id) => {
                if self.state == JobState::Waiting {
                    self.held.push(id);
                    Vec::new()
                } else {
                    self.in_flight.insert(id);
                    vec![Effect::Steer(id)]
                }
            }
            Event::SteeringReplied(id, outcome) => self.steering_replied(id, outcome),
            Event::RoundOpened => {
                self.rounds += 1;
                self.start_waiting()
            }
            Event::PermissionRequested(request) => {
                self.permissions.insert(request);
                self.start_waiting()
            }
            Event::RoundClosed => {
                self.rounds = self.rounds.saturating_sub(1);
                self.stop_waiting()
            }
            Event::PermissionAnswered(request) => {
                self.permissions.remove(&request);
                self.stop_waiting()
            }
            Event::TurnEnded => self.turn_ended(),
            Event::TurnFailed(reason) => self.finish(JobState::Failed, Some(reason), Vec::new()),
            Event::Exited(reason) => {
                // omp is gone already: no shutdown to run, and a permission asked is void.
                let mut effects = self.close_permissions(PermissionEnd::Void);
                effects.extend(self.discard_messages(NOT_DELIVERED_ENDED));
                self.become_final(JobState::Failed, Some(reason));
                effects.push(Effect::Ended);
                effects
            }
            Event::StopAgent => {
                let mut effects = self.close_permissions(PermissionEnd::Cancelled);
                effects.extend(self.finish(JobState::Stopped, None, Vec::new()));
                effects
            }
            Event::WindowClosed => {
                let mut effects = self.close_permissions(PermissionEnd::Cancelled);
                effects.extend(self.discard_messages(NOT_DELIVERED_SUSPENDED));
                self.state = JobState::Suspended;
                self.turn_running = false;
                self.rounds = 0;
                self.resuming = false;
                effects.push(Effect::ShutDown);
                effects
            }
            Event::SessionStarted
            | Event::StartFailed(_)
            | Event::WindowOpened
            | Event::SessionResumed
            | Event::SessionNotResumed(_) => Vec::new(),
        }
    }

    fn suspended(&mut self, event: Event) -> Vec<Effect> {
        match event {
            Event::WindowOpened if !self.resuming => {
                self.resuming = true;
                vec![Effect::Resume]
            }
            Event::SessionResumed if self.resuming => {
                self.resuming = false;
                self.state = JobState::Running;
                self.turn_running = true;
                vec![Effect::ResumePrompt]
            }
            Event::SessionNotResumed(reason) | Event::Exited(reason) if self.resuming => {
                self.resuming = false;
                self.finish(JobState::Failed, Some(reason), Vec::new())
            }
            Event::WindowClosed if self.resuming => {
                self.resuming = false;
                vec![Effect::ShutDown]
            }
            Event::StopAgent => {
                self.resuming = false;
                self.finish(JobState::Stopped, None, Vec::new())
            }
            _ => Vec::new(),
        }
    }

    fn start_waiting(&mut self) -> Vec<Effect> {
        if self.state == JobState::Waiting {
            return Vec::new();
        }
        self.state = JobState::Waiting;
        vec![Effect::Waiting]
    }

    fn stop_waiting(&mut self) -> Vec<Effect> {
        if self.state != JobState::Waiting || self.waiting() {
            return Vec::new();
        }
        self.state = JobState::Running;
        // The held messages go out as follow-ups now that the wait is over.
        let held = std::mem::take(&mut self.held);
        held.into_iter()
            .map(|id| {
                self.in_flight.insert(id);
                Effect::FollowUp(id)
            })
            .collect()
    }

    fn steering_replied(&mut self, id: MessageId, outcome: SteeringOutcome) -> Vec<Effect> {
        if !self.in_flight.remove(&id) {
            return Vec::new();
        }
        let mut effects = match outcome {
            SteeringOutcome::Injected => vec![Effect::Delivered(id)],
            SteeringOutcome::PromptRequired if self.turn_running => {
                // A turn is in flight after all: the message waits for the next one.
                self.held.push(id);
                Vec::new()
            }
            SteeringOutcome::PromptRequired => {
                self.turn_running = true;
                vec![Effect::Prompt(vec![id]), Effect::Delivered(id)]
            }
            SteeringOutcome::Failed(reason) => vec![Effect::NotDelivered(id, reason)],
        };
        effects.extend(self.finish_when_idle());
        effects
    }

    fn turn_ended(&mut self) -> Vec<Effect> {
        self.turn_running = false;
        if self.state == JobState::Waiting {
            // A permission still asked is void; the messages held never reach the agent.
            let mut effects = self.close_permissions(PermissionEnd::Void);
            effects.extend(self.discard_messages(NOT_DELIVERED_ENDED));
            self.rounds = 0;
            effects.extend(self.finish(JobState::Done, None, Vec::new()));
            return effects;
        }
        if !self.held.is_empty() {
            let held = std::mem::take(&mut self.held);
            self.turn_running = true;
            let mut effects = vec![Effect::Prompt(held.clone())];
            effects.extend(held.into_iter().map(Effect::Delivered));
            return effects;
        }
        // Messages steered and not answered yet decide: a `promptRequired` starts the new turn.
        self.finish_when_idle()
    }

    /// Running with nothing in flight and nothing held: the agent finished its work.
    fn finish_when_idle(&mut self) -> Vec<Effect> {
        if self.state == JobState::Running
            && !self.turn_running
            && self.in_flight.is_empty()
            && self.held.is_empty()
        {
            return self.finish(JobState::Done, None, Vec::new());
        }
        Vec::new()
    }

    fn close_permissions(&mut self, how: PermissionEnd) -> Vec<Effect> {
        if self.permissions.is_empty() {
            return Vec::new();
        }
        self.permissions.clear();
        vec![Effect::ClosePermissions(how)]
    }

    fn discard_messages(&mut self, reason: &str) -> Vec<Effect> {
        let ids: Vec<MessageId> = std::mem::take(&mut self.held)
            .into_iter()
            .chain(std::mem::take(&mut self.in_flight))
            .collect();
        ids.into_iter()
            .map(|id| Effect::NotDelivered(id, reason.to_owned()))
            .collect()
    }

    fn become_final(&mut self, state: JobState, reason: Option<String>) {
        self.state = state;
        self.reason = reason;
        self.turn_running = false;
        self.rounds = 0;
        self.resuming = false;
    }

    /// Reaches a final state: pending messages are not delivered, omp is ended, the job ends.
    fn finish(
        &mut self,
        state: JobState,
        reason: Option<String>,
        mut effects: Vec<Effect>,
    ) -> Vec<Effect> {
        effects.extend(self.close_permissions(PermissionEnd::Void));
        effects.extend(self.discard_messages(NOT_DELIVERED_ENDED));
        self.become_final(state, reason);
        effects.push(Effect::ShutDown);
        effects.push(Effect::Ended);
        effects
    }
}

#[cfg(test)]
mod tests {
    use super::Effect::*;
    use super::*;

    fn running() -> JobMachine {
        let mut machine = JobMachine::new();
        assert!(machine.handle(Event::SessionStarted).is_empty());
        machine
    }

    fn waiting_for_a_round() -> JobMachine {
        let mut machine = running();
        assert_eq!(machine.handle(Event::RoundOpened), [Waiting]);
        machine
    }

    #[test]
    fn a_queued_job_starts_only_when_the_session_does() {
        let mut machine = JobMachine::new();
        assert_eq!(machine.state(), JobState::Queued);
        assert!(!machine.accepts_messages() && !machine.accepts_agent_calls());
        // Nothing but the four facts of Queued changes it.
        for event in [
            Event::SendMessage(1),
            Event::RoundOpened,
            Event::TurnEnded,
            Event::Exited("x".into()),
            Event::WindowOpened,
        ] {
            assert!(machine.handle(event).is_empty());
            assert_eq!(machine.state(), JobState::Queued);
        }
        assert!(machine.handle(Event::SessionStarted).is_empty());
        assert_eq!(machine.state(), JobState::Running);
        assert!(machine.turn_running() && machine.accepts_messages());
    }

    #[test]
    fn a_queued_job_fails_with_the_reason_or_is_stopped() {
        let mut failed = JobMachine::new();
        assert_eq!(
            failed.handle(Event::StartFailed("the agent did not answer within 30 s".into())),
            [ShutDown, Ended]
        );
        assert_eq!(
            (failed.state(), failed.reason()),
            (JobState::Failed, Some("the agent did not answer within 30 s"))
        );

        for event in [Event::StopAgent, Event::WindowClosed] {
            let mut machine = JobMachine::new();
            assert_eq!(machine.handle(event), [ShutDown, Ended]);
            assert_eq!(machine.state(), JobState::Stopped);
        }
    }

    #[test]
    fn the_turn_ending_ends_a_running_job_when_no_message_waits() {
        let mut machine = running();
        assert_eq!(machine.handle(Event::TurnEnded), [ShutDown, Ended]);
        assert_eq!(machine.state(), JobState::Done);
        // A final job answers nothing.
        for event in [
            Event::StopAgent,
            Event::TurnFailed("x".into()),
            Event::WindowClosed,
            Event::SendMessage(3),
        ] {
            assert!(machine.handle(event).is_empty());
            assert_eq!(machine.state(), JobState::Done);
        }
    }

    #[test]
    fn a_failed_turn_or_an_exit_fails_the_job_with_its_reason() {
        let mut turn = running();
        assert_eq!(
            turn.handle(Event::TurnFailed("the agent refused".into())),
            [ShutDown, Ended]
        );
        assert_eq!(
            (turn.state(), turn.reason()),
            (JobState::Failed, Some("the agent refused"))
        );

        // omp is gone already: no shutdown to run.
        let mut exit = running();
        assert_eq!(
            exit.handle(Event::Exited("the agent exited with code 3".into())),
            [Ended]
        );
        assert_eq!(
            (exit.state(), exit.reason()),
            (JobState::Failed, Some("the agent exited with code 3"))
        );
    }

    #[test]
    fn a_running_job_ends_stopped_with_its_process_ended() {
        let mut machine = running();
        assert_eq!(machine.handle(Event::StopAgent), [ShutDown, Ended]);
        assert_eq!(machine.state(), JobState::Stopped);
    }

    #[test]
    fn a_message_while_running_is_steered_and_delivered_on_the_reply() {
        let mut machine = running();
        assert_eq!(machine.handle(Event::SendMessage(7)), [Steer(7)]);
        assert_eq!(
            machine.handle(Event::SteeringReplied(7, SteeringOutcome::Injected)),
            [Delivered(7)]
        );
        assert_eq!(
            machine.state(),
            JobState::Running,
            "the agent works on, undisturbed"
        );
        assert_eq!(machine.handle(Event::TurnEnded), [ShutDown, Ended]);
    }

    #[test]
    fn a_refused_steer_is_not_delivered_with_its_reason() {
        let mut machine = running();
        machine.handle(Event::SendMessage(1));
        assert_eq!(
            machine.handle(Event::SteeringReplied(
                1,
                SteeringOutcome::Failed("busy".into())
            )),
            [NotDelivered(1, "busy".into())]
        );
        assert_eq!(machine.state(), JobState::Running);
        // An answer for a message nobody sent is ignored.
        assert!(machine
            .handle(Event::SteeringReplied(9, SteeringOutcome::Injected))
            .is_empty());
    }

    #[test]
    fn prompt_required_sends_the_message_as_a_prompt_then_delivers_it() {
        let mut machine = running();
        machine.handle(Event::SendMessage(4));
        // The turn ended before omp saw the steer: the job does not end while the answer is due.
        assert!(machine.handle(Event::TurnEnded).is_empty());
        assert_eq!(machine.state(), JobState::Running);
        assert_eq!(
            machine.handle(Event::SteeringReplied(4, SteeringOutcome::PromptRequired)),
            [Prompt(vec![4]), Delivered(4)]
        );
        assert_eq!(machine.state(), JobState::Running);
        assert!(machine.turn_running());
        assert_eq!(machine.handle(Event::TurnEnded), [ShutDown, Ended]);
        assert_eq!(machine.state(), JobState::Done);
    }

    #[test]
    fn a_steer_answered_after_the_turn_ended_with_injected_ends_the_job() {
        let mut machine = running();
        machine.handle(Event::SendMessage(4));
        assert!(machine.handle(Event::TurnEnded).is_empty());
        assert_eq!(
            machine.handle(Event::SteeringReplied(4, SteeringOutcome::Injected)),
            [Delivered(4), ShutDown, Ended]
        );
        assert_eq!(machine.state(), JobState::Done);
    }

    #[test]
    fn a_round_makes_the_job_wait_and_its_closing_makes_it_run_again() {
        let mut machine = waiting_for_a_round();
        assert_eq!(machine.state(), JobState::Waiting);
        assert!(
            machine.state().is_working(),
            "waiting still counts as working"
        );
        assert_eq!(
            machine.handle(Event::RoundOpened),
            Vec::<Effect>::new(),
            "WAITING is emitted once"
        );
        assert!(machine.handle(Event::RoundClosed).is_empty());
        assert_eq!(
            machine.state(),
            JobState::Waiting,
            "one round of two is still open"
        );
        assert!(machine.handle(Event::RoundClosed).is_empty());
        assert_eq!(machine.state(), JobState::Running);
    }

    #[test]
    fn a_permission_waits_and_the_answer_resumes() {
        let mut machine = running();
        assert_eq!(machine.handle(Event::PermissionRequested(5)), [Waiting]);
        assert_eq!(machine.state(), JobState::Waiting);
        assert!(machine.handle(Event::PermissionAnswered(5)).is_empty());
        assert_eq!(machine.state(), JobState::Running);
    }

    #[test]
    fn a_message_while_waiting_is_held_and_never_answers_the_round() {
        let mut machine = waiting_for_a_round();
        assert!(machine.handle(Event::SendMessage(1)).is_empty());
        assert!(machine.handle(Event::SendMessage(2)).is_empty());
        assert_eq!(
            machine.state(),
            JobState::Waiting,
            "a message never answers the round or the permission"
        );
        assert_eq!(machine.held(), [1, 2]);
        // The wait ends: the held messages go out as follow-ups, in order.
        assert_eq!(
            machine.handle(Event::RoundClosed),
            [FollowUp(1), FollowUp(2)]
        );
        assert_eq!(machine.state(), JobState::Running);
        assert_eq!(
            machine.handle(Event::SteeringReplied(1, SteeringOutcome::Injected)),
            [Delivered(1)]
        );
        assert_eq!(
            machine.handle(Event::SteeringReplied(2, SteeringOutcome::Injected)),
            [Delivered(2)]
        );
    }

    #[test]
    fn the_turn_ending_while_waiting_ends_the_job_and_voids_the_permission() {
        let mut machine = running();
        machine.handle(Event::PermissionRequested(5));
        machine.handle(Event::SendMessage(3));
        assert_eq!(
            machine.handle(Event::TurnEnded),
            [
                ClosePermissions(PermissionEnd::Void),
                NotDelivered(3, NOT_DELIVERED_ENDED.into()),
                ShutDown,
                Ended
            ]
        );
        assert_eq!(machine.state(), JobState::Done);
    }

    #[test]
    fn a_failure_or_an_exit_while_waiting_fails_the_job() {
        let mut turn = waiting_for_a_round();
        assert_eq!(
            turn.handle(Event::TurnFailed("the agent ran out of tokens".into())),
            [ShutDown, Ended]
        );
        assert_eq!(
            (turn.state(), turn.reason()),
            (JobState::Failed, Some("the agent ran out of tokens"))
        );

        let mut exit = running();
        exit.handle(Event::PermissionRequested(1));
        assert_eq!(
            exit.handle(Event::Exited("boom".into())),
            [ClosePermissions(PermissionEnd::Void), Ended]
        );
        assert_eq!(exit.state(), JobState::Failed);
    }

    #[test]
    fn stop_cancels_a_permission_and_discards_held_messages() {
        let mut machine = running();
        machine.handle(Event::PermissionRequested(8));
        machine.handle(Event::SendMessage(2));
        assert_eq!(
            machine.handle(Event::StopAgent),
            [
                ClosePermissions(PermissionEnd::Cancelled),
                NotDelivered(2, NOT_DELIVERED_ENDED.into()),
                ShutDown,
                Ended
            ]
        );
        assert_eq!(machine.state(), JobState::Stopped);
    }

    #[test]
    fn closing_the_window_suspends_a_running_or_waiting_job() {
        let mut running = running();
        assert_eq!(running.handle(Event::WindowClosed), [ShutDown]);
        assert_eq!(running.state(), JobState::Suspended);
        assert!(!running.accepts_messages());

        let mut waiting = running_with_permission();
        assert_eq!(
            waiting.handle(Event::WindowClosed),
            [ClosePermissions(PermissionEnd::Cancelled), ShutDown]
        );
        assert_eq!(waiting.state(), JobState::Suspended);
    }

    fn running_with_permission() -> JobMachine {
        let mut machine = running();
        machine.handle(Event::PermissionRequested(1));
        machine
    }

    #[test]
    fn opening_the_window_resumes_a_suspended_job_once() {
        let mut machine = running();
        machine.handle(Event::WindowClosed);
        assert_eq!(machine.handle(Event::WindowOpened), [Resume]);
        assert_eq!(
            machine.state(),
            JobState::Suspended,
            "still suspended until the session is reloaded"
        );
        assert!(
            machine.handle(Event::WindowOpened).is_empty(),
            "no second resume"
        );
        assert_eq!(machine.handle(Event::SessionResumed), [ResumePrompt]);
        assert_eq!(machine.state(), JobState::Running);
        assert!(machine.turn_running());
        assert_eq!(machine.handle(Event::TurnEnded), [ShutDown, Ended]);
    }

    #[test]
    fn a_session_that_cannot_be_reloaded_fails_the_job() {
        for event in [
            Event::SessionNotResumed("the session could not be resumed".into()),
            Event::Exited("the session could not be resumed".into()),
        ] {
            let mut machine = JobMachine::at(JobState::Suspended);
            machine.handle(Event::WindowOpened);
            assert_eq!(machine.handle(event), [ShutDown, Ended]);
            assert_eq!(
                (machine.state(), machine.reason()),
                (JobState::Failed, Some("the session could not be resumed"))
            );
        }
    }

    #[test]
    fn a_window_closed_during_the_reload_leaves_the_job_suspended_and_the_next_opening_tries_again()
    {
        let mut machine = JobMachine::at(JobState::Suspended);
        machine.handle(Event::WindowOpened);
        assert_eq!(machine.handle(Event::WindowClosed), [ShutDown]);
        assert_eq!(machine.state(), JobState::Suspended);
        assert_eq!(machine.handle(Event::WindowOpened), [Resume]);
    }

    #[test]
    fn stop_ends_a_suspended_job() {
        let mut machine = JobMachine::at(JobState::Suspended);
        assert_eq!(machine.handle(Event::StopAgent), [ShutDown, Ended]);
        assert_eq!(machine.state(), JobState::Stopped);
        let mut reloading = JobMachine::at(JobState::Suspended);
        reloading.handle(Event::WindowOpened);
        assert_eq!(reloading.handle(Event::StopAgent), [ShutDown, Ended]);
    }

    #[test]
    fn a_suspended_job_takes_no_message_and_no_fact_of_the_agent() {
        let mut machine = JobMachine::at(JobState::Suspended);
        for event in [
            Event::SendMessage(1),
            Event::TurnEnded,
            Event::RoundOpened,
            Event::PermissionRequested(1),
            Event::SessionResumed,
        ] {
            assert!(machine.handle(event).is_empty());
            assert_eq!(machine.state(), JobState::Suspended);
        }
    }
}
