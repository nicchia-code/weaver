//! Which job may start (`requirements/rules#job`, the state Not started of the Machine `job`).
//!
//! Only one Run works in a project (Run all counts as one Run even when it starts one per
//! model), a Run and a resolve exclude each other, and designs, reworks and refinements always
//! start. While a merge of Solo mode is in progress, Runs, Run all, resolves, scans and imports
//! are refused, and the `merge` job starts only on a merge with files in conflict, one at a time.
//! The functions are pure: they read the picture of the project and the facts about the agent's
//! work, and answer with the jobs to create or with the reasons for the refusal. The decision is
//! made again, with fresh facts, when it is applied.

use crate::job_instruction::{Job, RunAllFilter};
use crate::jobs::kinds::JobKind;
use crate::jobs::names;
use crate::project::board::{self, AgentFacts};
use crate::project::cards::{Card, CardKind};
use crate::project::image::ProjectImage;
use crate::project::shortnum::ShortNumbers;
use crate::project::tasks::{self, BlockingIndex, Column};

/// Why a job does not start: the sentence the developer reads, reasons joined.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal(pub String);

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A job to create.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartPlan {
    pub kind: JobKind,
    /// The fixed name.
    pub name: String,
    /// What the instruction names.
    pub job: Job,
    /// What it works on: a Task, a Drift, a Proposal or a request identity.
    pub target: Option<String>,
    /// The Task of a Run of one Task (`WEAVER_TASK`).
    pub task: Option<String>,
    /// The model the developer chose for the Proposal, when it replaces the role's model.
    pub proposal_model: Option<String>,
    /// The sub-agent limit of the job: the setting, or its share of a Run all.
    pub max_agents: u32,
    /// Only for a scan after a merge: the files the merge brought (`JobRecord::merged`).
    pub merged: Option<Vec<String>>,
}

/// Reason of the `merge` job when no merge of Solo mode is in progress.
pub const REASON_NO_MERGE: &str = "No merge is in progress";
/// Reason of the `merge` job when no file of the merge has conflicts left.
pub const REASON_NO_CONFLICTS: &str = "No file has conflicts";
/// Reason of the `merge` job while another one has not ended.
pub const REASON_MERGE_JOB_UNENDED: &str = "The agent is resolving the conflicts";

/// Whether a job of `kind` may start now, given the facts about the work of the agent. Designs,
/// setups, reworks and refinements always may; scans and imports only while no merge is in
/// progress; a Run also excludes a resolve and a second Run, and a resolve a Run; the `merge` job
/// needs a merge with conflicts and no other `merge` job unended.
pub fn check_start(kind: JobKind, image: &ProjectImage, facts: &AgentFacts) -> Result<(), Refusal> {
    let mut reasons = Vec::new();
    match kind {
        JobKind::Run => {
            if facts.merge_in_progress {
                reasons.push(board::REASON_MERGE_IN_PROGRESS);
            }
            if board::drifts_resolving(image, facts) {
                reasons.push(board::REASON_DRIFTS_RESOLVING);
            }
            if facts.run_in_progress {
                reasons.push(board::REASON_RUN_IN_PROGRESS);
            }
        }
        JobKind::Resolve => {
            if facts.run_in_progress {
                reasons.push(board::REASON_RUN_IN_PROGRESS);
            }
            if facts.merge_in_progress {
                reasons.push(board::REASON_MERGE_IN_PROGRESS);
            }
        }
        JobKind::Scan | JobKind::Import if facts.merge_in_progress => {
            reasons.push(board::REASON_MERGE_IN_PROGRESS);
        }
        JobKind::Merge => {
            if !facts.merge_in_progress {
                reasons.push(REASON_NO_MERGE);
            } else if !facts.merge_conflicts {
                reasons.push(REASON_NO_CONFLICTS);
            }
            if facts.merge_job_unended {
                reasons.push(REASON_MERGE_JOB_UNENDED);
            }
        }
        _ => {}
    }
    refusal(&reasons)
}

fn refusal(reasons: &[&str]) -> Result<(), Refusal> {
    if reasons.is_empty() {
        Ok(())
    } else {
        Err(Refusal(reasons.join("; ")))
    }
}

