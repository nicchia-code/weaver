//! Jobs of the agent (task "Lavori dell'agente"): the work Weaver gives to omp.
//!
//! A job is one omp process (ACP server over stdio), one ACP session and one conversation. The
//! pure rules (the Machine `job`, who may start, the command line of omp, the registry record) are
//! in `weaver_core::jobs`; this module runs them.
//!
//! | Module | Responsibility |
//! |--------|----------------|
//! | [`registry`] | [`Jobs`]: the jobs in memory, the Machine of each, the registry, the windows' hooks |
//! | [`actor`] | The task that owns a job's omp process and ACP session |
//! | [`session`] | The ACP calls (initialize, new/load, prompt, steering) and their pure decisions |
//! | [`proc`] | The omp process: group, stderr tail, ordered shutdown |
//! | [`rpc`] | JSON-RPC over the standard streams |
//! | [`conversation`] | What the Jobs view shows, built from `session/update` |
//! | [`starts`] | Run, Run all, Scan, Import, Resolve, Rework, Refine, Design, Setup |
//! | [`authority`] | Who may run a `weaver` command, `job` and `announcement` commands, hooks |
//! | [`commands`] | The Tauri commands |
//! | [`bridges`] | Board facts and ghosts, the windows' hook, the Proposals' hook, [`install`] |
//! | [`host`] | What the jobs need from the app, behind one trait |
//! | [`wire`] | Rows, snapshots, observers |
//!
//! # Where other parts plug in
//!
//! * `job.WAITING` / `job.ENDED`: [`Jobs::observe`] with a [`wire::JobObserver`] (notifications,
//!   question rounds, design requests, Proposals).
//! * Question rounds: [`Jobs::round_opened`] / [`Jobs::round_closed`] (only a round of this job),
//!   and `weaver question new` as a [`authority::CommandExtension`] ([`Jobs::add_extension`]).
//! * The working General: [`Jobs::set_general`]. Design requests: [`Jobs::start_design`],
//!   [`Jobs::start_setup`], and [`Jobs::set_request_sender`] for `job new design`.
//! * Import: [`Jobs::start_import`]. Resolve: [`Jobs::start_resolve`] and [`Jobs::add_gate`] for
//!   the prepared question of a Drift.

pub mod actor;
pub mod authority;
pub mod bridges;
pub mod commands;
pub mod conversation;
pub mod host;
pub mod proc;
pub mod registry;
pub mod rpc;
pub mod session;
pub mod starts;
pub mod wire;

#[cfg(test)]
pub(crate) mod testing;
#[cfg(test)]
mod measure_tests;
#[cfg(test)]
mod tests;

pub use bridges::{app_authority, install, install_windows};
pub use registry::Jobs;
