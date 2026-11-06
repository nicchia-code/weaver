//! Jobs of the agent: the pure part (task "Lavori dell'agente").
//!
//! A job is a piece of work Weaver gives to omp: one process, one ACP session and one
//! conversation. Everything that can be decided without a process lives here, as functions over
//! text and pictures of the project with no I/O (the registry record is the one file): the app runs
//! the processes and the ACP session and asks these modules what to do.
//!
//! | Module | Responsibility |
//! |--------|----------------|
//! | [`kinds`] | The kinds, the roles and the states of a job |
//! | [`machine`] | The Machine `job`: events in, effects out |
//! | [`admission`] | Which job may start: Run, Run all by model with its sub-agent shares, release |
//! | [`names`] | The fixed names of the jobs |
//! | [`launch`] | The command line, the environment and the overlay of omp |
//! | [`notes`] | The notes of `weaver job edit` |
//! | [`record`] | The registry record `jobs/<job id>.json` and its retention |
//! | [`measure`] | The measure of an ended design, rework, refinement or Run: its line of `.weaver/timings/jobs.jsonl` |
//! | [`subagents`] | Sub-agents read from the `task` tool call and from omp's session files |
//! | [`general`] | General chat, which is not a job: the Machine `general-chat`, omp's command line, its record |
//! | [`ask`] | Ask more conversations, which are not jobs: the instruction of Weaver for the chosen thing |

pub mod admission;
pub mod ask;
pub mod general;
pub mod kinds;
pub mod launch;
pub mod machine;
pub mod measure;
pub mod names;
pub mod notes;
pub mod record;
pub mod subagents;

pub use kinds::{JobKind, JobState, Role};
pub use record::{AnnouncementRecord, FiledCard, JobRecord, JobStore};