/// The model chosen for a Proposal: `None` when none was chosen or it is empty.
fn chosen(chosen_model: &dyn Fn(&str) -> Option<String>, proposal: &str) -> Option<String> {
    chosen_model(proposal)
        .map(|m| m.trim().to_owned())
        .filter(|m| !m.is_empty())
}

/// `developer.RUN_TASK`: a Run of one Task. Refused when the Task is not in To do, is blocked or
/// held, or when another Run or a resolve works; every reason is listed, in the order of the
/// control (`tasks::run_reasons`).
pub fn plan_run_task(
    image: &ProjectImage,
    numbers: &ShortNumbers,
    facts: &AgentFacts,
    task_id: &str,
    chosen_model: &dyn Fn(&str) -> Option<String>,
    max_agents: u32,
) -> Result<StartPlan, Refusal> {
    let label = names::card_label(numbers, CardKind::Task, task_id);
    let Some(task) = image.card(CardKind::Task, task_id) else {
        return Err(Refusal(format!("{label} no longer exists")));
    };
    if tasks::column_of(task) != Some(Column::Todo) {
        return Err(Refusal(format!("{label} is no longer in To do")));
    }
    let blocking = BlockingIndex::new(image).of(task);
    let name = |kind: CardKind, id: &str| names::card_label(numbers, kind, id);
    let reasons = tasks::run_reasons(image, facts, task, &blocking, &name);
    if !reasons.is_empty() {
        return Err(Refusal(reasons.join("; ")));
    }
    Ok(StartPlan {
        kind: JobKind::Run,
        name: names::run_task(numbers, task_id),
        job: Job::RunTask {
            task: task_id.to_owned(),
        },
        target: Some(task_id.to_owned()),
        task: Some(task_id.to_owned()),
        proposal_model: task
            .str_key("proposal")
            .and_then(|p| chosen(chosen_model, p)),
        max_agents,
        merged: None,
    })
}

/// `developer.RUN_ALL`: a Run of the Tasks in To do that are not blocked, one Run for each model
/// they need (the model chosen for their Proposal, else the execution model), started together.
/// The sub-agent limit is shared among the Runs in proportion to their Tasks, at least 1 each.
/// Refused when no Task can run, or when another Run or a resolve works.
pub fn plan_run_all(
    image: &ProjectImage,
    facts: &AgentFacts,
    chosen_model: &dyn Fn(&str) -> Option<String>,
    execution_model: &str,
    max_agents: u32,
) -> Result<Vec<StartPlan>, Refusal> {
    let reasons = board::compute(image, facts).run_all_reasons;
    refusal(&reasons)?;
    let execution = execution_model.trim();
    let groups = tasks::run_all_groups(image, chosen_model, execution);
    if groups.is_empty() {
        return Err(Refusal(board::REASON_ALL_BLOCKED.to_owned()));
    }
    let counts: Vec<usize> = groups.iter().map(|g| g.tasks.len()).collect();
    let shares = tasks::split_sub_agents(max_agents as usize, &counts);

    // The Proposals of the Tasks in To do that carry a chosen model, in card order.
    let mut with_model: Vec<(String, String)> = Vec::new();
    for task in tasks::tasks_in(image, Column::Todo) {
        let Some(proposal) = task.str_key("proposal") else {
            continue;
        };
        if with_model.iter().any(|(id, _)| id == proposal) {
            continue;
        }
        if let Some(model) = chosen(chosen_model, proposal) {
            with_model.push((proposal.to_owned(), model));
        }
    }

    let several = groups.len() > 1;
    Ok(groups
        .iter()
        .zip(shares)
        .map(|(group, share)| {
            let is_execution = group.model == execution;
            let filter = if is_execution {
                let others: Vec<String> = with_model
                    .iter()
                    .filter(|(_, model)| model != execution)
                    .map(|(id, _)| id.clone())
                    .collect();
                if others.is_empty() {
                    RunAllFilter::All
                } else {
                    RunAllFilter::NotProposals(others)
                }
            } else {
                RunAllFilter::Proposals(
                    with_model
                        .iter()
                        .filter(|(_, model)| *model == group.model)
                        .map(|(id, _)| id.clone())
                        .collect(),
                )
            };
            StartPlan {
                kind: JobKind::Run,
                name: names::run_all(several.then_some(group.model.as_str())),
                job: Job::RunAll { filter },
                target: None,
                task: None,
                proposal_model: (!is_execution).then(|| group.model.clone()),
                max_agents: u32::try_from(share).unwrap_or(u32::MAX).max(1),
                merged: None,
            }
        })
        .collect())
}

