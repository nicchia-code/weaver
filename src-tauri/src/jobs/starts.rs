//! Starting jobs: the decisions "Run", "Run all", "Scan System", "Import", "Resolve", "Rework",
//! "Refine" and "Design" turned into jobs (`requirements/rules#job`).
//!
//! Every function here checks its conditions again at the moment it runs, against the picture of
//! the project it is given and the facts about the agent's work now, and creates the jobs or
//! refuses with the sentence the developer reads. Called inside the project's queue, so that two
//! decisions never see the same facts: the second finds the Run the first started.

use std::path::Path;

use weaver_core::job_instruction::{Job, ProposalKind};
use weaver_core::jobs::admission::{self, Refusal, StartPlan};
use weaver_core::jobs::{names, JobKind};
use weaver_core::project::board::{self, AgentFacts};
use weaver_core::project::cards::{Card, CardKind};
use weaver_core::project::image::ProjectImage;
use weaver_core::project::proposal::data::{self, Kind};
use weaver_core::project::proposal::models::ProposalModels;
use weaver_core::project::shortnum::{self, ShortNumbers};

use super::registry::{Jobs, NewJob};

/// The project a start is for, as the decision found it.
pub struct Ctx<'a> {
    /// The project hash.
    pub project_id: &'a str,
    pub root: &'a Path,
    pub image: &'a ProjectImage,
}

impl Ctx<'_> {
    fn numbers(&self) -> ShortNumbers {
        shortnum::peek(self.root, self.image.cards())
    }
}

/// Something that may refuse a job before it is created (the prepared question of a Drift, for
/// example). Installed with [`Jobs::add_gate`].
pub trait StartGate: Send + Sync {
    /// The reason the job must not start, in a sentence for the developer.
    fn refuse(&self, image: &ProjectImage, plan: &StartPlan) -> Option<String>;
}

impl Jobs {
    pub fn add_gate(&self, gate: std::sync::Arc<dyn StartGate>) {
        self.hooks.gates.write().expect("gates poisoned").push(gate);
    }

    fn chosen_models(&self, project_id: &str) -> ProposalModels {
        ProposalModels::load(self.host.dirs(), project_id)
    }

    fn max_agents(&self) -> u32 {
        self.host.settings().max_parallel_agents.max(1)
    }

    /// The facts about the agent's work now, with the merge of Solo mode read from the project
    /// the start is for.
    fn facts_in(&self, ctx: &Ctx) -> AgentFacts {
        let mut facts = self.facts_of(ctx.project_id);
        facts.merge_in_progress = weaver_core::solo_merge::in_progress(ctx.root);
        facts
    }

    /// The checks every start shares, then the job.
    fn start(&self, ctx: &Ctx, plan: StartPlan) -> Result<String, Refusal> {
        let facts = self.facts_in(ctx);
        self.start_with(ctx, plan, &facts)
    }

    fn start_with(
        &self,
        ctx: &Ctx,
        plan: StartPlan,
        facts: &AgentFacts,
    ) -> Result<String, Refusal> {
        admission::check_start(plan.kind, ctx.image, facts)?;
        for gate in self.hooks.gates.read().expect("gates poisoned").iter() {
            if let Some(reason) = gate.refuse(ctx.image, &plan) {
                return Err(Refusal(reason));
            }
        }
        self.create(NewJob {
            project_id: ctx.project_id.to_owned(),
            root: ctx.root.to_path_buf(),
            plan,
        })
        .map_err(Refusal)
    }

    fn plan(
        kind: JobKind,
        name: String,
        job: Job,
        target: Option<String>,
        max_agents: u32,
    ) -> StartPlan {
        StartPlan {
            kind,
            name,
            job,
            target,
            task: None,
            proposal_model: None,
            max_agents,
            merged: None,
        }
    }

    /// `developer.RUN_TASK`.
    pub fn start_run_task(&self, ctx: &Ctx, task: &str) -> Result<String, Refusal> {
        let models = self.chosen_models(ctx.project_id);
        let chosen = |proposal: &str| models.get(proposal).map(str::to_owned);
        let mut plan = admission::plan_run_task(
            ctx.image,
            &ctx.numbers(),
            &self.facts_in(ctx),
            task,
            &chosen,
            self.max_agents(),
        )?;
        // A Task of no Proposal runs on the execution model.
        plan.proposal_model = plan.proposal_model.filter(|model| !model.is_empty());
        self.start(ctx, plan)
    }

