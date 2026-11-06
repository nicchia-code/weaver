//! The jobs against the scripted fake ACP agent: every transition of the Machine `job` as the app
//! drives it, steering, permissions, forms, Stop, suspension and resume. No omp, no network.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{json, Value};
use weaver_core::jobs::machine::RESUME_PROMPT;
use weaver_core::jobs::subagents::SubAgentState;
use weaver_core::jobs::{FiledCard, JobState, JobStore};
use weaver_core::project::image::ProjectImage;
use weaver_core::settings::Permissions;

use super::conversation::{Delivery, EndLine, Entry, PermissionState};
use super::registry::Jobs;
use super::starts::Ctx;
use super::testing::{row, text_update, until, Project, TestHost, LIMIT, TASK};
use crate::subagents::wire::Step;

fn ctx<'a>(project: &'a Project, image: &'a ProjectImage) -> Ctx<'a> {
    Ctx {
        project_id: &project.id,
        root: &project.root,
        image,
    }
}

fn run_task(jobs: &Jobs, project: &Project, task: &str) -> Result<String, String> {
    let image = ProjectImage::load(&project.root);
    jobs.start_run_task(&ctx(project, &image), task)
        .map_err(|refusal| refusal.0)
}

async fn state_of(host: &TestHost, project: &Project, job: &str, wanted: JobState) {
    until(&format!("job {job} to be {wanted:?}"), LIMIT, || {
        row(host, &project.id, job).is_some_and(|r| r.state == wanted)
    })
    .await;
}

fn entries(jobs: &Jobs, project: &Project, job: &str) -> Vec<Entry> {
    jobs.conversation(&project.id, job).unwrap().entries
}

fn agent(text: &str) -> Value {
    json!({ "update": text_update("agent_message_chunk", text) })
}

fn setup() -> (Arc<TestHost>, Arc<Jobs>) {
    let host = TestHost::new();
    let jobs = Jobs::new(host.clone());
    (host, jobs)
}

async fn one_task() -> (Arc<TestHost>, Arc<Jobs>, tempfile::TempDir, Project) {
    let (host, jobs) = setup();
    let (dir, project) = host
        .project(&[("tasks/aaaaaaaa-first-task.md", TASK)])
        .await;
    (host, jobs, dir, project)
}

