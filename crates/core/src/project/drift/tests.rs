//! Tests of the rules of a Drift: the fingerprint, the prepared question, `new` and `edit`, every
//! transition, the decisions and their reasons, and what an answer becomes.

use std::fs;
use std::path::Path;

use serde_json::json;

use crate::job_instruction::ResolveAnswer;
use crate::project::board::{AgentFacts, ResolutionRow};
use crate::project::cards::CardKind;
use crate::project::changes::{Change, ChangeSet, MutationError};
use crate::project::image::ProjectImage;
use crate::project::layout;
use crate::project::proposal::analysis;
use crate::project::proposal::data::{self, Status};
use crate::project::proposal::files::{read_folders, Folders};
use crate::project::proposal::fixtures::*;
use crate::project::proposal::transitions::{self, Decider, SubmitContext};
use crate::project::random::ScriptedRandom;
use crate::project::shortnum::{self, ShortNumbers};
use crate::project::tasks;
use crate::question::Answer;

use super::answer::{self, Next};
use super::create::{self, EditDrift, NewDrift, PreparedInput};
use super::decide::{self, Decision};
use super::prepared::{self, Preparation, PreparedDecision};
use super::reasons;
use super::{drift_options, fingerprint, FixOption};

const DRIFT: &str = "dddddddd";
const LATER: &str = "2026-11-02T10:00:00.000Z";

fn rng() -> ScriptedRandom {
    ScriptedRandom::new((0u8..=200).collect::<Vec<u8>>())
}

fn state(root: &Path) -> (ProjectImage, Folders, ShortNumbers) {
    let image = ProjectImage::load(root);
    let folders = read_folders(root, &image);
    let numbers = shortnum::peek(root, image.cards());
    (image, folders, numbers)
}

fn apply(root: &Path, changes: &ChangeSet) {
    for (rel, change) in changes.iter() {
        let path = layout::absolute(root, rel);
        match change {
            Change::Write(content) => {
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::write(path, crate::project::text::file_content(content)).unwrap();
            }
            Change::Delete => {
                let _ = fs::remove_file(path);
            }
        }
    }
}

/// An Open Drift on `order/Paid` and `user`, with two fix options.
fn drift_text(status: &str, extra: &str) -> String {
    format!(
        "---\nstatus: {status}\nrefs:\n  - order/Paid\n  - user\n{extra}created: 2026-11-02T06:00:00.000Z\n---\n# The code ships twice\n\n## What happens\n\nIt ships twice.\n\n## What should happen\n\nIt ships once.\n\n## Fix options\n\n### Deduplicate\n\nKeep one shipment.\n\n### Lock\n\nLock the order.\n\n## Where in the code\n\n`src/ship.rs`\n"
    )
}

fn put_drift(root: &Path, id: &str, status: &str, extra: &str) {
    put(
        root,
        &format!("drifts/{id}-d.md"),
        &drift_text(status, extra),
    );
}

fn drift_of<'a>(image: &'a ProjectImage, id: &str) -> &'a crate::project::cards::Card {
    image.card(CardKind::Drift, id).expect("the Drift exists")
}

fn question(id: &str, labels: &[&str], recommended: usize) -> serde_json::Value {
    let options: Vec<_> = labels
        .iter()
        .map(|l| json!({"label": l, "description": format!("{l} means so")}))
        .collect();
    json!({"id": id, "question": "What now?", "options": options, "recommended": recommended, "multi": false})
}

fn decisions(list: &[(&str, Option<u32>)]) -> Vec<PreparedDecision> {
    list.iter()
        .map(|(d, o)| PreparedDecision {
            decision: (*d).to_owned(),
            option: *o,
        })
        .collect()
}

/// A prepared question for the Drift with its two fix options and the behaviour of the code.
fn prepared_input(behaviour: &str) -> PreparedInput {
    PreparedInput {
        question: question(DRIFT, &["Deduplicate", "Lock", "Keep as it is"], 0),
        decisions: decisions(&[
            ("fix-code", Some(1)),
            ("fix-code", Some(2)),
            (behaviour, None),
        ]),
    }
}

fn prepare(root: &Path, behaviour: &str) {
    let (image, _, _) = state(root);
    let edited = create::plan_edit(
        &image,
        DRIFT,
        &EditDrift {
            prepared: Some(prepared_input(behaviour)),
            proposal: None,
        },
        NOW,
    )
    .unwrap();
    apply(root, &edited.changes);
}

/// An Open Proposal that changes `order` and says the code already does it.
fn attached_proposal(root: &Path, id: &str, status: &str) {
    let front = format!(
        "status: {status}\nkind: change\nbase:\n  model/shop/order.md: {}\n",
        hash(ORDER)
    );
    proposal(
        root,
        id,
        &front,
        "# Paid orders\n\n## Already present\n\n- `order` — src/order.rs\n",
        &[(
            "model/shop/order.md",
            &ORDER.replace("### Paid\n", "### Paid\nThe customer paid.\n"),
        )],
        &[],
    );
}

fn decide_at(
    root: &Path,
    facts: &AgentFacts,
    decision: &Decision,
) -> Result<(ProjectImage, decide::Decided), String> {
    decide::decide(root, facts, DRIFT, decision, LATER, &mut rng()).map_err(|e| match e {
        MutationError::Refused(refused) => refused.notice,
        other => other.to_string(),
    })
}

fn snapshot(root: &Path) -> Vec<(String, Vec<u8>)> {
    fn walk(dir: &Path, out: &mut Vec<(String, Vec<u8>)>) {
        let Ok(listing) = fs::read_dir(dir) else {
            return;
        };
        for item in listing {
            let path = item.unwrap().path();
            if path.is_dir() {
                walk(&path, out);
            } else {
                out.push((
                    path.to_string_lossy().into_owned(),
                    fs::read(&path).unwrap(),
                ));
            }
        }
    }
    let mut out = Vec::new();
    walk(&root.join(".weaver"), &mut out);
    out.sort();
    out
}

// ---------------------------------------------------------------------------------------------
// The fingerprint
// ---------------------------------------------------------------------------------------------

#[test]
fn the_fingerprint_is_taken_over_the_references_in_order_with_their_text() {
    let dir = project();
    let (image, _, _) = state(dir.path());
    let refs = |list: &[&str]| list.iter().map(|r| (*r).to_owned()).collect::<Vec<_>>();
    let text = fingerprint::text_of(&image, &refs(&["order/Paid", "requirements/ui#General"]));
    assert!(text.starts_with("# order/Paid\n"), "{text}");
    assert!(text.contains("\n# requirements/ui#General\n"), "{text}");
    assert!(
        text.contains("- y"),
        "the Requirement sections that apply to the element are in its text: {text}"
    );
    assert_eq!(
        fingerprint::of_refs(&image, &refs(&["order/Paid", "user"])).len(),
        64
    );
    assert_ne!(
        fingerprint::of_refs(&image, &refs(&["order/Paid", "user"])),
        fingerprint::of_refs(&image, &refs(&["user", "order/Paid"])),
        "the order of the references counts"
    );
    assert_eq!(
        fingerprint::stored_of_refs(&image, &refs(&["user"])),
        format!("sha256:{}", fingerprint::of_refs(&image, &refs(&["user"])))
    );
}

#[test]
fn a_missing_reference_adds_missing_and_a_change_of_text_changes_the_fingerprint() {
    let dir = project();
    let root = dir.path();
    let refs = vec!["order/Paid".to_owned(), "ghost/Nowhere".to_owned()];
    let (image, _, _) = state(root);
    assert!(fingerprint::text_of(&image, &refs).ends_with("# ghost/Nowhere\n(missing)\n"));
    let before = fingerprint::of_refs(&image, &refs);
    assert_eq!(
        before,
        fingerprint::of_refs(&image, &refs),
        "the same text, the same fingerprint"
    );

    // The element changes.
    put(
        root,
        "model/shop/order.md",
        &ORDER.replace("### Paid\n", "### Paid\nPaid for.\n"),
    );
    let (changed, _, _) = state(root);
    assert_ne!(before, fingerprint::of_refs(&changed, &refs));

    // A Requirement section titled with the element changes.
    put(root, "model/shop/order.md", ORDER);
    put(root, "requirements/ui/main.md", &MAIN.replace("- y", "- z"));
    let (requirement, _, _) = state(root);
    assert_ne!(before, fingerprint::of_refs(&requirement, &refs));
    assert_ne!(
        fingerprint::of_refs(&image, &["requirements/ui#order/Paid".to_owned()]),
        fingerprint::of_refs(&requirement, &["requirements/ui#order/Paid".to_owned()])
    );
}

#[test]
fn line_endings_never_change_the_fingerprint() {
    let dir = project();
    let root = dir.path();
    let (image, _, _) = state(root);
    let refs = vec!["order/Paid".to_owned()];
    let before = fingerprint::of_refs(&image, &refs);
    fs::write(
        layout::absolute(root, "model/shop/order.md"),
        ORDER.replace('\n', "\r\n"),
    )
    .unwrap();
    fs::write(
        layout::absolute(root, "requirements/ui/main.md"),
        format!("\u{feff}{}", MAIN.replace('\n', "\r\n")),
    )
    .unwrap();
    let (crlf, _, _) = state(root);
    assert_eq!(before, fingerprint::of_refs(&crlf, &refs));
}

// ---------------------------------------------------------------------------------------------
// The prepared question
// ---------------------------------------------------------------------------------------------

#[test]
fn a_prepared_question_is_valid_until_what_it_concerns_changes() {
    let dir = project();
    let root = dir.path();
    put_drift(root, DRIFT, "open", "");
    let (image, _, _) = state(root);
    assert_eq!(
        prepared::check(&image, drift_of(&image, DRIFT)),
        Preparation::Missing
    );

    prepare(root, "dismissed");
    let (image, _, _) = state(root);
    let found = prepared::check(&image, drift_of(&image, DRIFT));
    let valid = found.valid().expect("valid");
    assert_eq!(valid.prepared, NOW);
    assert_eq!(valid.question.id, DRIFT);
    assert_eq!(
        valid.fingerprint,
        fingerprint::stored_of(&image, drift_of(&image, DRIFT))
    );
    assert_eq!(
        valid.decisions[2],
        PreparedDecision {
            decision: "dismissed".into(),
            option: None
        }
    );
    let file = fs::read_to_string(root.join(".weaver/drifts/dddddddd-d.md")).unwrap();
    assert!(
        file.contains("## Prepared question\n\n```json\n{"),
        "{file}"
    );
    assert_eq!(file.matches("```json").count(), 1);

    // The Model changes: the question no longer holds, and nothing changes it by itself.
    put(
        root,
        "model/shop/order.md",
        &ORDER.replace("### Paid\n", "### Paid\nPaid for.\n"),
    );
    let (changed, _, _) = state(root);
    let found = prepared::check(&changed, drift_of(&changed, DRIFT));
    assert!(
        matches!(&found, Preparation::Invalid(_, why) if why.contains("changed since")),
        "{found:?}"
    );
    assert!(!found.is_valid());
    assert_eq!(
        fs::read_to_string(root.join(".weaver/drifts/dddddddd-d.md")).unwrap(),
        file,
        "an invalid question is never changed by itself"
    );
}

