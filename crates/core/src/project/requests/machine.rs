//! The Machine `design-request`: which state a request is in and what each event does to it.
//!
//! [`step`] is pure: it takes the state and the event and answers the new state and the one
//! thing Weaver has to do about it ([`Effect`]). The store ([`super::store`]) reads the state
//! of a request from the files, calls [`step`] and carries out the effect, so the rules live
//! here and nowhere else.
//!
//! ```text
//! Not written --NEW_REQUEST--> Writing
//! Not written --agent SEND_REQUEST (not empty)--> Sent  creates the file with the agent's text and
//!                                                        starts a design job, or a setup job
//! Writing --SEND_REQUEST (not empty)--> Sent            starts a design job, or a setup job
//! Writing --DISCARD_REQUEST--> Discarded                deletes the file and the images
//! Writing | Sent --REQUEST_DELETED--> Discarded          nothing is ever sent afterwards
//! Sent --PROPOSAL_SUBMITTED--> Consumed                  only the first Proposal; deletes the files
//! Sent --JOB_ENDED (no Proposal handed over)--> Writing  the developer can change it and send it again
//! ```
//!
//! `Consumed` and `Discarded` are final: every event is ignored there.

/// The states of a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestState {
    /// No request exists yet (initial).
    NotWritten,
    /// The developer writes it; every change is saved at once.
    Writing,
    /// The agent works on it; it can be read, not sent again.
    Sent,
    /// A Proposal was born from it (final).
    Consumed,
    /// Thrown away without a Proposal (final).
    Discarded,
}

impl RequestState {
    pub fn is_final(self) -> bool {
        matches!(self, RequestState::Consumed | RequestState::Discarded)
    }
}

/// The events of the Machine (`developer.*`, `workspace.*`, `proposal.*`, `job.*`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestEvent {
    /// `developer.NEW_REQUEST`: the `+ Work Item` button or Start from Scratch.
    NewRequest,
    /// `developer.SEND_REQUEST`; `empty` is whether nothing is left after removing whitespace.
    SendRequest { empty: bool },
    /// `developer.DISCARD_REQUEST`.
    DiscardRequest,
    /// `workspace.REQUEST_DELETED`: the file is gone, deleted outside Weaver.
    RequestDeleted,
    /// `proposal.SUBMITTED` of a Proposal bound to the request.
    ProposalSubmitted,
    /// `job.ENDED` of the job that received the request; `proposal_handed_over` is whether a
    /// Proposal bound to the request was handed over.
    JobEnded { proposal_handed_over: bool },
    /// `agent.SEND_REQUEST`: the agent of Chat sends a request it wrote; `empty` is whether the text
    /// has nothing left after removing whitespace.
    AgentSendRequest { empty: bool },
}

/// What Weaver does because of an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    /// Nothing: the event does not apply (or must be ignored) in that state.
    None,
    /// Create the empty file of the request.
    CreateFile,
    /// Start the job (design, or setup when the Model is empty) and record the request as sent.
    StartJob,
    /// Create the file with the text the agent gave, start the job (design, or setup when the Model
    /// is empty) and record the request as sent; nothing is left behind when the job does not start.
    CreateAndStartJob,
    /// Delete `<id>.md` and `<id>.images/` in one step (discard, or consumption: see the state).
    DeleteFiles,
    /// The file is already gone: forget what Weaver recorded about the request (its job) and
    /// remove what is left of its images.
    ForgetRemains,
    /// Take the request out of the `sent` record: it is back in writing.
    ReturnToWriting,
}

/// The result of one event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Step {
    pub state: RequestState,
    pub effect: Effect,
}