#[tokio::test]
async fn a_run_goes_from_queued_through_running_to_done_with_its_conversation_and_its_shutdown() {
    let (host, jobs, _dir, project) = one_task().await;
    host.script(json!({
        "log": host.received_log(),
        "logEnv": ["WEAVER_JOB", "WEAVER_RUN", "WEAVER_TASK", "WEAVER_ENDPOINT"],
        "prompts": [[
            agent("Reading "), agent("the Task."),
            { "update": { "sessionUpdate": "tool_call", "toolCallId": "t1", "title": "Read a.rs", "kind": "read", "status": "in_progress" } },
            { "update": { "sessionUpdate": "tool_call_update", "toolCallId": "t1", "status": "completed" } },
            { "update": text_update("agent_thought_chunk", "hmm") },
        ]],
    }));
    let id = run_task(&jobs, &project, "aaaaaaaa").unwrap();
    assert_eq!(
        row(&host, &project.id, &id).unwrap().state,
        JobState::Queued,
        "created Queued"
    );
    state_of(&host, &project, &id, JobState::Done).await;

    let row = row(&host, &project.id, &id).unwrap();
    assert_eq!(row.name, "Run T1");
    assert!(row.started.is_some() && row.ended.is_some());

    let conversation = entries(&jobs, &project, &id);
    assert!(
        matches!(&conversation[0], Entry::Instruction { text } if text.starts_with("Use the weaver-implement skill on Task aaaaaaaa."))
    );
    assert!(conversation.contains(&Entry::Agent {
        text: "Reading the Task.".into(),
        complete: true
    }));
    assert!(conversation
        .iter()
        .any(|e| matches!(e, Entry::Tool(t) if t.title == "Read a.rs")));
    assert_eq!(conversation.last(), Some(&Entry::End(EndLine::Finished)));

    let received = host.received();
    let env = &received[0];
    // The inherited WEAVER_JOB never reaches omp; Weaver's own variables do.
    assert_eq!(env["env"]["WEAVER_JOB"], json!(id));
    assert_eq!(env["env"]["WEAVER_RUN"], "1");
    assert_eq!(env["env"]["WEAVER_TASK"], "aaaaaaaa");
    assert_eq!(env["env"]["WEAVER_ENDPOINT"], "/tmp/weaver-test.sock");
    let args: Vec<&str> = env["args"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert!(
        args.contains(&"--advisor") && args.ends_with(&["--approval-mode", "yolo", "acp"]),
        "{args:?}"
    );

    let initialize = received
        .iter()
        .find(|m| m["method"] == "initialize")
        .unwrap();
    assert_eq!(initialize["params"]["protocolVersion"], 1);
    assert!(
        initialize["params"]["clientCapabilities"]
            .get("fs")
            .is_none()
            && initialize["params"]["clientCapabilities"]
                .get("terminal")
                .is_none()
    );
    let new = received
        .iter()
        .find(|m| m["method"] == "session/new")
        .unwrap();
    assert_eq!(
        new["params"]["cwd"],
        project.root.to_string_lossy().as_ref()
    );
    let prompt = received
        .iter()
        .find(|m| m["method"] == "session/prompt")
        .unwrap();
    assert!(prompt["params"]["prompt"][0]["text"]
        .as_str()
        .unwrap()
        .starts_with("Use the weaver-implement skill on Task aaaaaaaa."));
    until("the shutdown", LIMIT, || {
        host.received()
            .iter()
            .any(|m| m["method"] == "session/close")
    })
    .await;

    // The registry keeps the record of the finished job; the overlay is gone.
    jobs.flush();
    let record = JobStore::new(&host.dirs).read(&id).unwrap().unwrap();
    assert_eq!(
        (record.state, record.session_id.as_deref()),
        (JobState::Done, Some("fake-session"))
    );
    assert!(record.args.contains(&"--advisor".to_owned()) && record.env["WEAVER_JOB"] == id);
    until("the overlay to be deleted", LIMIT, || {
        !weaver_core::jobs::record::overlay_file(&host.dirs, &id).exists()
    })
    .await;
}

#[tokio::test]
async fn a_session_omp_cannot_open_fails_the_job_with_the_reason_and_holds_nothing() {
    let (host, jobs, _dir, project) = one_task().await;
    host.script(json!({ "newError": "no provider is logged in" }));
    let id = run_task(&jobs, &project, "aaaaaaaa").unwrap();
    state_of(&host, &project, &id, JobState::Failed).await;
    assert_eq!(
        row(&host, &project.id, &id).unwrap().reason.as_deref(),
        Some("no provider is logged in")
    );
    assert!(
        matches!(entries(&jobs, &project, &id).last(), Some(Entry::End(EndLine::Failed { reason, .. })) if reason == "no provider is logged in")
    );
    assert!(
        row(&host, &project.id, &id).unwrap().started.is_none(),
        "it never started"
    );
    assert!(jobs.facts_of(&project.id).working_jobs.is_empty());
}

#[tokio::test]
async fn an_omp_without_steering_or_without_session_loading_is_a_start_failure() {
    let (host, jobs, _dir, project) = one_task().await;
    host.script(json!({ "steeringSupported": false }));
    let id = run_task(&jobs, &project, "aaaaaaaa").unwrap();
    state_of(&host, &project, &id, JobState::Failed).await;
    assert!(row(&host, &project.id, &id)
        .unwrap()
        .reason
        .unwrap()
        .contains("while the agent works"));
    host.script(json!({ "loadSession": false }));
    let again = run_task(&jobs, &project, "aaaaaaaa").unwrap();
    state_of(&host, &project, &again, JobState::Failed).await;
    assert!(row(&host, &project.id, &again)
        .unwrap()
        .reason
        .unwrap()
        .contains("reload"));
}

#[tokio::test]
async fn a_failed_turn_and_every_stop_reason_end_the_job_as_the_requirements_say() {
    for (reason, state, text) in [
        ("refusal", JobState::Failed, Some("the agent refused")),
        (
            "max_tokens",
            JobState::Failed,
            Some("the agent ran out of tokens"),
        ),
        (
            "max_turn_requests",
            JobState::Failed,
            Some("the agent reached its turn limit"),
        ),
        ("cancelled", JobState::Done, None),
    ] {
        let (host, jobs, _dir, project) = one_task().await;
        host.script(json!({ "prompts": [[{ "end": reason }]] }));
        let id = run_task(&jobs, &project, "aaaaaaaa").unwrap();
        state_of(&host, &project, &id, state).await;
        assert_eq!(
            row(&host, &project.id, &id).unwrap().reason.as_deref(),
            text,
            "{reason}"
        );
    }
    let (host, jobs, _dir, project) = one_task().await;
    host.script(json!({ "prompts": [[{ "error": "quota exceeded" }]] }));
    let id = run_task(&jobs, &project, "aaaaaaaa").unwrap();
    state_of(&host, &project, &id, JobState::Failed).await;
    assert_eq!(
        row(&host, &project.id, &id).unwrap().reason.as_deref(),
        Some("quota exceeded")
    );
}

#[tokio::test]
async fn omp_exiting_by_itself_fails_the_job_once_with_its_last_error_lines() {
    let (host, jobs, _dir, project) = one_task().await;
    host.script(json!({ "prompts": [[agent("Working"), { "stderr": "warming up" }, { "stderr": "error: the provider closed the connection" }, { "sleep": 100 }, { "exit": 3 }]] }));
    let id = run_task(&jobs, &project, "aaaaaaaa").unwrap();
    state_of(&host, &project, &id, JobState::Failed).await;
    let conversation = entries(&jobs, &project, &id);
    let Some(Entry::End(EndLine::Failed { reason, details })) = conversation.last() else {
        panic!("{conversation:?}")
    };
    assert_eq!(reason, "error: the provider closed the connection");
    assert_eq!(
        details.as_deref(),
        Some("warming up\nerror: the provider closed the connection")
    );
    assert_eq!(
        conversation
            .iter()
            .filter(|e| matches!(e, Entry::End(_)))
            .count(),
        1,
        "once"
    );
}

#[tokio::test]
async fn omp_exiting_without_a_word_is_the_exit_code() {
    let (host, jobs, _dir, project) = one_task().await;
    host.script(json!({ "prompts": [[{ "exit": 7 }]] }));
    let id = run_task(&jobs, &project, "aaaaaaaa").unwrap();
    state_of(&host, &project, &id, JobState::Failed).await;
    assert_eq!(
        row(&host, &project.id, &id).unwrap().reason.as_deref(),
        Some("omp exited with code 7")
    );
}

#[tokio::test]
async fn a_message_while_running_steers_the_agent_without_stopping_it() {
    let (host, jobs, _dir, project) = one_task().await;
    host.script(json!({ "log": host.received_log(), "steering": ["injected"], "prompts": [[agent("Start"), { "wait_steer": 1 }, agent("Got it")]] }));
    let id = run_task(&jobs, &project, "aaaaaaaa").unwrap();
    state_of(&host, &project, &id, JobState::Running).await;
    until("the first words", LIMIT, || {
        entries(&jobs, &project, &id)
            .iter()
            .any(|e| matches!(e, Entry::Agent { text, .. } if text == "Start"))
    })
    .await;
    jobs.send_message(&project.id, &id, "  Use the old parser  ")
        .unwrap();
    state_of(&host, &project, &id, JobState::Done).await;

    let steer = host
        .received()
        .into_iter()
        .find(|m| m["method"] == "_session/steering")
        .unwrap();
    assert_eq!(steer["params"]["prompt"][0]["text"], "Use the old parser");
    assert!(
        steer["params"].get("_meta").is_none(),
        "no `_meta` while running"
    );
    let conversation = entries(&jobs, &project, &id);
    assert!(conversation.iter().any(|e| matches!(e, Entry::Developer { text, delivery: Delivery::Delivered, .. } if text == "Use the old parser")));
    assert!(
        conversation.contains(&Entry::Agent {
            text: "Got it".into(),
            complete: true
        }),
        "{conversation:?}"
    );
}

#[tokio::test]
async fn a_refused_steer_shows_not_delivered_with_its_reason_and_the_job_goes_on() {
    let (host, jobs, _dir, project) = one_task().await;
    host.script(json!({ "steering": ["failed:the turn is closing"], "prompts": [[{ "wait_steer": 1 }, agent("Still here")]] }));
    let id = run_task(&jobs, &project, "aaaaaaaa").unwrap();
    state_of(&host, &project, &id, JobState::Running).await;
    jobs.send_message(&project.id, &id, "Hello").unwrap();
    state_of(&host, &project, &id, JobState::Done).await;
    assert!(entries(&jobs, &project, &id).iter().any(|e| matches!(e, Entry::Developer { delivery: Delivery::NotDelivered { reason }, .. } if reason == "the turn is closing")));
}

#[tokio::test]
async fn a_steer_that_needs_a_prompt_starts_a_new_turn_when_the_turn_ends() {
    let (host, jobs, _dir, project) = one_task().await;
    host.script(json!({
        "log": host.received_log(),
        "steering": ["promptRequired"],
        "prompts": [[{ "wait_steer": 1 }, agent("First turn")], [agent("Second turn")]],
    }));
    let id = run_task(&jobs, &project, "aaaaaaaa").unwrap();
    state_of(&host, &project, &id, JobState::Running).await;
    jobs.send_message(&project.id, &id, "And also this")
        .unwrap();
    state_of(&host, &project, &id, JobState::Done).await;
    let prompts: Vec<String> = host
        .received()
        .into_iter()
        .filter(|m| m["method"] == "session/prompt")
        .map(|m| {
            m["params"]["prompt"][0]["text"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect();
    assert_eq!(prompts.len(), 2);
    assert_eq!(
        prompts[1], "And also this",
        "the held message starts the new turn"
    );
    let conversation = entries(&jobs, &project, &id);
    assert!(conversation.contains(&Entry::Agent {
        text: "Second turn".into(),
        complete: true
    }));
    assert!(conversation.iter().any(|e| matches!(
        e,
        Entry::Developer {
            delivery: Delivery::Delivered,
            ..
        }
    )));
}

#[tokio::test]
async fn a_message_that_tags_assets_goes_to_omp_with_the_block_and_the_conversation_shows_only_the_text(
) {
    let (host, jobs, _dir, project) = one_task().await;
    *host.assets.lock().unwrap() = vec!["logo.png".to_owned(), "my notes.txt".to_owned()];
    host.script(json!({ "log": host.received_log(), "steering": ["injected"], "prompts": [[agent("Start"), { "wait_steer": 1 }, agent("Got it")]] }));
    let id = run_task(&jobs, &project, "aaaaaaaa").unwrap();
    state_of(&host, &project, &id, JobState::Running).await;
    until("the first words", LIMIT, || {
        entries(&jobs, &project, &id)
            .iter()
            .any(|e| matches!(e, Entry::Agent { text, .. } if text == "Start"))
    })
    .await;
    let written = "Use @my notes.txt, and @logo.png; not @missing.png";
    jobs.send_message(&project.id, &id, written).unwrap();
    state_of(&host, &project, &id, JobState::Done).await;

    let steer = host
        .received()
        .into_iter()
        .find(|m| m["method"] == "_session/steering")
        .unwrap();
    assert_eq!(
        steer["params"]["prompt"][0]["text"],
        format!("{written}\n\nAssets tagged by the developer (added by Weaver, not written by the developer):\nRead the file `.weaver/assets/my notes.txt`.\nRead the file `.weaver/assets/logo.png`.")
    );
    assert!(entries(&jobs, &project, &id).iter().any(|e| matches!(
        e,
        Entry::Developer { text, delivery: Delivery::Delivered, .. } if text == written
    )));
}

#[tokio::test]
async fn yolo_answers_a_permission_that_still_arrives_with_the_first_allow_always() {
    let (host, jobs, _dir, project) = one_task().await;
    host.script(
        json!({ "log": host.received_log(), "prompts": [[{ "permission": {
        "toolCall": { "toolCallId": "t1", "title": "$ rm -rf build", "kind": "execute" },
        "options": [
            { "optionId": "no", "name": "Reject", "kind": "reject_once" },
            { "optionId": "once", "name": "Allow once", "kind": "allow_once" },
            { "optionId": "always", "name": "Always allow", "kind": "allow_always" },
        ] } }]] }),
    );
    let id = run_task(&jobs, &project, "aaaaaaaa").unwrap();
    state_of(&host, &project, &id, JobState::Done).await;
    let answer = host
        .received()
        .into_iter()
        .find(|m| m["answerTo"] == "session/request_permission")
        .unwrap();
    assert_eq!(
        answer["result"],
        json!({ "outcome": { "outcome": "selected", "optionId": "always" } })
    );
    assert!(
        !entries(&jobs, &project, &id)
            .iter()
            .any(|e| matches!(e, Entry::Permission { .. })),
        "the UI receives nothing"
    );
}

#[tokio::test]
async fn ask_sends_the_permission_to_the_ui_and_waits_for_the_developer_without_a_limit() {
    let (host, jobs, _dir, project) = one_task().await;
    host.permissions(Permissions::Ask);
    host.script(json!({ "log": host.received_log(), "prompts": [[agent("About to run"), { "permission": {
        "toolCall": { "toolCallId": "t1", "title": "$ rm -rf build", "kind": "execute", "rawInput": { "command": "rm -rf build" } },
        "options": [
            { "optionId": "allow", "name": "Allow once", "kind": "allow_once" },
            { "optionId": "no", "name": "Reject", "kind": "reject_once" },
        ] } }, agent("Done")]] }));
    let id = run_task(&jobs, &project, "aaaaaaaa").unwrap();
    state_of(&host, &project, &id, JobState::Waiting).await;
    assert_eq!(
        jobs.working_count(&project.id),
        1,
        "waiting counts as working"
    );
    assert!(jobs.facts_of(&project.id).run_in_progress);
    let (request, options) = entries(&jobs, &project, &id)
        .into_iter()
        .find_map(|e| {
            if let Entry::Permission {
                request, options, ..
            } = e
            {
                Some((request, options))
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(
        options
            .iter()
            .map(|o| (o.name.as_str(), o.rejects))
            .collect::<Vec<_>>(),
        [("Allow once", false), ("Reject", true)]
    );

    // A message while waiting is held and never answers the permission.
    jobs.send_message(&project.id, &id, "Careful").unwrap();
    assert_eq!(
        row(&host, &project.id, &id).unwrap().state,
        JobState::Waiting
    );
    assert!(entries(&jobs, &project, &id).iter().any(|e| matches!(
        e,
        Entry::Developer {
            delivery: Delivery::Queued,
            ..
        }
    )));

    jobs.answer_permission(&project.id, &id, request, "allow")
        .unwrap();
    state_of(&host, &project, &id, JobState::Done).await;
    let answer = host
        .received()
        .into_iter()
        .find(|m| m["answerTo"] == "session/request_permission")
        .unwrap();
    assert_eq!(
        answer["result"],
        json!({ "outcome": { "outcome": "selected", "optionId": "allow" } })
    );
    let conversation = entries(&jobs, &project, &id);
    assert!(conversation.iter().any(|e| matches!(e, Entry::Permission { state: PermissionState::Chosen { name }, .. } if name == "Allow once")));
    // The held message went out as a follow-up when the wait ended.
    let steer = host
        .received()
        .into_iter()
        .find(|m| m["method"] == "_session/steering")
        .unwrap();
    assert_eq!(
        steer["params"]["_meta"],
        json!({ "steering": { "queue": "followUp" } })
    );
}

#[tokio::test]
async fn stop_cancels_a_permission_that_still_asks_ends_omp_and_stops_the_job() {
    let (host, jobs, _dir, project) = one_task().await;
    host.permissions(Permissions::Ask);
    host.script(
        json!({ "log": host.received_log(), "prompts": [[{ "permission": {
        "toolCall": { "toolCallId": "t1", "title": "Edit a.rs" },
        "options": [{ "optionId": "allow", "name": "Allow", "kind": "allow_once" }] } }]] }),
    );
    let id = run_task(&jobs, &project, "aaaaaaaa").unwrap();
    state_of(&host, &project, &id, JobState::Waiting).await;
    jobs.stop(&project.id, &id).unwrap();
    state_of(&host, &project, &id, JobState::Stopped).await;
    // What omp received: the agent may be ended before it logs the answer itself.
    until("omp to be answered", LIMIT, || {
        host.received()
            .iter()
            .any(|m| m["id"] == "req-1" && m.get("result").is_some())
    })
    .await;
    let answer = host
        .received()
        .into_iter()
        .find(|m| m["id"] == "req-1" && m.get("result").is_some())
        .unwrap();
    assert_eq!(
        answer["result"],
        json!({ "outcome": { "outcome": "cancelled" } })
    );
    let conversation = entries(&jobs, &project, &id);
    assert!(conversation.iter().any(|e| matches!(
        e,
        Entry::Permission {
            state: PermissionState::Cancelled,
            ..
        }
    )));
    assert_eq!(conversation.last(), Some(&Entry::End(EndLine::Stopped)));
}

#[tokio::test]
async fn a_permission_answered_once_is_not_asked_again() {
    let (host, jobs, _dir, project) = one_task().await;
    host.permissions(Permissions::Ask);
    // The fake agent's permission waits for an answer, so the turn ends only after the developer answers; a second
    // request asks nothing more: the card of the first stays Chosen and the job ends.
    host.script(json!({ "prompts": [[{ "permission": { "toolCall": { "toolCallId": "t1", "title": "Edit" }, "options": [{ "optionId": "a", "name": "Allow", "kind": "allow_once" }] } }]] }));
    let id = run_task(&jobs, &project, "aaaaaaaa").unwrap();
    state_of(&host, &project, &id, JobState::Waiting).await;
    let request = entries(&jobs, &project, &id)
        .into_iter()
        .find_map(|e| {
            if let Entry::Permission { request, .. } = e {
                Some(request)
            } else {
                None
            }
        })
        .unwrap();
    jobs.answer_permission(&project.id, &id, request, "a")
        .unwrap();
    state_of(&host, &project, &id, JobState::Done).await;
    assert!(
        jobs.answer_permission(&project.id, &id, request, "a")
            .is_err(),
        "it is not asked any more"
    );
}

#[tokio::test]
async fn forms_and_asks_are_declined_at_once_with_the_fixed_text_and_the_ui_hears_nothing() {
    let (host, jobs, _dir, project) = one_task().await;
    host.script(
        json!({ "log": host.received_log(), "prompts": [[{ "elicit": true }, agent("Went on")]] }),
    );
    let id = run_task(&jobs, &project, "aaaaaaaa").unwrap();
    state_of(&host, &project, &id, JobState::Done).await;
    let answer = host
        .received()
        .into_iter()
        .find(|m| m["answerTo"] == "elicitation/create")
        .unwrap();
    assert_eq!(answer["result"]["action"], "decline");
    assert_eq!(
        answer["result"]["_meta"]["message"],
        "In Weaver jobs, ask the developer only with `weaver question new` (bash timeout 0). Do not ask in any other way."
    );
    assert!(!entries(&jobs, &project, &id)
        .iter()
        .any(|e| matches!(e, Entry::Permission { .. })));
}

#[tokio::test]
async fn stop_cancels_the_turn_closes_the_session_and_ends_the_process_group() {
    let (host, jobs, _dir, project) = one_task().await;
    host.script(json!({ "log": host.received_log(), "prompts": [[agent("Working"), { "wait_cancel": true }]] }));
    let id = run_task(&jobs, &project, "aaaaaaaa").unwrap();
    state_of(&host, &project, &id, JobState::Running).await;
    until("the agent to work", LIMIT, || {
        host.received()
            .iter()
            .any(|m| m["method"] == "session/prompt")
    })
    .await;
    jobs.stop(&project.id, &id).unwrap();
    state_of(&host, &project, &id, JobState::Stopped).await;
    until("the shutdown", LIMIT, || {
        let methods: Vec<String> = host
            .received()
            .iter()
            .filter_map(|m| m["method"].as_str().map(str::to_owned))
            .collect();
        methods.contains(&"session/cancel".to_owned())
            && methods.contains(&"session/close".to_owned())
    })
    .await;
    let methods: Vec<String> = host
        .received()
        .iter()
        .filter_map(|m| m["method"].as_str().map(str::to_owned))
        .collect();
    let (cancel, close) = (
        methods.iter().position(|m| m == "session/cancel").unwrap(),
        methods.iter().position(|m| m == "session/close").unwrap(),
    );
    assert!(cancel < close, "cancel first, then close: {methods:?}");
    assert!(jobs.facts_of(&project.id).working_jobs.is_empty());
    assert!(
        jobs.stop(&project.id, &id).is_ok(),
        "stopping a stopped job changes nothing"
    );
}

#[tokio::test]
async fn a_job_that_ends_gives_back_the_tasks_it_held_in_the_same_step() {
    let (host, jobs, _dir, project) = one_task().await;
    host.script(json!({ "prompts": [[{ "wait_steer": 1 }]] }));
    let id = run_task(&jobs, &project, "aaaaaaaa").unwrap();
    state_of(&host, &project, &id, JobState::Running).await;
    // The agent takes the Task: In progress, held by this job.
    let file = project.root.join(".weaver/tasks/aaaaaaaa-first-task.md");
    std::fs::write(
        &file,
        TASK.replace("column: todo", "column: doing")
            .replace("created:", &format!("holder: {id}\ncreated:")),
    )
    .unwrap();
    jobs.send_message(&project.id, &id, "Wrap up").unwrap();
    state_of(&host, &project, &id, JobState::Done).await;
    until("the Task to be released", LIMIT, || {
        std::fs::read_to_string(&file)
            .unwrap()
            .contains("column: todo")
    })
    .await;
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(!text.contains("holder"), "{text}");
}

#[tokio::test]
async fn only_one_run_works_at_a_time_and_a_resolve_waits_for_it() {
    let (host, jobs, _dir, project) = one_task().await;
    host.script(json!({ "prompts": [[{ "hold": true }]] }));
    let id = run_task(&jobs, &project, "aaaaaaaa").unwrap();
    state_of(&host, &project, &id, JobState::Running).await;
    let refusal = run_task(&jobs, &project, "aaaaaaaa").unwrap_err();
    assert!(refusal.contains("A run is in progress"), "{refusal}");
    let image = ProjectImage::load(&project.root);
    let all = jobs.start_run_all(&ctx(&project, &image)).unwrap_err();
    assert!(all.0.contains("A run is in progress"), "{}", all.0);
    // A scan always starts.
    assert!(jobs.start_scan(&ctx(&project, &image)).is_ok());
    jobs.stop(&project.id, &id).unwrap();
    state_of(&host, &project, &id, JobState::Stopped).await;
    assert!(!jobs.facts_of(&project.id).run_in_progress);
}

/// The merge of Solo mode as `.weaver/merge/merge.json` records it.
fn start_merge_file(project: &Project) {
    let dir = project.root.join(".weaver/merge");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("merge.json"),
        r#"{"branch":"feature","current":"main","start":"a","incoming":"b","started":"2026-11-05T10:00:00.000Z"}"#,
    )
    .unwrap();
}

#[tokio::test]
async fn a_merge_in_progress_refuses_runs_scans_and_imports() {
    let (_host, jobs, _dir, project) = one_task().await;
    start_merge_file(&project);
    assert!(jobs.facts_of(&project.id).merge_in_progress);
    let refusal = run_task(&jobs, &project, "aaaaaaaa").unwrap_err();
    assert!(refusal.contains("A merge is in progress"), "{refusal}");
    let image = ProjectImage::load(&project.root);
    let ctx = ctx(&project, &image);
    assert_eq!(
        jobs.start_run_all(&ctx).unwrap_err().0,
        "A merge is in progress"
    );
    assert_eq!(
        jobs.start_scan(&ctx).unwrap_err().0,
        "A merge is in progress"
    );
    assert_eq!(
        jobs.start_import(&ctx, Vec::new()).unwrap_err().0,
        "A merge is in progress"
    );
}

#[tokio::test]
async fn the_merge_job_starts_only_on_a_merge_with_conflicts_and_one_at_a_time() {
    let (host, jobs, _dir, project) = one_task().await;
    host.script(json!({ "log": host.received_log(), "logEnv": ["WEAVER_JOB", "WEAVER_RUN"], "prompts": [[{ "hold": true }]] }));
    let image = ProjectImage::load(&project.root);
    let ctx = ctx(&project, &image);
    assert_eq!(
        jobs.start_merge(&ctx, 2).unwrap_err().0,
        "No merge is in progress"
    );
    start_merge_file(&project);
    assert_eq!(
        jobs.start_merge(&ctx, 0).unwrap_err().0,
        "No file has conflicts"
    );

    let id = jobs.start_merge(&ctx, 2).unwrap();
    let row = row(&host, &project.id, &id).unwrap();
    assert_eq!(row.name, "Merge conflicts");
    assert_eq!(row.kind.as_str(), "merge");
    state_of(&host, &project, &id, JobState::Running).await;
    assert_eq!(
        jobs.start_merge(&ctx, 2).unwrap_err().0,
        "The agent is resolving the conflicts"
    );
    assert!(jobs.facts_of(&project.id).merge_job_unended);

    let conversation = entries(&jobs, &project, &id);
    assert!(
        matches!(&conversation[0], Entry::Instruction { text } if text.starts_with("Use the weaver-resolve-conflicts skill on the merge in progress recorded in `.weaver/merge/merge.json`. Run at most ")),
        "{:?}",
        conversation[0]
    );
    let received = host.received();
    let start = &received[0];
    let args: Vec<&str> = start["args"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert!(
        !args.contains(&"--advisor") && !args.contains(&"--slow"),
        "{args:?}"
    );
    assert!(start["env"]["WEAVER_RUN"].is_null());
    assert_eq!(start["env"]["WEAVER_JOB"], json!(id));

    // It leaves the merge in progress when it ends: only a new job may start.
    jobs.stop(&project.id, &id).unwrap();
    state_of(&host, &project, &id, JobState::Stopped).await;
    assert!(!jobs.facts_of(&project.id).merge_job_unended);
    assert!(jobs.start_merge(&ctx, 1).is_ok());
}

#[tokio::test]
async fn the_scan_after_a_merge_is_a_scan_with_the_merged_files_that_resets_the_count() {
    let (host, jobs, _dir, project) = one_task().await;
    host.script(json!({ "prompts": [[{ "hold": true }]] }));
    let id = run_task(&jobs, &project, "aaaaaaaa").unwrap();
    state_of(&host, &project, &id, JobState::Running).await;
    jobs.task_finished(&project.root);
    assert_eq!(jobs.facts_of(&project.id).done_since_last_scan, 1);
    jobs.stop(&project.id, &id).unwrap();
    state_of(&host, &project, &id, JobState::Stopped).await;

    let image = ProjectImage::load(&project.root);
    let merged = vec!["src/a.rs".to_owned(), "src/b/c.rs".to_owned()];
    let scan = jobs
        .start_scan_after_merge(&ctx(&project, &image), merged.clone())
        .unwrap();
    let record = jobs.record(&scan).unwrap();
    assert_eq!(record.kind.as_str(), "scan");
    assert_eq!(record.name, "Scan");
    assert_eq!(record.merged, Some(merged));
    assert_eq!(jobs.facts_of(&project.id).done_since_last_scan, 0);
    state_of(&host, &project, &scan, JobState::Running).await;
    let conversation = entries(&jobs, &project, &scan);
    assert!(
        matches!(&conversation[0], Entry::Instruction { text } if text.starts_with("Use the weaver-scan skill on the whole System after a merge: read the files the merge brought with `weaver job show`. Run at most ")),
        "{:?}",
        conversation[0]
    );

    let plain = jobs.start_scan(&ctx(&project, &image)).unwrap();
    assert_eq!(jobs.record(&plain).unwrap().merged, None);
}

#[tokio::test]
async fn closing_the_window_suspends_the_job_and_opening_it_reloads_the_session() {
    let (host, jobs, _dir, project) = one_task().await;
    host.script(json!({
        "log": host.received_log(),
        "replay": [text_update("user_message_chunk", "ignored: the conversation of this run is kept"), text_update("agent_message_chunk", "replayed")],
        "prompts": [[agent("Before"), { "hold": true }], [agent("After the reload")]],
    }));
    let id = run_task(&jobs, &project, "aaaaaaaa").unwrap();
    state_of(&host, &project, &id, JobState::Running).await;
    until("the first words", LIMIT, || {
        entries(&jobs, &project, &id)
            .iter()
            .any(|e| matches!(e, Entry::Agent { text, .. } if text == "Before"))
    })
    .await;

    jobs.window_closed(&project.id).await;
    // The new omp process follows a script of its own: its first turn is the one after the reload.
    host.script(json!({
        "log": host.received_log(),
        "replay": [text_update("user_message_chunk", "ignored: the conversation of this run is kept"), text_update("agent_message_chunk", "replayed")],
        "prompts": [[agent("After the reload")]],
    }));
    assert_eq!(
        row(&host, &project.id, &id).unwrap().state,
        JobState::Suspended
    );
    assert_eq!(jobs.working_count(&project.id), 0);
    assert!(
        jobs.facts_of(&project.id).working_jobs.contains(&id),
        "its Tasks stay held while it is suspended"
    );
    assert!(jobs.stop(&project.id, "zzzzzzzz").is_err());
    let record = JobStore::new(&host.dirs).read(&id).unwrap().unwrap();
    assert_eq!(
        record.state,
        JobState::Suspended,
        "the registry was written before the window closed"
    );

    jobs.window_opened(&project.id, &project.root);
    state_of(&host, &project, &id, JobState::Done).await;
    let received = host.received();
    let load = received
        .iter()
        .find(|m| m["method"] == "session/load")
        .expect("the session is reloaded");
    assert_eq!(load["params"]["sessionId"], "fake-session");
    let prompts: Vec<&str> = received
        .iter()
        .filter(|m| m["method"] == "session/prompt")
        .map(|m| m["params"]["prompt"][0]["text"].as_str().unwrap())
        .collect();
    assert_eq!(prompts.last().copied(), Some(RESUME_PROMPT));
    let conversation = entries(&jobs, &project, &id);
    assert!(conversation.contains(&Entry::Reloaded));
    assert!(conversation.contains(&Entry::Agent {
        text: "After the reload".into(),
        complete: true
    }));
    assert!(
        !conversation
            .iter()
            .any(|e| matches!(e, Entry::Agent { text, .. } if text == "replayed")),
        "the replay adds nothing to a conversation kept in memory"
    );
}

#[tokio::test]
async fn a_job_left_working_by_a_crash_is_suspended_at_the_next_start_and_resumes_with_its_replayed_history(
) {
    let (host, first) = setup();
    let (_dir, project) = host
        .project(&[("tasks/aaaaaaaa-first-task.md", TASK)])
        .await;
    host.script(json!({ "prompts": [[{ "hold": true }]] }));
    let id = run_task(&first, &project, "aaaaaaaa").unwrap();
    state_of(&host, &project, &id, JobState::Running).await;
    first.flush();
    // The record as the crash left it: Running, with the session and no process alive.
    let store = JobStore::new(&host.dirs);
    let mut record = store.read(&id).unwrap().unwrap();
    assert_eq!(record.state, JobState::Running);
    let process_group = record.pid.take();
    record.pid_started = None;
    record.id = "crashed1".to_owned();
    record
        .env
        .insert("WEAVER_JOB".to_owned(), "crashed1".to_owned());
    store.write(&record).unwrap();
    first.stop(&project.id, &id).unwrap();
    state_of(&host, &project, &id, JobState::Stopped).await;
    first.flush();
    assert!(process_group.is_some());

    // A new run of the app: it reads the registry when the window opens.
    let second = Jobs::new(host.clone());
    host.script(json!({
        "log": host.received_log(),
        "replay": [text_update("user_message_chunk", "Use the weaver-implement skill on Task aaaaaaaa."), text_update("agent_message_chunk", "I was reading.")],
        "prompts": [[agent("Back at work")]],
    }));
    second.window_opened(&project.id, &project.root);
    until("the crashed job to finish", LIMIT, || {
        second
            .record("crashed1")
            .is_some_and(|r| r.state == JobState::Done)
    })
    .await;
    let conversation = second
        .conversation(&project.id, "crashed1")
        .unwrap()
        .entries;
    assert!(
        matches!(&conversation[0], Entry::Instruction { text } if text.starts_with("Use the weaver-implement skill"))
    );
    assert!(conversation
        .iter()
        .any(|e| matches!(e, Entry::Agent { text, .. } if text == "I was reading.")));
    assert!(conversation.contains(&Entry::Reloaded));
    // The stopped one of the first run is in the registry but not in the list.
    assert!(second.snapshot(&project.id).jobs.iter().all(|r| r.id != id));
}

#[tokio::test]
async fn a_session_that_cannot_be_reloaded_fails_the_suspended_job() {
    let (host, jobs, _dir, project) = one_task().await;
    host.script(json!({ "prompts": [[{ "hold": true }]] }));
    let id = run_task(&jobs, &project, "aaaaaaaa").unwrap();
    state_of(&host, &project, &id, JobState::Running).await;
    jobs.window_closed(&project.id).await;
    host.script(json!({ "loadError": "unknown session" }));
    jobs.window_opened(&project.id, &project.root);
    state_of(&host, &project, &id, JobState::Failed).await;
    assert_eq!(
        row(&host, &project.id, &id).unwrap().reason.as_deref(),
        Some("the session could not be resumed")
    );
}

#[tokio::test]
async fn closing_the_window_never_leaves_a_job_running() {
    let (host, jobs, _dir, project) = one_task().await;
    host.script(json!({ "prompts": [[{ "hold": true }]] }));
    let id = run_task(&jobs, &project, "aaaaaaaa").unwrap();
    jobs.window_closed(&project.id).await;
    let state = jobs.record(&id).unwrap().state;
    assert!(
        matches!(state, JobState::Stopped | JobState::Suspended),
        "{state:?}"
    );
}

#[tokio::test]
async fn notes_announcements_and_ghosts_follow_the_job_until_the_card_arrives_or_the_job_ends() {
    let (host, jobs, _dir, project) = one_task().await;
    host.script(json!({ "prompts": [[{ "wait_steer": 1 }]] }));
    let id = run_task(&jobs, &project, "aaaaaaaa").unwrap();
    state_of(&host, &project, &id, JobState::Running).await;

    let kept = jobs
        .declare_note(&id, "  Reading\nthe order Machine ")
        .unwrap();
    assert_eq!(kept.text, "Reading the order Machine");
    jobs.declare_note(&id, "Reading the order Machine").unwrap();
    assert_eq!(
        jobs.record(&id).unwrap().notes.len(),
        1,
        "a repeat only renews the instant"
    );
    assert_eq!(
        entries(&jobs, &project, &id)
            .iter()
            .filter(|e| matches!(e, Entry::Note { .. }))
            .count(),
        1
    );
    assert_eq!(
        row(&host, &project.id, &id).unwrap().note.as_deref(),
        Some("Reading the order Machine")
    );

    let first = jobs.announce(&id, "task", "Cancel an order").unwrap();
    let second = jobs.announce(&id, "proposal", "Order changes").unwrap();
    let ghosts = jobs.ghosts_of(&project.id);
    assert_eq!(
        ghosts
            .iter()
            .map(|g| (g.id.as_str(), g.title.as_str()))
            .collect::<Vec<_>>(),
        [
            (first.as_str(), "Cancel an order"),
            (second.as_str(), "Order changes")
        ]
    );
    assert_eq!(ghosts[0].note.as_deref(), Some("Reading the order Machine"));

    // An unknown or foreign announcement still creates the card, with a note.
    assert!(jobs
        .card_filed(Some(&id), "task", "bbbbbbbb", "zzzzzzzz")
        .unwrap()
        .contains("without a ghost"));
    assert!(jobs
        .card_filed(Some("otherjob"), "task", "bbbbbbbb", &first)
        .unwrap()
        .contains("without a ghost"));
    assert_eq!(jobs.ghosts_of(&project.id).len(), 2);

    // The card is filed: the ghost stays until the picture holds it (never both, never neither).
    assert_eq!(jobs.card_filed(Some(&id), "task", "bbbbbbbb", &first), None);
    assert_eq!(jobs.ghosts_of(&project.id).len(), 2);
    std::fs::write(
        project
            .root
            .join(".weaver/tasks/bbbbbbbb-cancel-an-order.md"),
        TASK.replace("First task", "Cancel an order"),
    )
    .unwrap();
    host.pictures
        .rebuild(&project.id, crate::pictures::Rebuild::All)
        .await
        .unwrap();
    assert_eq!(
        jobs.ghosts_of(&project.id)
            .iter()
            .map(|g| g.id.clone())
            .collect::<Vec<_>>(),
        std::slice::from_ref(&second)
    );

    // The job ends: what it announced and never filed is withdrawn.
    jobs.send_message(&project.id, &id, "Wrap up").unwrap();
    state_of(&host, &project, &id, JobState::Done).await;
    assert!(jobs.ghosts_of(&project.id).is_empty());
    assert!(
        jobs.declare_note(&id, "too late").is_err() && jobs.announce(&id, "task", "Late").is_err(),
        "a finished job declares nothing"
    );
}

fn ghost_ids(jobs: &Jobs, project: &Project) -> Vec<String> {
    jobs.ghosts_of(&project.id)
        .into_iter()
        .map(|ghost| ghost.id)
        .collect()
}

/// The file of a Task card arrives on the Board.
async fn task_arrives(host: &TestHost, project: &Project, id: &str, title: &str) {
    std::fs::write(
        project.root.join(format!(".weaver/tasks/{id}-card.md")),
        TASK.replace("First task", title),
    )
    .unwrap();
    host.pictures
        .rebuild(&project.id, crate::pictures::Rebuild::All)
        .await
        .unwrap();
}

#[tokio::test]
async fn weavers_own_ghost_of_an_import_comes_with_the_job_swaps_with_announced_ghosts_and_goes_with_the_job(
) {
    use crate::agent_work::{GhostKind, GhostPhase};

    let (host, jobs, _dir, project) = one_task().await;
    host.script(json!({ "prompts": [[{ "wait_steer": 1 }]] }));
    let image = ProjectImage::load(&project.root);
    let id = jobs
        .start_import(&ctx(&project, &image), Vec::new())
        .unwrap();
    let own = format!("job:{id}");

    // From the creation, before omp opens its session: no announcement of the agent needed.
    let ghosts = jobs.ghosts_of(&project.id);
    assert_eq!(ghosts.len(), 1);
    assert_eq!(
        (
            ghosts[0].id.as_str(),
            ghosts[0].job_id.as_str(),
            ghosts[0].kind,
            ghosts[0].title.as_str(),
            ghosts[0].phase,
            ghosts[0].note.as_deref()
        ),
        (
            own.as_str(),
            id.as_str(),
            GhostKind::Proposal,
            "Import from Code",
            GhostPhase::Preparing,
            None
        )
    );
    state_of(&host, &project, &id, JobState::Running).await;
    jobs.declare_note(&id, "Reading the code").unwrap();
    let ghosts = jobs.ghosts_of(&project.id);
    assert_eq!(ghosts.len(), 1);
    assert_eq!(ghosts[0].id, own, "the identity stays across pictures");
    assert_eq!(ghosts[0].note.as_deref(), Some("Reading the code"));
    assert_eq!(
        row(&host, &project.id, &id).unwrap().name,
        "Import",
        "the name of the job does not change"
    );

    // The first announced ghost replaces it in the same list, and nothing is stored for it.
    let first = jobs.announce(&id, "task", "Cancel an order").unwrap();
    assert_eq!(ghost_ids(&jobs, &project), std::slice::from_ref(&first));
    let second = jobs.announce(&id, "task", "Change an order").unwrap();
    assert_eq!(ghost_ids(&jobs, &project), [first.clone(), second.clone()]);
    jobs.flush();
    let stored = JobStore::new(&host.dirs).read(&id).unwrap().unwrap();
    assert_eq!(
        stored
            .announcements
            .iter()
            .map(|a| (a.id.clone(), a.card.clone()))
            .collect::<Vec<_>>(),
        [(first.clone(), None), (second.clone(), None)]
    );

    // The card is filed: the record keeps it, and the ghost stays until the card is on the Board.
    assert_eq!(jobs.card_filed(Some(&id), "task", "bbbbbbbb", &first), None);
    assert_eq!(jobs.ghosts_of(&project.id).len(), 2);
    jobs.flush();
    let stored = JobStore::new(&host.dirs).read(&id).unwrap().unwrap();
    assert_eq!(
        stored.announcements[0].card,
        Some(FiledCard {
            kind: "task".into(),
            id: "bbbbbbbb".into()
        })
    );
    assert_eq!(stored.announcements[1].card, None);
    task_arrives(&host, &project, "bbbbbbbb", "Cancel an order").await;
    assert_eq!(ghost_ids(&jobs, &project), std::slice::from_ref(&second));

    // The last announced ghost leaves: Weaver's own comes back in the same list.
    assert_eq!(
        jobs.card_filed(Some(&id), "task", "cccccccc", &second),
        None
    );
    task_arrives(&host, &project, "cccccccc", "Change an order").await;
    assert_eq!(ghost_ids(&jobs, &project), std::slice::from_ref(&own));

    // The job ends: it goes with the job.
    jobs.send_message(&project.id, &id, "Wrap up").unwrap();
    state_of(&host, &project, &id, JobState::Done).await;
    assert!(ghost_ids(&jobs, &project).is_empty());
    jobs.flush();
    assert!(JobStore::new(&host.dirs)
        .read(&id)
        .unwrap()
        .unwrap()
        .announcements
        .is_empty());
}

#[tokio::test]
async fn weavers_own_ghost_waits_for_the_answers_and_keeps_the_place_of_its_job_creation() {
    use crate::agent_work::GhostPhase;

    let (host, jobs, _dir, project) = one_task().await;
    host.permissions(Permissions::Ask);
    host.script(json!({ "prompts": [[{ "permission": {
        "toolCall": { "toolCallId": "t1", "title": "$ ls", "kind": "execute", "rawInput": { "command": "ls" } },
        "options": [{ "optionId": "allow", "name": "Allow once", "kind": "allow_once" }] } }]] }));
    let image = ProjectImage::load(&project.root);
    let import = jobs
        .start_import(&ctx(&project, &image), Vec::new())
        .unwrap();
    let setup = jobs
        .start_setup(&ctx(&project, &image), "2026-11-02T07-00-00", Vec::new())
        .unwrap();

    // A job created earlier sits above, also while both are queued.
    let ghosts = jobs.ghosts_of(&project.id);
    assert_eq!(
        ghosts
            .iter()
            .map(|g| (g.id.clone(), g.title.as_str()))
            .collect::<Vec<_>>(),
        [
            (format!("job:{import}"), "Import from Code"),
            (format!("job:{setup}"), "Setup")
        ]
    );
    assert!(ghosts.iter().all(|g| g.phase == GhostPhase::Preparing));

    state_of(&host, &project, &import, JobState::Waiting).await;
    state_of(&host, &project, &setup, JobState::Waiting).await;
    let ghosts = jobs.ghosts_of(&project.id);
    assert_eq!(ghosts.len(), 2);
    assert!(ghosts
        .iter()
        .all(|g| g.phase == GhostPhase::WaitingForAnswers));

    // An announcement made later comes after the ghost of a job created before it.
    let announced = jobs.announce(&import, "task", "Cancel an order").unwrap();
    assert_eq!(
        ghost_ids(&jobs, &project),
        [format!("job:{setup}"), announced]
    );

    // The other kinds of job get none.
    jobs.stop(&project.id, &import).unwrap();
    jobs.stop(&project.id, &setup).unwrap();
    state_of(&host, &project, &import, JobState::Stopped).await;
    state_of(&host, &project, &setup, JobState::Stopped).await;
    assert!(ghost_ids(&jobs, &project).is_empty());
    let task_run = run_task(&jobs, &project, "aaaaaaaa").unwrap();
    assert!(ghost_ids(&jobs, &project).is_empty(), "{task_run}");
    jobs.stop(&project.id, &task_run).unwrap();
}

#[tokio::test]
async fn after_a_restart_the_record_tells_which_announced_cards_arrived_and_the_own_ghost_comes_back(
) {
    let (host, first, _dir, project) = one_task().await;
    host.script(json!({ "prompts": [[{ "hold": true }]] }));
    let image = ProjectImage::load(&project.root);
    let id = first
        .start_setup(&ctx(&project, &image), "2026-11-02T07-00-00", Vec::new())
        .unwrap();
    state_of(&host, &project, &id, JobState::Running).await;
    let announced = first.announce(&id, "task", "Cancel an order").unwrap();
    assert_eq!(
        first.card_filed(Some(&id), "task", "bbbbbbbb", &announced),
        None
    );
    task_arrives(&host, &project, "bbbbbbbb", "Cancel an order").await;
    assert_eq!(ghost_ids(&first, &project), [format!("job:{id}")]);
    first.window_closed(&project.id).await;
    assert_eq!(
        row(&host, &project.id, &id).unwrap().state,
        JobState::Suspended
    );
    assert_eq!(
        ghost_ids(&first, &project),
        [format!("job:{id}")],
        "suspended, it keeps the ghost of Weaver"
    );

    // A new run of the app: only the registry knows that the card arrived.
    let second = Jobs::new(host.clone());
    host.script(json!({ "replay": [], "prompts": [[{ "hold": true }]] }));
    second.window_opened(&project.id, &project.root);
    until("the job to be resumed", LIMIT, || {
        second
            .record(&id)
            .is_some_and(|r| r.state == JobState::Running)
    })
    .await;
    assert_eq!(
        ghost_ids(&second, &project),
        [format!("job:{id}")],
        "no ghost of the card that already arrived"
    );
    second.stop(&project.id, &id).unwrap();
    until("the job to be stopped", LIMIT, || {
        second
            .record(&id)
            .is_some_and(|r| r.state == JobState::Stopped)
    })
    .await;
    assert!(ghost_ids(&second, &project).is_empty());
}

#[tokio::test]
async fn the_authority_accepts_only_a_working_job_of_the_project_and_runs_the_job_commands() {
    use weaver_cli::exec::{execute, Authority};
    use weaver_cli::spec::Caller;
    use weaver_core::command::CommandRequest;

    let (host, jobs, _dir, project) = one_task().await;
    host.script(json!({ "prompts": [[{ "wait_steer": 1 }]] }));
    let authority = super::authority::AppAuthority { jobs: jobs.clone() };
    let id = run_task(&jobs, &project, "aaaaaaaa").unwrap();
    assert!(
        !authority.accepts_job(&project.root, &id),
        "Queued is not working yet"
    );
    state_of(&host, &project, &id, JobState::Running).await;
    assert!(authority.accepts_job(&project.root, &id));
    assert!(
        !authority.accepts_job(std::path::Path::new("/elsewhere"), &id)
            && !authority.accepts_job(&project.root, "unknown1")
    );

    let request = |object: &str, verb: &str, input: Value| CommandRequest {
        object: object.into(),
        verb: verb.into(),
        root: project.root.to_string_lossy().into_owned(),
        card: None,
        state: None,
        input: input.as_object().unwrap().clone(),
        general: None,
        ask: None,
    };
    let caller = Caller {
        job: Some(id.clone()),
        ..Caller::default()
    };
    let edit = execute(
        &request(
            "job",
            "edit",
            json!({ "note": format!("{}x", "y".repeat(130)) }),
        ),
        &caller,
        &authority,
    )
    .unwrap();
    assert_eq!(edit.json["cut"], true);
    assert_eq!(edit.json["note"].as_str().unwrap().chars().count(), 120);
    let announced = execute(
        &request(
            "announcement",
            "new",
            json!({ "kind": "task", "title": "Cancel an order" }),
        ),
        &caller,
        &authority,
    )
    .unwrap();
    let announcement = announced.json["id"].as_str().unwrap().to_owned();
    // The card is filed with it: the reply carries no note, and the ghost waits for the picture.
    let filed = execute(
        &request(
            "task",
            "new",
            json!({ "title": "Cancel an order", "technical": true, "announcement": announcement }),
        ),
        &caller,
        &authority,
    )
    .unwrap();
    assert!(filed.note.is_none(), "{:?}", filed.note);
    let foreign = execute(
        &request(
            "task",
            "new",
            json!({ "title": "Another", "technical": true, "announcement": "zzzzzzzz" }),
        ),
        &caller,
        &authority,
    )
    .unwrap();
    assert!(foreign.note.unwrap().contains("without a ghost"));

    let list = execute(&request("job", "list", json!({})), &caller, &authority).unwrap();
    assert_eq!(list.json["jobs"][0]["id"], json!(id));
    let shown = execute(
        &CommandRequest {
            card: Some(id[..4].to_owned()),
            ..request("job", "show", json!({}))
        },
        &Caller::default(),
        &authority,
    )
    .unwrap();
    assert_eq!(shown.json["job"]["task"], "aaaaaaaa");
    // Not a job of this app, or a job after its end: forbidden.
    let stranger = Caller {
        job: Some("nobody01".into()),
        ..Caller::default()
    };
    assert_eq!(
        execute(
            &request("job", "edit", json!({ "note": "x" })),
            &stranger,
            &authority
        )
        .unwrap_err()
        .code,
        weaver_core::command::code::FORBIDDEN
    );
    // `job new` is for the working General only.
    assert_eq!(
        execute(
            &request("job", "new", json!({ "kind": "scan" })),
            &caller,
            &authority
        )
        .unwrap_err()
        .code,
        weaver_core::command::code::FORBIDDEN
    );

    jobs.send_message(&project.id, &id, "Wrap up").unwrap();
    state_of(&host, &project, &id, JobState::Done).await;
    assert_eq!(
        execute(
            &request("job", "edit", json!({ "note": "x" })),
            &caller,
            &authority
        )
        .unwrap_err()
        .code,
        weaver_core::command::code::FORBIDDEN
    );
}

#[tokio::test]
async fn the_general_starts_a_job_with_the_developers_refusals() {
    use weaver_cli::exec::execute;
    use weaver_cli::spec::Caller;
    use weaver_core::command::CommandRequest;

    struct Always;
    impl super::authority::GeneralAuthority for Always {
        fn accepts(&self, _root: &std::path::Path, token: &str) -> bool {
            token == "general-token"
        }
        fn accepts_ask(&self, _root: &std::path::Path, token: &str) -> bool {
            token == "ask-token"
        }
        fn chat_working(&self, _project_id: &str) -> bool {
            false
        }
        fn job_requested(
            &self,
            _root: &std::path::Path,
            _token: &str,
            _kind: &str,
            _outcome: Result<&[(String, String)], &str>,
        ) {
        }
    }
    let (host, jobs, _dir, project) = one_task().await;
    host.script(json!({ "prompts": [[{ "hold": true }]] }));
    jobs.set_general(Arc::new(Always));
    let authority = super::authority::AppAuthority { jobs: jobs.clone() };
    let request = |input: Value| CommandRequest {
        object: "job".into(),
        verb: "new".into(),
        root: project.root.to_string_lossy().into_owned(),
        card: None,
        state: None,
        input: input.as_object().unwrap().clone(),
        general: Some("general-token".into()),
        ask: None,
    };
    let general = Caller {
        general: Some("general-token".into()),
        ..Caller::default()
    };
    let started = execute(
        &request(json!({ "kind": "run", "task": "t1" })),
        &general,
        &authority,
    )
    .unwrap();
    let id = started.json["id"].as_str().unwrap().to_owned();
    assert_eq!(started.json["kind"], "run");
    assert_eq!(jobs.record(&id).unwrap().name, "Run T1");
    // The developer's refusal: another Run works.
    let refused = execute(&request(json!({ "kind": "run" })), &general, &authority).unwrap_err();
    assert_eq!(refused.code, weaver_core::command::code::REFUSED);
    assert!(
        refused.message.contains("A run is in progress"),
        "{}",
        refused.message
    );
    assert!(
        execute(&request(json!({ "kind": "scan" })), &general, &authority).is_ok(),
        "scans always start"
    );
    let wrong = Caller {
        general: Some("another".into()),
        ..Caller::default()
    };
    assert_eq!(
        execute(&request(json!({ "kind": "scan" })), &wrong, &authority)
            .unwrap_err()
            .code,
        weaver_core::command::code::FORBIDDEN
    );
    // A design request cannot be sent before the design requests install their sender.
    assert!(execute(
        &request(json!({ "kind": "design", "request": "2026-11-02T10-00-00" })),
        &general,
        &authority
    )
    .is_err());
}

#[tokio::test]
async fn a_finished_job_leaves_the_list_with_its_x_and_never_while_it_works() {
    let (host, jobs, _dir, project) = one_task().await;
    host.script(json!({ "prompts": [[{ "wait_steer": 1 }]] }));
    let id = run_task(&jobs, &project, "aaaaaaaa").unwrap();
    state_of(&host, &project, &id, JobState::Running).await;
    assert!(jobs.remove(&project.id, &id).is_err());
    jobs.send_message(&project.id, &id, "Wrap up").unwrap();
    state_of(&host, &project, &id, JobState::Done).await;
    assert!(jobs.snapshot(&project.id).jobs.iter().any(|r| r.id == id));
    jobs.remove(&project.id, &id).unwrap();
    assert!(jobs.snapshot(&project.id).jobs.is_empty());
    assert!(
        JobStore::new(&host.dirs).read(&id).unwrap().is_some(),
        "the registry keeps it"
    );
}

#[tokio::test]
async fn the_list_is_ordered_waiting_running_queued_then_the_finished_and_keeps_30_of_them() {
    let (host, jobs, _dir, project) = one_task().await;
    host.script(json!({ "prompts": [[{ "end": "end_turn" }]] }));
    let image = ProjectImage::load(&project.root);
    let mut finished = Vec::new();
    for _ in 0..32 {
        finished.push(jobs.start_scan(&ctx(&project, &image)).unwrap());
    }
    for id in &finished {
        until("a scan to be done", LIMIT, || {
            jobs.record(id).is_some_and(|r| r.state == JobState::Done)
        })
        .await;
    }
    let snapshot = jobs.snapshot(&project.id);
    assert_eq!(
        snapshot.jobs.len(),
        30,
        "only the 30 latest finished jobs are kept"
    );
    assert!(
        snapshot
            .jobs
            .windows(2)
            .all(|pair| pair[0].ended >= pair[1].ended),
        "latest end first"
    );
    assert!(snapshot.jobs.iter().all(|r| r.name == "Scan"));
}

const CERTIFICATE_NAMES: [&str; 3] = [
    "NODE_EXTRA_CA_CERTS",
    "SSL_CERT_FILE",
    "PI_REQUEST_COMPRESSION",
];

/// The setting `System Certificates` and the bundle file, as the Machine leaves them.
fn system_certificates(host: &TestHost, enabled: bool, bundle: bool) {
    host.settings.lock().unwrap().system_certificates.enabled = enabled;
    let file = host.dirs.system_certificates_file();
    if bundle {
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, "pem").unwrap();
    } else {
        let _ = std::fs::remove_file(&file);
    }
}

/// The script of an omp process that logs the certificate variables it received.
fn logging_script(host: &TestHost, prompts: Value) -> Value {
    json!({
        "log": host.received_log(),
        "logEnv": CERTIFICATE_NAMES,
        "prompts": prompts,
    })
}

/// The environment logged at each start of the agent, in order.
fn started_environments(host: &TestHost) -> Vec<Value> {
    host.received()
        .into_iter()
        .filter(|line| line.get("env").is_some())
        .collect()
}

#[tokio::test]
async fn a_job_starts_with_the_certificate_variables_only_with_the_setting_on_and_a_bundle() {
    for (enabled, bundle, expected) in [
        (true, true, true),
        (false, true, false),
        (true, false, false),
    ] {
        let (host, jobs, _dir, project) = one_task().await;
        system_certificates(&host, enabled, bundle);
        host.script(logging_script(&host, json!([[{ "end": "end_turn" }]])));
        let id = run_task(&jobs, &project, "aaaaaaaa").unwrap();
        state_of(&host, &project, &id, JobState::Done).await;

        let environments = started_environments(&host);
        assert_eq!(environments.len(), 1);
        let bundle_path = host.dirs.system_certificates_file();
        for name in CERTIFICATE_NAMES {
            let expected_value = match (expected, name) {
                (false, _) => Value::Null,
                (true, "PI_REQUEST_COMPRESSION") => json!("0"),
                (true, _) => json!(bundle_path.to_string_lossy()),
            };
            assert_eq!(
                environments[0]["env"][name], expected_value,
                "{name} {enabled} {bundle}"
            );
        }

        // The registry never holds them, with the setting on or off.
        jobs.flush();
        let record = JobStore::new(&host.dirs).read(&id).unwrap().unwrap();
        for name in CERTIFICATE_NAMES {
            assert!(!record.env.contains_key(name), "{name} in the registry");
        }
        assert_eq!(record.env["WEAVER_JOB"], id);
    }
}

#[tokio::test]
async fn a_resumed_job_gets_the_certificate_variables_of_that_moment_not_those_of_its_first_start()
{
    // Started with the setting on, resumed with it off: the process gets none.
    let (host, jobs, _dir, project) = one_task().await;
    system_certificates(&host, true, true);
    host.script(logging_script(&host, json!([[{ "hold": true }]])));
    let id = run_task(&jobs, &project, "aaaaaaaa").unwrap();
    state_of(&host, &project, &id, JobState::Running).await;
    assert!(started_environments(&host)[0]["env"]["SSL_CERT_FILE"].is_string());
    jobs.window_closed(&project.id).await;
    jobs.flush();
    let record = JobStore::new(&host.dirs).read(&id).unwrap().unwrap();
    assert!(CERTIFICATE_NAMES
        .iter()
        .all(|name| !record.env.contains_key(*name)));

    system_certificates(&host, false, true);
    host.script(logging_script(&host, json!([[{ "end": "end_turn" }]])));
    jobs.window_opened(&project.id, &project.root);
    state_of(&host, &project, &id, JobState::Done).await;
    let environments = started_environments(&host);
    assert_eq!(environments.len(), 2);
    assert!(CERTIFICATE_NAMES
        .iter()
        .all(|name| environments[1]["env"][name].is_null()));
    // The job id and the endpoint of the saved env are still there.
    assert_eq!(jobs.record(&id).unwrap().env["WEAVER_JOB"], id);

    // Started with the setting off, resumed with it on: the process gets them.
    let (host, jobs, _dir, project) = one_task().await;
    system_certificates(&host, false, true);
    host.script(logging_script(&host, json!([[{ "hold": true }]])));
    let id = run_task(&jobs, &project, "aaaaaaaa").unwrap();
    state_of(&host, &project, &id, JobState::Running).await;
    assert!(CERTIFICATE_NAMES
        .iter()
        .all(|name| started_environments(&host)[0]["env"][name].is_null()));
    jobs.window_closed(&project.id).await;

    system_certificates(&host, true, true);
    host.script(logging_script(&host, json!([[{ "end": "end_turn" }]])));
    jobs.window_opened(&project.id, &project.root);
    state_of(&host, &project, &id, JobState::Done).await;
    let environments = started_environments(&host);
    assert_eq!(environments.len(), 2);
    assert_eq!(environments[1]["env"]["PI_REQUEST_COMPRESSION"], "0");
    jobs.flush();
    let record = JobStore::new(&host.dirs).read(&id).unwrap().unwrap();
    assert!(CERTIFICATE_NAMES
        .iter()
        .all(|name| !record.env.contains_key(*name)));
}

/// A certificate error and an HTTP error, each with the setting on and off, and the explanation
/// each one must produce.
fn proxy_cases() -> Vec<(String, bool, String)> {
    let mut cases = Vec::new();
    for text in [
        "fetch failed: UNABLE_TO_GET_ISSUER_CERT_LOCALLY",
        "request failed: HTTP 407",
    ] {
        for on in [false, true] {
            let expected = weaver_core::omp::failure::explain(text, on).unwrap();
            assert_eq!(expected.contains("turning on System Certificates"), !on);
            cases.push((text.to_owned(), on, expected));
        }
    }
    cases
}

fn set_system_certificates(host: &TestHost, on: bool) {
    host.settings.lock().unwrap().system_certificates.enabled = on;
}

async fn failed_reason(host: &TestHost, project: &Project, id: &str) -> Option<String> {
    state_of(host, project, id, JobState::Failed).await;
    row(host, &project.id, id).unwrap().reason
}

#[tokio::test]
async fn an_exit_with_a_proxy_error_in_the_stderr_tail_is_explained_and_its_details_stay_omps_lines(
) {
    for (text, on, expected) in proxy_cases() {
        let (host, jobs, _dir, project) = one_task().await;
        set_system_certificates(&host, on);
        // The error is not on the last line: the whole tail is tested.
        host.script(json!({ "prompts": [[agent("Working"), { "stderr": text }, { "stderr": "error: giving up" }, { "sleep": 100 }, { "exit": 3 }]] }));
        let id = run_task(&jobs, &project, "aaaaaaaa").unwrap();
        assert_eq!(
            failed_reason(&host, &project, &id).await,
            Some(expected),
            "{text} {on}"
        );
        let conversation = entries(&jobs, &project, &id);
        let Some(Entry::End(EndLine::Failed { details, .. })) = conversation.last() else {
            panic!("{conversation:?}")
        };
        assert_eq!(
            details.as_deref(),
            Some(format!("{text}\nerror: giving up").as_str())
        );
    }
}

#[tokio::test]
async fn an_opening_a_start_and_a_turn_that_fail_with_a_proxy_error_are_explained() {
    for (text, on, expected) in proxy_cases() {
        // `session/new` fails.
        let (host, jobs, _dir, project) = one_task().await;
        set_system_certificates(&host, on);
        host.script(json!({ "newError": text }));
        let id = run_task(&jobs, &project, "aaaaaaaa").unwrap();
        assert_eq!(
            failed_reason(&host, &project, &id).await.as_deref(),
            Some(expected.as_str()),
            "opening {text} {on}"
        );

        // The reply to the first `session/prompt` is an error.
        let (host, jobs, _dir, project) = one_task().await;
        set_system_certificates(&host, on);
        host.script(json!({ "prompts": [[{ "error": text }]] }));
        let id = run_task(&jobs, &project, "aaaaaaaa").unwrap();
        assert_eq!(
            failed_reason(&host, &project, &id).await.as_deref(),
            Some(expected.as_str()),
            "start {text} {on}"
        );

        // A later turn fails.
        let (host, jobs, _dir, project) = one_task().await;
        set_system_certificates(&host, on);
        host.script(json!({ "prompts": [[agent("Working"), { "error": text }]] }));
        let id = run_task(&jobs, &project, "aaaaaaaa").unwrap();
        assert_eq!(
            failed_reason(&host, &project, &id).await.as_deref(),
            Some(expected.as_str()),
            "turn {text} {on}"
        );
        // The reason kept is the explanation: a later change of the setting rewrites nothing.
        set_system_certificates(&host, !on);
        assert_eq!(
            jobs.record(&id).unwrap().reason.as_deref(),
            Some(expected.as_str())
        );
    }
}

#[tokio::test]
async fn a_resume_that_fails_with_a_proxy_error_is_explained_else_it_stays_not_resumed() {
    for (text, on, expected) in proxy_cases() {
        let (host, jobs, _dir, project) = one_task().await;
        host.script(json!({ "prompts": [[{ "hold": true }]] }));
        let id = run_task(&jobs, &project, "aaaaaaaa").unwrap();
        state_of(&host, &project, &id, JobState::Running).await;
        jobs.window_closed(&project.id).await;
        set_system_certificates(&host, on);
        host.script(json!({ "loadError": text }));
        jobs.window_opened(&project.id, &project.root);
        assert_eq!(
            failed_reason(&host, &project, &id).await.as_deref(),
            Some(expected.as_str()),
            "{text} {on}"
        );
    }
}

#[tokio::test]
async fn a_refused_message_with_a_proxy_error_shows_the_explanation() {
    for (text, on, expected) in proxy_cases() {
        let (host, jobs, _dir, project) = one_task().await;
        set_system_certificates(&host, on);
        host.script(json!({ "steering": [format!("failed:{text}")], "prompts": [[{ "wait_steer": 1 }, agent("Still here")]] }));
        let id = run_task(&jobs, &project, "aaaaaaaa").unwrap();
        state_of(&host, &project, &id, JobState::Running).await;
        jobs.send_message(&project.id, &id, "Hello").unwrap();
        state_of(&host, &project, &id, JobState::Done).await;
        assert!(
            entries(&jobs, &project, &id).iter().any(|e| matches!(e, Entry::Developer { delivery: Delivery::NotDelivered { reason }, .. } if *reason == expected)),
            "{text} {on}"
        );
    }
}

#[tokio::test]
async fn errors_that_are_not_certificate_or_http_refusals_keep_their_reasons() {
    for text in ["status 404", "HTTP 429 Too Many Requests", "quota exceeded"] {
        let (host, jobs, _dir, project) = one_task().await;
        set_system_certificates(&host, false);
        host.script(json!({ "prompts": [[{ "error": text }]] }));
        let id = run_task(&jobs, &project, "aaaaaaaa").unwrap();
        assert_eq!(
            failed_reason(&host, &project, &id).await.as_deref(),
            Some(text)
        );
    }
    // A fixed reason stays when nothing in the error or in stderr is a proxy problem.
    let (host, jobs, _dir, project) = one_task().await;
    host.script(json!({ "prompts": [[{ "stderr": "retrying" }, { "end": "refusal" }]] }));
    let id = run_task(&jobs, &project, "aaaaaaaa").unwrap();
    assert_eq!(
        failed_reason(&host, &project, &id).await.as_deref(),
        Some("the agent refused")
    );
}

// ---------------------------------------------------------------------------------------------
// Sub-agents
// ---------------------------------------------------------------------------------------------

/// The main session file of the fake agent's session under the sessions folder of the test host,
/// and the folder of its sub-agents' transcripts.
fn session_files(host: &TestHost) -> (PathBuf, PathBuf) {
    let folder = host
        .dirs
        .local_data_dir()
        .join("omp-sessions")
        .join("--project--");
    std::fs::create_dir_all(&folder).unwrap();
    let main = folder.join("2026-11-02T07-00-00-000Z_fake-session.jsonl");
    std::fs::write(&main, "{}\n").unwrap();
    let agents = folder.join("2026-11-02T07-00-00-000Z_fake-session");
    (main, agents)
}

fn record_line(record: Value) -> String {
    format!("{record}\n")
}

fn user_line(text: &str) -> String {
    record_line(
        json!({ "type": "message", "message": { "role": "user", "content": [{ "type": "text", "text": text }] } }),
    )
}

fn assistant_line(text: &str) -> String {
    record_line(
        json!({ "type": "message", "message": { "role": "assistant", "content": [{ "type": "text", "text": text }] } }),
    )
}

fn exit_line(kind: &str) -> String {
    record_line(json!({ "type": "custom", "customType": "session_exit", "data": { "kind": kind } }))
}

/// A step of the fake agent that appends `text` to the transcript `file`.
fn write_step(file: &Path, text: &str) -> Value {
    json!({ "write": { "path": file.to_string_lossy(), "text": text } })
}

fn progress_step(call: &str, status: &str, progress: Value) -> Value {
    json!({ "update": { "sessionUpdate": "tool_call_update", "toolCallId": call, "status": status, "rawOutput": { "details": { "progress": progress } } } })
}

fn step(call: &str, id: &str) -> Step {
    Step {
        call: call.into(),
        id: id.into(),
    }
}

fn states(jobs: &Jobs, project: &Project, job: &str) -> Vec<(String, SubAgentState)> {
    jobs.sub_agents(&project.id, job)
        .unwrap()
        .nodes
        .iter()
        .map(|n| (n.id.clone(), n.state))
        .collect()
}

fn state_of_agent(jobs: &Jobs, project: &Project, job: &str, agent: &str) -> Option<SubAgentState> {
    states(jobs, project, job)
        .into_iter()
        .find(|(id, _)| id == agent)
        .map(|(_, state)| state)
}

#[tokio::test]
async fn the_sub_agents_of_a_job_are_listed_with_their_states_and_their_transcripts_read_while_shown(
) {
    let (host, jobs, _dir, project) = one_task().await;
    let (_main, agents) = session_files(&host);
    let (gate_one, gate_two) = (
        host.dirs.local_data_dir().join("gate-one"),
        host.dirs.local_data_dir().join("gate-two"),
    );
    let b_line = assistant_line("Working on B.");
    let (b_first, b_second) = b_line.split_at(15);
    host.script(json!({
        "prompts": [[
            { "update": { "sessionUpdate": "tool_call", "toolCallId": "t1", "title": "Task", "status": "in_progress" } },
            progress_step("t1", "in_progress", json!([
                { "id": "0-A", "name": "A", "status": "running" },
                { "id": "1-B", "name": "B", "status": "running" },
                { "id": "2-C", "name": "C", "status": "running" },
                { "id": "__advisor.turn", "status": "running" },
            ])),
            write_step(&agents.join("0-A.jsonl"), &(user_line("Read the code.") + &assistant_line("Done reading.") + &exit_line("normal"))),
            write_step(&agents.join("1-B.jsonl"), &(user_line("Write the test.") + b_first)),
            write_step(&agents.join("__advisor.turn.jsonl"), &user_line("advise")),
            // The call returns while they still work: they run in the background.
            progress_step("t1", "completed", json!([
                { "id": "0-A", "name": "A", "status": "pending" },
                { "id": "1-B", "name": "B", "status": "pending" },
                { "id": "2-C", "name": "C", "status": "pending" },
            ])),
            { "wait_file": gate_one },
            write_step(&agents.join("1-B.jsonl"), b_second),
            { "update": { "sessionUpdate": "tool_call", "toolCallId": "w1", "title": "Wait", "status": "completed",
                "rawOutput": { "details": { "jobs": [{ "id": "1-B", "status": "failed" }] } } } },
            { "wait_file": gate_two },
        ]],
    }));
    let id = run_task(&jobs, &project, "aaaaaaaa").unwrap();
    until("the three sub-agents to be listed", LIMIT, || {
        states(&jobs, &project, &id).len() == 3
    })
    .await;

    // The list is on screen and A's conversation is shown: its transcript is read, and its exit
    // record (the fallback) settles it while the task call says it still works.
    let path_a = vec![step("t1", "0-A")];
    jobs.sub_agents_view(&project.id, &id, true, Some(path_a.clone()))
        .unwrap();
    until("A to be read", LIMIT, || {
        state_of_agent(&jobs, &project, &id, "0-A") == Some(SubAgentState::Succeeded)
    })
    .await;
    let a = jobs
        .sub_agent_conversation(&project.id, &id, &path_a)
        .unwrap();
    assert_eq!(a.unavailable, None);
    assert_eq!(
        a.entries,
        [
            Entry::Assignment {
                from: "Run T1".into(),
                text: "Read the code.".into()
            },
            Entry::Agent {
                text: "Done reading.".into(),
                complete: true
            },
        ]
    );
    assert_eq!(
        state_of_agent(&jobs, &project, &id, "1-B"),
        Some(SubAgentState::Working)
    );

    // B's half-written line waits; C has no transcript yet, which is only its conversation's problem.
    let path_b = vec![step("t1", "1-B")];
    jobs.sub_agents_view(&project.id, &id, true, Some(path_b.clone()))
        .unwrap();
    until("B's first record", LIMIT, || {
        jobs.sub_agent_conversation(&project.id, &id, &path_b)
            .is_ok_and(|c| c.entries.len() == 1)
    })
    .await;
    let c = jobs
        .sub_agent_conversation(&project.id, &id, &[step("t1", "2-C")])
        .unwrap();
    assert!(c.entries.is_empty());
    until("C's transcript to be reported missing", LIMIT, || {
        jobs.sub_agent_conversation(&project.id, &id, &[step("t1", "2-C")])
            .is_ok_and(|c| c.unavailable.as_deref() == Some("the transcript file was not found"))
    })
    .await;
    assert_eq!(
        state_of_agent(&jobs, &project, &id, "2-C"),
        Some(SubAgentState::Working),
        "it keeps its state from the other sources"
    );
    assert!(jobs
        .sub_agent_conversation(&project.id, &id, &[step("t1", "__advisor.turn")])
        .is_err());

    // The rest of B's line, then a `wait` that says it failed: the change reaches the UI.
    std::fs::write(&gate_one, "").unwrap();
    until("B to fail and its message to arrive", LIMIT, || {
        state_of_agent(&jobs, &project, &id, "1-B") == Some(SubAgentState::Failed)
            && host.agent_entries.lock().unwrap().iter().any(|event| {
                event.path == path_b
                    && event.changes.iter().any(|change| {
                        matches!(&change.entry, Entry::Agent { text, .. } if text == "Working on B.")
                    })
            })
    })
    .await;
    assert!(host
        .lists
        .lock()
        .unwrap()
        .last()
        .is_some_and(|list| list.nodes.len() == 3));

    // The job ends with C still working: C is interrupted, in the list and under the task row.
    std::fs::write(&gate_two, "").unwrap();
    state_of(&host, &project, &id, JobState::Done).await;
    assert_eq!(
        states(&jobs, &project, &id),
        [
            ("0-A".to_owned(), SubAgentState::Succeeded),
            ("1-B".to_owned(), SubAgentState::Failed),
            ("2-C".to_owned(), SubAgentState::Interrupted),
        ]
    );
    let row_states: Vec<SubAgentState> = entries(&jobs, &project, &id)
        .iter()
        .filter_map(|entry| match entry {
            Entry::Tool(tool) if tool.id == "t1" => Some(tool.sub_agents.iter().map(|a| a.state)),
            _ => None,
        })
        .flatten()
        .collect();
    assert_eq!(
        row_states,
        [
            SubAgentState::Succeeded,
            SubAgentState::Failed,
            SubAgentState::Interrupted
        ]
    );
}

#[tokio::test]
async fn the_sub_agents_are_rebuilt_from_the_replayed_history_and_read_again_after_a_reload() {
    let (host, first) = setup();
    let (_dir, project) = host
        .project(&[("tasks/aaaaaaaa-first-task.md", TASK)])
        .await;
    host.script(json!({ "prompts": [[{ "hold": true }]] }));
    let id = run_task(&first, &project, "aaaaaaaa").unwrap();
    state_of(&host, &project, &id, JobState::Running).await;
    first.flush();
    let store = JobStore::new(&host.dirs);
    let mut record = store.read(&id).unwrap().unwrap();
    record.pid = None;
    record.pid_started = None;
    record.id = "crashed1".to_owned();
    record
        .env
        .insert("WEAVER_JOB".to_owned(), "crashed1".to_owned());
    store.write(&record).unwrap();
    first.stop(&project.id, &id).unwrap();
    state_of(&host, &project, &id, JobState::Stopped).await;
    first.flush();

    let (_main, agents) = session_files(&host);
    std::fs::create_dir_all(&agents).unwrap();
    std::fs::write(
        agents.join("0-A.jsonl"),
        user_line("Read the code.") + &assistant_line("Read it.") + &exit_line("normal"),
    )
    .unwrap();

    let second = Jobs::new(host.clone());
    host.script(json!({
        "replay": [
            text_update("user_message_chunk", "Use the weaver-implement skill on Task aaaaaaaa."),
            { "sessionUpdate": "tool_call", "toolCallId": "t1", "title": "Task", "status": "completed",
              "rawOutput": { "details": { "progress": [{ "id": "0-A", "name": "A", "status": "completed" }] } } },
        ],
        "prompts": [[agent("Back at work")]],
    }));
    second.window_opened(&project.id, &project.root);
    until("the crashed job to finish", LIMIT, || {
        second
            .record("crashed1")
            .is_some_and(|r| r.state == JobState::Done)
    })
    .await;
    assert_eq!(
        states(&second, &project, "crashed1"),
        [("0-A".to_owned(), SubAgentState::Succeeded)],
        "rebuilt from the history, not from the live updates"
    );
    let path = vec![step("t1", "0-A")];
    second
        .sub_agents_view(&project.id, "crashed1", true, Some(path.clone()))
        .unwrap();
    until("the transcript to be read from the start", LIMIT, || {
        second
            .sub_agent_conversation(&project.id, "crashed1", &path)
            .is_ok_and(|c| c.entries.len() == 2)
    })
    .await;
}
