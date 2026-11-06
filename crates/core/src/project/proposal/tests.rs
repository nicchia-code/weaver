//! Tests of the rules of a Proposal: staleness, scope, the order of the reasons, every
//! transition, the all-or-nothing approval and the auto-approval of a setup.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::project::board::{self, AgentFacts, ProposalState};
use crate::project::cards::CardKind;
use crate::project::changes::{Change, ChangeSet, MutationError};
use crate::project::image::ProjectImage;
use crate::project::random::ScriptedRandom;
use crate::project::shortnum::{self, ShortNumbers};

use super::analysis::{self, analyse, Analysis, Gate, Reason, StaleHow};
use super::create::{plan_edit, plan_new, DriftDecision, EditRequest, NewProposal, Writer};
use super::data::{self, Kind, Status};
use super::files::{read_folders, Folders};
use super::fixtures::*;
use super::resolution;
use super::transitions::*;

fn rng() -> ScriptedRandom {
    ScriptedRandom::new((0u8..=200).collect::<Vec<u8>>())
}

fn state(root: &Path) -> (ProjectImage, Folders, ShortNumbers) {
    let image = ProjectImage::load(root);
    let folders = read_folders(root, &image);
    let numbers = shortnum::peek(root, image.cards());
    (image, folders, numbers)
}

fn analyse_of(root: &Path, id: &str) -> Analysis {
    let (image, folders, numbers) = state(root);
    analyse(&image, &folders, &numbers, id).expect("the Proposal exists")
}

fn base_lines(entries: &[(&str, &str)]) -> String {
    let mut text = String::from("base:\n");
    for (path, hash) in entries {
        text.push_str(&format!("  {path}: {hash}\n"));
    }
    text
}

fn changed_order() -> String {
    ORDER.replace("### Paid\n", "### Paid\nThe customer paid.\n")
}

/// An Open change of the Machine `order`, with one Task that covers its whole scope.
fn ready_change(root: &Path, id: &str) {
    let front = format!(
        "status: open\nkind: change\n{}label: Shop\n",
        base_lines(&[("model/shop/order.md", &hash(ORDER))])
    );
    proposal(
        root,
        id,
        &front,
        "# Pay\n\nWhy.\n",
        &[("model/shop/order.md", &changed_order())],
        &[("tttttttt", "pay", &task_file("Pay", false, &["order"]))],
    );
}