#[test]
fn a_prepared_question_holds_only_while_the_drift_is_open_and_its_options_and_proposal_exist() {
    let dir = project();
    let root = dir.path();
    put_drift(root, DRIFT, "open", "proposal: pppppppp\n");
    attached_proposal(root, "pppppppp", "open");
    prepare(root, "proposal");
    let (image, _, _) = state(root);
    assert!(prepared::check(&image, drift_of(&image, DRIFT)).is_valid());

    // The attached Proposal is no longer in Draft or Open.
    let text = fs::read_to_string(root.join(".weaver/proposals/pppppppp-t/proposal.md")).unwrap();
    put(
        root,
        "proposals/pppppppp-t/proposal.md",
        &text.replace("status: open", "status: discarded"),
    );
    let (gone, _, _) = state(root);
    assert!(
        matches!(prepared::check(&gone, drift_of(&gone, DRIFT)), Preparation::Invalid(_, why) if why.contains("attached Proposal"))
    );
    put(root, "proposals/pppppppp-t/proposal.md", &text);

    // A fix option is gone.
    let file = fs::read_to_string(root.join(".weaver/drifts/dddddddd-d.md")).unwrap();
    put(
        root,
        "drifts/dddddddd-d.md",
        &file.replace("### Lock\n\nLock the order.\n\n", ""),
    );
    let (fewer, _, _) = state(root);
    assert!(
        matches!(prepared::check(&fewer, drift_of(&fewer, DRIFT)), Preparation::Invalid(_, why) if why.contains("fix option")),
        "{:?}",
        prepared::check(&fewer, drift_of(&fewer, DRIFT))
    );
    put(root, "drifts/dddddddd-d.md", &file);

    // The Drift is decided.
    put(
        root,
        "drifts/dddddddd-d.md",
        &file.replace("status: open", "status: dismissed"),
    );
    let (decided, _, _) = state(root);
    assert!(
        matches!(prepared::check(&decided, drift_of(&decided, DRIFT)), Preparation::Invalid(_, why) if why.contains("no longer open"))
    );
}

#[test]
fn a_block_that_does_not_read_counts_as_no_question_and_is_reported() {
    let dir = project();
    let root = dir.path();
    let text = drift_text("open", "");
    for (block, expected) in [
        (
            "## Prepared question\n\n```json\n{ not json\n```\n",
            "does not read",
        ),
        ("## Prepared question\n\nNothing here.\n", "no `json` block"),
        (
            "## Prepared question\n\n```json\n{}\n```\n\n```json\n{}\n```\n",
            "more than one",
        ),
    ] {
        put(root, "drifts/dddddddd-d.md", &format!("{text}\n{block}"));
        let (image, _, _) = state(root);
        let found = prepared::check(&image, drift_of(&image, DRIFT));
        assert!(
            matches!(&found, Preparation::Unreadable(why) if why.contains(expected)),
            "{expected}: {found:?}"
        );
        assert!(!found.is_valid() && found.problem().is_some());
        assert!(
            reasons::is_unprepared(&image, drift_of(&image, DRIFT)),
            "it counts as no prepared question"
        );
    }
}

#[test]
fn a_new_preparation_replaces_the_old_one() {
    let dir = project();
    let root = dir.path();
    put_drift(root, DRIFT, "open", "");
    prepare(root, "dismissed");
    let (image, _, _) = state(root);
    let edited = create::plan_edit(
        &image,
        DRIFT,
        &EditDrift {
            prepared: Some(PreparedInput {
                question: question(DRIFT, &["Deduplicate", "Lock", "Leave it"], 1),
                decisions: decisions(&[
                    ("fix-code", Some(1)),
                    ("fix-code", Some(2)),
                    ("dismissed", None),
                ]),
            }),
            proposal: None,
        },
        LATER,
    )
    .unwrap();
    apply(root, &edited.changes);
    let file = fs::read_to_string(root.join(".weaver/drifts/dddddddd-d.md")).unwrap();
    assert_eq!(file.matches("## Prepared question").count(), 1);
    let (image, _, _) = state(root);
    let valid = prepared::check(&image, drift_of(&image, DRIFT));
    assert_eq!(
        (
            valid.valid().unwrap().prepared.as_str(),
            valid.valid().unwrap().question.recommended
        ),
        (LATER, 1)
    );
}

// ---------------------------------------------------------------------------------------------
// drift edit
// ---------------------------------------------------------------------------------------------

fn edit_problems(root: &Path, input: &EditDrift) -> Vec<(String, String)> {
    let (image, _, _) = state(root);
    create::plan_edit(&image, DRIFT, input, NOW)
        .unwrap_err()
        .into_iter()
        .map(|p| (p.path, p.message))
        .collect()
}

#[test]
fn edit_checks_the_shape_of_the_question_and_of_the_decisions() {
    let dir = project();
    let root = dir.path();
    put_drift(root, DRIFT, "open", "");
    let ask = |question: serde_json::Value, list: &[(&str, Option<u32>)]| {
        edit_problems(
            root,
            &EditDrift {
                prepared: Some(PreparedInput {
                    question,
                    decisions: decisions(list),
                }),
                proposal: None,
            },
        )
    };
    let ok = [
        ("fix-code", Some(1)),
        ("fix-code", Some(2)),
        ("dismissed", None),
    ];

    let wrong_id = ask(question("eeeeeeee", &["a", "b", "c"], 0), &ok);
    assert!(
        wrong_id
            .iter()
            .any(|(path, why)| path == "prepared.question.id"
                && why.contains("identity of the Drift")),
        "{wrong_id:?}"
    );
    let mut multi = question(DRIFT, &["a", "b", "c"], 0);
    multi["multi"] = json!(true);
    assert!(ask(multi, &ok)
        .iter()
        .any(|(path, _)| path == "prepared.question.multi"));
    let few = ask(
        question(DRIFT, &["a", "b"], 0),
        &[("fix-code", Some(1)), ("dismissed", None)],
    );
    assert!(
        few.iter()
            .any(|(path, why)| path == "prepared.question.options"
                && why.contains("needs 3 options")),
        "{few:?}"
    );
    let count = ask(question(DRIFT, &["a", "b", "c"], 0), &ok[..2]);
    assert!(
        count.iter().any(|(path, _)| path == "prepared.decisions"),
        "{count:?}"
    );
    let order = ask(
        question(DRIFT, &["a", "b", "c"], 0),
        &[
            ("fix-code", Some(2)),
            ("fix-code", Some(1)),
            ("dismissed", None),
        ],
    );
    assert!(
        order
            .iter()
            .any(|(path, _)| path == "prepared.decisions[0]")
            && order
                .iter()
                .any(|(path, _)| path == "prepared.decisions[1]"),
        "{order:?}"
    );
    let last = ask(
        question(DRIFT, &["a", "b", "c"], 0),
        &[
            ("fix-code", Some(1)),
            ("fix-code", Some(2)),
            ("fix-code", Some(1)),
        ],
    );
    assert!(
        last.iter()
            .any(|(path, why)| path == "prepared.decisions[2]"
                && why.contains("behaviour the code has")),
        "{last:?}"
    );
    let needs_proposal = ask(
        question(DRIFT, &["a", "b", "c"], 0),
        &[
            ("fix-code", Some(1)),
            ("fix-code", Some(2)),
            ("proposal", None),
        ],
    );
    assert!(
        needs_proposal
            .iter()
            .any(|(_, why)| why.contains("needs a Proposal in Draft or Open")),
        "{needs_proposal:?}"
    );
    let invalid_round = ask(
        json!({"id": DRIFT, "question": " ", "options": [], "recommended": 0, "multi": false}),
        &ok,
    );
    assert!(
        invalid_round
            .iter()
            .any(|(path, _)| path.starts_with("prepared.question")),
        "{invalid_round:?}"
    );

    // Nothing was written by any refusal.
    assert_eq!(
        fs::read_to_string(root.join(".weaver/drifts/dddddddd-d.md")).unwrap(),
        drift_text("open", "")
    );
}

#[test]
fn a_drift_without_fix_options_has_exactly_proposal_then_dismissed() {
    let dir = project();
    let root = dir.path();
    let bare = drift_text("open", "proposal: pppppppp\n").replace("## Fix options\n\n### Deduplicate\n\nKeep one shipment.\n\n### Lock\n\nLock the order.\n\n", "");
    put(root, "drifts/dddddddd-d.md", &bare);
    attached_proposal(root, "pppppppp", "draft");
    let (image, _, _) = state(root);
    assert!(drift_options(drift_of(&image, DRIFT)).is_empty());
    let good = EditDrift {
        prepared: Some(PreparedInput {
            question: question(DRIFT, &["Adopt it", "Close it"], 0),
            decisions: decisions(&[("proposal", None), ("dismissed", None)]),
        }),
        proposal: None,
    };
    let edited = create::plan_edit(&image, DRIFT, &good, NOW).unwrap();
    apply(root, &edited.changes);
    assert!(state(root)
        .0
        .card(CardKind::Drift, DRIFT)
        .is_some_and(|d| prepared::check(&state(root).0, d).is_valid()));

    let swapped = EditDrift {
        prepared: Some(PreparedInput {
            question: question(DRIFT, &["Close it", "Adopt it"], 0),
            decisions: decisions(&[("dismissed", None), ("proposal", None)]),
        }),
        proposal: None,
    };
    assert!(!edit_problems(root, &swapped).is_empty());
    let not_recommended = EditDrift {
        prepared: Some(PreparedInput {
            question: question(DRIFT, &["Adopt it", "Close it"], 1),
            decisions: decisions(&[("proposal", None), ("dismissed", None)]),
        }),
        proposal: None,
    };
    assert!(edit_problems(root, &not_recommended)
        .iter()
        .any(|(path, _)| path == "prepared.question.recommended"));
}