/// Applies `event` to a request in `state`.
pub fn step(state: RequestState, event: RequestEvent) -> Step {
    use Effect as E;
    use RequestEvent as Ev;
    use RequestState as S;
    let stay = Step {
        state,
        effect: E::None,
    };
    match (state, event) {
        (S::NotWritten, Ev::NewRequest) => Step {
            state: S::Writing,
            effect: E::CreateFile,
        },

        (S::NotWritten, Ev::AgentSendRequest { empty: false }) => Step {
            state: S::Sent,
            effect: E::CreateAndStartJob,
        },
        // A text with nothing in it creates no request.
        (S::NotWritten, Ev::AgentSendRequest { empty: true }) => stay,

        (S::Writing, Ev::SendRequest { empty: false }) => Step {
            state: S::Sent,
            effect: E::StartJob,
        },
        // Sending something empty does nothing.
        (S::Writing, Ev::SendRequest { empty: true }) => stay,
        (S::Writing, Ev::DiscardRequest) => Step {
            state: S::Discarded,
            effect: E::DeleteFiles,
        },
        (S::Writing | S::Sent, Ev::RequestDeleted) => Step {
            state: S::Discarded,
            effect: E::ForgetRemains,
        },

        (S::Sent, Ev::ProposalSubmitted) => Step {
            state: S::Consumed,
            effect: E::DeleteFiles,
        },
        (
            S::Sent,
            Ev::JobEnded {
                proposal_handed_over: false,
            },
        ) => Step {
            state: S::Writing,
            effect: E::ReturnToWriting,
        },

        // Anything else: a request that was sent cannot be sent again, a request that is not written
        // receives nothing, and the final states never change.
        _ => stay,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Effect as E;
    use RequestEvent as Ev;
    use RequestState as S;

    fn is(state: S, event: Ev, to: S, effect: E) {
        assert_eq!(
            step(state, event),
            Step { state: to, effect },
            "{state:?} + {event:?}"
        );
    }

    #[test]
    fn a_new_request_is_created_and_opened_for_writing() {
        is(S::NotWritten, Ev::NewRequest, S::Writing, E::CreateFile);
    }

    #[test]
    fn sending_something_empty_does_nothing() {
        is(
            S::Writing,
            Ev::SendRequest { empty: true },
            S::Writing,
            E::None,
        );
        is(
            S::Writing,
            Ev::SendRequest { empty: false },
            S::Sent,
            E::StartJob,
        );
    }

    #[test]
    fn the_agent_of_chat_sends_a_request_it_creates_and_only_when_it_is_not_empty() {
        is(
            S::NotWritten,
            Ev::AgentSendRequest { empty: false },
            S::Sent,
            E::CreateAndStartJob,
        );
        is(
            S::NotWritten,
            Ev::AgentSendRequest { empty: true },
            S::NotWritten,
            E::None,
        );
        // An existing request is never sent by the agent.
        for state in [S::Writing, S::Sent] {
            is(state, Ev::AgentSendRequest { empty: false }, state, E::None);
        }
    }

    #[test]
    fn a_request_that_was_sent_is_not_sent_again() {
        is(S::Sent, Ev::SendRequest { empty: false }, S::Sent, E::None);
        is(S::Sent, Ev::DiscardRequest, S::Sent, E::None);
    }

    #[test]
    fn discarding_deletes_the_files() {
        is(S::Writing, Ev::DiscardRequest, S::Discarded, E::DeleteFiles);
    }

    #[test]
    fn a_file_deleted_from_outside_ends_a_request_in_writing_or_sent() {
        is(
            S::Writing,
            Ev::RequestDeleted,
            S::Discarded,
            E::ForgetRemains,
        );
        is(S::Sent, Ev::RequestDeleted, S::Discarded, E::ForgetRemains);
    }

    #[test]
    fn a_proposal_consumes_a_sent_request_and_only_the_first_one_does() {
        let first = step(S::Sent, Ev::ProposalSubmitted);
        assert_eq!(
            first,
            Step {
                state: S::Consumed,
                effect: E::DeleteFiles
            }
        );
        // The second Proposal bound to it finds a consumed request.
        is(first.state, Ev::ProposalSubmitted, S::Consumed, E::None);
        // A request still being written is not consumed by a Proposal.
        is(S::Writing, Ev::ProposalSubmitted, S::Writing, E::None);
    }

    #[test]
    fn a_job_that_ends_without_a_proposal_returns_the_request_to_writing() {
        is(
            S::Sent,
            Ev::JobEnded {
                proposal_handed_over: false,
            },
            S::Writing,
            E::ReturnToWriting,
        );
        is(
            S::Sent,
            Ev::JobEnded {
                proposal_handed_over: true,
            },
            S::Sent,
            E::None,
        );
        is(
            S::Writing,
            Ev::JobEnded {
                proposal_handed_over: false,
            },
            S::Writing,
            E::None,
        );
    }

    #[test]
    fn consumed_and_discarded_are_final() {
        let events = [
            Ev::NewRequest,
            Ev::SendRequest { empty: false },
            Ev::AgentSendRequest { empty: false },
            Ev::DiscardRequest,
            Ev::RequestDeleted,
            Ev::ProposalSubmitted,
            Ev::JobEnded {
                proposal_handed_over: false,
            },
        ];
        for state in [S::Consumed, S::Discarded] {
            assert!(state.is_final());
            for event in events {
                is(state, event, state, E::None);
            }
        }
    }
}
