//! Proposals: the machine `proposal`, whole.
//!
//! A Proposal is the folder `.weaver/proposals/<id>-<slug>/` holding `proposal.md`, the full new
//! version of every file it changes (at the path it has under `.weaver/`) and the `tasks/` it
//! creates when approved. The agent writes it; the developer decides on it as a whole.
//!
//! Everything here is a pure function over the project image and the text of the Proposal
//! folders ([`files::Folders`], read once by the caller), plus a few small file readers. The
//! app and the `weaver` command line both go through it.
//!
//! | Module | Responsibility |
//! |--------|----------------|
//! | [`data`] | `proposal.md` keys, status and kind, rewriting the front matter |
//! | [`label`] | The label (1–30 characters, one line) and its colour (FNV-1a modulo 6) |
//! | [`files`] | The text inside Proposal folders; registered, held and removed paths |
//! | [`scope`] | Scope, coverage and the code-removal Tasks, computed live, never stored |
//! | [`analysis`] | Staleness, validity and every reason Approve or hand-over is refused |
//! | [`resolution`] | Decisions of a resolution on its Drifts |
//! | [`create`] | `proposal new` and `proposal edit` |
//! | [`transitions`] | Hand-over, approve, reject, rework, discard, refinement |
//! | [`models`] | The model chosen for a Proposal, kept in the app's local data |
//!
//! Hooks for the jobs of the agent (the Jobs Task):
//!
//! * a refinement job starts when [`transitions::plan_approve`] answers
//!   [`transitions::Approval::Refine`]; its agent rewrites the Proposal with `weaver proposal
//!   edit <id> {rebase: true, ...}` and hands it over again, and when the job ends without
//!   handing it over [`transitions::plan_refinement_ended`] sends the Proposal back to Draft;
//! * a rework job starts when [`transitions::plan_reject`] rejects with `rework`: it writes new
//!   Proposals with `weaver proposal new {reworks: <id>}`;
//! * a refined Proposal handed over while a Run works stays Refining
//!   ([`transitions::Submitted::WaitsForRun`]): the app hands it over again when the Run ends;
//! * a setup or import job records whether the Model had no Feature and no Actor when it
//!   reached Running, and says so in [`transitions::SubmitContext::job_found_empty_model`].
//! * the agent of Chat writes only changes and resolutions, on the developer's order: its Proposals
//!   carry `chat: true`, and `chatWriting: true` from `proposal new` or `proposal edit` until the
//!   hand-over ([`create::Writer`]); a turn of Chat keeps the agent finishing for them
//!   ([`crate::project::board::AgentFacts::is_finishing`]); the Clear of Chat discards the Drafts
//!   it created ([`transitions::clear_chat_drafts`]).

pub mod analysis;
pub mod create;
pub mod data;
pub mod files;
pub mod label;
pub mod models;
pub mod resolution;
pub mod scope;
pub mod transitions;

#[cfg(test)]
pub(crate) mod fixtures;

#[cfg(test)]
mod tests;
