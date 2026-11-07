//! Ask more: the thing read from the files and from the round, and the opening
//! (`requirements/tech#ask-conversation`).

use std::path::Path;
use std::sync::Arc;

use weaver_core::paths::canonical_root;
use weaver_core::question::RoundRequest;

use super::build::{build, ActiveCards, AskTarget, Sources};
use crate::general::wire::AskKind;
use crate::rounds::service::{CardPublication, JobFacts, RoundsHost};
use crate::rounds::view::QuestionRoundChanged;
use crate::rounds::Rounds;

const PROJECT: &str = "0123456789abcdef";

struct NoHost;

impl RoundsHost for NoHost {
    fn job(&self, _job: &str) -> Option<JobFacts> {
        None
    }
    fn round_opened(&self, _job: &str) {}
    fn round_closed(&self, _job: &str) {}
    fn publish_card(&self, _token: &str, _request: RoundRequest) -> CardPublication {
        CardPublication::Unavailable
    }
    fn focus_project(&self, _project_id: &str) {}
    fn changed(&self, _change: &QuestionRoundChanged) {}
}

fn put(root: &Path, rel: &str, text: &str) {
    let path = root.join(".weaver").join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

const TASK: &str = "---\ncolumn: todo\ntechnical: false\nrefs:\n  - requirements/ui#General\nproposal: pppppppp\ndrift: dddddddd\ncreated: 2026-11-02T07:00:01.000Z\n---\n# First task\n\nDo the thing.\n\nThen the other.\n";
const DONE: &str = "---\ncolumn: done\ntechnical: true\nrefs: []\nfinished: 2026-11-02T08:00:00.000Z\ncreated: 2026-11-02T07:00:01.000Z\n---\n# Done task\n";
const PROPOSAL: &str = "---\nstatus: open\nkind: change\ncreated: 2026-11-02T07:00:00.000Z\n---\n# Cancel an order\n\nWhy the order is cancelled.\n";
const PROPOSAL_TASK: &str = "---\ntechnical: true\nrefs: []\n---\n# Cancel it\n\nDo it.\n";
const DRIFT: &str = "---\nstatus: open\nrefs:\n  - requirements/ui#General\norigin: aaaaaaaa\ncreated: 2026-11-02T07:00:03.000Z\n---\n# A drift\n\n## What happens\n\nIt does A.\n\n## What should happen\n\nIt does B.\n";
const HISTORY: &str = "---\ncreated: 2026-11-02T14:59:02.830Z\nupdated: 2026-11-02T14:59:48.511Z\nsummary: Checkout works.\ntasks:\n- zzzzzzzz\n- bbbbbbbb\n- yyyyyyyy\ncleared:\n  zzzzzzzz:\n    title: Pay with a card\n    refs:\n    - requirements/ui#General\n    technical: false\n    proposal: pppppppp\n    label: Checkout\n    finished: 2026-11-02T14:46:56.993Z\n    body: Card payments succeed.\n---\n# Checkout works\n\nThe entry text.\n";
const AREA: &str = "---\ndescription: d\n---\n# UI\n";
const MAIN: &str = "---\ndescription: d\n---\n# Main\n\n## General\n- the general text\n";

/// The cards of the Chat: one card of `PROJECT`, active or not.
#[derive(Default)]
struct Cards {
    active: std::sync::Mutex<Option<(String, RoundRequest)>>,
}

impl ActiveCards for Cards {
    fn active_card_request(
        &self,
        project_id: &str,
        card: &str,
        index: usize,
    ) -> Option<RoundRequest> {
        let active = self.active.lock().unwrap();
        let (id, request) = active.as_ref()?;
        (project_id == PROJECT && id == card && index < request.questions.len())
            .then(|| request.clone())
    }
}

struct Fixture {
    _dir: tempfile::TempDir,
    root: std::path::PathBuf,
    rounds: Arc<Rounds>,
    cards: Cards,
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let root = canonical_root(dir.path()).unwrap();
    put(&root, "tasks/aaaaaaaa-first-task.md", TASK);
    put(&root, "tasks/bbbbbbbb-done-task.md", DONE);
    put(&root, "task-images/aaaaaaaa/shot.png", "png");
    put(&root, "proposals/pppppppp-t/proposal.md", PROPOSAL);
    put(
        &root,
        "proposals/pppppppp-t/tasks/tttttttt-cancel-it.md",
        PROPOSAL_TASK,
    );
    put(&root, "drifts/dddddddd-a-drift.md", DRIFT);
    put(&root, "history/hhhhhhhh-checkout-works.md", HISTORY);
    put(&root, "requirements/ui/area.md", AREA);
    put(&root, "requirements/ui/main.md", MAIN);
    Fixture {
        _dir: dir,
        root,
        rounds: Rounds::new(Arc::new(NoHost)),
        cards: Cards::default(),
    }
}

fn built(f: &Fixture, target: AskTarget, language: Option<&str>) -> Option<super::build::Built> {
    build(
        &Sources {
            project_id: PROJECT,
            root: &f.root,
            rounds: &f.rounds,
            cards: &f.cards,
            language,
        },
        &target,
    )
}

#[test]
fn a_task_is_read_from_its_file_with_its_links_and_images() {
    let f = fixture();
    let found = built(
        &f,
        AskTarget::Task {
            id: "aaaaaaaa".into(),
        },
        None,
    )
    .unwrap();
    assert_eq!(
        (found.name.as_str(), found.kind, found.round),
        ("Ask T1", AskKind::Task, None)
    );
    for line in [
        "Weaver: Ask more\n",
        "Permanent identity: aaaaaaaa\n",
        "Short number (as the developer sees it): T1\n",
        "Title: First task\n",
        "- requirements/ui#General\n",
        "- .weaver/task-images/aaaaaaaa/shot.png\n",
        "Proposal whose design it belongs to: pppppppp (Cancel an order)\n",
        "Drift it fixes: dddddddd\n",
        "Do the thing.\n\nThen the other.\n",
        "- `weaver task show aaaaaaaa`\n",
    ] {
        assert!(
            found.instruction.contains(line),
            "missing {line:?} in {}",
            found.instruction
        );
    }
    assert!(!found.instruction.contains("Write the texts"));
}

#[test]
fn the_language_sentence_is_added_only_when_a_language_is_set() {
    let f = fixture();
    let found = built(
        &f,
        AskTarget::Task {
            id: "aaaaaaaa".into(),
        },
        Some("Italian"),
    )
    .unwrap();
    assert!(found
        .instruction
        .ends_with("\nWrite the texts for the developer in Italian.\n"));
}

#[test]
fn a_task_that_is_gone_or_done_opens_nothing() {
    let f = fixture();
    assert!(built(
        &f,
        AskTarget::Task {
            id: "nnnnnnnn".into()
        },
        None
    )
    .is_none());
    assert!(built(
        &f,
        AskTarget::Task {
            id: "bbbbbbbb".into()
        },
        None
    )
    .is_none());
    std::fs::remove_file(f.root.join(".weaver/tasks/aaaaaaaa-first-task.md")).unwrap();
    assert!(built(
        &f,
        AskTarget::Task {
            id: "aaaaaaaa".into()
        },
        None
    )
    .is_none());
}

#[test]
fn a_proposal_names_its_tasks_and_a_proposal_that_is_not_live_opens_nothing() {
    let f = fixture();
    let found = built(
        &f,
        AskTarget::Proposal {
            id: "pppppppp".into(),
        },
        None,
    )
    .unwrap();
    assert_eq!(
        (found.name.as_str(), found.kind),
        ("Ask P1", AskKind::Proposal)
    );
    for line in [
        "State: open\n",
        "Title: Cancel an order\n",
        "- tttttttt (Cancel it)\n",
        "Why the order is cancelled.\n",
    ] {
        assert!(
            found.instruction.contains(line),
            "missing {line:?} in {}",
            found.instruction
        );
    }
    assert!(built(
        &f,
        AskTarget::Proposal {
            id: "nnnnnnnn".into()
        },
        None
    )
    .is_none());
    put(
        &f.root,
        "proposals/pppppppp-t/proposal.md",
        &PROPOSAL.replace("status: open", "status: approved"),
    );
    assert!(built(
        &f,
        AskTarget::Proposal {
            id: "pppppppp".into()
        },
        None
    )
    .is_none());
}

#[test]
fn a_drift_gives_its_two_sides_in_full_and_the_cards_it_is_linked_to() {
    let f = fixture();
    let found = built(
        &f,
        AskTarget::Drift {
            id: "dddddddd".into(),
        },
        None,
    )
    .unwrap();
    assert_eq!(
        (found.name.as_str(), found.kind),
        ("Ask D1", AskKind::Drift)
    );
    for line in [
        "What happens:\nIt does A.\n",
        "What should happen:\nIt does B.\n",
        "Task it was born from: aaaaaaaa\n",
        "- `weaver task show aaaaaaaa`\n",
        "- `weaver model show requirements/ui#General`\n",
    ] {
        assert!(
            found.instruction.contains(line),
            "missing {line:?} in {}",
            found.instruction
        );
    }
    put(
        &f.root,
        "drifts/dddddddd-a-drift.md",
        &DRIFT.replace("status: open", "status: dismissed"),
    );
    assert!(built(
        &f,
        AskTarget::Drift {
            id: "dddddddd".into()
        },
        None
    )
    .is_none());
}

#[test]
fn a_history_entry_is_read_whole_with_the_tasks_it_cites_and_is_named_by_its_identity() {
    let f = fixture();
    let target = AskTarget::History {
        entry: "hhhhhhhh".into(),
    };
    let found = built(&f, target, None).unwrap();
    assert_eq!(
        (found.name.as_str(), found.kind, found.round),
        ("Ask history hhhhhhhh", AskKind::History, None)
    );
    for line in [
        "Permanent identity: hhhhhhhh\nTitle: Checkout works\nSummary:\nCheckout works.\nText:\nThe entry text.\n",
        // From the copy in the entry.
        "\nTask 1 of 3:\nPermanent identity: zzzzzzzz\nTitle: Pay with a card\n",
        "Label: Checkout\n",
        "Finished: 2026-11-02T14:46:56.993Z\n",
        "Body:\nCard payments succeed.\n",
        // From its file, still there.
        "\nTask 2 of 3:\nPermanent identity: bbbbbbbb\nTitle: Done task\nFinished: 2026-11-02T08:00:00.000Z\n",
        // Neither copy nor file.
        "\nTask 3 of 3:\nPermanent identity: yyyyyyyy\nNot available: neither its copy in the entry nor its file can be read.\n",
        "- `weaver history show hhhhhhhh`\n- `weaver proposal show pppppppp`\n- `weaver model show requirements/ui#General`\n",
    ] {
        assert!(
            found.instruction.contains(line),
            "missing {line:?} in {}",
            found.instruction
        );
    }
    assert_eq!(found.instruction.matches("weaver proposal show").count(), 1);
    assert_eq!(found.instruction.matches("weaver model show").count(), 1);
    // A Proposal that is gone is not listed, and an unreadable Task never prevents opening.
    std::fs::remove_dir_all(f.root.join(".weaver/proposals/pppppppp-t")).unwrap();
    let target = AskTarget::History {
        entry: "hhhhhhhh".into(),
    };
    let found = built(&f, target, None).unwrap();
    assert!(!found.instruction.contains("weaver proposal show"));
    assert!(found
        .instruction
        .contains("Proposal whose design it belonged to: pppppppp\n"));
    // The entry is gone: nothing opens.
    let missing = AskTarget::History {
        entry: "nnnnnnnn".into(),
    };
    assert!(built(&f, missing, None).is_none());
}

#[test]
fn the_requirements_open_without_reading_any_text_and_name_no_area_fragment_or_section() {
    let f = fixture();
    let found = built(&f, AskTarget::Requirements { overlay: None }, None).unwrap();
    assert_eq!(
        (found.name.as_str(), found.kind, found.round),
        ("Ask Requirements", AskKind::Requirements, None)
    );
    for line in [
        "Chosen thing: the Requirements of the project\n",
        "Requirement skills `weaver-req-*` installed in the project",
        "Load by yourself the ones that fit the developer's message",
    ] {
        assert!(
            found.instruction.contains(line),
            "missing {line:?} in {}",
            found.instruction
        );
    }
    let part = found.instruction.split("Chosen thing:").nth(1).unwrap();
    for absent in [
        "the general text",
        "Area",
        "Fragment",
        "requirements/ui",
        "overlay",
        "Commands that read it",
        "weaver ",
    ] {
        assert!(!part.contains(absent), "unexpected {absent:?} in {part}");
    }
}

#[test]
fn the_language_of_the_developer_ends_the_requirements_instruction() {
    let f = fixture();
    let found = built(
        &f,
        AskTarget::Requirements { overlay: None },
        Some("Italian"),
    )
    .unwrap();
    assert!(found
        .instruction
        .ends_with("\nWrite the texts for the developer in Italian.\n"));
}

fn present(f: &Fixture) -> u64 {
    let request: RoundRequest = serde_json::from_value(serde_json::json!({
        "questions": [
            { "id": "a", "question": "Which one?", "options": [
                { "label": "x", "description": "first" }, { "label": "y", "description": "second" } ], "recommended": 0, "multi": false },
            { "id": "b", "question": "And then?", "options": [
                { "label": "p", "description": "third" }, { "label": "q", "description": "fourth" } ], "recommended": 1, "multi": true },
        ]
    }))
    .unwrap();
    let _outcome = f
        .rounds
        .present_prepared(PROJECT, "dddddddd", "D1", "A drift", request);
    f.rounds.view(PROJECT).expect("the round is presented").id
}

#[test]
fn the_question_in_view_is_read_from_the_presented_round_with_the_identity_of_its_drift() {
    let f = fixture();
    let round = present(&f);
    let found = built(&f, AskTarget::Question { round, index: 0 }, None).unwrap();
    assert_eq!(
        (found.name.as_str(), found.kind, found.round),
        ("Ask question", AskKind::Question, Some(round))
    );
    for line in [
        "Question 1 of 2:\n",
        "Question:\nWhich one?\n",
        "2. And then?\n",
        "Prepared question of the Drift: dddddddd\n",
        "The round is still open",
        "- `weaver drift show dddddddd`\n",
    ] {
        assert!(
            found.instruction.contains(line),
            "missing {line:?} in {}",
            found.instruction
        );
    }
}

fn two_questions() -> RoundRequest {
    serde_json::from_value(serde_json::json!({
        "questions": [
            { "id": "a", "question": "Which one?", "context": "Because.", "options": [
                { "label": "x", "description": "first", "details": "More on x." },
                { "label": "y", "description": "second" } ], "recommended": 1, "multi": false },
            { "id": "b", "question": "And then?", "options": [
                { "label": "p", "description": "third" }, { "label": "q", "description": "fourth" } ], "recommended": 0, "multi": true },
        ]
    }))
    .unwrap()
}

#[test]
fn the_question_in_view_of_the_active_card_of_the_chat_is_read_without_commands() {
    let f = fixture();
    *f.cards.active.lock().unwrap() = Some(("cardcard".into(), two_questions()));
    let found = built(
        &f,
        AskTarget::Card {
            card: "cardcard".into(),
            index: 1,
        },
        None,
    )
    .unwrap();
    assert_eq!(
        (
            found.name.as_str(),
            found.kind,
            found.round,
            found.card.as_deref()
        ),
        ("Ask question", AskKind::Question, None, Some("cardcard"))
    );
    for line in [
        "Question 2 of 2:\n",
        "Question:\nAnd then?\n",
        "The developer can choose several options.\n",
        "1. Which one?\n   - x: first\n   - y: second\n",
        "2. (the question in view, described above)\n",
        "Asked by: Chat\n",
        "The card is still active: the developer has not answered it. Do not answer it for the developer.\n",
    ] {
        assert!(
            found.instruction.contains(line),
            "missing {line:?} in {}",
            found.instruction
        );
    }
    assert!(!found.instruction.contains("round"));
    assert!(!found.instruction.contains("weaver drift show"));
    assert!(!found.instruction.contains("Commands that read it"));
}

#[test]
fn a_card_that_is_not_active_or_not_the_one_asked_or_has_no_such_question_opens_nothing() {
    let f = fixture();
    let ask = |card: &str, index: usize| {
        built(
            &f,
            AskTarget::Card {
                card: card.into(),
                index,
            },
            None,
        )
    };
    // No active card: sent, closed by a message, replaced or cleared by `New chat`.
    assert!(ask("cardcard", 0).is_none());
    *f.cards.active.lock().unwrap() = Some(("cardcard".into(), two_questions()));
    assert!(ask("cardcard", 0).is_some());
    assert!(ask("othercrd", 0).is_none());
    assert!(ask("cardcard", 2).is_none());
}

#[test]
fn a_question_of_another_round_or_position_or_a_closed_round_opens_nothing() {
    let f = fixture();
    let round = present(&f);
    assert!(built(
        &f,
        AskTarget::Question {
            round: round + 1,
            index: 0
        },
        None
    )
    .is_none());
    assert!(built(&f, AskTarget::Question { round, index: 1 }, None).is_none());
    assert!(built(&f, AskTarget::Question { round, index: 9 }, None).is_none());
    f.rounds.window_closed(PROJECT);
    assert!(built(&f, AskTarget::Question { round, index: 0 }, None).is_none());
}

#[test]
fn an_open_overlay_proposal_is_named_with_its_short_number_and_is_the_only_command() {
    let f = fixture();
    let target = AskTarget::Requirements {
        overlay: Some("pppppppp".into()),
    };
    let found = built(&f, target, None).unwrap();
    assert_eq!(found.name, "Ask Requirements");
    for line in [
        "The view shows the Requirements with the changes of a Proposal applied:\n",
        "  Permanent identity: pppppppp\n",
        "  Short number (as the developer sees it): P1\n",
        "  Title: Cancel an order\n",
        "Commands that read it:\n- `weaver proposal show pppppppp`\n",
    ] {
        assert!(
            found.instruction.contains(line),
            "missing {line:?} in {}",
            found.instruction
        );
    }
    let part = found.instruction.split("Chosen thing:").nth(1).unwrap();
    assert_eq!(part.matches("weaver ").count(), 1);
}

#[test]
fn an_overlay_proposal_that_is_not_open_is_left_out_and_the_conversation_opens_all_the_same() {
    let f = fixture();
    let without = built(&f, AskTarget::Requirements { overlay: None }, None).unwrap();
    // A Proposal that does not exist.
    let missing = AskTarget::Requirements {
        overlay: Some("nnnnnnnn".into()),
    };
    assert_eq!(built(&f, missing, None).unwrap(), without);
    // A Proposal that is no longer open.
    put(
        &f.root,
        "proposals/pppppppp-t/proposal.md",
        &PROPOSAL.replace("status: open", "status: rejected"),
    );
    let closed = AskTarget::Requirements {
        overlay: Some("pppppppp".into()),
    };
    assert_eq!(built(&f, closed, None).unwrap(), without);
    // A Proposal attached to a Drift is not one the overlay picker offers.
    put(&f.root, "proposals/pppppppp-t/proposal.md", PROPOSAL);
    put(
        &f.root,
        "drifts/eeeeeeee-attached.md",
        "---\nstatus: open\nrefs: []\nproposal: pppppppp\norigin: aaaaaaaa\ncreated: 2026-11-02T07:00:04.000Z\n---\n# Attached\n\n## What happens\n\nIt does A.\n\n## What should happen\n\nIt does B.\n",
    );
    let attached = AskTarget::Requirements {
        overlay: Some("pppppppp".into()),
    };
    assert_eq!(built(&f, attached, None).unwrap(), without);
}