fn snapshot(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(dir: &Path, base: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        let Ok(listing) = fs::read_dir(dir) else {
            return;
        };
        for item in listing {
            let path = item.unwrap().path();
            let key = path
                .strip_prefix(base)
                .unwrap()
                .to_string_lossy()
                .into_owned();
            if path.is_dir() {
                out.insert(format!("{key}/"), Vec::new());
                walk(&path, base, out);
            } else {
                out.insert(key, fs::read(&path).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

fn tag(reason: &Reason) -> String {
    match reason {
        Reason::AgentFinishing => "finishing".into(),
        Reason::RunInProgress => "run".into(),
        Reason::Required { problem, .. } => format!("required:{}", problem.text()),
        Reason::Stale(_) => "stale".into(),
        Reason::Invalid { .. } => "invalid".into(),
        Reason::Uncovered { .. } => "uncovered".into(),
        Reason::UnknownReference { .. } => "unknown-reference".into(),
        Reason::Label(_) => "label".into(),
        Reason::FixNotApplicable { .. } => "fix".into(),
    }
}

fn tags(reasons: &[Reason]) -> Vec<String> {
    let mut out: Vec<String> = reasons.iter().map(tag).collect();
    out.dedup();
    out
}

const NOW: &str = super::fixtures::NOW;

// ---------------------------------------------------------------------------------------------
// Scope, coverage and reasons
// ---------------------------------------------------------------------------------------------

#[test]
fn a_covered_change_is_not_blocked_and_has_no_hand_over_problem() {
    let dir = project();
    ready_change(dir.path(), "pppppppp");
    let (image, folders, numbers) = state(dir.path());
    let a = analyse(&image, &folders, &numbers, "pppppppp").unwrap();
    assert!(a.blocking().is_empty(), "{:?}", a.blocking());
    assert!(
        a.handover_refusals(&numbers).is_empty(),
        "{:?}",
        a.handover_refusals(&numbers)
    );
    assert_eq!(a.files.changed, ["model/shop/order.md"]);
    let scope: Vec<(&str, &str)> = a
        .scope
        .iter()
        .map(|c| (c.entry.reference.as_str(), c.entry.reason.text()))
        .collect();
    assert_eq!(
        scope,
        [
            ("order/Paid", "changed"),
            ("order/Draft/order.CONFIRM", "leads into a change")
        ]
    );
    assert!(a.scope.iter().all(|c| c.coverage.is_some()));
    assert_eq!(a.tasks.len(), 1);
    // The scope refs and the overlay serve the views.
    assert_eq!(
        analysis::scope_refs(&image, &folders, "pppppppp"),
        ["order/Paid", "order/Draft/order.CONFIRM"]
    );
    let overlay = analysis::overlay(&image, &folders, "pppppppp");
    assert!(overlay.model().machines["order"]
        .find_state("Paid")
        .unwrap()
        .description
        .contains("paid"));
}

#[test]
fn an_import_and_a_resolution_have_no_scope() {
    let dir = project();
    let front = format!(
        "status: open\nkind: import\n{}",
        base_lines(&[("model/shop/order.md", &hash(ORDER))])
    );
    proposal(
        dir.path(),
        "pppppppp",
        &front,
        "# Import\n",
        &[("model/shop/order.md", &changed_order())],
        &[],
    );
    let (image, folders, numbers) = state(dir.path());
    let a = analyse(&image, &folders, &numbers, "pppppppp").unwrap();
    assert!(a.scope.is_empty() && a.blocking().is_empty());
    assert!(analysis::scope_refs(&image, &folders, "pppppppp").is_empty());
}

#[test]
fn every_reason_is_listed_in_the_fixed_order() {
    let dir = project();
    let root = dir.path();
    proposal(root, "qqqqqqqq", "status: open\n", "# Q\n", &[], &[]);
    proposal(root, "rrrrrrrr", "status: rejected\n", "# R\n", &[], &[]);
    proposal(root, "ssssssss", "status: discarded\n", "# S\n", &[], &[]);
    proposal(root, "uuuuuuuu", "status: draft\n", "# U\n", &[], &[]);
    proposal(
        root,
        "vvvvvvvv",
        "status: open\nrequires:\n  - pppppppp\n",
        "# V\n",
        &[],
        &[],
    );
    let main = format!("{MAIN}\n## ghost/Nope\n- z\n");
    let front = format!(
        "status: open\nkind: change\nrequires:\n  - zzzzzzzz\n  - qqqqqqqq\n  - rrrrrrrr\n  - ssssssss\n  - uuuuuuuu\n  - vvvvvvvv\n{}",
        base_lines(&[
            ("model/shop/gone.md", &hash(ORDER)),
            ("requirements/ui/area.md", "new"),
            ("requirements/ui/main.md", &hash(MAIN)),
            ("specs.yaml", "deadbeef"),
        ])
    );
    proposal(
        root,
        "pppppppp",
        &front,
        "# P\n",
        &[("requirements/ui/main.md", &main)],
        &[("tttttttt", "pay", &task_file("Pay", false, &["nothing"]))],
    );
    let (image, folders, numbers) = state(root);
    let a = analyse(&image, &folders, &numbers, "pppppppp").unwrap();
    let gate = Gate {
        agent_finishing: true,
        run_in_progress: true,
    };
    let reasons = a.approve_reasons(gate);
    assert_eq!(
        tags(&reasons),
        [
            "finishing",
            "run",
            "required:is missing",
            "required:is not approved yet",
            "required:was rejected",
            "required:was discarded",
            "required:is still being written",
            "required:forms a cycle",
            "stale",
            "invalid",
            "uncovered",
            "unknown-reference",
            "label",
        ]
    );
    let stale: Vec<(&str, StaleHow)> = a.stale.iter().map(|s| (s.path.as_str(), s.how)).collect();
    assert_eq!(
        stale,
        [
            ("model/shop/gone.md", StaleHow::Gone),
            ("requirements/ui/area.md", StaleHow::NowExists),
            ("specs.yaml", StaleHow::Changed)
        ]
    );

    // The texts name the Proposals, the files and the elements.
    let texts: Vec<String> = reasons.iter().map(|r| r.text(&numbers)).collect();
    assert_eq!(texts[0], "The agent is still finishing");
    assert_eq!(texts[1], "A run is in progress");
    assert_eq!(texts[2], "Proposal zzzzzzzz is missing");
    assert!(
        texts[3].ends_with(" is not approved yet") && texts[3].starts_with("Proposal P"),
        "{}",
        texts[3]
    );
    assert_eq!(texts[4], "Proposal rrrrrrrr was rejected");
    assert_eq!(texts[5], "Proposal ssssssss was discarded");
    assert!(texts[6].ends_with(" is still being written"));
    assert!(texts[7].ends_with(" forms a cycle"));
    assert_eq!(texts[8], "Stale: `model/shop/gone.md` is gone");
    assert!(texts
        .iter()
        .any(|t| t == "`requirements/ui#ghost/Nope` (changed) is not covered by a Task"));
    assert!(texts
        .iter()
        .any(|t| t.starts_with("Task \"Pay\" names `nothing`, which would not exist")));
    assert_eq!(texts.last().unwrap(), "The label is missing");

    // The hand-over is refused for 5 to 7 only: never for staleness or required Proposals.
    let refusals = a.handover_refusals(&numbers);
    assert!(refusals.iter().any(|r| r.contains("ghost/Nope")));
    assert!(
        refusals
            .iter()
            .all(|r| !r.starts_with("Stale") && !r.starts_with("Proposal ")),
        "{refusals:?}"
    );

    // Without the agent and the Run, the first two reasons are gone.
    assert_eq!(
        tags(&a.approve_reasons(Gate::default()))[0],
        "required:is missing"
    );
}

#[test]
fn staleness_counts_the_required_proposals_as_applied() {
    let dir = project();
    let root = dir.path();
    // Q changes the Machine; P changes it again, starting from Q's version.
    let q_order = changed_order();
    let front_q = format!(
        "status: open\n{}label: Q\n",
        base_lines(&[("model/shop/order.md", &hash(ORDER))])
    );
    proposal(
        root,
        "qqqqqqqq",
        &front_q,
        "# Q\n",
        &[("model/shop/order.md", &q_order)],
        &[("11111111", "q", &task_file("Q", false, &["order"]))],
    );
    let p_order = q_order.replace("The customer paid.", "The customer paid in full.");
    let front_p = |base: &str| {
        format!(
            "status: open\nrequires:\n  - qqqqqqqq\n{}label: P\n",
            base_lines(&[("model/shop/order.md", base)])
        )
    };
    proposal(
        root,
        "pppppppp",
        &front_p(&hash(&q_order)),
        "# P\n",
        &[("model/shop/order.md", &p_order)],
        &[("22222222", "p", &task_file("P", false, &["order"]))],
    );
    let a = analyse_of(root, "pppppppp");
    assert!(a.stale.is_empty(), "{:?}", a.stale);
    assert_eq!(
        a.blocking().iter().map(tag).collect::<Vec<_>>(),
        ["required:is not approved yet"]
    );
    // Scope: against Q applied below, only the new sentence is a change, plus what leads into it.
    assert_eq!(
        a.scope
            .iter()
            .map(|c| c.entry.reference.as_str())
            .collect::<Vec<_>>(),
        ["order/Paid", "order/Draft/order.CONFIRM"]
    );

    // Started from the file as it is on disk (without Q): stale once Q is applied below.
    proposal(
        root,
        "pppppppp",
        &front_p(&hash(ORDER)),
        "# P\n",
        &[("model/shop/order.md", &p_order)],
        &[("22222222", "p", &task_file("P", false, &["order"]))],
    );
    assert_eq!(
        analyse_of(root, "pppppppp")
            .stale
            .iter()
            .map(|s| s.how)
            .collect::<Vec<_>>(),
        [StaleHow::Changed]
    );

    // Q approved: its version is the Model's, and P's base matches it again.
    put(root, "model/shop/order.md", &q_order);
    proposal(
        root,
        "qqqqqqqq",
        &format!(
            "status: approved\n{}label: Q\n",
            base_lines(&[("model/shop/order.md", &hash(ORDER))])
        ),
        "# Q\n",
        &[("model/shop/order.md", &q_order)],
        &[],
    );
    proposal(
        root,
        "pppppppp",
        &front_p(&hash(&q_order)),
        "# P\n",
        &[("model/shop/order.md", &p_order)],
        &[("22222222", "p", &task_file("P", false, &["order"]))],
    );
    let a = analyse_of(root, "pppppppp");
    assert!(
        a.stale.is_empty() && a.blocking().is_empty(),
        "{:?}",
        a.blocking()
    );
}

#[test]
fn a_new_file_that_now_exists_and_a_hashed_file_that_is_gone_are_stale_and_text_is_hashed_with_lf()
{
    let dir = project();
    let root = dir.path();
    // The hash of the LF text matches a file written with CRLF and a BOM.
    put(
        root,
        "specs.yaml",
        &format!("\u{feff}{}", SPECS.replace('\n', "\r\n")),
    );
    let front = format!(
        "status: open\n{}",
        base_lines(&[
            ("specs.yaml", &hash(SPECS)),
            ("model/shop/gone.md", "abc"),
            ("requirements/ui/area.md", "new")
        ])
    );
    proposal(root, "pppppppp", &front, "# P\n", &[], &[]);
    let a = analyse_of(root, "pppppppp");
    assert_eq!(
        a.stale
            .iter()
            .map(|s| (s.path.as_str(), s.how))
            .collect::<Vec<_>>(),
        [
            ("model/shop/gone.md", StaleHow::Gone),
            ("requirements/ui/area.md", StaleHow::NowExists)
        ]
    );
}

#[test]
fn task_files_and_the_proposal_file_never_make_a_proposal_stale() {
    let dir = project();
    ready_change(dir.path(), "pppppppp");
    put(
        dir.path(),
        "proposals/pppppppp-t/tasks/tttttttt-pay.md",
        &task_file("Pay, again", false, &["order"]),
    );
    assert!(analyse_of(dir.path(), "pppppppp").stale.is_empty());
}

#[test]
fn the_label_is_required_with_tasks_only() {
    let dir = project();
    let root = dir.path();
    let front = format!(
        "status: open\n{}",
        base_lines(&[("model/shop/order.md", &hash(ORDER))])
    );
    proposal(
        root,
        "pppppppp",
        &front,
        "# P\n",
        &[("model/shop/order.md", &changed_order())],
        &[("tttttttt", "pay", &task_file("Pay", false, &["order"]))],
    );
    assert_eq!(tags(&analyse_of(root, "pppppppp").blocking()), ["label"]);
    let long = format!("{front}label: {}\n", "x".repeat(31));
    proposal(
        root,
        "pppppppp",
        &long,
        "# P\n",
        &[("model/shop/order.md", &changed_order())],
        &[("tttttttt", "pay", &task_file("Pay", false, &["order"]))],
    );
    assert_eq!(tags(&analyse_of(root, "pppppppp").blocking()), ["label"]);
    // No Task and no scope: no label needed.
    proposal(root, "qqqqqqqq", "status: open\n", "# P\n", &[], &[]);
    assert!(analyse_of(root, "qqqqqqqq").blocking().is_empty());
}

#[test]
fn a_removal_is_covered_by_the_removal_tasks_and_a_dangling_reference_is_found() {
    let dir = project();
    let root = dir.path();
    let without_paid = ORDER
        .replace(
            "### Paid\n- `user.CANCEL` → Draft: refunds the customer.\n\n",
            "",
        )
        .replace("- `CONFIRM` → Paid: only if the cart is not empty.\n", "");
    let front = format!(
        "status: open\n{}label: Cut\n",
        base_lines(&[
            ("model/shop/order.md", &hash(ORDER)),
            ("requirements/ui/main.md", &hash(MAIN))
        ])
    );
    let main = "---\ndescription: d\n---\n# Main\n\n## General\n- x\n";
    proposal(
        root,
        "pppppppp",
        &front,
        "# Cut\n",
        &[
            ("model/shop/order.md", &without_paid),
            ("requirements/ui/main.md", main),
        ],
        &[],
    );
    let a = analyse_of(root, "pppppppp");
    assert!(a.blocking().is_empty(), "{:?}", a.blocking());
    let titles: Vec<String> = a.removals.iter().map(|r| r.title()).collect();
    assert!(
        titles.contains(&"Remove code of `order/Paid`".to_owned()),
        "{titles:?}"
    );
    assert!(
        titles.contains(&"Remove code of `order/Draft/order.CONFIRM`".to_owned()),
        "{titles:?}"
    );
}

// ---------------------------------------------------------------------------------------------
// Hand-over
// ---------------------------------------------------------------------------------------------

fn submit_ctx(empty: bool, run: bool) -> SubmitContext<'static> {
    SubmitContext {
        now: NOW,
        job_found_empty_model: empty,
        run_in_progress: run,
    }
}

fn submit_plan(root: &Path, id: &str, ctx: &SubmitContext) -> Result<Submission, Refusal> {
    let (image, folders, numbers) = state(root);
    plan_submit(&image, &folders, &numbers, id, ctx, &mut rng())
}

#[test]
fn a_draft_is_handed_over_as_open_and_consumes_its_request() {
    let dir = project();
    let root = dir.path();
    ready_change(root, "pppppppp");
    let text = fs::read_to_string(root.join(".weaver/proposals/pppppppp-t/proposal.md")).unwrap();
    put(
        root,
        "proposals/pppppppp-t/proposal.md",
        &text
            .replace(
                "status: open",
                "status: draft\nrequest: 2026-11-02T07-00-00",
            )
            .replace("---\n# Pay", "refining: true\n---\n# Pay"),
    );
    let submission = submit_plan(root, "pppppppp", &submit_ctx(false, false)).unwrap();
    assert_eq!(submission.outcome, Submitted::Open);
    assert_eq!(
        submission.consumed_request.as_deref(),
        Some("2026-11-02T07-00-00")
    );
    let written = match submission.changes.get("proposals/pppppppp-t/proposal.md") {
        Some(Change::Write(text)) => text.clone(),
        other => panic!("{other:?}"),
    };
    assert!(
        written.contains("status: open") && !written.contains("refining"),
        "{written}"
    );
}

#[test]
fn the_hand_over_is_refused_naming_every_problem() {
    let dir = project();
    let root = dir.path();
    let front = format!(
        "status: draft\nkind: import\n{}",
        base_lines(&[("model/shop/order.md", &hash(ORDER))])
    );
    proposal(
        root,
        "pppppppp",
        &front,
        "# P\n\n## Already present\n\n- `ghost` — nowhere\n",
        &[("model/shop/order.md", &changed_order()), ("stray.md", "x")],
        &[("tttttttt", "pay", &task_file("Pay", true, &["order"]))],
    );
    let refusal = submit_plan(root, "pppppppp", &submit_ctx(false, false)).unwrap_err();
    let all = refusal.0.join("\n");
    assert!(all.contains("`stray.md` is not registered"), "{all}");
    assert!(all.contains("An import carries no Task"), "{all}");
    assert!(all.contains("technical but names a Model element"), "{all}");
    assert!(all.contains("`## Already present` names `ghost`"), "{all}");

    // A Task of a change: not technical without a Model reference.
    let front = format!(
        "status: draft\n{}label: L\n",
        base_lines(&[("model/shop/order.md", &hash(ORDER))])
    );
    proposal(
        root,
        "pppppppp",
        &front,
        "# P\n",
        &[("model/shop/order.md", &changed_order())],
        &[(
            "tttttttt",
            "pay",
            &task_file("Pay", false, &["requirements/ui"]),
        )],
    );
    let all = submit_plan(root, "pppppppp", &submit_ctx(false, false))
        .unwrap_err()
        .0
        .join("\n");
    assert!(
        all.contains("is not technical but names no Model element"),
        "{all}"
    );
    assert!(
        all.contains("`order/Paid` (changed) is not covered by a Task"),
        "{all}"
    );
}

#[test]
fn the_hand_over_ignores_staleness_and_required_proposals() {
    let dir = project();
    let root = dir.path();
    let front = format!(
        "status: draft\nrequires:\n  - zzzzzzzz\n{}label: Shop\n",
        base_lines(&[
            ("model/shop/order.md", "0000"),
            ("specs.yaml", &hash(SPECS))
        ])
    );
    proposal(
        root,
        "pppppppp",
        &front,
        "# P\n",
        &[("model/shop/order.md", &changed_order())],
        &[("tttttttt", "pay", &task_file("Pay", false, &["order"]))],
    );
    let submission = submit_plan(root, "pppppppp", &submit_ctx(false, false)).unwrap();
    assert_eq!(submission.outcome, Submitted::Open);
    assert!(
        submission
            .warnings
            .iter()
            .any(|w| w == "Proposal zzzzzzzz is missing"),
        "{:?}",
        submission.warnings
    );
    assert!(
        submission
            .warnings
            .iter()
            .any(|w| w.starts_with("Stale: `model/shop/order.md`")),
        "{:?}",
        submission.warnings
    );
}

/// A setup that creates the Machine `order`, its Feature and its Actor.
fn setup(root: &Path, id: &str, status: &str, kind: &str) {
    let order = ORDER.replace(
        "- `user.CANCEL` → Draft: refunds the customer.\n",
        "- `CONFIRM` → Draft\n",
    );
    let front = format!(
        "status: {status}\nkind: {kind}\n{}label: Shop\n",
        base_lines(&[
            ("model/actors/user.md", "new"),
            ("model/shop/feature.md", "new"),
            ("model/shop/order.md", "new")
        ])
    );
    let tasks: Vec<(&str, &str, String)> = if kind == "setup" {
        vec![
            ("11111111", "shop", task_file("Shop", false, &["shop"])),
            ("22222222", "user", task_file("User", false, &["user"])),
        ]
    } else {
        Vec::new()
    };
    let tasks: Vec<(&str, &str, &str)> =
        tasks.iter().map(|(a, b, c)| (*a, *b, c.as_str())).collect();
    proposal(
        root,
        id,
        &front,
        "# Shop\n",
        &[
            ("model/actors/user.md", USER),
            ("model/shop/feature.md", FEATURE),
            ("model/shop/order.md", &order),
        ],
        &tasks,
    );
}

#[test]
fn a_setup_of_a_job_that_found_an_empty_model_is_approved_at_the_hand_over_and_run_all_follows() {
    let dir = empty_project();
    let root = dir.path();
    setup(root, "pppppppp", "draft", "setup");
    let submission = submit_plan(root, "pppppppp", &submit_ctx(true, false)).unwrap();
    let Submitted::Approved(plan) = &submission.outcome else {
        panic!("{:?}", submission.outcome)
    };
    assert!(plan.start_run_all);
    assert_eq!(plan.created_tasks, ["11111111", "22222222"]);
    assert!(plan.removal_tasks.is_empty());

    // Written whole: the files, the Tasks in To do with the same identities and `created`.
    let (after, submission) =
        submit(root, "pppppppp", &submit_ctx(true, false), &mut rng()).unwrap();
    assert!(matches!(submission.outcome, Submitted::Approved(_)));
    assert!(root.join(".weaver/model/shop/order.md").is_file());
    assert!(root.join(".weaver/model/actors/user.md").is_file());
    let tasks: Vec<_> = after.cards_of(CardKind::Task).collect();
    assert_eq!(
        tasks.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(),
        ["11111111", "22222222"]
    );
    for task in &tasks {
        assert_eq!(task.str_key("column"), Some("todo"));
        assert_eq!(task.str_key("proposal"), Some("pppppppp"));
        assert_eq!(task.created.as_deref(), Some(NOW));
    }
    assert_eq!(
        data::status_of(after.card(CardKind::Proposal, "pppppppp").unwrap()),
        Some(Status::Approved)
    );
    // Run all can start: Tasks are in To do and nothing blocks them.
    let board = board::compute(&after, &AgentFacts::default());
    assert!(
        board.run_all_reasons.is_empty(),
        "{:?}",
        board.run_all_reasons
    );
    assert_eq!(board.active.len(), 2);
    assert!(!after.has_errors(), "{:?}", after.diagnostics());
}

#[test]
fn a_setup_of_another_job_or_a_run_in_progress_stays_open_with_its_reasons() {
    let dir = empty_project();
    let root = dir.path();
    setup(root, "pppppppp", "draft", "setup");
    let other = submit_plan(root, "pppppppp", &submit_ctx(false, false)).unwrap();
    assert_eq!(other.outcome, Submitted::Open);
    assert!(other.warnings.is_empty());
    let run = submit_plan(root, "pppppppp", &submit_ctx(true, true)).unwrap();
    assert_eq!(run.outcome, Submitted::Open);
    assert_eq!(run.warnings, ["A run is in progress"]);
}

#[test]
fn a_chain_of_imports_of_one_job_is_approved_whole() {
    let dir = empty_project();
    let root = dir.path();
    let order = ORDER.replace(
        "- `user.CANCEL` → Draft: refunds the customer.\n",
        "- `CONFIRM` → Draft\n",
    );
    let first = format!(
        "status: draft\nkind: import\n{}",
        base_lines(&[
            ("model/shop/feature.md", "new"),
            ("model/shop/order.md", "new")
        ])
    );
    proposal(
        root,
        "aaaaaaaa",
        &first,
        "# First\n",
        &[
            ("model/shop/feature.md", FEATURE),
            ("model/shop/order.md", &order),
        ],
        &[],
    );
    let second = format!(
        "status: draft\nkind: import\nrequires:\n  - aaaaaaaa\n{}",
        base_lines(&[("model/actors/user.md", "new")])
    );
    proposal(
        root,
        "bbbbbbbb",
        &second,
        "# Second\n",
        &[("model/actors/user.md", USER)],
        &[],
    );
    let ctx = submit_ctx(true, false);

    // The second waits for the first: handed over alone, it stays Open, saying so.
    let early = submit_plan(root, "bbbbbbbb", &ctx).unwrap();
    assert_eq!(early.outcome, Submitted::Open);
    assert_eq!(early.warnings.len(), 1);
    assert!(
        early.warnings[0].ends_with("is still being written"),
        "{:?}",
        early.warnings
    );

    let (_, one) = submit(root, "aaaaaaaa", &ctx, &mut rng()).unwrap();
    assert!(matches!(&one.outcome, Submitted::Approved(plan) if !plan.start_run_all));
    let (after, two) = submit(root, "bbbbbbbb", &ctx, &mut rng()).unwrap();
    assert!(matches!(two.outcome, Submitted::Approved(_)));
    for id in ["aaaaaaaa", "bbbbbbbb"] {
        assert_eq!(
            data::status_of(after.card(CardKind::Proposal, id).unwrap()),
            Some(Status::Approved),
            "{id}"
        );
    }
    assert!(
        root.join(".weaver/model/actors/user.md").is_file()
            && root.join(".weaver/model/shop/order.md").is_file()
    );
    assert!(
        after.cards_of(CardKind::Task).next().is_none(),
        "an import creates no Task"
    );
}

#[test]
fn the_first_proposal_of_a_rework_takes_the_place_in_the_requirements_of_the_proposals_that_waited()
{
    let dir = project();
    let root = dir.path();
    ready_change(root, "dddddddd");
    let front = fs::read_to_string(root.join(".weaver/proposals/dddddddd-t/proposal.md")).unwrap();
    put(
        root,
        "proposals/dddddddd-t/proposal.md",
        &front.replace("status: open", "status: draft\nreworks: oooooooo"),
    );
    proposal(
        root,
        "oooooooo",
        "status: rejected\nreworkRequested: true\n",
        "# Old\n",
        &[],
        &[],
    );
    proposal(
        root,
        "wwwwwwww",
        "status: open\nrequires:\n  - oooooooo\n  - aaaaaaaa\n",
        "# Waiting\n",
        &[],
        &[],
    );
    let submission = submit_plan(root, "dddddddd", &submit_ctx(false, false)).unwrap();
    let Some(Change::Write(text)) = submission.changes.get("proposals/wwwwwwww-t/proposal.md")
    else {
        panic!("no rewrite of the waiting Proposal")
    };
    assert!(text.contains("- dddddddd\n- aaaaaaaa"), "{text}");
    assert!(!text.contains("oooooooo"), "{text}");
}

// ---------------------------------------------------------------------------------------------
// Approve
// ---------------------------------------------------------------------------------------------

fn approve_plan(root: &Path, id: &str, gate: Gate, by: Decider) -> Result<Approval, Refusal> {
    let (image, folders, numbers) = state(root);
    plan_approve(&image, &folders, &numbers, id, gate, NOW, &mut rng(), by)
}

#[test]
fn approving_an_open_proposal_replaces_the_files_and_creates_its_tasks_in_one_step() {
    let dir = project();
    let root = dir.path();
    ready_change(root, "pppppppp");
    let (after, approval) = approve(
        root,
        &AgentFacts::default(),
        "pppppppp",
        Decider::Developer,
        NOW,
        &mut rng(),
    )
    .unwrap();
    let Approval::Approve(plan) = approval else {
        panic!("{approval:?}")
    };
    assert!(!plan.requirements_changed && !plan.start_run_all);
    assert!(fs::read_to_string(root.join(".weaver/model/shop/order.md"))
        .unwrap()
        .contains("The customer paid."));
    let task = after
        .card(CardKind::Task, "tttttttt")
        .expect("the Task keeps its identity");
    assert_eq!(
        (task.str_key("column"), task.created.as_deref()),
        (Some("todo"), Some(NOW))
    );
    assert_eq!(task.str_key("proposal"), Some("pppppppp"));
    assert_eq!(task.path, "tasks/tttttttt-pay.md");
    assert_eq!(task.title, "Pay");
    assert_eq!(
        data::status_of(after.card(CardKind::Proposal, "pppppppp").unwrap()),
        Some(Status::Approved)
    );
    // The folder stays as it was.
    assert!(root
        .join(".weaver/proposals/pppppppp-t/model/shop/order.md")
        .is_file());
    assert!(root
        .join(".weaver/proposals/pppppppp-t/tasks/tttttttt-pay.md")
        .is_file());
}

#[test]
fn approving_a_change_of_requirements_says_so_for_the_skills() {
    let dir = project();
    let root = dir.path();
    let front = format!(
        "status: open\n{}label: R\n",
        base_lines(&[("requirements/ui/main.md", &hash(MAIN))])
    );
    let main = MAIN.replace("- y", "- changed");
    proposal(
        root,
        "pppppppp",
        &front,
        "# R\n",
        &[("requirements/ui/main.md", &main)],
        &[(
            "tttttttt",
            "r",
            &task_file("R", false, &["requirements/ui"]),
        )],
    );
    let Approval::Approve(plan) =
        approve_plan(root, "pppppppp", Gate::default(), Decider::Developer).unwrap()
    else {
        panic!()
    };
    assert!(plan.requirements_changed);
}

#[test]
fn approving_a_change_of_requirements_writes_the_requirement_skills_in_the_same_step() {
    let dir = project();
    let root = dir.path();
    let front = format!(
        "status: open\n{}label: R\n",
        base_lines(&[("requirements/ui/main.md", &hash(MAIN))])
    );
    let main = MAIN.replace("- y", "- changed");
    proposal(
        root,
        "pppppppp",
        &front,
        "# R\n",
        &[("requirements/ui/main.md", &main)],
        &[(
            "tttttttt",
            "r",
            &task_file("R", false, &["requirements/ui"]),
        )],
    );
    let (_, approval) = approve(
        root,
        &AgentFacts::default(),
        "pppppppp",
        Decider::Developer,
        NOW,
        &mut rng(),
    )
    .unwrap();
    let Approval::Approve(plan) = approval else {
        panic!("{approval:?}")
    };
    let report = plan
        .skills
        .expect("the skills were generated again")
        .expect("without error");
    assert!(
        report
            .changes
            .iter()
            .any(|c| c.path == ".agents/skills/weaver-req-ui-main/SKILL.md"),
        "{:?}",
        report.changes
    );
    let skill =
        fs::read_to_string(root.join(".agents/skills/weaver-req-ui-main/SKILL.md")).unwrap();
    assert!(skill.contains("changed"), "{skill}");
    // Nothing else of the installed list is written by an approval.
    assert!(!root.join(".omp").exists());
}

#[test]
fn an_approval_that_leaves_the_requirements_alone_generates_nothing() {
    let dir = project();
    let root = dir.path();
    ready_change(root, "pppppppp");
    let (_, approval) = approve(
        root,
        &AgentFacts::default(),
        "pppppppp",
        Decider::Developer,
        NOW,
        &mut rng(),
    )
    .unwrap();
    let Approval::Approve(plan) = approval else {
        panic!()
    };
    assert!(plan.skills.is_none());
    assert!(!root.join(".agents").exists());
}

#[test]
fn approval_creates_a_removal_task_for_each_removed_element() {
    let dir = project();
    let root = dir.path();
    let without_paid = ORDER
        .replace(
            "### Paid\n- `user.CANCEL` → Draft: refunds the customer.\n\n",
            "",
        )
        .replace("- `CONFIRM` → Paid: only if the cart is not empty.\n", "");
    let main = "---\ndescription: d\n---\n# Main\n\n## General\n- x\n";
    let front = format!(
        "status: open\n{}label: Cut\n",
        base_lines(&[
            ("model/shop/order.md", &hash(ORDER)),
            ("requirements/ui/main.md", &hash(MAIN))
        ])
    );
    proposal(
        root,
        "pppppppp",
        &front,
        "# Cut\n",
        &[
            ("model/shop/order.md", &without_paid),
            ("requirements/ui/main.md", main),
        ],
        &[],
    );
    let (after, approval) = approve(
        root,
        &AgentFacts::default(),
        "pppppppp",
        Decider::Developer,
        NOW,
        &mut rng(),
    )
    .unwrap();
    let Approval::Approve(plan) = approval else {
        panic!()
    };
    assert_eq!(plan.removal_tasks.len(), 2);
    let tasks: Vec<_> = after.cards_of(CardKind::Task).collect();
    let paid = tasks
        .iter()
        .find(|t| t.title == "Remove code of `order/Paid`")
        .expect("a removal Task");
    assert_eq!(tasks::refs_of(paid), ["order"]);
    assert!(!tasks::is_technical(paid));
    assert_eq!(paid.str_key("proposal"), Some("pppppppp"));
    assert_eq!(paid.str_key("column"), Some("todo"));
}

use crate::project::tasks;

#[test]
fn approval_creates_a_technical_removal_task_for_a_removed_actor() {
    let dir = project();
    let root = dir.path();
    let order = ORDER.replace("user.CANCEL", "CONFIRM");
    let front = format!(
        "status: open\n{}label: Cut\n",
        base_lines(&[
            ("model/actors/user.md", &hash(USER)),
            ("model/shop/order.md", &hash(ORDER))
        ])
    );
    proposal(
        root,
        "pppppppp",
        &front,
        "# Cut\n",
        &[("model/shop/order.md", &order)],
        &[("tttttttt", "cut", &task_file("Cut", false, &["order"]))],
    );
    put(
        root,
        "proposals/pppppppp-t/proposal.md",
        &format!("---\n{front}delete:\n  - model/actors/user.md\ncreated: {CREATED}\n---\n# Cut\n"),
    );
    let (after, _) = approve(
        root,
        &AgentFacts::default(),
        "pppppppp",
        Decider::Developer,
        NOW,
        &mut rng(),
    )
    .unwrap();
    let removal = after
        .cards_of(CardKind::Task)
        .find(|t| t.title == "Remove code of `user`")
        .expect("a removal Task");
    assert!(tasks::is_technical(removal) && tasks::refs_of(removal).is_empty());
    assert!(!root.join(".weaver/model/actors/user.md").exists());
}

#[test]
fn approve_is_refused_with_every_reason_in_order_and_writes_nothing() {
    let dir = project();
    let root = dir.path();
    ready_change(root, "pppppppp");
    let front = fs::read_to_string(root.join(".weaver/proposals/pppppppp-t/proposal.md")).unwrap();
    // No label, and a required Proposal that does not exist.
    put(
        root,
        "proposals/pppppppp-t/proposal.md",
        &front
            .replace("label: Shop\n", "")
            .replace("base:\n", "requires:\n  - zzzzzzzz\nbase:\n"),
    );
    let facts = AgentFacts {
        run_in_progress: true,
        ..AgentFacts::default()
    };
    let before = snapshot(root);
    let Err(MutationError::Refused(refused)) = approve(
        root,
        &facts,
        "pppppppp",
        Decider::Developer,
        NOW,
        &mut rng(),
    ) else {
        panic!("expected a refusal")
    };
    assert_eq!(
        refused.notice,
        "Could not approve P1: A run is in progress; Proposal zzzzzzzz is missing; The label is missing."
    );
    assert_eq!(snapshot(root), before, "a refusal writes nothing");
}

/// Turns the `kind: change` of the Proposal `id` into `kind`.
fn with_kind(root: &Path, id: &str, kind: &str) {
    let path = root.join(format!(".weaver/proposals/{id}-t/proposal.md"));
    let text = fs::read_to_string(&path).unwrap();
    fs::write(path, text.replace("kind: change", &format!("kind: {kind}"))).unwrap();
}

fn make_order_stale(root: &Path) {
    put(
        root,
        "model/shop/order.md",
        &ORDER.replace("The customer is choosing.", "The customer is browsing."),
    );
}

#[test]
fn a_stale_open_proposal_of_any_kind_is_refined_also_during_a_run() {
    let dir = project();
    let root = dir.path();
    for kind in ["change", "setup", "import"] {
        ready_change(root, "pppppppp");
        with_kind(root, "pppppppp", kind);
        make_order_stale(root);
        for run in [false, true] {
            let gate = Gate {
                agent_finishing: false,
                run_in_progress: run,
            };
            let approval = approve_plan(root, "pppppppp", gate, Decider::Developer).unwrap();
            let Approval::Refine(changes) = approval else {
                panic!("{kind}: {approval:?}")
            };
            let Some(Change::Write(text)) = changes.get("proposals/pppppppp-t/proposal.md") else {
                panic!()
            };
            assert!(
                text.contains("status: refining")
                    && text.contains("refining: true")
                    && text.contains(&format!("kind: {kind}")),
                "{text}"
            );
        }
        // Not while the agent is still finishing.
        let gate = Gate {
            agent_finishing: true,
            run_in_progress: false,
        };
        let refusal = approve_plan(root, "pppppppp", gate, Decider::Developer).unwrap_err();
        assert_eq!(refusal.0, ["The agent is still finishing"], "{kind}");
    }
}

#[test]
fn a_stale_proposal_is_refined_whatever_else_blocks_it_and_a_drift_decides_for_its_own() {
    let dir = project();
    let root = dir.path();
    ready_change(root, "pppppppp");
    make_order_stale(root);
    let front = fs::read_to_string(root.join(".weaver/proposals/pppppppp-t/proposal.md")).unwrap();
    // Also unlabeled and requiring a missing Proposal: reasons beyond staleness do not matter.
    put(
        root,
        "proposals/pppppppp-t/proposal.md",
        &front
            .replace("label: Shop\n", "")
            .replace("base:\n", "requires:\n  - zzzzzzzz\nbase:\n"),
    );
    let gate = Gate {
        agent_finishing: false,
        run_in_progress: true,
    };
    assert!(matches!(
        approve_plan(root, "pppppppp", gate, Decider::Developer).unwrap(),
        Approval::Refine(_)
    ));

    // Attached to a Drift: the developer's own approval is refused, the Drift refines it.
    put(root, "proposals/pppppppp-t/proposal.md", &front);
    put(
        root,
        "drifts/dddddddd-d.md",
        &drift_file("open", "pppppppp"),
    );
    let refusal = approve_plan(root, "pppppppp", gate, Decider::Developer).unwrap_err();
    assert!(
        refusal.0[0].contains("decided through its Drift"),
        "{refusal:?}"
    );
    assert!(matches!(
        approve_plan(root, "pppppppp", gate, Decider::Drift).unwrap(),
        Approval::Refine(_)
    ));
    let finishing = Gate {
        agent_finishing: true,
        run_in_progress: false,
    };
    assert_eq!(
        approve_plan(root, "pppppppp", finishing, Decider::Drift)
            .unwrap_err()
            .0,
        ["The agent is still finishing"]
    );
}

#[test]
fn a_stale_resolution_is_refined_only_while_one_of_its_drifts_is_open() {
    let dir = project();
    let root = dir.path();
    let rows = "drifts:\n  - drift: dddddddd\n    decision: dismissed\n";
    let front = format!(
        "status: open\nkind: resolution\n{rows}{}",
        base_lines(&[("model/shop/order.md", &hash(ORDER))])
    );
    proposal(
        root,
        "pppppppp",
        &front,
        "# R\n",
        &[("model/shop/order.md", &changed_order())],
        &[],
    );
    make_order_stale(root);
    put(root, "drifts/dddddddd-d.md", &drift_file("open", ""));
    assert!(matches!(
        approve_plan(root, "pppppppp", Gate::default(), Decider::Developer).unwrap(),
        Approval::Refine(_)
    ));
    put(root, "drifts/dddddddd-d.md", &drift_file("dismissed", ""));
    assert_eq!(
        approve_plan(root, "pppppppp", Gate::default(), Decider::Developer)
            .unwrap_err()
            .0,
        ["None of its Drifts is still open"]
    );

    // A resolution with no file has no `base`: never stale, never refined.
    put(root, "drifts/dddddddd-d.md", &drift_file("open", ""));
    proposal(
        root,
        "pppppppp",
        &format!("status: open\nkind: resolution\n{rows}"),
        "# R\n",
        &[],
        &[],
    );
    assert!(analyse_of(root, "pppppppp").stale.is_empty());
    assert!(matches!(
        approve_plan(root, "pppppppp", Gate::default(), Decider::Developer).unwrap(),
        Approval::Approve(_)
    ));
}

#[test]
fn a_draft_cannot_be_approved_unless_a_refinement_left_it_half_way() {
    let dir = project();
    let root = dir.path();
    proposal(root, "pppppppp", "status: draft\n", "# P\n", &[], &[]);
    assert!(approve_plan(root, "pppppppp", Gate::default(), Decider::Developer).is_err());
    proposal(
        root,
        "pppppppp",
        "status: draft\nrefining: true\n",
        "# P\n",
        &[],
        &[],
    );
    let Approval::Continue(changes) =
        approve_plan(root, "pppppppp", Gate::default(), Decider::Developer).unwrap()
    else {
        panic!()
    };
    let Some(Change::Write(text)) = changes.get("proposals/pppppppp-t/proposal.md") else {
        panic!()
    };
    assert!(text.contains("status: refining"), "{text}");
    proposal(
        root,
        "pppppppp",
        "status: refining\nrefining: true\n",
        "# P\n",
        &[],
        &[],
    );
    assert_eq!(
        approve_plan(root, "pppppppp", Gate::default(), Decider::Developer)
            .unwrap_err()
            .0,
        ["Being refined"]
    );
}

#[test]
fn an_interrupted_draft_is_continued_with_the_same_conditions_as_a_refinement() {
    let dir = project();
    let root = dir.path();
    let finishing = Gate {
        agent_finishing: true,
        run_in_progress: false,
    };
    proposal(
        root,
        "pppppppp",
        "status: draft\nrefining: true\n",
        "# P\n",
        &[],
        &[],
    );
    assert_eq!(
        approve_plan(root, "pppppppp", finishing, Decider::Developer)
            .unwrap_err()
            .0,
        ["The agent is still finishing"]
    );

    // A resolution continues only while one of its Drifts is still open.
    let resolution = "status: draft\nrefining: true\nkind: resolution\ndrifts:\n  - drift: dddddddd\n    decision: dismissed\n";
    proposal(root, "pppppppp", resolution, "# R\n", &[], &[]);
    put(root, "drifts/dddddddd-d.md", &drift_file("dismissed", ""));
    assert_eq!(
        approve_plan(root, "pppppppp", Gate::default(), Decider::Developer)
            .unwrap_err()
            .0,
        ["None of its Drifts is still open"]
    );
    put(root, "drifts/dddddddd-d.md", &drift_file("open", ""));
    assert!(matches!(
        approve_plan(root, "pppppppp", Gate::default(), Decider::Developer).unwrap(),
        Approval::Continue(_)
    ));
    // Both refusals are named.
    put(root, "drifts/dddddddd-d.md", &drift_file("dismissed", ""));
    assert_eq!(
        approve_plan(root, "pppppppp", finishing, Decider::Developer)
            .unwrap_err()
            .0,
        [
            "The agent is still finishing",
            "None of its Drifts is still open"
        ]
    );

    // Attached to a Drift, it is continued from the Drift, never from the Board.
    proposal(
        root,
        "pppppppp",
        "status: draft\nrefining: true\n",
        "# P\n",
        &[],
        &[],
    );
    put(
        root,
        "drifts/dddddddd-d.md",
        &drift_file("open", "pppppppp"),
    );
    assert!(approve_plan(root, "pppppppp", Gate::default(), Decider::Developer).is_err());
    assert!(matches!(
        approve_plan(root, "pppppppp", Gate::default(), Decider::Drift).unwrap(),
        Approval::Continue(_)
    ));
}

#[test]
fn an_interrupted_refinement_goes_back_to_draft_and_shows_on_the_board() {
    let dir = project();
    let root = dir.path();
    proposal(
        root,
        "pppppppp",
        "status: refining\nrefining: true\n",
        "# P\n",
        &[],
        &[],
    );
    let image = ProjectImage::load(root);
    assert!(matches!(
        board::compute(&image, &AgentFacts::default()).backlog[0],
        board::BacklogEntry::Proposal {
            state: ProposalState::Refining,
            ..
        }
    ));
    let changes = plan_refinement_ended(&image, "pppppppp");
    let after = image.with_changes(&changes);
    let card = after.card(CardKind::Proposal, "pppppppp").unwrap();
    assert!(data::is_interrupted(card));
    assert!(matches!(
        board::compute(&after, &AgentFacts::default()).backlog[0],
        board::BacklogEntry::Proposal {
            state: ProposalState::Interrupted,
            ..
        }
    ));
    // An Open Proposal is not touched by the end of a refinement job.
    ready_change(root, "qqqqqqqq");
    assert!(plan_refinement_ended(&ProjectImage::load(root), "qqqqqqqq").is_empty());
}

#[test]
fn a_refined_proposal_handed_over_is_approved_waits_for_a_run_or_goes_back_to_open() {
    let dir = project();
    let root = dir.path();
    ready_change(root, "pppppppp");
    let front = fs::read_to_string(root.join(".weaver/proposals/pppppppp-t/proposal.md")).unwrap();
    put(
        root,
        "proposals/pppppppp-t/proposal.md",
        &front.replace("status: open", "status: refining\nrefining: true"),
    );
    let waits = submit_plan(root, "pppppppp", &submit_ctx(false, true)).unwrap();
    assert_eq!(waits.outcome, Submitted::WaitsForRun);
    assert!(waits.changes.is_empty());
    let approved = submit_plan(root, "pppppppp", &submit_ctx(false, false)).unwrap();
    assert!(matches!(approved.outcome, Submitted::Approved(_)));
    assert!(approved.consumed_request.is_none());
    // Still blocked by something other than staleness: Open, with the reasons.
    put(
        root,
        "proposals/pppppppp-t/proposal.md",
        &front.replace(
            "status: open",
            "status: refining\nrefining: true\nrequires:\n  - zzzzzzzz",
        ),
    );
    let open = submit_plan(root, "pppppppp", &submit_ctx(false, false)).unwrap();
    assert_eq!(open.outcome, Submitted::Open);
    assert_eq!(open.warnings, ["Proposal zzzzzzzz is missing"]);
}

#[test]
fn a_refined_proposal_attached_to_a_drift_is_always_open_and_a_refined_setup_keeps_its_kind() {
    let dir = project();
    let root = dir.path();
    ready_change(root, "pppppppp");
    let refining = |root: &Path| {
        let path = root.join(".weaver/proposals/pppppppp-t/proposal.md");
        let text = fs::read_to_string(&path).unwrap();
        fs::write(
            path,
            text.replace("status: open", "status: refining\nrefining: true"),
        )
        .unwrap();
    };
    // Attached to a Drift it carries no Task: its scope is covered by what the code already does.
    fs::remove_file(root.join(".weaver/proposals/pppppppp-t/tasks/tttttttt-pay.md")).unwrap();
    let path = root.join(".weaver/proposals/pppppppp-t/proposal.md");
    let text = fs::read_to_string(&path).unwrap();
    fs::write(
        path,
        format!("{text}\n## Already present\n\n- `order` — `src/order.rs`\n"),
    )
    .unwrap();
    refining(root);
    put(
        root,
        "drifts/dddddddd-d.md",
        &drift_file("open", "pppppppp"),
    );
    // Nothing blocks it, and still it is not approved nor does it wait for a Run.
    for run in [false, true] {
        let open = submit_plan(root, "pppppppp", &submit_ctx(false, run)).unwrap();
        assert_eq!(open.outcome, Submitted::Open, "run: {run}");
        let Some(Change::Write(text)) = open.changes.get("proposals/pppppppp-t/proposal.md") else {
            panic!()
        };
        assert!(
            text.contains("status: open") && !text.contains("refining"),
            "{text}"
        );
    }

    // A refined setup is approved at hand-over like any change and still starts Run all.
    fs::remove_file(root.join(".weaver/drifts/dddddddd-d.md")).unwrap();
    ready_change(root, "pppppppp");
    with_kind(root, "pppppppp", "setup");
    refining(root);
    let approved = submit_plan(root, "pppppppp", &submit_ctx(false, false)).unwrap();
    let Submitted::Approved(plan) = approved.outcome else {
        panic!("{:?}", approved.outcome)
    };
    assert!(plan.start_run_all);
    let Some(Change::Write(text)) = plan.changes.get("proposals/pppppppp-t/proposal.md") else {
        panic!()
    };
    assert!(
        text.contains("kind: setup") && text.contains("status: approved"),
        "{text}"
    );
}

#[test]
fn an_approval_that_fails_half_way_leaves_every_file_as_it_was() {
    let dir = project();
    let root = dir.path();
    ready_change(root, "pppppppp");
    // `tasks` is a file: the Task cannot be created, after the Model and the Proposal were written.
    fs::write(root.join(".weaver/tasks"), "in the way").unwrap();
    let before = snapshot(root);
    let result = approve(
        root,
        &AgentFacts::default(),
        "pppppppp",
        Decider::Developer,
        NOW,
        &mut rng(),
    );
    assert!(
        matches!(result, Err(MutationError::Write { .. })),
        "{:?}",
        result.map(|(_, a)| a)
    );
    assert_eq!(
        snapshot(root),
        before,
        "the files written before the failure are put back"
    );
    assert!(
        !fs::read_to_string(root.join(".weaver/model/shop/order.md"))
            .unwrap()
            .contains("The customer paid.")
    );
}

// ---------------------------------------------------------------------------------------------
// Reject, rework, discard
// ---------------------------------------------------------------------------------------------

fn reject_plan(
    root: &Path,
    id: &str,
    feedback: Option<&str>,
    rework: bool,
    gate: Gate,
    by: Decider,
) -> Result<Rejection, Refusal> {
    let (image, _, numbers) = state(root);
    plan_reject(&image, &numbers, id, feedback, rework, gate, by)
}

fn written(changes: &ChangeSet, id: &str) -> String {
    match changes.get(&format!("proposals/{id}-t/proposal.md")) {
        Some(Change::Write(text)) => text.clone(),
        other => panic!("{other:?}"),
    }
}

#[test]
fn reject_keeps_the_reason_only_when_it_has_text() {
    let dir = project();
    let root = dir.path();
    ready_change(root, "pppppppp");
    let with = reject_plan(
        root,
        "pppppppp",
        Some("  Too big \n"),
        false,
        Gate::default(),
        Decider::Developer,
    )
    .unwrap();
    let text = written(&with.changes, "pppppppp");
    assert!(
        text.contains("status: rejected") && text.ends_with("Why.\n\n## Feedback\n\nToo big\n"),
        "{text}"
    );
    assert!(!with.rework && !text.contains("reworkRequested"));
    for blank in [None, Some("   \n")] {
        let text = written(
            &reject_plan(
                root,
                "pppppppp",
                blank,
                false,
                Gate::default(),
                Decider::Developer,
            )
            .unwrap()
            .changes,
            "pppppppp",
        );
        assert!(
            text.contains("status: rejected") && !text.contains("## Feedback"),
            "{text}"
        );
    }
}

#[test]
fn rework_marks_the_proposal_and_is_allowed_once() {
    let dir = project();
    let root = dir.path();
    ready_change(root, "pppppppp");
    let rejection = reject_plan(
        root,
        "pppppppp",
        Some("Split it"),
        true,
        Gate::default(),
        Decider::Developer,
    )
    .unwrap();
    assert!(rejection.rework);
    let text = written(&rejection.changes, "pppppppp");
    assert!(
        text.contains("status: rejected")
            && text.contains("reworkRequested: true")
            && text.contains("## Feedback\n\nSplit it"),
        "{text}"
    );
    // A Proposal that already reworks another one is not reworked again.
    let open = fs::read_to_string(root.join(".weaver/proposals/pppppppp-t/proposal.md")).unwrap();
    put(
        root,
        "proposals/pppppppp-t/proposal.md",
        &open.replace("kind: change", "kind: change\nreworks: oooooooo"),
    );
    let refusal = reject_plan(
        root,
        "pppppppp",
        Some("again"),
        true,
        Gate::default(),
        Decider::Developer,
    )
    .unwrap_err();
    assert!(refusal.0[0].contains("at most once"), "{refusal:?}");
    // A rework without feedback is the UI's to confirm; the rule accepts it.
    put(root, "proposals/pppppppp-t/proposal.md", &open);
    assert!(reject_plan(
        root,
        "pppppppp",
        None,
        true,
        Gate::default(),
        Decider::Developer
    )
    .is_ok());
}

#[test]
fn reject_and_rework_are_refused_while_the_agent_finishes_or_the_proposal_is_refined_or_attached() {
    let dir = project();
    let root = dir.path();
    ready_change(root, "pppppppp");
    let finishing = Gate {
        agent_finishing: true,
        run_in_progress: false,
    };
    for rework in [false, true] {
        let refusal = reject_plan(
            root,
            "pppppppp",
            None,
            rework,
            finishing,
            Decider::Developer,
        )
        .unwrap_err();
        assert_eq!(refusal.0, ["The agent is still finishing"]);
    }
    // A run does not matter to Reject.
    let run = Gate {
        agent_finishing: false,
        run_in_progress: true,
    };
    assert!(reject_plan(root, "pppppppp", None, false, run, Decider::Developer).is_ok());

    let open = fs::read_to_string(root.join(".weaver/proposals/pppppppp-t/proposal.md")).unwrap();
    put(
        root,
        "proposals/pppppppp-t/proposal.md",
        &open.replace("status: open", "status: refining\nrefining: true"),
    );
    assert_eq!(
        reject_plan(
            root,
            "pppppppp",
            None,
            true,
            Gate::default(),
            Decider::Developer
        )
        .unwrap_err()
        .0,
        ["Being refined"]
    );
    // A refinement may end in the developer removing a change that is already in the Model.
    assert!(reject_plan(
        root,
        "pppppppp",
        Some("already there"),
        false,
        Gate::default(),
        Decider::Developer
    )
    .is_ok());

    put(root, "proposals/pppppppp-t/proposal.md", &open);
    put(
        root,
        "drifts/dddddddd-d.md",
        &drift_file("open", "pppppppp"),
    );
    assert!(reject_plan(
        root,
        "pppppppp",
        None,
        false,
        Gate::default(),
        Decider::Developer
    )
    .unwrap_err()
    .0[0]
        .contains("through its Drift"));
    assert!(reject_plan(
        root,
        "pppppppp",
        None,
        false,
        Gate::default(),
        Decider::Drift
    )
    .is_ok());
}

#[test]
fn discard_is_for_a_draft_no_job_writes_and_for_the_drift_of_an_attached_one() {
    let dir = project();
    let root = dir.path();
    proposal(root, "pppppppp", "status: draft\n", "# P\n", &[], &[]);
    let (image, _, numbers) = state(root);
    let plan = |gate: Gate, by: Decider| plan_discard(&image, &numbers, "pppppppp", gate, by);
    let changes = plan(Gate::default(), Decider::Developer).unwrap();
    assert!(written(&changes, "pppppppp").contains("status: discarded"));
    let finishing = Gate {
        agent_finishing: true,
        run_in_progress: false,
    };
    assert_eq!(
        plan(finishing, Decider::Developer).unwrap_err().0,
        ["The agent is still finishing"]
    );

    proposal(root, "pppppppp", "status: open\n", "# P\n", &[], &[]);
    let (image, _, numbers) = state(root);
    assert!(plan_discard(
        &image,
        &numbers,
        "pppppppp",
        Gate::default(),
        Decider::Developer
    )
    .is_err());
    assert!(plan_discard(
        &image,
        &numbers,
        "pppppppp",
        Gate::default(),
        Decider::Drift
    )
    .is_ok());
    proposal(root, "pppppppp", "status: approved\n", "# P\n", &[], &[]);
    let (image, _, numbers) = state(root);
    assert!(plan_discard(
        &image,
        &numbers,
        "pppppppp",
        Gate::default(),
        Decider::Drift
    )
    .is_err());
}

#[test]
fn the_model_can_be_chosen_only_for_an_open_proposal_that_carries_tasks() {
    let dir = project();
    let root = dir.path();
    ready_change(root, "pppppppp");
    proposal(root, "qqqqqqqq", "status: open\n", "# Q\n", &[], &[]);
    proposal(
        root,
        "rrrrrrrr",
        "status: draft\n",
        "# R\n",
        &[],
        &[("33333333", "r", &task_file("R", true, &[]))],
    );
    let image = ProjectImage::load(root);
    assert!(check_model_choice(&image, "pppppppp").is_ok());
    assert!(check_model_choice(&image, "qqqqqqqq").is_err());
    assert!(check_model_choice(&image, "rrrrrrrr").is_err());
}

// ---------------------------------------------------------------------------------------------
// Resolution
// ---------------------------------------------------------------------------------------------

fn drift_file(status: &str, proposal: &str) -> String {
    let attached = if proposal.is_empty() {
        String::new()
    } else {
        format!("proposal: {proposal}\n")
    };
    format!(
        "---\nstatus: {status}\nrefs:\n  - order/Paid\n  - order/Ghost\n{attached}created: 2026-11-02T06:00:00.000Z\n---\n# The code ships twice\n\n## What happens\n\nIt ships twice.\n\n## What should happen\n\nIt ships once.\n\n## Fix options\n\n### Deduplicate\n\nKeep one shipment.\n\n### Lock\n\nLock the order.\n\n## Where in the code\n\n`src/ship.rs`\n"
    )
}

fn resolution_front(drifts: &str) -> String {
    format!("status: open\nkind: resolution\ndrifts:\n{drifts}")
}

#[test]
fn a_resolution_checks_its_drifts_at_hand_over() {
    let dir = project();
    let root = dir.path();
    put(root, "drifts/dddddddd-d.md", &drift_file("open", ""));
    put(root, "drifts/eeeeeeee-e.md", &drift_file("dismissed", ""));
    proposal(
        root,
        "oooooooo",
        &resolution_front("  - drift: dddddddd\n    decision: dismissed\n"),
        "# Other\n",
        &[],
        &[],
    );
    let front = format!(
        "status: draft\nkind: resolution\ndrifts:\n{}",
        concat!(
            "  - drift: dddddddd\n    decision: fix-code\n",
            "  - drift: eeeeeeee\n    decision: dismissed\n",
            "  - drift: ffffffff\n    decision: dismissed\n",
            "  - drift: dddddddd\n    decision: proposal\n",
        )
    );
    proposal(root, "pppppppp", &front, "# R\n", &[], &[]);
    let all = submit_plan(root, "pppppppp", &submit_ctx(false, false))
        .unwrap_err()
        .0
        .join("\n");
    assert!(all.contains("is already in the resolution"), "{all}");
    assert!(
        all.contains("`fix-code` needs `option`, from 1 to 2"),
        "{all}"
    );
    assert!(all.contains("Drift eeeeeeee is not open"), "{all}");
    assert!(all.contains("Drift ffffffff does not exist"), "{all}");
    assert!(all.contains("listed twice"), "{all}");

    proposal(
        root,
        "pppppppp",
        "status: draft\nkind: resolution\n",
        "# R\n",
        &[],
        &[],
    );
    assert!(submit_plan(root, "pppppppp", &submit_ctx(false, false))
        .unwrap_err()
        .0
        .iter()
        .any(|r| r.contains("lists no Drift")));
    let bad = "  - drift: dddddddd\n    decision: fix-code\n    option: 3\n";
    proposal(
        root,
        "pppppppp",
        &format!("status: draft\nkind: resolution\ndrifts:\n{bad}"),
        "# R\n",
        &[],
        &[],
    );
    assert!(submit_plan(root, "pppppppp", &submit_ctx(false, false))
        .unwrap_err()
        .0
        .iter()
        .any(|r| r.contains("option 3 does not exist (from 1 to 2)")));
    let misplaced = "  - drift: dddddddd\n    decision: dismissed\n    option: 1\n    note: x\n";
    proposal(
        root,
        "pppppppp",
        &format!("status: draft\nkind: resolution\ndrifts:\n{misplaced}"),
        "# R\n",
        &[],
        &[],
    );
    let all = submit_plan(root, "pppppppp", &submit_ctx(false, false))
        .unwrap_err()
        .0
        .join("\n");
    assert!(
        all.contains("`option` goes only with `fix-code`")
            && all.contains("`note` goes only with `fix-code`"),
        "{all}"
    );
}

#[test]
fn approving_a_resolution_gives_each_open_drift_its_decision() {
    let dir = project();
    let root = dir.path();
    put(root, "drifts/dddddddd-d.md", &drift_file("open", ""));
    put(root, "drifts/eeeeeeee-e.md", &drift_file("open", ""));
    put(root, "drifts/ffffffff-f.md", &drift_file("open", ""));
    put(root, "drifts/gggggggg-g.md", &drift_file("fixed", ""));
    let drifts = concat!(
        "  - drift: dddddddd\n    decision: fix-code\n    option: 2\n    note: Mind the lock\n",
        "  - drift: eeeeeeee\n    decision: proposal\n",
        "  - drift: ffffffff\n    decision: dismissed\n",
        "  - drift: gggggggg\n    decision: dismissed\n",
    );
    proposal(
        root,
        "pppppppp",
        &resolution_front(drifts),
        "# Resolve\n",
        &[],
        &[],
    );
    let (after, approval) = approve(
        root,
        &AgentFacts::default(),
        "pppppppp",
        Decider::Developer,
        NOW,
        &mut rng(),
    )
    .unwrap();
    let Approval::Approve(plan) = approval else {
        panic!()
    };
    assert_eq!(plan.fix_tasks.len(), 1);
    assert_eq!(
        plan.decided_drifts,
        [
            ("dddddddd".to_owned(), "code-to-be-fixed"),
            ("eeeeeeee".to_owned(), "proposal-accepted"),
            ("ffffffff".to_owned(), "dismissed")
        ]
    );
    let status = |id: &str| {
        after
            .card(CardKind::Drift, id)
            .unwrap()
            .str_key("status")
            .unwrap()
            .to_owned()
    };
    assert_eq!(
        (
            status("dddddddd").as_str(),
            status("eeeeeeee").as_str(),
            status("ffffffff").as_str()
        ),
        ("code-to-be-fixed", "proposal-accepted", "dismissed")
    );
    assert_eq!(
        status("gggggggg"),
        "fixed",
        "a Drift that is no longer open is left as it is"
    );
    let fix = after.card(CardKind::Task, &plan.fix_tasks[0]).unwrap();
    assert_eq!(fix.str_key("drift"), Some("dddddddd"));
    assert_eq!(
        tasks::refs_of(fix),
        ["order/Paid", "order"],
        "the State that is gone is replaced by its Machine"
    );
    assert_eq!(fix.title, "Fix the code: The code ships twice");
    for expected in [
        "## What happens\n\nIt ships twice.",
        "## Chosen option\n\n### Lock\n\nLock the order.",
        "## Note of the developer\n\nMind the lock",
        "## Where in the code\n\n`src/ship.rs`",
    ] {
        assert!(fix.body.contains(expected), "{}", fix.body);
    }
    let drift = after.card(CardKind::Drift, "dddddddd").unwrap();
    assert_eq!(
        (
            drift.str_key("task"),
            drift.str_key("note"),
            drift.str_key("decided")
        ),
        (
            Some(plan.fix_tasks[0].as_str()),
            Some("Mind the lock"),
            Some(NOW)
        )
    );
    assert_eq!(drift.front.get("option").and_then(|v| v.as_u64()), Some(2));
}

#[test]
fn a_code_fix_that_could_no_longer_be_applied_blocks_the_approval() {
    let dir = project();
    let root = dir.path();
    let gone =
        drift_file("open", "").replace("  - order/Paid\n  - order/Ghost\n", "  - ghost/Nowhere\n");
    put(root, "drifts/dddddddd-d.md", &gone);
    proposal(
        root,
        "pppppppp",
        &resolution_front("  - drift: dddddddd\n    decision: fix-code\n    option: 1\n"),
        "# R\n",
        &[],
        &[],
    );
    let a = analyse_of(root, "pppppppp");
    assert_eq!(tags(&a.blocking()), ["fix"]);
    let (_, _, numbers) = state(root);
    assert_eq!(
        a.blocking()[0].text(&numbers),
        "Drift D1: the code fix can no longer be applied: no Model element it concerns is left"
    );
    assert!(a
        .handover_refusals(&numbers)
        .iter()
        .any(|r| r.contains("code fix can no longer be applied")));
}

#[test]
fn the_nearest_container_replaces_a_missing_element_and_a_missing_section_is_dropped() {
    let dir = project();
    let root = dir.path();
    let drift = drift_file("open", "").replace(
        "  - order/Paid\n  - order/Ghost\n",
        "  - order/Paid/user.CANCEL\n  - order/Gone/user.CANCEL\n  - order/Gone\n  - user.NOPE\n  - requirements/ui#order/Gone\n  - requirements/ui#General\n  - specs#data\n  - specs#languages\n",
    );
    put(root, "drifts/dddddddd-d.md", &drift);
    let image = ProjectImage::load(root);
    let card = image.card(CardKind::Drift, "dddddddd").unwrap();
    // Transition → State → Machine; Event → Actor; the sections that exist stay, the others go.
    assert_eq!(
        resolution::fix_refs(&image, &image, card),
        [
            "order/Paid/user.CANCEL",
            "order",
            "user",
            "requirements/ui#General",
            "specs#languages"
        ]
    );
}

// ---------------------------------------------------------------------------------------------
// new and edit
// ---------------------------------------------------------------------------------------------

fn new_input(title: &str) -> NewProposal {
    NewProposal {
        title: title.to_owned(),
        ..NewProposal::default()
    }
}

fn plan_new_in(root: &Path, input: &NewProposal) -> Result<super::create::Created, Vec<String>> {
    let (image, _, numbers) = state(root);
    plan_new(&image, &numbers, input, CREATED, &mut rng())
}

#[test]
fn a_new_proposal_is_a_draft_with_its_keys_in_order() {
    let dir = project();
    let mut input = new_input("Cancel an order");
    input.body = Some("Why.".into());
    input.label = Some(" Cancel ".into());
    input.requires = vec!["qqqqqqqq".into()];
    input.job = Some("jjjjjjjj".into());
    let created = plan_new_in(dir.path(), &input).unwrap();
    assert!(
        created.path.starts_with("proposals/")
            && created.path.ends_with("-cancel-an-order/proposal.md"),
        "{}",
        created.path
    );
    let Some(Change::Write(text)) = created.changes.get(&created.path) else {
        panic!()
    };
    assert_eq!(
        text,
        &format!("---\nstatus: draft\nkind: change\nrequires:\n- qqqqqqqq\ncreated: {CREATED}\nlabel: Cancel\njob: jjjjjjjj\n---\n# Cancel an order\n\nWhy.\n")
    );
    assert!(created.note.is_none());
}

#[test]
fn a_new_proposal_is_refused_with_every_problem() {
    let dir = project();
    let mut input = new_input("  ");
    input.kind = Some(Kind::Setup);
    input.label = Some("x".repeat(31));
    input.request = Some("r".into());
    input.reworks = Some("w".into());
    let problems = plan_new_in(dir.path(), &input).unwrap_err();
    assert_eq!(problems.len(), 4, "{problems:?}");
    assert!(problems.iter().any(|p| p.starts_with("title:")));
    assert!(problems.iter().any(|p| p.starts_with("label:")));
    assert!(problems
        .iter()
        .any(|p| p.contains("`setup` is for a project whose Model has no Feature and no Actor")));
    assert!(problems
        .iter()
        .any(|p| p.contains("cannot go together with `reworks`")));
    // A setup is fine on an empty Model.
    let empty = empty_project();
    let mut setup = new_input("Shop");
    setup.kind = Some(Kind::Setup);
    assert!(plan_new_in(empty.path(), &setup).is_ok());
}

fn job_of_request(request: &str) -> Writer {
    Writer::Job {
        request: Some(request.into()),
        reworks: None,
    }
}

fn job_of_rework(target: &str) -> Writer {
    Writer::Job {
        request: None,
        reworks: Some(target.into()),
    }
}

fn note_of(created: Result<super::create::Created, Vec<String>>) -> String {
    created.unwrap().note.expect("a note saying why")
}

#[test]
fn a_job_binds_only_the_request_its_instruction_names_and_only_with_its_first_proposal() {
    let dir = project();
    let root = dir.path();
    put(root, "requests/2026-11-02T07-00-00.md", "# A request\n");
    put(root, "requests/2026-11-02T08-00-00.md", "# Another\n");
    let mut input = new_input("First");
    input.request = Some("2026-11-02T07-00-00".into());
    input.writer = job_of_request("2026-11-02T07-00-00");
    let first = plan_new_in(root, &input).unwrap();
    assert!(first.note.is_none());
    let Some(Change::Write(text)) = first.changes.get(&first.path) else {
        panic!()
    };
    assert!(text.contains("request: 2026-11-02T07-00-00"));
    assert!(!text.contains("chat"));

    // Once a Proposal is bound to it, another is created unbound with a note.
    proposal(
        root,
        "pppppppp",
        "status: draft\nrequest: 2026-11-02T07-00-00\n",
        "# P\n",
        &[],
        &[],
    );
    let second = plan_new_in(root, &input).unwrap();
    assert!(
        second
            .note
            .as_deref()
            .unwrap()
            .contains("is not bound to the request"),
        "{:?}",
        second.note
    );
    let Some(Change::Write(text)) = second.changes.get(&second.path) else {
        panic!()
    };
    assert!(!text.contains("request:"));

    // Another request than the one the job received binds nothing, though it exists.
    let mut other = new_input("Other");
    other.request = Some("2026-11-02T08-00-00".into());
    other.writer = job_of_request("2026-11-02T07-00-00");
    assert!(note_of(plan_new_in(root, &other)).contains("the job did not receive that request"));
    // A job with no request binds none.
    other.writer = Writer::Job {
        request: None,
        reworks: None,
    };
    assert!(note_of(plan_new_in(root, &other)).contains("did not receive"));
    // A request that does not exist binds nothing.
    let mut missing = new_input("Missing");
    missing.request = Some("2020-01-01T00-00-00".into());
    missing.writer = job_of_request("2020-01-01T00-00-00");
    assert!(note_of(plan_new_in(root, &missing)).contains("no such request"));
}

#[test]
fn a_rework_job_binds_only_the_proposal_its_instruction_names_if_rejected_with_rework_and_not_replaced(
) {
    let dir = project();
    let root = dir.path();
    proposal(
        root,
        "oooooooo",
        "status: rejected\nreworkRequested: true\n",
        "# Old\n",
        &[],
        &[],
    );
    proposal(
        root,
        "nnnnnnnn",
        "status: rejected\n",
        "# Not reworked\n",
        &[],
        &[],
    );
    let mut rework = new_input("Again");
    rework.reworks = Some("oooooooo".into());
    rework.writer = job_of_rework("oooooooo");
    let bound = plan_new_in(root, &rework).unwrap();
    assert!(bound.note.is_none());
    let Some(Change::Write(text)) = bound.changes.get(&bound.path) else {
        panic!()
    };
    assert!(text.contains("reworks: oooooooo"));
    // Not rejected with Rework, though the instruction names it.
    rework.reworks = Some("nnnnnnnn".into());
    rework.writer = job_of_rework("nnnnnnnn");
    assert!(note_of(plan_new_in(root, &rework)).contains("not rejected with Rework"));
    // Another Proposal than the one the instruction names.
    rework.writer = job_of_rework("oooooooo");
    assert!(note_of(plan_new_in(root, &rework)).contains("does not rework that Proposal"));
    // A job of another kind names none.
    rework.reworks = Some("oooooooo".into());
    rework.writer = job_of_request("2026-11-02T07-00-00");
    assert!(note_of(plan_new_in(root, &rework)).contains("does not rework that Proposal"));
    // Replaced already.
    proposal(
        root,
        "mmmmmmmm",
        "status: draft\nreworks: oooooooo\n",
        "# Replacement\n",
        &[],
        &[],
    );
    rework.writer = job_of_rework("oooooooo");
    assert!(note_of(plan_new_in(root, &rework)).contains("is not bound to the Proposal it reworks"));
}

#[test]
fn no_other_caller_binds_a_request_or_a_proposal_and_the_note_says_so() {
    let dir = project();
    let root = dir.path();
    put(root, "requests/2026-11-02T07-00-00.md", "# A request\n");
    proposal(
        root,
        "oooooooo",
        "status: rejected\nreworkRequested: true\n",
        "# Old\n",
        &[],
        &[],
    );
    let mut input = new_input("Nobody");
    input.request = Some("2026-11-02T07-00-00".into());
    let created = plan_new_in(root, &input).unwrap();
    let note = created.note.clone().unwrap();
    assert!(note.contains("only the agent of the job that received the request"));
    let Some(Change::Write(text)) = created.changes.get(&created.path) else {
        panic!()
    };
    assert!(!text.contains("request:"));
    let mut rework = new_input("Nobody again");
    rework.reworks = Some("oooooooo".into());
    assert!(note_of(plan_new_in(root, &rework)).contains("only the agent of a rework job"));
}

#[test]
fn only_a_working_job_keeps_the_agent_finishing_and_a_draft_of_chat_is_an_ordinary_draft() {
    let dir = project();
    let root = dir.path();
    proposal(
        root,
        "aaaaaaaa",
        "status: draft\nchat: true\nchatWriting: true\n",
        "# Of an earlier version\n",
        &[],
        &[],
    );
    proposal(
        root,
        "bbbbbbbb",
        "status: draft\njob: jjjjjjjj\n",
        "# A job's\n",
        &[],
        &[],
    );
    let (image, _, numbers) = state(root);
    let working = AgentFacts {
        working_jobs: ["jjjjjjjj".to_owned()].into(),
        ..AgentFacts::default()
    };
    let gate =
        |facts: &AgentFacts, id: &str| gate_of(facts, image.card(CardKind::Proposal, id).unwrap());
    assert!(!gate(&working, "aaaaaaaa").agent_finishing);
    assert!(plan_discard(
        &image,
        &numbers,
        "aaaaaaaa",
        gate(&working, "aaaaaaaa"),
        Decider::Developer
    )
    .is_ok());
    assert_eq!(
        plan_discard(
            &image,
            &numbers,
            "bbbbbbbb",
            gate(&working, "bbbbbbbb"),
            Decider::Developer
        )
        .unwrap_err()
        .0,
        ["The agent is still finishing"]
    );
    assert!(plan_discard(
        &image,
        &numbers,
        "bbbbbbbb",
        gate(&AgentFacts::default(), "bbbbbbbb"),
        Decider::Developer
    )
    .is_ok());
}

fn edit_plan(
    root: &Path,
    id: &str,
    request: &EditRequest,
) -> Result<super::create::Edited, Vec<String>> {
    let (image, folders, _) = state(root);
    plan_edit(
        &image,
        &folders,
        id,
        request,
        &|path| super::create::read_project_file(root, path),
        &mut rng(),
    )
}

fn edit_of(paths: &[&str]) -> EditRequest {
    EditRequest {
        edit: paths.iter().map(|p| (*p).to_owned()).collect(),
        ..EditRequest::default()
    }
}

#[test]
fn edit_copies_the_current_content_records_base_once_and_sends_an_open_proposal_back_to_draft() {
    let dir = project();
    let root = dir.path();
    proposal(root, "pppppppp", "status: open\n", "# P\n", &[], &[]);
    let edited = edit_plan(
        root,
        "pppppppp",
        &edit_of(&["model/shop/order.md", "model/shop/new.md"]),
    )
    .unwrap();
    assert!(edited.sent_back_to_draft);
    assert_eq!(edited.status, Status::Draft);
    assert_eq!(edited.files[0].path, "model/shop/order.md");
    assert_eq!(
        edited.files[0].file,
        "proposals/pppppppp-t/model/shop/order.md"
    );
    let Some(Change::Write(copy)) = edited
        .changes
        .get("proposals/pppppppp-t/model/shop/order.md")
    else {
        panic!()
    };
    assert_eq!(copy, ORDER);
    assert!(
        edited
            .changes
            .get("proposals/pppppppp-t/model/shop/new.md")
            .is_none(),
        "a new file is written by the agent"
    );
    let Some(Change::Write(text)) = edited.changes.get("proposals/pppppppp-t/proposal.md") else {
        panic!()
    };
    assert!(
        text.contains("status: draft")
            && text.contains(&format!("model/shop/order.md: {}", hash(ORDER)))
            && text.contains("model/shop/new.md: new"),
        "{text}"
    );

    // Written, then edited again: `base` is kept, and the copy is not made again.
    let mut changes = edited.changes.clone();
    changes.write("proposals/pppppppp-t/model/shop/order.md", changed_order());
    crate::project::changes::apply(root, &changes, |_| Ok::<(), ()>(())).unwrap();
    put(
        root,
        "model/shop/order.md",
        &ORDER.replace("The customer is choosing.", "The customer is browsing."),
    );
    let again = edit_plan(root, "pppppppp", &edit_of(&["model/shop/order.md"])).unwrap();
    assert!(!again.sent_back_to_draft, "it is a Draft already");
    assert!(again
        .changes
        .get("proposals/pppppppp-t/model/shop/order.md")
        .is_none());
    let Some(Change::Write(text)) = again.changes.get("proposals/pppppppp-t/proposal.md") else {
        panic!()
    };
    assert!(
        text.contains(&hash(ORDER)),
        "base keeps the first hash: {text}"
    );
}

#[test]
fn edit_without_input_still_sends_an_open_proposal_back_to_draft_and_a_decided_one_is_refused() {
    let dir = project();
    let root = dir.path();
    proposal(root, "pppppppp", "status: open\n", "# P\n", &[], &[]);
    let edited = edit_plan(root, "pppppppp", &EditRequest::default()).unwrap();
    assert!(edited.sent_back_to_draft && edited.files.is_empty());
    proposal(root, "pppppppp", "status: refining\n", "# P\n", &[], &[]);
    assert_eq!(
        edit_plan(root, "pppppppp", &EditRequest::default())
            .unwrap()
            .status,
        Status::Refining
    );
    proposal(root, "pppppppp", "status: approved\n", "# P\n", &[], &[]);
    assert!(edit_plan(root, "pppppppp", &EditRequest::default()).is_err());
}

#[test]
fn edit_gives_a_new_task_its_identity_and_keeps_the_identity_of_an_existing_one() {
    let dir = project();
    let root = dir.path();
    proposal(
        root,
        "pppppppp",
        "status: draft\n",
        "# P\n",
        &[],
        &[("aaaaaaaa", "known", &task_file("Known", true, &[]))],
    );
    let edited = edit_plan(
        root,
        "pppppppp",
        &edit_of(&["tasks/cancel-order.md", "tasks/known.md"]),
    )
    .unwrap();
    let new = &edited.files[0];
    assert!(
        new.path.starts_with("tasks/")
            && new.path.ends_with("-cancel-order.md")
            && new.path.len() == "tasks/".len() + 8 + "-cancel-order.md".len(),
        "{}",
        new.path
    );
    assert_eq!(new.file, format!("proposals/pppppppp-t/{}", new.path));
    let Some(Change::Write(skeleton)) = edited.changes.get(&new.file) else {
        panic!()
    };
    assert!(
        skeleton.contains("# Cancel order") && skeleton.contains("technical: false"),
        "{skeleton}"
    );
    assert_eq!(edited.files[1].path, "tasks/aaaaaaaa-known.md");
    assert!(edited
        .changes
        .get("proposals/pppppppp-t/tasks/aaaaaaaa-known.md")
        .is_none());
    // The identity is the Task's whole life: asking again for the same slug finds it.
    crate::project::changes::apply(root, &edited.changes, |_| Ok::<(), ()>(())).unwrap();
    let again = edit_plan(root, "pppppppp", &edit_of(&["tasks/cancel-order.md"])).unwrap();
    assert_eq!(again.files[0].path, new.path);
}

#[test]
fn edit_refuses_task_paths_in_an_import_and_a_resolution_and_bad_paths_and_every_problem_at_once() {
    let dir = project();
    let root = dir.path();
    proposal(
        root,
        "pppppppp",
        "status: draft\nkind: import\n",
        "# P\n",
        &[],
        &[],
    );
    let problems = edit_plan(
        root,
        "pppppppp",
        &edit_of(&[
            "tasks/a.md",
            "elsewhere.md",
            "../x.md",
            "model/shop/.hidden.md",
        ]),
    )
    .unwrap_err();
    assert_eq!(problems.len(), 4, "{problems:?}");
    assert!(problems[0].contains("carries no Task"));
    assert!(problems[1].contains("is not a file of the Model"));
    proposal(
        root,
        "pppppppp",
        "status: draft\nkind: resolution\n",
        "# P\n",
        &[],
        &[],
    );
    assert!(edit_plan(root, "pppppppp", &edit_of(&["tasks/a.md"])).is_err());
}

#[test]
fn delete_is_refused_for_a_path_the_proposal_holds_or_that_does_not_exist() {
    let dir = project();
    let root = dir.path();
    let front = format!(
        "status: draft\n{}",
        base_lines(&[("model/shop/order.md", &hash(ORDER))])
    );
    proposal(
        root,
        "pppppppp",
        &front,
        "# P\n",
        &[("model/shop/order.md", ORDER)],
        &[],
    );
    let request = EditRequest {
        delete: vec!["model/shop/order.md".into(), "model/shop/none.md".into()],
        ..EditRequest::default()
    };
    let problems = edit_plan(root, "pppppppp", &request).unwrap_err();
    assert_eq!(problems.len(), 2, "{problems:?}");
    assert!(problems[0].contains("is held by this Proposal"));
    assert!(problems[1].contains("does not exist"));
    let ok = EditRequest {
        delete: vec!["model/actors/user.md".into()],
        ..EditRequest::default()
    };
    let edited = edit_plan(root, "pppppppp", &ok).unwrap();
    let Some(Change::Write(text)) = edited.changes.get("proposals/pppppppp-t/proposal.md") else {
        panic!()
    };
    assert!(
        text.contains("delete:\n- model/actors/user.md")
            && text.contains(&format!("model/actors/user.md: {}", hash(USER))),
        "{text}"
    );
}

#[test]
fn drifts_belong_to_a_resolution() {
    let dir = project();
    let root = dir.path();
    let decision = |decision: &str, option: Option<u32>, note: Option<&str>| DriftDecision {
        drift: "dddddddd".into(),
        decision: decision.into(),
        option,
        note: note.map(str::to_owned),
    };
    proposal(root, "pppppppp", "status: draft\n", "# P\n", &[], &[]);
    let request = EditRequest {
        drifts: vec![decision("dismissed", None, None)],
        ..EditRequest::default()
    };
    assert!(edit_plan(root, "pppppppp", &request).unwrap_err()[0].contains("only a resolution"));

    proposal(
        root,
        "pppppppp",
        "status: open\nkind: resolution\n",
        "# R\n",
        &[],
        &[],
    );
    let request = EditRequest {
        drifts: vec![
            decision("fix-code", Some(2), Some("note")),
            decision("dismissed", Some(1), None),
        ],
        ..EditRequest::default()
    };
    assert!(
        edit_plan(root, "pppppppp", &request).unwrap_err()[0].contains("go only with `fix-code`")
    );
    let request = EditRequest {
        drifts: vec![decision("fix-code", Some(2), Some("note"))],
        ..EditRequest::default()
    };
    let edited = edit_plan(root, "pppppppp", &request).unwrap();
    let Some(Change::Write(text)) = edited.changes.get("proposals/pppppppp-t/proposal.md") else {
        panic!()
    };
    assert!(
        text.contains("drifts:")
            && text.contains("decision: fix-code")
            && text.contains("option: 2")
            && text.contains("note: note"),
        "{text}"
    );
    // Replaced, then removed.
    crate::project::changes::apply(root, &edited.changes, |_| Ok::<(), ()>(())).unwrap();
    let request = EditRequest {
        remove_drifts: vec!["dddddddd".into()],
        ..EditRequest::default()
    };
    let removed = edit_plan(root, "pppppppp", &request).unwrap();
    let Some(Change::Write(text)) = removed.changes.get("proposals/pppppppp-t/proposal.md") else {
        panic!()
    };
    assert!(text.contains("drifts: []"), "{text}");

    // Rebase of a resolution is served as for any other kind (`rebase_serves_every_kind…`).
    assert!(edit_plan(
        root,
        "pppppppp",
        &EditRequest {
            rebase: true,
            ..EditRequest::default()
        }
    )
    .is_ok());
}

/// The `requires` of `id` in the project after `changes`.
fn requires_after(root: &Path, changes: &ChangeSet, id: &str) -> Vec<String> {
    let image = ProjectImage::load(root).with_changes(changes);
    data::requires_of(image.card(CardKind::Proposal, id).unwrap())
}

#[test]
fn rebase_drops_broken_requirements_and_closed_drifts_of_a_resolution_and_lists_them() {
    let dir = project();
    let root = dir.path();
    let current = ORDER.replace("The customer is choosing.", "The customer is browsing.");
    put(root, "model/shop/order.md", &current);
    put(root, "drifts/dddddddd-d.md", &drift_file("open", ""));
    put(root, "drifts/eeeeeeee-e.md", &drift_file("dismissed", ""));
    proposal(root, "aaaaaaaa", "status: approved\n", "# A\n", &[], &[]);
    proposal(root, "rrrrrrrr", "status: rejected\n", "# R\n", &[], &[]);
    proposal(root, "ssssssss", "status: discarded\n", "# S\n", &[], &[]);
    // Rejected with Rework, nothing handed over yet: it counts as still being written.
    proposal(
        root,
        "oooooooo",
        "status: rejected\nreworkRequested: true\n",
        "# O\n",
        &[],
        &[],
    );
    // Rejected with Rework, and the rework was handed over: it is dropped.
    proposal(
        root,
        "tttttttt",
        "status: rejected\nreworkRequested: true\n",
        "# T\n",
        &[],
        &[],
    );
    proposal(
        root,
        "uuuuuuuu",
        "status: open\nreworks: tttttttt\n",
        "# U\n",
        &[],
        &[],
    );
    proposal(root, "qqqqqqqq", "status: open\n", "# Q\n", &[], &[]);
    // A cycle: only the entry of the refined Proposal goes; the other Proposal is not touched.
    proposal(
        root,
        "cccccccc",
        "status: open\nrequires:\n  - pppppppp\n",
        "# C\n",
        &[],
        &[],
    );
    let front = format!(
        "status: open\nkind: resolution\nrequires:\n  - rrrrrrrr\n  - ssssssss\n  - mmmmmmmm\n  - aaaaaaaa\n  - oooooooo\n  - tttttttt\n  - qqqqqqqq\n  - cccccccc\ndrifts:\n  - drift: dddddddd\n    decision: fix-code\n    option: 1\n    note: Lock it\n  - drift: eeeeeeee\n    decision: dismissed\n  - drift: ffffffff\n    decision: dismissed\n{}",
        base_lines(&[("model/shop/order.md", &hash(ORDER))])
    );
    proposal(
        root,
        "pppppppp",
        &front,
        "# R\n",
        &[("model/shop/order.md", &changed_order())],
        &[],
    );
    let edited = edit_plan(
        root,
        "pppppppp",
        &EditRequest {
            rebase: true,
            ..EditRequest::default()
        },
    )
    .unwrap();
    let dropped: Vec<&str> = edited
        .dropped_requires
        .iter()
        .map(|r| r.id.as_str())
        .collect();
    assert_eq!(
        dropped,
        ["rrrrrrrr", "ssssssss", "mmmmmmmm", "tttttttt", "cccccccc"]
    );
    let problems: Vec<&str> = edited
        .dropped_requires
        .iter()
        .map(|r| r.problem.unwrap().text())
        .collect();
    assert_eq!(
        problems,
        [
            "was rejected",
            "was discarded",
            "is missing",
            "was rejected",
            "forms a cycle"
        ]
    );
    let rows: Vec<&str> = edited
        .dropped_drifts
        .iter()
        .map(|r| r.drift.as_str())
        .collect();
    assert_eq!(rows, ["eeeeeeee", "ffffffff"]);
    assert_eq!(
        requires_after(root, &edited.changes, "pppppppp"),
        ["aaaaaaaa", "oooooooo", "qqqqqqqq"]
    );
    assert!(
        edited
            .changes
            .get("proposals/cccccccc-t/proposal.md")
            .is_none(),
        "the other Proposal of the cycle is not touched"
    );
    let image = ProjectImage::load(root).with_changes(&edited.changes);
    let card = image.card(CardKind::Proposal, "pppppppp").unwrap();
    let kept = crate::project::board::resolution_rows(card);
    assert_eq!(kept.len(), 1);
    assert_eq!(
        (
            kept[0].drift.as_str(),
            kept[0].option,
            kept[0].note.as_deref()
        ),
        ("dddddddd", Some(1), Some("Lock it")),
        "the decision on a Drift still open stays taken"
    );
    assert!(data::refining_flag(card));
    assert_eq!(
        data::base_of(card).get("model/shop/order.md"),
        Some(&hash(&current)),
        "base is recorded again"
    );
}

#[test]
fn rebase_serves_every_kind_attached_to_a_drift_or_not() {
    let dir = project();
    let root = dir.path();
    let current = ORDER.replace("The customer is choosing.", "The customer is browsing.");
    put(root, "model/shop/order.md", &current);
    for kind in ["change", "setup", "import"] {
        ready_change(root, "pppppppp");
        with_kind(root, "pppppppp", kind);
        for attached in [false, true] {
            if attached {
                put(
                    root,
                    "drifts/dddddddd-d.md",
                    &drift_file("open", "pppppppp"),
                );
            }
            let edited = edit_plan(
                root,
                "pppppppp",
                &EditRequest {
                    rebase: true,
                    ..EditRequest::default()
                },
            )
            .unwrap();
            assert!(edited.dropped_requires.is_empty() && edited.dropped_drifts.is_empty());
            let Some(Change::Write(text)) = edited.changes.get("proposals/pppppppp-t/proposal.md")
            else {
                panic!()
            };
            assert!(
                text.contains("refining: true") && text.contains(&hash(&current)),
                "{kind}: {text}"
            );
        }
    }
}

#[test]
fn rebase_records_base_again_and_sets_refining() {
    let dir = project();
    let root = dir.path();
    let front = format!(
        "status: open\n{}",
        base_lines(&[
            ("model/shop/order.md", &hash(ORDER)),
            ("model/shop/new.md", "new")
        ])
    );
    proposal(
        root,
        "pppppppp",
        &front,
        "# P\n",
        &[("model/shop/order.md", &changed_order())],
        &[],
    );
    let current = ORDER.replace("The customer is choosing.", "The customer is browsing.");
    put(root, "model/shop/order.md", &current);
    let edited = edit_plan(
        root,
        "pppppppp",
        &EditRequest {
            rebase: true,
            ..EditRequest::default()
        },
    )
    .unwrap();
    assert_eq!(edited.status, Status::Draft);
    let Some(Change::Write(text)) = edited.changes.get("proposals/pppppppp-t/proposal.md") else {
        panic!()
    };
    assert!(
        text.contains("refining: true")
            && text.contains(&hash(&current))
            && text.contains("model/shop/new.md: new"),
        "{text}"
    );
    assert!(!text.contains(&hash(ORDER)), "{text}");
    crate::project::changes::apply(root, &edited.changes, |_| Ok::<(), ()>(())).unwrap();
    assert!(
        analyse_of(root, "pppppppp").stale.is_empty(),
        "no longer stale once rebased"
    );
}

#[test]
fn a_rework_takes_the_place_of_the_rejected_proposal_on_the_board() {
    let dir = project();
    let root = dir.path();
    // Created in the order: the rejected one (07:00:00), a later Proposal, then its rework.
    proposal(
        root,
        "oooooooo",
        "status: rejected\nreworkRequested: true\ncreated: 2026-11-02T07:00:00.000Z\n",
        "# Old\n",
        &[],
        &[],
    );
    proposal(
        root,
        "llllllll",
        "status: open\ncreated: 2026-11-02T08:00:00.000Z\n",
        "# Later\n",
        &[],
        &[],
    );
    proposal(
        root,
        "nnnnnnnn",
        "status: open\nreworks: oooooooo\ncreated: 2026-11-02T09:00:00.000Z\n",
        "# New\n",
        &[],
        &[],
    );
    let image = ProjectImage::load(root);
    let order: Vec<String> = board::compute(&image, &AgentFacts::default())
        .backlog
        .iter()
        .map(|e| e.id().to_owned())
        .collect();
    assert_eq!(
        order,
        ["nnnnnnnn", "llllllll"],
        "the rework sits where the rejected Proposal sat"
    );
}