    /// `developer.RUN_ALL`: one Run for each model the runnable Tasks need, started together. Returns
    /// the identities of the jobs.
    pub fn start_run_all(&self, ctx: &Ctx) -> Result<Vec<String>, Refusal> {
        let models = self.chosen_models(ctx.project_id);
        let chosen = |proposal: &str| models.get(proposal).map(str::to_owned);
        let execution = self.host.settings().models.execution;
        let plans = admission::plan_run_all(
            ctx.image,
            &self.facts_in(ctx),
            &chosen,
            &execution,
            self.max_agents(),
        )?;
        let mut started = Vec::new();
        for plan in plans {
            // The Runs of one Run all do not exclude each other: the check is for the first.
            started.push(self.start_unchecked_after_first(ctx, plan, started.is_empty())?);
        }
        Ok(started)
    }

    fn start_unchecked_after_first(
        &self,
        ctx: &Ctx,
        plan: StartPlan,
        first: bool,
    ) -> Result<String, Refusal> {
        if first {
            return self.start(ctx, plan);
        }
        self.create(NewJob {
            project_id: ctx.project_id.to_owned(),
            root: ctx.root.to_path_buf(),
            plan,
        })
        .map_err(Refusal)
    }

    /// `developer.RUN_ALL` after an approved setup: the same as the developer pressing `Run all`;
    /// nothing to run is no error here.
    pub fn start_run_all_after_setup(&self, ctx: &Ctx) -> Vec<String> {
        self.start_run_all(ctx).unwrap_or_default()
    }

    /// `developer.SCAN_SYSTEM`.
    pub fn start_scan(&self, ctx: &Ctx) -> Result<String, Refusal> {
        self.scan(ctx, Job::Scan, None)
    }

    /// `solo-merge.CONCLUDED`: the scan of the whole System that Weaver starts by itself, with no
    /// confirmation, through the path of `Scan System`. `merged` are the files the merge brought
    /// (relative to the project root with `/`, sorted); they go into the job's record, where
    /// `weaver job show` reads them. The merge must already be forgotten (`merge.json` deleted),
    /// or the scan is refused.
    pub fn start_scan_after_merge(
        &self,
        ctx: &Ctx,
        merged: Vec<String>,
    ) -> Result<String, Refusal> {
        self.scan(ctx, Job::ScanAfterMerge, Some(merged))
    }

    fn scan(&self, ctx: &Ctx, job: Job, merged: Option<Vec<String>>) -> Result<String, Refusal> {
        let mut plan = Self::plan(
            JobKind::Scan,
            names::SCAN.to_owned(),
            job,
            None,
            self.max_agents(),
        );
        plan.merged = merged;
        self.start(ctx, plan)
    }

    /// `developer.RESOLVE_MERGE_CONFLICTS`: a `merge` job on the merge in progress. `conflicts` is
    /// the number of files that still have conflicts, as git has just told the caller; the job is
    /// refused when it is 0, when no merge is in progress or when another `merge` job of the
    /// project has not ended.
    pub fn start_merge(&self, ctx: &Ctx, conflicts: u32) -> Result<String, Refusal> {
        let mut facts = self.facts_in(ctx);
        facts.merge_conflicts = conflicts > 0;
        self.start_with(
            ctx,
            Self::plan(
                JobKind::Merge,
                names::MERGE.to_owned(),
                Job::MergeConflicts,
                None,
                self.max_agents(),
            ),
            &facts,
        )
    }

    /// `developer.IMPORT_CODE`, leaving out the folders the developer excluded.
    pub fn start_import(&self, ctx: &Ctx, excluded: Vec<String>) -> Result<String, Refusal> {
        self.start(
            ctx,
            Self::plan(
                JobKind::Import,
                names::IMPORT.to_owned(),
                Job::Import { excluded },
                None,
                self.max_agents(),
            ),
        )
    }

    /// `developer.SEND_REQUEST` on a Model that has a Feature or an Actor: a design job on the
    /// written request; `assets` are the assets the request tags.
    pub fn start_design(
        &self,
        ctx: &Ctx,
        request: &str,
        assets: Vec<String>,
    ) -> Result<String, Refusal> {
        self.start(
            ctx,
            Self::plan(
                JobKind::Design,
                names::DESIGN.to_owned(),
                Job::Design {
                    request: request.to_owned(),
                    assets,
                },
                Some(request.to_owned()),
                self.max_agents(),
            ),
        )
    }

    /// `developer.SEND_REQUEST` on an empty Model: a setup job; `assets` are the assets the
    /// request tags.
    pub fn start_setup(
        &self,
        ctx: &Ctx,
        request: &str,
        assets: Vec<String>,
    ) -> Result<String, Refusal> {
        self.start(
            ctx,
            Self::plan(
                JobKind::Setup,
                names::SETUP.to_owned(),
                Job::Setup {
                    request: request.to_owned(),
                    assets,
                },
                Some(request.to_owned()),
                self.max_agents(),
            ),
        )
    }

