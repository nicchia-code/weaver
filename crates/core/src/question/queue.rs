//! The rounds of one project: one is presented, the others wait first in, first out
//! (`requirements/rules#question-round`).
//!
//! A queue belongs to one project; projects never wait for each other, because each has a queue of
//! its own. The queue only decides and reports: what the app does about each [`Effect`] (tell the
//! job, answer the agent, show the overlay) is the app's business.

use super::round::{Action, ActionError, Answer, Applied, Origin, Round, RoundId};

/// How a round ended without being answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ending {
    /// `agent.STOP_WAITING`: the agent stopped waiting; nothing is delivered.
    Abandoned,
    /// `job.ENDED` or `window.CLOSED`: every answer is discarded and the agent, if still there, is
    /// told so.
    Interrupted,
}

/// What a round came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Answered(Vec<Answer>),
    Abandoned,
    Interrupted,
}

impl From<Ending> for Outcome {
    fn from(ending: Ending) -> Self {
        match ending {
            Ending::Abandoned => Outcome::Abandoned,
            Ending::Interrupted => Outcome::Interrupted,
        }
    }
}

/// What the app must do after a change, in this order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// The round ended, however it ended: whoever waits for it gets `outcome`.
    Ended {
        id: RoundId,
        origin: Origin,
        outcome: Outcome,
    },
    /// `question-round.CLOSED`: the presented round ended.
    Closed { id: RoundId, origin: Origin },
    /// `question-round.OPENED`: the round is presented, and the project's window comes forward.
    Opened { id: RoundId, origin: Origin },
}

/// The queue of a project.
#[derive(Debug, Default)]
pub struct Queue {
    /// In order of submission; the presented round, when there is one, is the first of them
    /// (rounds are presented in order and a presented round never gives way).
    rounds: Vec<Round>,
    presented: Option<RoundId>,
}