/// The Tasks a job held go back to To do when it ends, whatever the reason (`job.ENDED`).
pub fn plan_release(image: &ProjectImage, job: &str) -> crate::project::changes::ChangeSet {
    tasks::plan_job_ended(image, job)
}

/// The Tasks that a Run-all group would take, for the tests and the Board: the identities in card
/// order.
pub fn group_tasks<'a>(
    image: &'a ProjectImage,
    chosen_model: &dyn Fn(&str) -> Option<String>,
    execution_model: &str,
) -> Vec<Vec<&'a Card>> {
    tasks::run_all_groups(image, chosen_model, execution_model)
        .into_iter()
        .map(|g| g.tasks)
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::Path;

    use super::*;
    use crate::project::cards::CardKind;
    use crate::project::shortnum::{self};
    use crate::project::testing::put;

    fn at(n: u32) -> String {
        format!("2026-11-02T07:00:{n:02}.000Z")
    }

    fn task(root: &Path, id: &str, n: u32, column: &str, extra: &str) {
        put(
            root,
            &format!("tasks/{id}-t.md"),
            &format!("---\ncolumn: {column}\ntechnical: true\nrefs: []\ncreated: {}\n{extra}---\n# Task {id}\n\nBody {id}.\n", at(n)),
        );
    }

    fn setup(build: impl FnOnce(&Path)) -> (tempfile::TempDir, ProjectImage, ShortNumbers) {
        let dir = tempfile::tempdir().unwrap();
        build(dir.path());
        let image = ProjectImage::load(dir.path());
        let numbers = shortnum::peek(dir.path(), image.cards());
        (dir, image, numbers)
    }

    fn no_model(_: &str) -> Option<String> {
        None
    }

    fn facts() -> AgentFacts {
        AgentFacts::default()
    }

    #[test]
    fn scans_designs_and_imports_always_start_and_a_run_and_a_resolve_exclude_each_other() {
        let (_dir, image, _) = setup(|_| {});
        let run = AgentFacts {
            run_in_progress: true,
            ..facts()
        };
        let resolve = AgentFacts {
            resolve_working: true,
            ..facts()
        };
        for kind in [
            JobKind::Scan,
            JobKind::Design,
            JobKind::Setup,
            JobKind::Import,
            JobKind::Rework,
            JobKind::Refinement,
        ] {
            for f in [&facts(), &run, &resolve] {
                assert_eq!(check_start(kind, &image, f), Ok(()), "{kind:?}");
            }
        }
        assert_eq!(check_start(JobKind::Run, &image, &facts()), Ok(()));
        assert_eq!(check_start(JobKind::Resolve, &image, &facts()), Ok(()));
        assert_eq!(
            check_start(JobKind::Run, &image, &run),
            Err(Refusal("A run is in progress".into()))
        );
        assert_eq!(
            check_start(JobKind::Run, &image, &resolve),
            Err(Refusal("Drifts are being resolved".into()))
        );
        assert_eq!(
            check_start(JobKind::Resolve, &image, &run),
            Err(Refusal("A run is in progress".into()))
        );
        assert_eq!(
            check_start(JobKind::Resolve, &image, &resolve),
            Ok(()),
            "two resolves may work together"
        );
        let both = AgentFacts {
            run_in_progress: true,
            resolve_working: true,
            ..facts()
        };
        assert_eq!(
            check_start(JobKind::Run, &image, &both).unwrap_err().0,
            "Drifts are being resolved; A run is in progress"
        );
    }

    #[test]
    fn a_merge_in_progress_refuses_runs_resolves_scans_and_imports_but_not_designs() {
        let (_dir, image, _) = setup(|_| {});
        let merging = AgentFacts {
            merge_in_progress: true,
            merge_conflicts: true,
            ..facts()
        };
        for kind in [
            JobKind::Run,
            JobKind::Resolve,
            JobKind::Scan,
            JobKind::Import,
        ] {
            assert_eq!(
                check_start(kind, &image, &merging),
                Err(Refusal("A merge is in progress".into())),
                "{kind:?}"
            );
        }
        for kind in [
            JobKind::Design,
            JobKind::Setup,
            JobKind::Rework,
            JobKind::Refinement,
        ] {
            assert_eq!(check_start(kind, &image, &merging), Ok(()), "{kind:?}");
        }
        let run = AgentFacts {
            run_in_progress: true,
            ..merging.clone()
        };
        assert_eq!(
            check_start(JobKind::Run, &image, &run).unwrap_err().0,
            "A merge is in progress; A run is in progress"
        );
        assert_eq!(
            check_start(JobKind::Resolve, &image, &run).unwrap_err().0,
            "A run is in progress; A merge is in progress",
            "the order of the Resolve control"
        );
    }

    #[test]
    fn the_merge_job_needs_a_merge_with_conflicts_and_no_other_merge_job_unended() {
        let (_dir, image, _) = setup(|_| {});
        let conflicts = AgentFacts {
            merge_in_progress: true,
            merge_conflicts: true,
            ..facts()
        };
        assert_eq!(check_start(JobKind::Merge, &image, &conflicts), Ok(()));
        assert_eq!(
            check_start(JobKind::Merge, &image, &facts()),
            Err(Refusal("No merge is in progress".into()))
        );
        let resolved = AgentFacts {
            merge_conflicts: false,
            ..conflicts.clone()
        };
        assert_eq!(
            check_start(JobKind::Merge, &image, &resolved),
            Err(Refusal("No file has conflicts".into()))
        );
        let working = AgentFacts {
            merge_job_unended: true,
            ..conflicts
        };
        assert_eq!(
            check_start(JobKind::Merge, &image, &working),
            Err(Refusal("The agent is resolving the conflicts".into()))
        );
    }

    #[test]
    fn a_resolution_being_written_keeps_a_run_out() {
        let (_dir, image, _) = setup(|root| {
            put(
                root,
                "proposals/rrrrrrrr-r/proposal.md",
                &format!(
                    "---\nstatus: draft\nkind: resolution\ncreated: {}\n---\n# R\n",
                    at(1)
                ),
            );
        });
        assert_eq!(
            check_start(JobKind::Run, &image, &facts()).unwrap_err().0,
            "Drifts are being resolved"
        );
        assert_eq!(check_start(JobKind::Resolve, &image, &facts()), Ok(()));
    }

    #[test]
    fn a_run_of_one_task_is_named_by_its_number_and_takes_the_proposals_model() {
        let (_dir, image, numbers) = setup(|root| {
            task(root, "aaaaaaaa", 1, "todo", "proposal: pppppppp\n");
            task(root, "bbbbbbbb", 2, "todo", "");
        });
        let chosen = |p: &str| (p == "pppppppp").then(|| " openai/gpt-5.2:high ".to_owned());
        let plan = plan_run_task(&image, &numbers, &facts(), "aaaaaaaa", &chosen, 4).unwrap();
        assert_eq!(plan.name, "Run T1");
        assert_eq!(
            plan.job,
            Job::RunTask {
                task: "aaaaaaaa".into()
            }
        );
        assert_eq!(
            (plan.task.as_deref(), plan.target.as_deref()),
            (Some("aaaaaaaa"), Some("aaaaaaaa"))
        );
        assert_eq!(plan.proposal_model.as_deref(), Some("openai/gpt-5.2:high"));
        assert_eq!(plan.max_agents, 4);
        let plain = plan_run_task(&image, &numbers, &facts(), "bbbbbbbb", &chosen, 4).unwrap();
        assert_eq!(plain.proposal_model, None);
    }

    #[test]
    fn a_run_of_one_task_is_refused_with_every_reason_in_the_order_of_the_control() {
        let (_dir, image, numbers) = setup(|root| {
            task(root, "aaaaaaaa", 1, "todo", "job: jjjjjjjj\n");
            task(root, "bbbbbbbb", 2, "todo", "");
            task(root, "cccccccc", 3, "doing", "holder: jjjjjjjj\n");
            put(
                root,
                "drifts/dddddddd-d.md",
                &format!(
                    "---\nstatus: open\nrefs: []\norigin: bbbbbbbb\ncreated: {}\n---\n# Blocks b\n",
                    at(4)
                ),
            );
        });
        let finishing = AgentFacts {
            working_jobs: ["jjjjjjjj".to_owned()].into(),
            run_in_progress: true,
            ..facts()
        };
        let error =
            plan_run_task(&image, &numbers, &finishing, "aaaaaaaa", &no_model, 4).unwrap_err();
        assert_eq!(
            error.0,
            "The agent is still finishing; A run is in progress"
        );
        assert_eq!(
            plan_run_task(&image, &numbers, &facts(), "bbbbbbbb", &no_model, 4)
                .unwrap_err()
                .0,
            "Blocked by Drift D1"
        );
        let both = AgentFacts {
            run_in_progress: true,
            ..facts()
        };
        assert_eq!(
            plan_run_task(&image, &numbers, &both, "bbbbbbbb", &no_model, 4)
                .unwrap_err()
                .0,
            "A run is in progress; Blocked by Drift D1"
        );
        // Not in To do any longer, or gone.
        assert_eq!(
            plan_run_task(&image, &numbers, &facts(), "cccccccc", &no_model, 4)
                .unwrap_err()
                .0,
            "T3 is no longer in To do"
        );
        assert_eq!(
            plan_run_task(&image, &numbers, &facts(), "zzzzzzzz", &no_model, 4)
                .unwrap_err()
                .0,
            "zzzzzzzz no longer exists"
        );
    }

    #[test]
    fn run_all_is_one_run_when_every_task_uses_the_execution_model() {
        let (_dir, image, _) = setup(|root| {
            task(root, "aaaaaaaa", 1, "todo", "");
            task(root, "bbbbbbbb", 2, "todo", "");
        });
        let plans = plan_run_all(&image, &facts(), &no_model, "exec/m", 6).unwrap();
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].name, "All tasks");
        assert_eq!(
            plans[0].job,
            Job::RunAll {
                filter: RunAllFilter::All
            }
        );
        assert_eq!(
            (
                plans[0].proposal_model.as_deref(),
                plans[0].max_agents,
                plans[0].target.clone()
            ),
            (None, 6, None)
        );
    }

    #[test]
    fn run_all_starts_one_run_per_model_and_shares_the_sub_agent_limit() {
        let (_dir, image, _) = setup(|root| {
            // Oldest first: a default Task, three of P1 (model A), one of P2 (model B), one default.
            task(root, "aaaaaaaa", 1, "todo", "");
            task(root, "bbbbbbbb", 2, "todo", "proposal: p1111111\n");
            task(root, "cccccccc", 3, "todo", "proposal: p1111111\n");
            task(root, "dddddddd", 4, "todo", "proposal: p1111111\n");
            task(root, "eeeeeeee", 5, "todo", "proposal: p2222222\n");
            task(root, "ffffffff", 6, "todo", "");
            task(root, "gggggggg", 7, "doing", "holder: jjjjjjjj\n");
        });
        let models: BTreeMap<&str, &str> =
            BTreeMap::from([("p1111111", "a/model"), ("p2222222", " b/model:low ")]);
        let chosen = |p: &str| models.get(p).map(|m| (*m).to_owned());
        let plans = plan_run_all(&image, &facts(), &chosen, " exec/m ", 10).unwrap();
        assert_eq!(plans.len(), 3);
        let view: Vec<(&str, &Job, Option<&str>, u32)> = plans
            .iter()
            .map(|p| {
                (
                    p.name.as_str(),
                    &p.job,
                    p.proposal_model.as_deref(),
                    p.max_agents,
                )
            })
            .collect();
        // Groups in the order of their oldest Task; 2 + 3 + 1 Tasks share 10 sub-agents (at least 1 each).
        assert_eq!(
            view,
            [
                (
                    "All tasks · exec/m",
                    &Job::RunAll {
                        filter: RunAllFilter::NotProposals(vec![
                            "p1111111".into(),
                            "p2222222".into()
                        ])
                    },
                    None,
                    3
                ),
                (
                    "All tasks · a/model",
                    &Job::RunAll {
                        filter: RunAllFilter::Proposals(vec!["p1111111".into()])
                    },
                    Some("a/model"),
                    5
                ),
                (
                    "All tasks · b/model:low",
                    &Job::RunAll {
                        filter: RunAllFilter::Proposals(vec!["p2222222".into()])
                    },
                    Some("b/model:low"),
                    2
                ),
            ]
        );
        assert_eq!(plans.iter().map(|p| p.max_agents).sum::<u32>(), 10);
        assert!(plans
            .iter()
            .all(|p| p.kind == JobKind::Run && p.task.is_none()));
    }

    #[test]
    fn a_group_that_needs_a_model_alone_has_a_name_with_it_and_every_group_gets_at_least_one() {
        let (_dir, image, _) = setup(|root| {
            task(root, "aaaaaaaa", 1, "todo", "proposal: p1111111\n");
            task(root, "bbbbbbbb", 2, "todo", "proposal: p2222222\n");
            task(root, "cccccccc", 3, "todo", "proposal: p3333333\n");
        });
        let chosen = |p: &str| Some(format!("model-{p}"));
        let plans = plan_run_all(&image, &facts(), &chosen, "", 2).unwrap();
        assert_eq!(plans.len(), 3);
        assert!(
            plans.iter().all(|p| p.max_agents >= 1),
            "{:?}",
            plans.iter().map(|p| p.max_agents).collect::<Vec<_>>()
        );
        assert_eq!(plans[0].name, "All tasks · model-p1111111");
        let default = plan_run_all(&image, &facts(), &|_: &str| None, "", 2).unwrap();
        assert_eq!(default[0].name, "All tasks");
    }

    #[test]
    fn run_all_is_refused_when_nothing_can_run_or_another_run_or_a_resolve_works() {
        let (_dir, none, _) = setup(|_| {});
        assert_eq!(
            plan_run_all(&none, &facts(), &no_model, "", 4)
                .unwrap_err()
                .0,
            "No Task is in To do"
        );

        let (_dir, blocked, _) = setup(|root| {
            task(root, "aaaaaaaa", 1, "todo", "");
            put(
                root,
                "drifts/dddddddd-d.md",
                &format!(
                    "---\nstatus: open\nrefs: []\norigin: aaaaaaaa\ncreated: {}\n---\n# D\n",
                    at(2)
                ),
            );
        });
        assert_eq!(
            plan_run_all(&blocked, &facts(), &no_model, "", 4)
                .unwrap_err()
                .0,
            "All the Tasks in To do are blocked"
        );

        let (_dir, ready, _) = setup(|root| task(root, "aaaaaaaa", 1, "todo", ""));
        let busy = AgentFacts {
            run_in_progress: true,
            resolve_working: true,
            ..facts()
        };
        assert_eq!(
            plan_run_all(&ready, &busy, &no_model, "", 4).unwrap_err().0,
            "Drifts are being resolved; A run is in progress"
        );
        assert!(plan_run_all(&ready, &facts(), &no_model, "", 4).is_ok());
    }

    #[test]
    fn the_tasks_a_job_held_go_back_to_to_do_when_it_ends_and_others_stay() {
        let (_dir, image, _) = setup(|root| {
            task(root, "aaaaaaaa", 1, "doing", "holder: jjjjjjjj\n");
            task(root, "bbbbbbbb", 2, "doing", "holder: kkkkkkkk\n");
            task(root, "cccccccc", 3, "todo", "");
            task(
                root,
                "dddddddd",
                4,
                "done",
                "finished: 2026-11-02T08:00:00.000Z\n",
            );
        });
        let changes = plan_release(&image, "jjjjjjjj");
        let after = image.with_changes(&changes);
        let column = |id: &str| {
            after
                .card(CardKind::Task, id)
                .unwrap()
                .str_key("column")
                .map(str::to_owned)
        };
        assert_eq!(column("aaaaaaaa").as_deref(), Some("todo"));
        assert!(after
            .card(CardKind::Task, "aaaaaaaa")
            .unwrap()
            .str_key("holder")
            .is_none());
        assert_eq!(
            column("bbbbbbbb").as_deref(),
            Some("doing"),
            "another job's Task stays held"
        );
        assert_eq!(column("dddddddd").as_deref(), Some("done"));
        assert!(plan_release(&image, "zzzzzzzz").is_empty());
    }
}