    /// `developer.RESOLVE_DRIFT` (and `question-round.ANSWERED` with the answer given): a resolve
    /// job on an open Drift.
    pub fn start_resolve(
        &self,
        ctx: &Ctx,
        drift: &str,
        answer: Option<weaver_core::job_instruction::ResolveAnswer>,
    ) -> Result<String, Refusal> {
        let label = names::card_label(&ctx.numbers(), CardKind::Drift, drift);
        match ctx.image.card(CardKind::Drift, drift) {
            None => return Err(Refusal(format!("{label} no longer exists"))),
            Some(card) if card.str_key("status") != Some("open") => {
                return Err(Refusal(format!("{label} is no longer open")))
            }
            Some(_) => {}
        }
        let name = names::resolve(&ctx.numbers(), drift);
        self.start(
            ctx,
            Self::plan(
                JobKind::Resolve,
                name,
                Job::Resolve {
                    drift: drift.to_owned(),
                    answer,
                },
                Some(drift.to_owned()),
                self.max_agents(),
            ),
        )
    }

    /// The kind of a Proposal as the instruction needs it, for a rework or a refinement: for a
    /// resolution, its Drifts still open, `None` when no Drift is.
    fn proposal_kind(ctx: &Ctx, card: &Card) -> Option<ProposalKind> {
        Some(match data::kind_of(card) {
            Kind::Change => ProposalKind::Change,
            Kind::Setup => {
                let model = ctx.image.model();
                ProposalKind::Setup {
                    model_is_empty: model.features.is_empty() && model.actors.is_empty(),
                }
            }
            Kind::Import => ProposalKind::Import,
            Kind::Resolution => {
                let open: Vec<String> = board::resolution_rows(card)
                    .into_iter()
                    .filter(|row| {
                        ctx.image
                            .card(CardKind::Drift, &row.drift)
                            .is_some_and(|d| d.str_key("status") == Some("open"))
                    })
                    .map(|row| row.drift)
                    .collect();
                if open.is_empty() {
                    return None;
                }
                ProposalKind::Resolution { open_drifts: open }
            }
        })
    }

    /// `developer.REWORK_PROPOSAL`: a rework job with the skill of the Proposal's kind; for a
    /// resolution only on its Drifts still open, and none when no Drift is open.
    pub fn start_rework(&self, ctx: &Ctx, proposal: &str) -> Result<Option<String>, Refusal> {
        let Some(card) = ctx.image.card(CardKind::Proposal, proposal) else {
            return Err(Refusal(format!("{proposal} no longer exists")));
        };
        let Some(kind) = Self::proposal_kind(ctx, card) else {
            return Ok(None);
        };
        let name = names::rework(&ctx.numbers(), proposal);
        let mut plan = Self::plan(
            JobKind::Rework,
            name,
            Job::Rework {
                proposal: proposal.to_owned(),
                kind,
            },
            Some(proposal.to_owned()),
            self.max_agents(),
        );
        plan.proposal_model = self
            .chosen_models(ctx.project_id)
            .get(proposal)
            .map(str::to_owned);
        self.start(ctx, plan).map(Some)
    }

    /// A refinement job on a stale Proposal, with the skill of its kind as for a rework; for a
    /// resolution only on its Drifts still open. Started by `developer.APPROVE_PROPOSAL` when the
    /// Proposal approved must be refined first, and by `developer.REFINE_DRIFT_PROPOSAL` on the
    /// Proposal attached to a Drift. A resolution with no Drift still open is refused (its
    /// approval is refused before anything is written, so this is only a second check).
    pub fn start_refinement(&self, ctx: &Ctx, proposal: &str) -> Result<String, Refusal> {
        let Some(card) = ctx.image.card(CardKind::Proposal, proposal) else {
            return Err(Refusal(format!("{proposal} no longer exists")));
        };
        let Some(kind) = Self::proposal_kind(ctx, card) else {
            return Err(Refusal("None of its Drifts is still open".to_owned()));
        };
        let name = names::refine(&ctx.numbers(), proposal);
        let mut plan = Self::plan(
            JobKind::Refinement,
            name,
            Job::Refinement {
                proposal: proposal.to_owned(),
                kind,
            },
            Some(proposal.to_owned()),
            self.max_agents(),
        );
        plan.proposal_model = self
            .chosen_models(ctx.project_id)
            .get(proposal)
            .map(str::to_owned);
        self.start(ctx, plan)
    }
}