#[test]
fn merge_is_written_by_drift_new_before_created_and_kept_by_every_later_write() {
    let dir = project();
    let root = dir.path();
    let (image, _, _) = state(root);
    let mut input = new_input();
    input.job = Some("jjjjjjjj".into());
    input.merge = true;
    let filed = create::plan_new(&image, &input, NOW, &mut rng()).unwrap();
    let Some(Change::Write(text)) = filed.changes.get(&filed.path) else {
        panic!()
    };
    let keys: Vec<&str> = text
        .lines()
        .skip(1)
        .take_while(|l| *l != "---")
        .filter(|l| !l.starts_with(' ') && !l.starts_with('-'))
        .map(|l| l.split(':').next().unwrap())
        .collect();
    assert_eq!(keys, ["status", "refs", "job", "merge", "created"]);
    assert!(text.contains("\nmerge: true\n"), "{text}");
    apply(root, &filed.changes);
    let (image, _, _) = state(root);
    assert!(super::is_merge(drift_of(&image, &filed.id)));

    // A Drift that is not asked to come from a merge never has the key.
    let plain = create::plan_new(&image, &new_input(), NOW, &mut rng()).unwrap();
    let Some(Change::Write(text)) = plain.changes.get(&plain.path) else {
        panic!()
    };
    assert!(!text.contains("merge"), "{text}");

    // Preparing its question and dismissing it leave the key as it was.
    put_drift(root, DRIFT, "open", "merge: true\nproposal: pppppppp\n");
    attached_proposal(root, "pppppppp", "draft");
    let (image, _, _) = state(root);
    let edited = create::plan_edit(
        &image,
        DRIFT,
        &EditDrift {
            prepared: Some(merge_question()),
            proposal: None,
        },
        NOW,
    )
    .unwrap();
    apply(root, &edited.changes);
    assert!(super::is_merge(drift_of(&state(root).0, DRIFT)));
    let (image, _) = decide_at(root, &AgentFacts::default(), &Decision::Dismiss).unwrap();
    assert!(super::is_merge(drift_of(&image, DRIFT)));
}

/// The question of a merge Drift with two fix options: adapting the Model is the third, the one
/// recommended.
fn merge_question() -> PreparedInput {
    PreparedInput {
        question: question(DRIFT, &["Deduplicate", "Lock", "Adapt the Model"], 2),
        decisions: decisions(&[
            ("fix-code", Some(1)),
            ("fix-code", Some(2)),
            ("proposal", None),
        ]),
    }
}

#[test]
fn the_prepared_question_of_a_merge_drift_always_recommends_adapting_the_model() {
    let dir = project();
    let root = dir.path();
    put_drift(root, DRIFT, "open", "merge: true\nproposal: pppppppp\n");
    attached_proposal(root, "pppppppp", "draft");
    let ask = |input: PreparedInput| {
        edit_problems(
            root,
            &EditDrift {
                prepared: Some(input),
                proposal: None,
            },
        )
    };
    let (image, _, _) = state(root);
    let edited = create::plan_edit(
        &image,
        DRIFT,
        &EditDrift {
            prepared: Some(merge_question()),
            proposal: None,
        },
        NOW,
    )
    .unwrap();
    apply(root, &edited.changes);
    let (image, _, _) = state(root);
    let drift = drift_of(&image, DRIFT);
    assert!(prepared::check(&image, drift).is_valid());
    // The fix options keep their order, the first one still listed first.
    let stored = prepared::read_block(drift).unwrap().unwrap();
    assert_eq!(
        stored
            .question
            .options
            .iter()
            .map(|o| o.label.as_str())
            .collect::<Vec<_>>(),
        ["Deduplicate", "Lock", "Adapt the Model"]
    );

    // The usual recommendation of the first fix option is refused for a merge Drift.
    let first = ask(PreparedInput {
        question: question(DRIFT, &["Deduplicate", "Lock", "Adapt the Model"], 0),
        decisions: merge_question().decisions,
    });
    assert_eq!(first.len(), 1, "{first:?}");
    assert_eq!(first[0].0, "prepared.question.recommended");
    assert!(first[0].1.contains("must be 2"), "{first:?}");

    // Closing the Drift is not what the code's behaviour is: `proposal` is always offered.
    let dismissed = ask(PreparedInput {
        question: question(DRIFT, &["Deduplicate", "Lock", "Close it"], 2),
        decisions: decisions(&[
            ("fix-code", Some(1)),
            ("fix-code", Some(2)),
            ("dismissed", None),
        ]),
    });
    assert!(
        dismissed
            .iter()
            .any(|(path, why)| path == "prepared.decisions[2]" && why.contains("proposal")),
        "{dismissed:?}"
    );

    // The same questions are fine for a Drift that does not come from a merge.
    put_drift(root, DRIFT, "open", "proposal: pppppppp\n");
    assert!(create::plan_edit(
        &state(root).0,
        DRIFT,
        &EditDrift {
            prepared: Some(PreparedInput {
                question: question(DRIFT, &["Deduplicate", "Lock", "Close it"], 0),
                decisions: decisions(&[
                    ("fix-code", Some(1)),
                    ("fix-code", Some(2)),
                    ("dismissed", None),
                ]),
            }),
            proposal: None,
        },
        NOW,
    )
    .is_ok());
}

#[test]
fn a_merge_drift_without_fix_options_recommends_adopting_the_attached_proposal() {
    let dir = project();
    let root = dir.path();
    let bare = drift_text("open", "proposal: pppppppp\nmerge: true\n").replace("## Fix options\n\n### Deduplicate\n\nKeep one shipment.\n\n### Lock\n\nLock the order.\n\n", "");
    put(root, "drifts/dddddddd-d.md", &bare);
    attached_proposal(root, "pppppppp", "open");
    let good = EditDrift {
        prepared: Some(PreparedInput {
            question: question(DRIFT, &["Adopt it", "Close it"], 0),
            decisions: decisions(&[("proposal", None), ("dismissed", None)]),
        }),
        proposal: None,
    };
    let edited = create::plan_edit(&state(root).0, DRIFT, &good, NOW).unwrap();
    apply(root, &edited.changes);
    let (image, _, _) = state(root);
    assert!(prepared::check(&image, drift_of(&image, DRIFT)).is_valid());
}