impl Queue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.rounds.is_empty()
    }

    /// The round on screen.
    pub fn presented(&self) -> Option<&Round> {
        self.presented.and_then(|id| self.get(id))
    }

    pub fn get(&self, id: RoundId) -> Option<&Round> {
        self.rounds.iter().find(|round| round.id == id)
    }

    /// How many rounds wait behind the presented one.
    pub fn waiting(&self) -> usize {
        self.rounds.len() - usize::from(self.presented.is_some())
    }

    /// `agent.ASK_QUESTIONS` (and `developer.RESOLVE_DRIFT` for a prepared question): the round
    /// is presented when none is, else it waits behind the others.
    pub fn submit(&mut self, round: Round) -> Vec<Effect> {
        self.rounds.push(round);
        self.present_next()
    }

    fn present_next(&mut self) -> Vec<Effect> {
        if self.presented.is_some() {
            return Vec::new();
        }
        match self.rounds.first_mut() {
            Some(round) => {
                round.present();
                self.presented = Some(round.id);
                vec![Effect::Opened {
                    id: round.id,
                    origin: round.origin.clone(),
                }]
            }
            None => Vec::new(),
        }
    }

    /// What the developer does in a round. Only the presented round takes actions.
    pub fn act(&mut self, id: RoundId, action: Action) -> Result<Vec<Effect>, ActionError> {
        if self.get(id).is_none() {
            return Err(ActionError::NoSuchRound);
        }
        if self.presented != Some(id) {
            return Err(ActionError::NotPresented);
        }
        let round = self
            .rounds
            .iter_mut()
            .find(|round| round.id == id)
            .expect("found above");
        match round.apply(action)? {
            Applied::Changed => Ok(Vec::new()),
            Applied::Finished(answers) => Ok(self.end(id, Outcome::Answered(answers))),
        }
    }

    /// A round ends without an answer.
    pub fn finish(&mut self, id: RoundId, ending: Ending) -> Vec<Effect> {
        self.end(id, ending.into())
    }

    /// `job.ENDED`: every round of the job ends interrupted, the oldest first.
    pub fn interrupt_job(&mut self, job: &str) -> Vec<Effect> {
        let ids: Vec<RoundId> = self
            .rounds
            .iter()
            .filter(|round| round.origin.job() == Some(job))
            .map(|round| round.id)
            .collect();
        ids.into_iter()
            .flat_map(|id| self.end(id, Outcome::Interrupted))
            .collect()
    }

    /// `window.CLOSED`: every round of the project ends interrupted, none is presented after.
    pub fn interrupt_all(&mut self) -> Vec<Effect> {
        let mut effects = Vec::new();
        for round in std::mem::take(&mut self.rounds) {
            let was_presented = self.presented == Some(round.id);
            effects.push(Effect::Ended {
                id: round.id,
                origin: round.origin.clone(),
                outcome: Outcome::Interrupted,
            });
            if was_presented {
                effects.push(Effect::Closed {
                    id: round.id,
                    origin: round.origin,
                });
            }
        }
        self.presented = None;
        effects
    }

    fn end(&mut self, id: RoundId, outcome: Outcome) -> Vec<Effect> {
        let Some(position) = self.rounds.iter().position(|round| round.id == id) else {
            return Vec::new();
        };
        let round = self.rounds.remove(position);
        let mut effects = vec![Effect::Ended {
            id,
            origin: round.origin.clone(),
            outcome,
        }];
        if self.presented == Some(id) {
            self.presented = None;
            effects.push(Effect::Closed {
                id,
                origin: round.origin,
            });
            effects.extend(self.present_next());
        }
        effects
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::question::request::{Question, QuestionOption, RoundRequest};
    use crate::question::round::Position;

    fn request() -> RoundRequest {
        let option = |label: &str| QuestionOption {
            label: label.into(),
            description: "d".into(),
            details: None,
        };
        RoundRequest {
            questions: vec![Question {
                id: "q".into(),
                question: "Q".into(),
                context: None,
                options: vec![option("a"), option("b")],
                recommended: 0,
                multi: false,
            }],
        }
    }

    fn agent(id: RoundId, job: &str) -> Round {
        Round::new(id, Origin::Agent { job: job.into() }, request())
    }

    fn drift(id: RoundId) -> Round {
        Round::new(
            id,
            Origin::Drift {
                id: "d1d1d1d1".into(),
                drift: "D1".into(),
                title: "A drift".into(),
            },
            request(),
        )
    }

    fn opened(effects: &[Effect]) -> Vec<RoundId> {
        effects
            .iter()
            .filter_map(|e| {
                if let Effect::Opened { id, .. } = e {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect()
    }

    fn ended(effects: &[Effect]) -> Vec<(RoundId, Outcome)> {
        effects
            .iter()
            .filter_map(|e| {
                if let Effect::Ended { id, outcome, .. } = e {
                    Some((*id, outcome.clone()))
                } else {
                    None
                }
            })
            .collect()
    }

    #[test]
    fn the_first_round_is_presented_and_the_others_wait_first_in_first_out() {
        let mut queue = Queue::new();
        assert_eq!(opened(&queue.submit(agent(1, "job1"))), [1]);
        assert!(
            queue.submit(agent(2, "job2")).is_empty(),
            "never replaces the presented round"
        );
        assert!(
            queue.submit(drift(3)).is_empty(),
            "a prepared question of a Drift waits like the others"
        );
        assert_eq!(
            (queue.presented().map(|r| r.id), queue.waiting()),
            (Some(1), 2)
        );

        // Answering the first presents the oldest waiting one, then the next.
        assert!(queue
            .act(1, Action::Confirm { index: 0 })
            .unwrap()
            .is_empty());
        let effects = queue.act(1, Action::Send).unwrap();
        assert!(matches!(
            effects[0],
            Effect::Ended {
                id: 1,
                outcome: Outcome::Answered(_),
                ..
            }
        ));
        assert!(matches!(effects[1], Effect::Closed { id: 1, .. }));
        assert_eq!(opened(&effects), [2]);
        let effects = queue.act(2, Action::SkipAll).unwrap();
        assert_eq!(opened(&effects), [3]);
        assert_eq!(queue.waiting(), 0);
    }

    #[test]
    fn projects_never_wait_for_each_other() {
        let (mut one, mut other) = (Queue::new(), Queue::new());
        assert_eq!(opened(&one.submit(agent(1, "job1"))), [1]);
        assert_eq!(opened(&other.submit(agent(2, "job2"))), [2]);
    }

    #[test]
    fn a_round_that_is_not_presented_takes_no_action() {
        let mut queue = Queue::new();
        queue.submit(agent(1, "job1"));
        queue.submit(agent(2, "job2"));
        assert_eq!(
            queue.act(2, Action::SkipAll),
            Err(ActionError::NotPresented)
        );
        assert_eq!(queue.act(9, Action::SkipAll), Err(ActionError::NoSuchRound));
        assert_eq!(
            queue.act(1, Action::Go(Position::Question { index: 4 })),
            Err(ActionError::NoSuchQuestion(4))
        );
    }

    #[test]
    fn an_abandoned_waiting_round_leaves_the_presented_one_alone() {
        let mut queue = Queue::new();
        queue.submit(agent(1, "job1"));
        queue.submit(agent(2, "job2"));
        let effects = queue.finish(2, Ending::Abandoned);
        assert_eq!(
            effects.len(),
            1,
            "a round that never was presented emits no CLOSED: {effects:?}"
        );
        assert_eq!(ended(&effects), [(2, Outcome::Abandoned)]);
        assert_eq!(queue.presented().map(|r| r.id), Some(1));
    }

    #[test]
    fn an_abandoned_presented_round_closes_and_the_next_one_opens() {
        let mut queue = Queue::new();
        queue.submit(agent(1, "job1"));
        queue.submit(agent(2, "job2"));
        let effects = queue.finish(1, Ending::Abandoned);
        assert!(matches!(effects[1], Effect::Closed { id: 1, .. }));
        assert_eq!(opened(&effects), [2]);
        assert!(
            queue.finish(1, Ending::Abandoned).is_empty(),
            "a round that is gone ends nowhere"
        );
    }

    #[test]
    fn the_end_of_a_job_interrupts_its_rounds_and_nobody_else_s() {
        let mut queue = Queue::new();
        queue.submit(agent(1, "job1"));
        queue.submit(agent(2, "job2"));
        queue.submit(agent(3, "job1"));
        queue.submit(drift(4));
        let effects = queue.interrupt_job("job1");
        assert_eq!(
            ended(&effects),
            [(1, Outcome::Interrupted), (3, Outcome::Interrupted)]
        );
        assert_eq!(opened(&effects), [2], "the oldest round left is presented");
        assert_eq!(queue.waiting(), 1);
    }

    #[test]
    fn a_round_of_the_chat_is_no_job_s_and_its_opening_and_closing_name_the_chat() {
        let chat = Origin::Chat {
            project: "p1".into(),
        };
        assert_eq!((chat.job(), chat.chat()), (None, Some("p1")));
        let mut queue = Queue::new();
        queue.submit(agent(1, "job1"));
        let effects = queue.submit(Round::new(2, chat.clone(), request()));
        assert!(effects.is_empty(), "it waits behind the presented round");

        // No job's end touches it.
        let effects = queue.interrupt_job("job1");
        assert_eq!(ended(&effects), [(1, Outcome::Interrupted)]);
        assert!(effects.contains(&Effect::Opened {
            id: 2,
            origin: chat.clone()
        }));

        let effects = queue.finish(2, Ending::Abandoned);
        assert!(effects.contains(&Effect::Closed { id: 2, origin: chat }));
    }

    #[test]
    fn closing_the_window_interrupts_every_round_including_a_prepared_question() {
        let mut queue = Queue::new();
        queue.submit(agent(1, "job1"));
        queue.submit(drift(2));
        let effects = queue.interrupt_all();
        assert_eq!(
            ended(&effects),
            [(1, Outcome::Interrupted), (2, Outcome::Interrupted)]
        );
        assert_eq!(
            effects
                .iter()
                .filter(|e| matches!(e, Effect::Closed { .. }))
                .count(),
            1,
            "only the presented one emits CLOSED"
        );
        assert!(opened(&effects).is_empty());
        assert!(queue.is_empty() && queue.presented().is_none());
    }

    #[test]
    fn a_round_submitted_after_the_last_one_ended_is_presented_at_once() {
        let mut queue = Queue::new();
        queue.submit(agent(1, "job1"));
        queue.finish(1, Ending::Interrupted);
        assert_eq!(opened(&queue.submit(agent(2, "job1"))), [2]);
    }
}