#[test]
fn a_question_prepared_before_without_the_merge_rule_is_not_valid_for_a_merge_drift() {
    let dir = project();
    let root = dir.path();
    put_drift(root, DRIFT, "open", "proposal: pppppppp\n");
    attached_proposal(root, "pppppppp", "draft");
    // Prepared as for any Drift: the first fix option recommended, `proposal` third.
    prepare(root, "proposal");
    let (image, _, _) = state(root);
    assert!(prepared::check(&image, drift_of(&image, DRIFT)).is_valid());

    // The Drift is a merge Drift: the same question no longer holds.
    let file = fs::read_dir(root.join(".weaver/drifts"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let text = fs::read_to_string(&file).unwrap();
    fs::write(
        &file,
        text.replacen("\ncreated:", "\nmerge: true\ncreated:", 1),
    )
    .unwrap();
    let (image, _, _) = state(root);
    let drift = drift_of(&image, DRIFT);
    assert!(
        matches!(prepared::check(&image, drift), prepared::Preparation::Invalid(_, why) if why.contains("a Drift from a merge")),
        "{:?}",
        prepared::check(&image, drift)
    );

    // Unless its recommended option is the `proposal` decision.
    let mut text = fs::read_to_string(&file).unwrap();
    text = text.replace("\"recommended\": 0", "\"recommended\": 2");
    fs::write(&file, text).unwrap();
    let (image, _, _) = state(root);
    assert!(
        prepared::check(&image, drift_of(&image, DRIFT)).is_valid(),
        "{:?}",
        prepared::check(&image, drift_of(&image, DRIFT))
    );
}

#[test]
fn edit_attaches_a_proposal_to_a_drift_that_has_none_and_only_to_an_open_drift() {
    let dir = project();
    let root = dir.path();
    put_drift(root, DRIFT, "open", "");
    attached_proposal(root, "pppppppp", "draft");
    let (image, _, _) = state(root);
    let request = EditDrift {
        prepared: Some(prepared_input("proposal")),
        proposal: Some("pppppppp".to_owned()),
    };
    let edited = create::plan_edit(&image, DRIFT, &request, NOW).unwrap();
    assert_eq!(edited.attached.as_deref(), Some("pppppppp"));
    apply(root, &edited.changes);
    let (image, _, _) = state(root);
    assert_eq!(
        drift_of(&image, DRIFT).str_key("proposal"),
        Some("pppppppp")
    );
    assert!(
        prepared::check(&image, drift_of(&image, DRIFT)).is_valid(),
        "a `proposal` decision counts the Proposal attached by the same command"
    );

    // A Drift that has one refuses another; so does a Proposal another Drift holds.
    let again = create::plan_edit(
        &image,
        DRIFT,
        &EditDrift {
            prepared: None,
            proposal: Some("pppppppp".to_owned()),
        },
        NOW,
    )
    .unwrap_err();
    assert!(again[0].message.contains("attached already"), "{again:?}");
    put_drift(root, "eeeeeeee", "open", "");
    let (image, _, _) = state(root);
    let held = create::plan_edit(
        &image,
        "eeeeeeee",
        &EditDrift {
            prepared: None,
            proposal: Some("pppppppp".to_owned()),
        },
        NOW,
    )
    .unwrap_err();
    assert!(held[0].message.contains("another Drift holds"), "{held:?}");

    put_drift(root, "ffffffff", "dismissed", "");
    let (image, _, _) = state(root);
    let closed = create::plan_edit(&image, "ffffffff", &EditDrift::default(), NOW).unwrap_err();
    assert!(
        closed[0].message.contains("only an Open Drift changes"),
        "{closed:?}"
    );
}

// ---------------------------------------------------------------------------------------------
// drift new
// ---------------------------------------------------------------------------------------------

fn new_input() -> NewDrift {
    NewDrift {
        title: "The code ships twice".into(),
        refs: vec![
            "order/Paid".into(),
            "requirements/ui#General".into(),
            "order/Paid".into(),
        ],
        happens: "It ships twice.".into(),
        should: "It ships once.".into(),
        where_in_code: "`src/ship.rs`".into(),
        options: vec![
            FixOption {
                title: "Deduplicate".into(),
                text: "Keep one shipment.".into(),
            },
            FixOption {
                title: "Lock".into(),
                text: "Lock the order.".into(),
            },
        ],
        ..NewDrift::default()
    }
}

fn new_problems(root: &Path, input: &NewDrift) -> Vec<(String, String)> {
    let (image, _, _) = state(root);
    create::plan_new(&image, input, NOW, &mut rng())
        .unwrap_err()
        .into_iter()
        .map(|p| (p.path, p.message))
        .collect()
}

#[test]
fn a_new_drift_is_open_with_its_keys_and_sections_in_order() {
    let dir = project();
    let root = dir.path();
    put(root, "tasks/aaaaaaaa-t.md", crate::project::testing::TASK);
    let mut input = new_input();
    input.task = Some("aaaaaaaa".into());
    input.job = Some("jjjjjjjj".into());
    let (image, _, _) = state(root);
    let filed = create::plan_new(&image, &input, NOW, &mut rng()).unwrap();
    assert!(
        filed.path.starts_with("drifts/") && filed.path.ends_with("-the-code-ships-twice.md"),
        "{}",
        filed.path
    );
    let Some(Change::Write(text)) = filed.changes.get(&filed.path) else {
        panic!()
    };
    let keys: Vec<&str> = text
        .lines()
        .skip(1)
        .take_while(|l| *l != "---")
        .filter(|l| !l.starts_with(' ') && !l.starts_with('-'))
        .map(|l| l.split(':').next().unwrap())
        .collect();
    assert_eq!(keys, ["status", "refs", "origin", "job", "created"]);
    assert!(
        text.contains("refs:\n- order/Paid\n- requirements/ui#General\n"),
        "references are written once: {text}"
    );
    let headings: Vec<&str> = text.lines().filter(|l| l.starts_with('#')).collect();
    assert_eq!(
        headings,
        [
            "# The code ships twice",
            "## What happens",
            "## What should happen",
            "## Fix options",
            "### Deduplicate",
            "### Lock",
            "## Where in the code"
        ]
    );

    apply(root, &filed.changes);
    let (image, _, _) = state(root);
    let card = drift_of(&image, &filed.id);
    assert_eq!(
        (
            card.str_key("status"),
            card.str_key("origin"),
            card.str_key("job"),
            card.created.as_deref()
        ),
        (Some("open"), Some("aaaaaaaa"), Some("jjjjjjjj"), Some(NOW))
    );
    assert_eq!(drift_options(card).len(), 2);
    assert_eq!(
        super::section_text(&card.body, super::WHERE_IN_CODE),
        "`src/ship.rs`"
    );
}

#[test]
fn a_drift_found_during_a_fix_task_repeats_the_drift_of_that_task() {
    let dir = project();
    let root = dir.path();
    put(root, "tasks/ffffffff-fix.md", "---\ncolumn: doing\ntechnical: false\nrefs:\n  - order\ndrift: eeeeeeee\nholder: jjjjjjjj\ncreated: 2026-11-02T07:00:01.000Z\n---\n# Fix\n");
    put_drift(root, "eeeeeeee", "code-to-be-fixed", "task: ffffffff\n");
    put(root, "drifts/gggggggg-g.md", &drift_text("dismissed", ""));
    let mut input = new_input();
    input.task = Some("ffffffff".into());
    let (image, _, _) = state(root);
    let filed = create::plan_new(&image, &input, NOW, &mut rng()).unwrap();
    assert_eq!(filed.previous.as_deref(), Some("eeeeeeee"));
    let Some(Change::Write(text)) = filed.changes.get(&filed.path) else {
        panic!()
    };
    assert!(text.contains("previous: eeeeeeee\n"), "{text}");

    // An explicit `previous` wins; a task that is not a fix Task sets none.
    input.previous = Some("gggggggg".into());
    assert_eq!(
        create::plan_new(&image, &input, NOW, &mut rng())
            .unwrap()
            .previous
            .as_deref(),
        Some("gggggggg")
    );
    put(root, "tasks/aaaaaaaa-t.md", crate::project::testing::TASK);
    let (image, _, _) = state(root);
    input.previous = None;
    input.task = Some("aaaaaaaa".into());
    assert_eq!(
        create::plan_new(&image, &input, NOW, &mut rng())
            .unwrap()
            .previous,
        None
    );
}

#[test]
fn a_new_drift_offers_two_to_five_options_or_none_only_with_a_proposal_and_names_real_things() {
    let dir = project();
    let root = dir.path();
    let mut bare = new_input();
    bare.options.clear();
    let found = new_problems(root, &bare);
    assert!(
        found
            .iter()
            .any(|(path, why)| path == "options" && why.contains("attached `proposal`")),
        "{found:?}"
    );

    let mut one = new_input();
    one.options.truncate(1);
    assert!(new_problems(root, &one)
        .iter()
        .any(|(path, why)| path == "options" && why.contains("2 to 5")));
    let mut six = new_input();
    six.options = (0..6)
        .map(|n| FixOption {
            title: format!("O{n}"),
            text: "t".into(),
        })
        .collect();
    assert!(new_problems(root, &six)
        .iter()
        .any(|(path, why)| path == "options" && why.contains("2 to 5")));

    let mut wrong = new_input();
    wrong.title = " ".into();
    wrong.refs = vec!["ghost".into(), "order//".into()];
    wrong.happens = " ".into();
    wrong.task = Some("zzzzzzzz".into());
    wrong.previous = Some("zzzzzzzz".into());
    wrong.proposal = Some("zzzzzzzz".into());
    let paths: Vec<String> = new_problems(root, &wrong)
        .into_iter()
        .map(|(path, _)| path)
        .collect();
    for expected in [
        "title", "refs[0]", "refs[1]", "happens", "task", "previous", "proposal",
    ] {
        assert!(
            paths.contains(&expected.to_owned()),
            "{expected} in {paths:?}"
        );
    }
    let mut none = new_input();
    none.refs.clear();
    assert!(new_problems(root, &none)
        .iter()
        .any(|(path, _)| path == "refs"));

    // A Drift with no options but with a Proposal in Draft or Open that no other Drift holds is fine.
    attached_proposal(root, "pppppppp", "draft");
    let (image, _, _) = state(root);
    assert!(create::plan_new(&image, &bare_with("pppppppp"), NOW, &mut rng()).is_ok());
    put_drift(root, "eeeeeeee", "open", "proposal: pppppppp\n");
    let held = new_problems(root, &bare_with("pppppppp"));
    assert!(
        held.iter()
            .any(|(path, why)| path == "proposal" && why.contains("another Drift holds")),
        "{held:?}"
    );
    let resolution = put_resolution(root);
    let found = new_problems(root, &bare_with(&resolution));
    assert!(
        found
            .iter()
            .any(|(path, why)| path == "proposal" && why.contains("resolution")),
        "{found:?}"
    );
}

fn bare_with(proposal: &str) -> NewDrift {
    let mut input = new_input();
    input.options.clear();
    input.proposal = Some(proposal.to_owned());
    input
}

fn put_resolution(root: &Path) -> String {
    proposal(
        root,
        "rrrrrrrr",
        "status: draft\nkind: resolution\n",
        "# R\n",
        &[],
        &[],
    );
    "rrrrrrrr".to_owned()
}

// ---------------------------------------------------------------------------------------------
// Fix the code
// ---------------------------------------------------------------------------------------------

#[test]
fn fixing_the_code_creates_the_fix_task_and_puts_the_origin_back_to_todo() {
    let dir = project();
    let root = dir.path();
    put(
        root,
        "tasks/aaaaaaaa-t.md",
        &crate::project::testing::TASK.replace("column: todo", "column: doing\nholder: jjjjjjjj"),
    );
    put_drift(
        root,
        DRIFT,
        "open",
        "origin: aaaaaaaa\nproposal: pppppppp\n",
    );
    attached_proposal(root, "pppppppp", "open");
    let (image, decided) = decide_at(
        root,
        &AgentFacts::default(),
        &Decision::FixCode {
            option: Some(2),
            note: Some(" Mind the lock ".into()),
        },
    )
    .unwrap();
    assert_eq!(decided.status, "code-to-be-fixed");
    let fix_id = decided.fix_task.clone().expect("a fix Task");

    let drift = drift_of(&image, DRIFT);
    assert_eq!(
        (
            drift.str_key("status"),
            drift.str_key("task"),
            drift.str_key("note"),
            drift.str_key("decided")
        ),
        (
            Some("code-to-be-fixed"),
            Some(fix_id.as_str()),
            Some("Mind the lock"),
            Some(LATER)
        )
    );
    assert_eq!(drift.front.get("option").and_then(|v| v.as_u64()), Some(2));
    let fix = image.card(CardKind::Task, &fix_id).unwrap();
    assert_eq!(
        (fix.str_key("drift"), fix.str_key("column")),
        (Some(DRIFT), Some("todo"))
    );
    assert_eq!(fix.title, "Fix the code: The code ships twice");
    assert_eq!(tasks::refs_of(fix), ["order/Paid", "user"]);
    assert!(!tasks::is_technical(fix));
    for expected in [
        "## What happens\n\nIt ships twice.",
        "## What should happen\n\nIt ships once.",
        "## Chosen option\n\n### Lock\n\nLock the order.",
        "## Note of the developer\n\nMind the lock",
        "## Where in the code\n\n`src/ship.rs`",
    ] {
        assert!(fix.body.contains(expected), "{}", fix.body);
    }
    let origin = image.card(CardKind::Task, "aaaaaaaa").unwrap();
    assert_eq!(
        (origin.str_key("column"), origin.str_key("holder")),
        (Some("todo"), None),
        "the origin goes back to To do"
    );
    // It waits for the fix Task: blocked by nothing else, waiting for the fix.
    assert_eq!(tasks::blocking_of(&image, origin).fix_tasks, [fix_id]);
    // `drift.CODE_TO_BE_FIXED`: an Open attached Proposal is rejected.
    assert_eq!(
        data::status_of(image.card(CardKind::Proposal, "pppppppp").unwrap()),
        Some(Status::Rejected)
    );
}

#[test]
fn a_draft_attached_proposal_is_discarded_when_the_code_is_to_be_fixed_and_when_the_drift_is_dismissed(
) {
    for (decision, expected) in [
        (
            Decision::FixCode {
                option: Some(1),
                note: None,
            },
            Status::Discarded,
        ),
        (Decision::Dismiss, Status::Discarded),
    ] {
        let dir = project();
        let root = dir.path();
        put_drift(root, DRIFT, "open", "proposal: pppppppp\n");
        attached_proposal(root, "pppppppp", "draft");
        let (image, _) = decide_at(root, &AgentFacts::default(), &decision).unwrap();
        assert_eq!(
            data::status_of(image.card(CardKind::Proposal, "pppppppp").unwrap()),
            Some(expected)
        );
    }
    // An Open one is discarded by a dismissal, rejected by a code fix.
    let dir = project();
    let root = dir.path();
    put_drift(root, DRIFT, "open", "proposal: pppppppp\n");
    attached_proposal(root, "pppppppp", "open");
    let (image, _) = decide_at(root, &AgentFacts::default(), &Decision::Dismiss).unwrap();
    assert_eq!(
        data::status_of(image.card(CardKind::Proposal, "pppppppp").unwrap()),
        Some(Status::Discarded)
    );
}

#[test]
fn the_fix_option_is_required_when_the_drift_offers_some_and_refused_when_it_offers_none() {
    let dir = project();
    let root = dir.path();
    put_drift(root, DRIFT, "open", "");
    let before = snapshot(root);
    for (option, expected) in [
        (None, "an option is required: the Drift offers 2"),
        (Some(3), "option 3 does not exist (from 1 to 2)"),
        (Some(0), "option 0 does not exist"),
    ] {
        let why = decide_at(
            root,
            &AgentFacts::default(),
            &Decision::FixCode { option, note: None },
        )
        .unwrap_err();
        assert!(
            why.starts_with("Could not fix the code of D1: ") && why.contains(expected),
            "{why}"
        );
    }
    assert_eq!(snapshot(root), before, "a refusal writes nothing");

    let bare = drift_text("open", "").replace("## Fix options\n\n### Deduplicate\n\nKeep one shipment.\n\n### Lock\n\nLock the order.\n\n", "");
    put(root, "drifts/dddddddd-d.md", &bare);
    let why = decide_at(
        root,
        &AgentFacts::default(),
        &Decision::FixCode {
            option: Some(1),
            note: None,
        },
    )
    .unwrap_err();
    assert!(why.contains("offers no fix option"), "{why}");
    let (image, decided) = decide_at(
        root,
        &AgentFacts::default(),
        &Decision::FixCode {
            option: None,
            note: Some("Do it".into()),
        },
    )
    .unwrap();
    assert_eq!(decided.status, "code-to-be-fixed");
    assert_eq!(drift_of(&image, DRIFT).front.get("option"), None);
}

#[test]
fn fixing_the_code_uses_the_nearest_container_and_is_refused_when_nothing_is_left() {
    let dir = project();
    let root = dir.path();
    // Transition → State → Machine, Event → Actor; sections that exist stay, the others go.
    let refs = "  - order/Paid/user.CANCEL\n  - order/Gone/user.CANCEL\n  - order/Gone\n  - user.NOPE\n  - requirements/ui#order/Gone\n  - requirements/ui#General\n  - specs#data\n  - specs#languages\n";
    put(
        root,
        "drifts/dddddddd-d.md",
        &drift_text("open", "").replace("  - order/Paid\n  - user\n", refs),
    );
    let (image, decided) = decide_at(
        root,
        &AgentFacts::default(),
        &Decision::FixCode {
            option: Some(1),
            note: None,
        },
    )
    .unwrap();
    let fix = image
        .card(CardKind::Task, decided.fix_task.as_deref().unwrap())
        .unwrap();
    assert_eq!(
        tasks::refs_of(fix),
        [
            "order/Paid/user.CANCEL",
            "order",
            "user",
            "requirements/ui#General",
            "specs#languages"
        ]
    );

    // No Model element is left: refused before any write.
    let dir = project();
    let root = dir.path();
    put(
        root,
        "drifts/dddddddd-d.md",
        &drift_text("open", "").replace(
            "  - order/Paid\n  - user\n",
            "  - ghost/Nowhere\n  - requirements/ui#General\n",
        ),
    );
    let before = snapshot(root);
    let why = decide_at(
        root,
        &AgentFacts::default(),
        &Decision::FixCode {
            option: Some(1),
            note: None,
        },
    )
    .unwrap_err();
    assert!(
        why.contains("no Model element it concerns is left"),
        "{why}"
    );
    assert_eq!(snapshot(root), before);
}

#[test]
fn a_drift_on_requirements_only_is_fixed_by_a_technical_task_unless_no_section_is_left() {
    let dir = project();
    let root = dir.path();
    put(
        root,
        "drifts/dddddddd-d.md",
        &drift_text("open", "").replace(
            "  - order/Paid\n  - user\n",
            "  - requirements/ui#General\n  - requirements/ui#Gone\n",
        ),
    );
    let (image, decided) = decide_at(
        root,
        &AgentFacts::default(),
        &Decision::FixCode {
            option: Some(1),
            note: None,
        },
    )
    .unwrap();
    let fix = image
        .card(CardKind::Task, decided.fix_task.as_deref().unwrap())
        .unwrap();
    assert!(tasks::is_technical(fix));
    assert_eq!(tasks::refs_of(fix), ["requirements/ui#General"]);

    let dir = project();
    let root = dir.path();
    put(
        root,
        "drifts/dddddddd-d.md",
        &drift_text("open", "").replace("  - order/Paid\n  - user\n", "  - requirements/ui#Gone\n"),
    );
    let why = decide_at(
        root,
        &AgentFacts::default(),
        &Decision::FixCode {
            option: Some(1),
            note: None,
        },
    )
    .unwrap_err();
    assert!(
        why.contains("no Requirement section it concerns is left"),
        "{why}"
    );
}

#[test]
fn every_decision_is_refused_while_the_agent_that_filed_the_drift_works_and_when_it_is_decided() {
    let dir = project();
    let root = dir.path();
    put_drift(root, DRIFT, "open", "job: jjjjjjjj\n");
    let working = AgentFacts {
        working_jobs: ["jjjjjjjj".to_owned()].into(),
        ..AgentFacts::default()
    };
    let before = snapshot(root);
    for decision in [
        Decision::FixCode {
            option: Some(1),
            note: None,
        },
        Decision::AcceptProposal,
        Decision::Dismiss,
    ] {
        let why = decide_at(root, &working, &decision).unwrap_err();
        assert!(why.contains("The agent is still finishing"), "{why}");
    }
    assert_eq!(snapshot(root), before);

    put_drift(root, DRIFT, "dismissed", "");
    let why = decide_at(root, &AgentFacts::default(), &Decision::Dismiss).unwrap_err();
    assert!(why.contains("the Drift is dismissed"), "{why}");
    let missing = decide::decide(
        root,
        &AgentFacts::default(),
        "zzzzzzzz",
        &Decision::Dismiss,
        LATER,
        &mut rng(),
    )
    .unwrap_err();
    assert!(
        matches!(missing, MutationError::Refused(r) if r.notice.contains("no Drift has the identity"))
    );
}

// ---------------------------------------------------------------------------------------------
// Dismiss
// ---------------------------------------------------------------------------------------------

#[test]
fn dismissing_records_the_fingerprint_and_it_stands_until_the_text_changes() {
    let dir = project();
    let root = dir.path();
    put_drift(root, DRIFT, "open", "");
    let (image, decided) = decide_at(root, &AgentFacts::default(), &Decision::Dismiss).unwrap();
    assert_eq!((decided.status, decided.fix_task), ("dismissed", None));
    let drift = drift_of(&image, DRIFT);
    assert_eq!(
        (drift.str_key("status"), drift.str_key("decided")),
        (Some("dismissed"), Some(LATER))
    );
    assert!(drift
        .str_key("fingerprint")
        .is_some_and(|f| f.starts_with("sha256:")));
    assert!(reasons::dismissal_stands(&image, drift));
    assert!(
        reasons::concerns(&image, drift, &["order".to_owned()]),
        "a Drift on a State concerns its Machine"
    );
    assert!(
        reasons::concerns(&image, drift, &["shop".to_owned()]),
        "and the Feature that contains it"
    );

    put(
        root,
        "model/actors/user.md",
        &USER.replace("asks to cancel", "asks for the cancellation of"),
    );
    let (changed, _, _) = state(root);
    assert!(!reasons::dismissal_stands(
        &changed,
        drift_of(&changed, DRIFT)
    ));
}

#[test]
fn a_drift_in_a_resolution_being_written_or_waiting_is_not_dismissed() {
    for status in ["draft", "open"] {
        let dir = project();
        let root = dir.path();
        put_drift(root, DRIFT, "open", "");
        proposal(root, "rrrrrrrr", &format!("status: {status}\nkind: resolution\ndrifts:\n  - drift: dddddddd\n    decision: dismissed\n"), "# R\n", &[], &[]);
        let before = snapshot(root);
        let why = decide_at(root, &AgentFacts::default(), &Decision::Dismiss).unwrap_err();
        assert!(why.contains("This Drift is in a resolution"), "{why}");
        assert_eq!(snapshot(root), before);
    }
}

// ---------------------------------------------------------------------------------------------
// Accept the Proposal
// ---------------------------------------------------------------------------------------------

#[test]
fn accepting_the_proposal_approves_it_and_the_drift_is_proposal_accepted() {
    let dir = project();
    let root = dir.path();
    put_drift(root, DRIFT, "open", "proposal: pppppppp\n");
    attached_proposal(root, "pppppppp", "open");
    let (image, decided) =
        decide_at(root, &AgentFacts::default(), &Decision::AcceptProposal).unwrap();
    assert_eq!(decided.status, "proposal-accepted");
    let drift = drift_of(&image, DRIFT);
    assert_eq!(
        (drift.str_key("status"), drift.str_key("decided")),
        (Some("proposal-accepted"), Some(LATER))
    );
    assert_eq!(
        data::status_of(image.card(CardKind::Proposal, "pppppppp").unwrap()),
        Some(Status::Approved)
    );
    assert!(
        fs::read_to_string(root.join(".weaver/model/shop/order.md"))
            .unwrap()
            .contains("The customer paid."),
        "the Model says what the code does"
    );
}

#[test]
fn accepting_needs_a_proposal_that_could_be_approved() {
    let dir = project();
    let root = dir.path();
    put_drift(root, DRIFT, "open", "");
    let why = decide_at(root, &AgentFacts::default(), &Decision::AcceptProposal).unwrap_err();
    assert!(why.contains("no attached Proposal"), "{why}");

    put_drift(root, DRIFT, "open", "proposal: pppppppp\n");
    attached_proposal(root, "pppppppp", "draft");
    let before = snapshot(root);
    let why = decide_at(root, &AgentFacts::default(), &Decision::AcceptProposal).unwrap_err();
    assert!(
        why.contains("still writing"),
        "a Draft cannot be approved: {why}"
    );

    // Stale: the file it changes is no longer the one it started from.
    attached_proposal(root, "pppppppp", "open");
    put(
        root,
        "model/shop/order.md",
        &ORDER.replace("The customer is choosing.", "The customer is browsing."),
    );
    let before_stale = snapshot(root);
    let why = decide_at(root, &AgentFacts::default(), &Decision::AcceptProposal).unwrap_err();
    assert!(
        why.contains("cannot be approved now"),
        "a stale Proposal is refined, never accepted: {why}"
    );
    assert_eq!(snapshot(root), before_stale, "a refusal writes nothing");
    assert_ne!(before, before_stale);

    // A Run works: the same reason the control shows.
    put(root, "model/shop/order.md", ORDER);
    let run = AgentFacts {
        run_in_progress: true,
        ..AgentFacts::default()
    };
    let why = decide_at(root, &run, &Decision::AcceptProposal).unwrap_err();
    assert!(why.contains("A run is in progress"), "{why}");
}

// ---------------------------------------------------------------------------------------------
// Refine the attached Proposal
// ---------------------------------------------------------------------------------------------

/// Makes the attached Proposal `pppppppp` stale: the file it changes is no longer the one it
/// started from.
fn make_stale(root: &Path) {
    put(
        root,
        "model/shop/order.md",
        &ORDER.replace("The customer is choosing.", "The customer is browsing."),
    );
}

fn status_of_proposal(image: &ProjectImage) -> Option<Status> {
    data::status_of(image.card(CardKind::Proposal, "pppppppp").unwrap())
}

#[test]
fn refining_a_stale_attached_proposal_makes_it_refining_and_leaves_the_drift_open() {
    let dir = project();
    let root = dir.path();
    put_drift(root, DRIFT, "open", "proposal: pppppppp\n");
    attached_proposal(root, "pppppppp", "open");
    make_stale(root);
    let drift_before = fs::read(root.join(".weaver/drifts/dddddddd-d.md")).unwrap();
    // A Run does not matter.
    let run = AgentFacts {
        run_in_progress: true,
        ..AgentFacts::default()
    };
    let (image, decided) = decide_at(root, &run, &Decision::RefineProposal).unwrap();
    assert_eq!(decided.refine.as_deref(), Some("pppppppp"));
    assert_eq!(decided.status, "open");
    assert_eq!(status_of_proposal(&image), Some(Status::Refining));
    let attached = image.card(CardKind::Proposal, "pppppppp").unwrap();
    assert!(data::refining_flag(attached), "refining: true is recorded");
    assert_eq!(
        fs::read(root.join(".weaver/drifts/dddddddd-d.md")).unwrap(),
        drift_before,
        "the Drift is not written"
    );
    assert_eq!(drift_of(&image, DRIFT).str_key("status"), Some("open"));
    assert_eq!(
        decide::Decision::RefineProposal.verb(),
        "refine the Proposal of"
    );
}

#[test]
fn refining_continues_an_attached_draft_a_refinement_left_half_way() {
    let dir = project();
    let root = dir.path();
    put_drift(root, DRIFT, "open", "proposal: pppppppp\n");
    attached_proposal(root, "pppppppp", "draft\nrefining: true");
    let (image, decided) =
        decide_at(root, &AgentFacts::default(), &Decision::RefineProposal).unwrap();
    assert_eq!(decided.refine.as_deref(), Some("pppppppp"));
    assert_eq!(status_of_proposal(&image), Some(Status::Refining));
    assert_eq!(drift_of(&image, DRIFT).str_key("status"), Some("open"));
}

#[test]
fn refining_is_refused_without_a_stale_or_interrupted_attached_proposal_and_writes_nothing() {
    let dir = project();
    let root = dir.path();
    put_drift(root, DRIFT, "open", "");
    let why = decide_at(root, &AgentFacts::default(), &Decision::RefineProposal).unwrap_err();
    assert!(why.contains("no attached Proposal"), "{why}");

    put_drift(root, DRIFT, "open", "proposal: pppppppp\n");
    // Open and not stale.
    attached_proposal(root, "pppppppp", "open");
    let before = snapshot(root);
    let why = decide_at(root, &AgentFacts::default(), &Decision::RefineProposal).unwrap_err();
    assert!(why.contains("not stale"), "{why}");
    assert_eq!(snapshot(root), before);

    // A Draft the agent is still writing.
    attached_proposal(root, "pppppppp", "draft");
    let before = snapshot(root);
    let why = decide_at(root, &AgentFacts::default(), &Decision::RefineProposal).unwrap_err();
    assert!(why.contains("still being written"), "{why}");
    assert_eq!(snapshot(root), before);

    // Already Refining.
    attached_proposal(root, "pppppppp", "refining\nrefining: true");
    make_stale(root);
    let before = snapshot(root);
    let why = decide_at(root, &AgentFacts::default(), &Decision::RefineProposal).unwrap_err();
    assert!(why.contains("Being refined"), "{why}");
    assert_eq!(snapshot(root), before);
}

#[test]
fn refining_is_refused_while_the_agent_that_handed_the_proposal_over_or_the_drift_s_works() {
    let dir = project();
    let root = dir.path();
    put_drift(root, DRIFT, "open", "proposal: pppppppp\n");
    attached_proposal(root, "pppppppp", "open\njob: jjjjjjjj");
    make_stale(root);
    let before = snapshot(root);
    let working = AgentFacts {
        working_jobs: ["jjjjjjjj".to_owned()].into(),
        ..AgentFacts::default()
    };
    let why = decide_at(root, &working, &Decision::RefineProposal).unwrap_err();
    assert!(why.contains("The agent is still finishing"), "{why}");
    assert_eq!(snapshot(root), before);

    put_drift(root, DRIFT, "open", "proposal: pppppppp\njob: kkkkkkkk\n");
    let drift_working = AgentFacts {
        working_jobs: ["kkkkkkkk".to_owned()].into(),
        ..AgentFacts::default()
    };
    let why = decide_at(root, &drift_working, &Decision::RefineProposal).unwrap_err();
    assert!(why.contains("The agent is still finishing"), "{why}");
}

#[test]
fn refining_writes_nothing_when_no_refinement_job_can_start() {
    let dir = project();
    let root = dir.path();
    put_drift(root, DRIFT, "open", "proposal: pppppppp\n");
    attached_proposal(root, "pppppppp", "open");
    make_stale(root);
    let before = snapshot(root);
    let error = decide::decide_with(
        root,
        &AgentFacts::default(),
        DRIFT,
        &Decision::RefineProposal,
        LATER,
        &mut rng(),
        &|| Err("no agent job can start yet".to_owned()),
    )
    .map(|_| ())
    .unwrap_err();
    let MutationError::Refused(refused) = error else {
        panic!("a refusal was expected");
    };
    assert_eq!(
        refused.notice,
        "Could not refine the Proposal of D1: no agent job can start yet."
    );
    assert_eq!(snapshot(root), before);
}

#[test]
fn the_drift_discards_only_an_attached_draft_a_refinement_left_half_way() {
    let dir = project();
    let root = dir.path();
    put_drift(root, DRIFT, "open", "proposal: pppppppp\n");
    attached_proposal(root, "pppppppp", "draft\nrefining: true\njob: jjjjjjjj");
    let drift_before = fs::read(root.join(".weaver/drifts/dddddddd-d.md")).unwrap();

    let working = AgentFacts {
        working_jobs: ["jjjjjjjj".to_owned()].into(),
        ..AgentFacts::default()
    };
    let before = snapshot(root);
    let why = decide_at(root, &working, &Decision::DiscardProposal).unwrap_err();
    assert!(why.contains("The agent is still finishing"), "{why}");
    assert_eq!(snapshot(root), before);

    let (image, decided) =
        decide_at(root, &AgentFacts::default(), &Decision::DiscardProposal).unwrap();
    assert_eq!((decided.status, decided.refine), ("open", None));
    assert_eq!(status_of_proposal(&image), Some(Status::Discarded));
    assert_eq!(drift_of(&image, DRIFT).str_key("status"), Some("open"));
    assert_eq!(
        fs::read(root.join(".weaver/drifts/dddddddd-d.md")).unwrap(),
        drift_before,
        "the Drift is not written"
    );
    assert_eq!(Decision::DiscardProposal.verb(), "discard the Proposal of");

    for status in ["open", "draft", "refining\nrefining: true"] {
        attached_proposal(root, "pppppppp", status);
        let before = snapshot(root);
        let why = decide_at(root, &AgentFacts::default(), &Decision::DiscardProposal).unwrap_err();
        assert!(
            why.contains("not a Draft left half-way") || why.contains("Being refined"),
            "{status}: {why}"
        );
        assert_eq!(snapshot(root), before);
    }
}

#[test]
fn no_decision_but_refine_is_accepted_while_the_attached_proposal_is_refining() {
    let dir = project();
    let root = dir.path();
    put_drift(root, DRIFT, "open", "proposal: pppppppp\n");
    attached_proposal(root, "pppppppp", "refining\nrefining: true");
    let before = snapshot(root);
    for decision in [
        Decision::AcceptProposal,
        Decision::FixCode {
            option: Some(1),
            note: None,
        },
        Decision::Dismiss,
    ] {
        let why = decide_at(root, &AgentFacts::default(), &decision).unwrap_err();
        assert!(why.contains("Being refined"), "{decision:?}: {why}");
        assert_eq!(snapshot(root), before, "{decision:?} writes nothing");
    }
    let (image, _, _) = state(root);
    let drift = drift_of(&image, DRIFT);
    assert_eq!(reasons::fix_reasons(&image, drift), ["Being refined"]);
    assert_eq!(
        reasons::dismiss_reasons(&image, &AgentFacts::default(), drift),
        ["Being refined"]
    );
}

// ---------------------------------------------------------------------------------------------
// The other transitions
// ---------------------------------------------------------------------------------------------

#[test]
fn the_fix_task_finished_fixes_the_drift_and_closed_unchanged_dismisses_it_with_the_note() {
    for unchanged in [None, Some("The code already does it")] {
        let dir = project();
        let root = dir.path();
        put_drift(root, DRIFT, "open", "");
        let (image, decided) = decide_at(
            root,
            &AgentFacts::default(),
            &Decision::FixCode {
                option: Some(1),
                note: None,
            },
        )
        .unwrap();
        let fix = decided.fix_task.unwrap();
        // The agent takes the fix Task.
        let text = fs::read_to_string(root.join(format!(
            ".weaver/{}",
            image.card(CardKind::Task, &fix).unwrap().path
        )))
        .unwrap();
        put(
            root,
            &image.card(CardKind::Task, &fix).unwrap().path,
            &text.replace("column: todo", "column: doing\nholder: jjjjjjjj"),
        );
        let (image, _, numbers) = state(root);
        let plan = tasks::plan_finish(
            &image,
            &numbers,
            &fix,
            "jjjjjjjj",
            "2026-11-02T11:00:00.000Z",
            unchanged,
        )
        .unwrap();
        assert_eq!(plan.drift.as_deref(), Some(DRIFT));
        let after = image.with_changes(&plan.changes);
        let drift = drift_of(&after, DRIFT);
        match unchanged {
            None => {
                assert_eq!(drift.str_key("status"), Some("fixed"));
                assert_eq!(
                    drift.str_key("fingerprint"),
                    None,
                    "a fix records no fingerprint"
                );
            }
            Some(note) => {
                assert_eq!(drift.str_key("status"), Some("dismissed"));
                assert_eq!(drift.str_key("note"), Some(note));
                assert!(reasons::dismissal_stands(&after, drift));
            }
        }
    }
}

#[test]
fn approving_a_resolution_records_the_fingerprint_of_a_dismissal_and_disposes_the_attached_proposal(
) {
    let dir = project();
    let root = dir.path();
    put_drift(root, DRIFT, "open", "proposal: pppppppp\n");
    attached_proposal(root, "pppppppp", "open");
    proposal(
        root,
        "rrrrrrrr",
        "status: open\nkind: resolution\ndrifts:\n  - drift: dddddddd\n    decision: dismissed\n",
        "# R\n",
        &[],
        &[],
    );
    let (after, _) = transitions::approve(
        root,
        &AgentFacts::default(),
        "rrrrrrrr",
        Decider::Developer,
        NOW,
        &mut rng(),
    )
    .unwrap();
    let drift = drift_of(&after, DRIFT);
    assert_eq!(drift.str_key("status"), Some("dismissed"));
    assert!(reasons::dismissal_stands(&after, drift));
    assert_eq!(
        data::status_of(after.card(CardKind::Proposal, "pppppppp").unwrap()),
        Some(Status::Discarded)
    );
}

// ---------------------------------------------------------------------------------------------
// The reasons of the controls
// ---------------------------------------------------------------------------------------------

fn name(kind: CardKind, id: &str) -> String {
    format!("{}:{id}", kind.short_letter().unwrap_or('?'))
}

#[test]
fn resolve_lists_every_reason_in_the_fixed_order() {
    let dir = project();
    let root = dir.path();
    put_drift(root, DRIFT, "open", "job: jjjjjjjj\n");
    proposal(
        root,
        "oooooooo",
        "status: draft\nkind: resolution\ndrifts:\n  - drift: eeeeeeee\n    decision: dismissed\n",
        "# W\n",
        &[],
        &[],
    );
    proposal(
        root,
        "pppppppp",
        "status: open\nkind: resolution\ndrifts:\n  - drift: ffffffff\n    decision: dismissed\n",
        "# A\n",
        &[],
        &[],
    );
    proposal(
        root,
        "qqqqqqqq",
        "status: open\nkind: resolution\ndrifts:\n  - drift: gggggggg\n    decision: dismissed\n",
        "# B\n",
        &[],
        &[],
    );
    let (image, _, _) = state(root);
    let facts = AgentFacts {
        working_jobs: ["jjjjjjjj".to_owned()].into(),
        run_in_progress: true,
        resolving_drifts: [DRIFT.to_owned()].into(),
        ..AgentFacts::default()
    };
    assert_eq!(
        reasons::resolve_reasons(&image, &facts, &name, drift_of(&image, DRIFT)),
        [
            "The agent is still finishing",
            "A run is in progress",
            "This Drift is being resolved",
            "Another resolution is being written",
            "Proposal P:pppppppp resolving Drifts awaits a decision",
            "Proposal P:qqqqqqqq resolving Drifts awaits a decision",
        ]
    );
    assert!(
        reasons::resolve_reasons(
            &image,
            &AgentFacts::default(),
            &name,
            drift_of(&image, DRIFT)
        )
        .len()
            == 3
    );
    assert!(reasons::is_resolving(
        &image,
        &facts,
        drift_of(&image, DRIFT)
    ));
}

#[test]
fn a_resolution_draft_that_lists_the_drift_is_resolving_it() {
    let dir = project();
    let root = dir.path();
    put_drift(root, DRIFT, "open", "");
    proposal(
        root,
        "oooooooo",
        "status: draft\nkind: resolution\ndrifts:\n  - drift: dddddddd\n    decision: dismissed\n",
        "# W\n",
        &[],
        &[],
    );
    let (image, _, _) = state(root);
    let drift = drift_of(&image, DRIFT);
    assert!(reasons::is_resolving(&image, &AgentFacts::default(), drift));
    assert_eq!(
        reasons::resolve_reasons(&image, &AgentFacts::default(), &name, drift),
        ["This Drift is being resolved"],
        "its own Draft is not another resolution"
    );
    assert_eq!(
        reasons::dismiss_reasons(&image, &AgentFacts::default(), drift),
        ["This Drift is in a resolution"]
    );
    assert!(
        !reasons::is_unprepared(&image, drift),
        "a Drift in a resolution is never listed as unprepared"
    );
}

#[test]
fn dismiss_lists_the_agent_first_then_the_resolution() {
    let dir = project();
    let root = dir.path();
    put_drift(root, DRIFT, "open", "job: jjjjjjjj\n");
    proposal(
        root,
        "oooooooo",
        "status: open\nkind: resolution\ndrifts:\n  - drift: dddddddd\n    decision: dismissed\n",
        "# W\n",
        &[],
        &[],
    );
    let (image, _, _) = state(root);
    let facts = AgentFacts {
        working_jobs: ["jjjjjjjj".to_owned()].into(),
        ..AgentFacts::default()
    };
    assert_eq!(
        reasons::dismiss_reasons(&image, &facts, drift_of(&image, DRIFT)),
        [
            "The agent is still finishing",
            "This Drift is in a resolution"
        ]
    );
    assert!(
        reasons::dismiss_reasons(&image, &AgentFacts::default(), drift_of(&image, DRIFT)).len()
            == 1
    );
}

#[test]
fn a_resolution_being_refined_counts_as_one_being_written() {
    let dir = project();
    let root = dir.path();
    put_drift(root, DRIFT, "open", "");
    proposal(
        root,
        "rrrrrrrr",
        "status: refining\nrefining: true\nkind: resolution\ndrifts:\n  - drift: dddddddd\n    decision: dismissed\n",
        "# R\n",
        &[],
        &[],
    );
    proposal(
        root,
        "oooooooo",
        "status: refining\nrefining: true\nkind: resolution\ndrifts:\n  - drift: eeeeeeee\n    decision: dismissed\n",
        "# O\n",
        &[],
        &[],
    );
    let (image, _, _) = state(root);
    let drift = drift_of(&image, DRIFT);
    let none = AgentFacts::default();
    assert!(
        reasons::is_resolving(&image, &none, drift),
        "the badge `resolving`"
    );
    assert_eq!(
        reasons::resolve_reasons(&image, &none, &name, drift),
        [
            "This Drift is being resolved",
            "Another resolution is being written"
        ]
    );
    assert_eq!(
        reasons::dismiss_reasons(&image, &none, drift),
        ["This Drift is in a resolution"]
    );
    assert!(!reasons::is_unprepared(&image, drift));
    assert!(crate::project::board::drifts_resolving(&image, &none));
}

#[test]
fn a_resolution_left_half_way_by_a_refinement_counts_as_one_waiting_for_a_decision() {
    let dir = project();
    let root = dir.path();
    put_drift(root, DRIFT, "open", "");
    proposal(
        root,
        "rrrrrrrr",
        "status: draft\nrefining: true\nkind: resolution\ndrifts:\n  - drift: dddddddd\n    decision: dismissed\n",
        "# R\n",
        &[],
        &[],
    );
    let (image, _, _) = state(root);
    let drift = drift_of(&image, DRIFT);
    let none = AgentFacts::default();
    assert!(
        !reasons::is_resolving(&image, &none, drift),
        "no badge `resolving`"
    );
    assert_eq!(
        reasons::resolve_reasons(&image, &none, &name, drift),
        ["Proposal P:rrrrrrrr resolving Drifts awaits a decision"],
        "not `Another resolution is being written`"
    );
    assert_eq!(
        reasons::dismiss_reasons(&image, &none, drift),
        ["This Drift is in a resolution"]
    );
    assert!(!reasons::is_unprepared(&image, drift));
    assert!(
        !crate::project::board::drifts_resolving(&image, &none),
        "Run all is not held back by it"
    );
}

#[test]
fn resolve_names_being_refined_after_a_run_and_before_the_resolution_reasons() {
    let dir = project();
    let root = dir.path();
    put_drift(root, DRIFT, "open", "job: jjjjjjjj\nproposal: pppppppp\n");
    attached_proposal(root, "pppppppp", "refining\nrefining: true");
    proposal(
        root,
        "oooooooo",
        "status: draft\nkind: resolution\ndrifts:\n  - drift: eeeeeeee\n    decision: dismissed\n",
        "# O\n",
        &[],
        &[],
    );
    proposal(
        root,
        "qqqqqqqq",
        "status: open\nkind: resolution\ndrifts:\n  - drift: gggggggg\n    decision: dismissed\n",
        "# Q\n",
        &[],
        &[],
    );
    let (image, _, _) = state(root);
    let facts = AgentFacts {
        working_jobs: ["jjjjjjjj".to_owned()].into(),
        run_in_progress: true,
        resolving_drifts: [DRIFT.to_owned()].into(),
        ..AgentFacts::default()
    };
    let drift = drift_of(&image, DRIFT);
    assert_eq!(
        reasons::resolve_reasons(&image, &facts, &name, drift),
        [
            "The agent is still finishing",
            "A run is in progress",
            "Being refined",
            "This Drift is being resolved",
            "Another resolution is being written",
            "Proposal P:qqqqqqqq resolving Drifts awaits a decision",
        ]
    );
    assert_eq!(
        reasons::dismiss_reasons(&image, &facts, drift),
        ["The agent is still finishing", "Being refined"]
    );
}

#[test]
fn unprepared_drifts_are_the_open_ones_with_no_valid_question_in_no_resolution() {
    let dir = project();
    let root = dir.path();
    put_drift(root, DRIFT, "open", "");
    put_drift(root, "eeeeeeee", "dismissed", "");
    let (image, _, _) = state(root);
    assert!(reasons::is_unprepared(&image, drift_of(&image, DRIFT)));
    assert!(
        !reasons::is_unprepared(&image, drift_of(&image, "eeeeeeee")),
        "a decided Drift is not listed"
    );
    prepare(root, "dismissed");
    let (image, _, _) = state(root);
    assert!(!reasons::is_unprepared(&image, drift_of(&image, DRIFT)));
}

// ---------------------------------------------------------------------------------------------
// The answer
// ---------------------------------------------------------------------------------------------

fn prepared_of(root: &Path) -> prepared::Prepared {
    let (image, _, _) = state(root);
    prepared::check(&image, drift_of(&image, DRIFT))
        .valid()
        .expect("valid")
        .clone()
}

#[test]
fn an_answer_is_a_decision_a_job_with_the_answer_or_nothing() {
    let dir = project();
    let root = dir.path();
    put_drift(root, DRIFT, "open", "proposal: pppppppp\n");
    attached_proposal(root, "pppppppp", "open");
    prepare(root, "proposal");
    let prepared = prepared_of(root);
    let answered = |labels: &[&str], text: Option<&str>| Answer::Answered {
        selected: labels.iter().map(|l| (*l).to_owned()).collect(),
        text: text.map(str::to_owned),
    };
    let row = |decision: &str, option: Option<u32>, note: Option<&str>| {
        Next::Resolution(ResolutionRow {
            drift: DRIFT.to_owned(),
            decision: decision.to_owned(),
            option,
            note: note.map(str::to_owned),
        })
    };
    let job = |labels: &[&str], text: Option<&str>| {
        Next::Job(ResolveAnswer {
            selected: labels.iter().map(|l| (*l).to_owned()).collect(),
            text: text.map(str::to_owned),
        })
    };

    // A fix option is `fix-code`; its free text is the note.
    assert_eq!(
        answer::next_of(DRIFT, &prepared, &answered(&["Lock"], None)),
        row("fix-code", Some(2), None)
    );
    assert_eq!(
        answer::next_of(
            DRIFT,
            &prepared,
            &answered(&["Lock"], Some("  Mind the lock "))
        ),
        row("fix-code", Some(2), Some("Mind the lock"))
    );
    // `proposal` and `dismissed` without text are that decision.
    assert_eq!(
        answer::next_of(DRIFT, &prepared, &answered(&["Keep as it is"], None)),
        row("proposal", None, None)
    );
    assert_eq!(
        answer::next_of(DRIFT, &prepared, &answered(&["Keep as it is"], Some("   "))),
        row("proposal", None, None)
    );
    // A free answer next to a choice other than a code fix, or alone, starts a resolve job.
    assert_eq!(
        answer::next_of(
            DRIFT,
            &prepared,
            &answered(&["Keep as it is"], Some("only for guests"))
        ),
        job(&["Keep as it is"], Some("only for guests"))
    );
    assert_eq!(
        answer::next_of(DRIFT, &prepared, &answered(&[], Some("something else"))),
        job(&[], Some("something else"))
    );
    // A label the question does not have is handed to the agent as it came.
    assert_eq!(
        answer::next_of(DRIFT, &prepared, &answered(&["Unknown"], None)),
        job(&["Unknown"], None)
    );
    // A skipped question, or an answer with nothing, leaves the Drift as it is.
    assert_eq!(
        answer::next_of(DRIFT, &prepared, &Answer::Skipped),
        Next::Nothing
    );
    assert_eq!(
        answer::next_of(DRIFT, &prepared, &answered(&[], None)),
        Next::Nothing
    );
}

fn written_resolution(root: &Path, row: &ResolutionRow) -> Result<answer::Written, Vec<String>> {
    let (image, folders, numbers) = state(root);
    let drift = drift_of(&image, DRIFT);
    answer::plan_resolution(
        &image,
        &folders,
        &numbers,
        drift,
        row,
        NOW,
        &|path| create_read(root, path),
        &mut rng(),
    )
}

fn create_read(root: &Path, path: &str) -> Option<String> {
    crate::project::proposal::create::read_project_file(root, path)
}

fn row_of(decision: &str, option: Option<u32>, note: Option<&str>) -> ResolutionRow {
    ResolutionRow {
        drift: DRIFT.to_owned(),
        decision: decision.to_owned(),
        option,
        note: note.map(str::to_owned),
    }
}

#[test]
fn weaver_writes_the_resolution_of_a_code_fix_and_hands_it_over() {
    let dir = project();
    let root = dir.path();
    put_drift(root, DRIFT, "open", "");
    let written =
        written_resolution(root, &row_of("fix-code", Some(2), Some("Mind the lock"))).unwrap();
    apply(root, &written.changes);
    let (image, _, _) = state(root);
    let resolution = image.card(CardKind::Proposal, &written.id).unwrap();
    assert_eq!(resolution.title, "Resolve: The code ships twice");
    assert_eq!(
        (data::status_of(resolution), data::kind_of(resolution)),
        (Some(Status::Draft), data::Kind::Resolution)
    );
    assert_eq!(
        resolution.str_key("job"),
        None,
        "Weaver wrote it: no agent is finishing"
    );
    assert!(
        resolution.body.contains(&format!("`{DRIFT}`")) && !resolution.body.contains("D1"),
        "no short number in a file: {}",
        resolution.body
    );
    let rows = crate::project::board::resolution_rows(resolution);
    assert_eq!(rows, [row_of("fix-code", Some(2), Some("Mind the lock"))]);

    // Hand-over with the agent's checks: it opens.
    let ctx = SubmitContext {
        now: NOW,
        job_found_empty_model: false,
        run_in_progress: false,
    };
    let (image, submission) = transitions::submit(root, &written.id, &ctx, &mut rng()).unwrap();
    assert!(matches!(submission.outcome, transitions::Submitted::Open));
    assert_eq!(
        data::status_of(image.card(CardKind::Proposal, &written.id).unwrap()),
        Some(Status::Open)
    );

    // Approving it gives the Drift its decision and creates the fix Task.
    let (after, approval) = transitions::approve(
        root,
        &AgentFacts::default(),
        &written.id,
        Decider::Developer,
        LATER,
        &mut rng(),
    )
    .unwrap();
    let transitions::Approval::Approve(plan) = approval else {
        panic!()
    };
    assert_eq!(
        plan.decided_drifts,
        [(DRIFT.to_owned(), "code-to-be-fixed")]
    );
    assert_eq!(
        drift_of(&after, DRIFT).str_key("note"),
        Some("Mind the lock")
    );
}

#[test]
fn weaver_writes_the_resolution_of_a_dismissal_without_files() {
    let dir = project();
    let root = dir.path();
    put_drift(root, DRIFT, "open", "");
    let written = written_resolution(root, &row_of("dismissed", None, None)).unwrap();
    assert_eq!(written.changes.iter().count(), 1, "only proposal.md");
    apply(root, &written.changes);
    let ctx = SubmitContext {
        now: NOW,
        job_found_empty_model: false,
        run_in_progress: false,
    };
    transitions::submit(root, &written.id, &ctx, &mut rng()).unwrap();
}

#[test]
fn a_proposal_decision_carries_the_files_of_the_attached_proposal() {
    let dir = project();
    let root = dir.path();
    put_drift(root, DRIFT, "open", "proposal: pppppppp\n");
    attached_proposal(root, "pppppppp", "open");
    let written = written_resolution(root, &row_of("proposal", None, None)).unwrap();
    apply(root, &written.changes);
    let (image, folders, numbers) = state(root);
    let folder = folders.get(&written.id).expect("the resolution folder");
    assert!(
        folder
            .text_of("model/shop/order.md")
            .is_some_and(|t| t.contains("The customer paid.")),
        "it carries the attached Proposal's text"
    );
    let analysed = analysis::analyse(&image, &folders, &numbers, &written.id).unwrap();
    assert!(
        analysed.stale.is_empty() && analysed.files.changed == ["model/shop/order.md"],
        "{analysed:?}"
    );

    let ctx = SubmitContext {
        now: NOW,
        job_found_empty_model: false,
        run_in_progress: false,
    };
    transitions::submit(root, &written.id, &ctx, &mut rng()).unwrap();
    let (after, _) = transitions::approve(
        root,
        &AgentFacts::default(),
        &written.id,
        Decider::Developer,
        LATER,
        &mut rng(),
    )
    .unwrap();
    assert_eq!(
        drift_of(&after, DRIFT).str_key("status"),
        Some("proposal-accepted")
    );
    assert!(fs::read_to_string(root.join(".weaver/model/shop/order.md"))
        .unwrap()
        .contains("The customer paid."));
    assert_eq!(
        data::status_of(after.card(CardKind::Proposal, "pppppppp").unwrap()),
        Some(Status::Discarded),
        "the attached Proposal is superseded"
    );
}

#[test]
fn a_stale_attached_proposal_cannot_be_carried_and_a_missing_one_neither() {
    let dir = project();
    let root = dir.path();
    put_drift(root, DRIFT, "open", "");
    let why = written_resolution(root, &row_of("proposal", None, None)).unwrap_err();
    assert!(why[0].contains("no attached Proposal"), "{why:?}");

    put_drift(root, DRIFT, "open", "proposal: pppppppp\n");
    attached_proposal(root, "pppppppp", "open");
    put(
        root,
        "model/shop/order.md",
        &ORDER.replace("The customer is choosing.", "The customer is browsing."),
    );
    let why = written_resolution(root, &row_of("proposal", None, None)).unwrap_err();
    assert!(why[0].contains("stale"), "{why:?}");
}

#[test]
fn a_refused_hand_over_is_the_caller_s_to_discard() {
    let dir = project();
    let root = dir.path();
    // A code fix on a Drift whose references are all gone is refused at hand-over; the Draft is then discarded.
    put(
        root,
        "drifts/dddddddd-d.md",
        &drift_text("open", "").replace("  - order/Paid\n  - user\n", "  - ghost/Nowhere\n"),
    );
    let written = written_resolution(root, &row_of("fix-code", Some(1), None)).unwrap();
    apply(root, &written.changes);
    let ctx = SubmitContext {
        now: NOW,
        job_found_empty_model: false,
        run_in_progress: false,
    };
    let refused = transitions::submit(root, &written.id, &ctx, &mut rng()).unwrap_err();
    assert!(
        matches!(&refused, MutationError::Refused(r) if r.0.iter().any(|m| m.contains("no Model element it concerns is left"))),
        "{refused:?}"
    );
    let image = transitions::discard(
        root,
        &AgentFacts::default(),
        &written.id,
        Decider::Developer,
    )
    .unwrap();
    assert_eq!(
        data::status_of(image.card(CardKind::Proposal, &written.id).unwrap()),
        Some(Status::Discarded)
    );
}

// ---------------------------------------------------------------------------------------------
// Where in the code
// ---------------------------------------------------------------------------------------------

#[test]
fn the_files_of_where_in_the_code_come_with_their_first_line() {
    use super::places::{places, Place};
    let place = |file: &str, line: Option<u32>| Place {
        file: file.to_owned(),
        line,
    };
    let text = "- `src/ship.rs:42` — `ship()` ships the order\n- `src/ship.rs:60-70` again, and `web/cart.tsx`\n- `src/lib/util.ts#L7`, `Order::ship`, `user.CANCEL`, `https://example.com/a.rs`\n```\n`ignored/in/fence.rs`\n```\nAlso `Cargo.toml` and `docs/notes`.\n";
    assert_eq!(
        places(text),
        [
            place("src/ship.rs", Some(42)),
            place("web/cart.tsx", None),
            place("src/lib/util.ts", Some(7)),
            place("Cargo.toml", None)
        ]
    );
    assert_eq!(places("`src/ship.rs`").len(), 1);
    assert_eq!(
        places("`src/ship.rs` then `src/ship.rs:9`"),
        [place("src/ship.rs", Some(9))],
        "the first line given wins over none"
    );
    assert!(places("Nothing named here, only `ship_order` and `Order::ship`.").is_empty());
}
