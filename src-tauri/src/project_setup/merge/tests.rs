//! Merge in the app, against real git repositories in temporary folders (`requirements/quality#solo-merge`).

use std::fs;
use std::path::{Path, PathBuf};

use weaver_core::project_files::initialise;
use weaver_core::solo_mode::Mode;

use super::super::tests::{open, setup, until, FakeEnvironment, ID};
use super::*;
use crate::project_setup::ProjectStatus;
use crate::windows::project::ProjectHooks;

fn git(dir: &Path, args: &[&str]) -> String {
    let null = if cfg!(windows) { "NUL" } else { "/dev/null" };
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", null)
        .env("GIT_CONFIG_SYSTEM", null)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn save(root: &Path, rel: &str, content: &str, message: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
    git(root, &["add", "-f", "--", rel]);
    git(root, &["commit", "-q", "-m", message]);
}

/// A repository on `main`, with a project in it in Solo mode `mode`. `core.autocrlf` is off in its
/// own config, which also binds Weaver's git, so that a Windows git keeps the LF files of the tests.
fn repository(dir: &Path, mode: Mode) -> PathBuf {
    let root = weaver_core::paths::canonical_root(dir).unwrap();
    git(&root, &["init", "-q"]);
    git(&root, &["symbolic-ref", "HEAD", "refs/heads/main"]);
    for (key, value) in [
        ("user.name", "Weaver Test"),
        ("user.email", "test@example.com"),
        ("commit.gpgsign", "false"),
        ("merge.ff", "true"),
        ("core.autocrlf", "false"),
    ] {
        git(&root, &["config", key, value]);
    }
    save(&root, "a.txt", "base\n", "base");
    initialise::initialise(&root, false, mode, &Git::system()).unwrap();
    root
}

/// `dev` brings `b.txt` and `main` its own `c.txt`: they merge without conflicts.
fn diverged(root: &Path) {
    git(root, &["checkout", "-q", "-b", "dev"]);
    save(root, "b.txt", "dev\n", "dev work");
    git(root, &["checkout", "-q", "main"]);
    save(root, "c.txt", "main\n", "main work");
}

/// `dev` and `main` change `a.txt` differently, and `dev` adds `d.txt`.
fn conflicting(root: &Path) {
    git(root, &["checkout", "-q", "-b", "dev"]);
    save(root, "a.txt", "dev\n", "dev edits a");
    save(root, "d.txt", "dev only\n", "dev adds d");
    git(root, &["checkout", "-q", "main"]);
    save(root, "a.txt", "main\n", "main edits a");
}

fn resolve_and_commit(root: &Path) {
    fs::write(root.join("a.txt"), "resolved\n").unwrap();
    git(root, &["add", "a.txt", "d.txt"]);
    git(root, &["commit", "-q", "--no-edit"]);
}

/// A window opened on `root`, with the opening run done.
async fn opened(root: &Path) -> (Arc<FakeEnvironment>, Arc<ProjectSetup>) {
    let env = FakeEnvironment::new(root);
    let setup = setup(&env);
    open(&setup, root);
    until(&env, |all| {
        all.last().is_some_and(|status| status.merge.is_some())
    })
    .await;
    (env, setup)
}

fn merge_of(status: &ProjectStatus) -> &MergeStatus {
    status.merge.as_ref().expect("a merge status")
}

#[tokio::test]
async fn a_project_without_solo_mode_has_no_merge_and_with_it_merge_is_available() {
    let dir = tempfile::tempdir().unwrap();
    let root = repository(dir.path(), Mode::Off);
    let env = FakeEnvironment::new(&root);
    let setup = setup(&env);
    open(&setup, &root);
    until(&env, |all| !all.is_empty()).await;
    assert_eq!(setup.merge_refresh(ID).await.unwrap(), None);
    assert!(env.last().unwrap().merge.is_none());
    let json = serde_json::to_value(env.last().unwrap()).unwrap();
    assert!(json.get("merge").is_none(), "absent, not null");

    // Turning Solo mode on brings Merge.
    setup.set_solo_mode(ID, Mode::Folders).await.unwrap();
    until(&env, |all| {
        all.last().is_some_and(|status| status.merge.is_some())
    })
    .await;
    let status = env.last().unwrap();
    assert_eq!(
        merge_of(&status),
        &MergeStatus {
            available: true,
            reason: None,
            current: Some("main".to_owned()),
            in_progress: None
        }
    );
}

#[tokio::test]
async fn the_status_says_why_merge_is_unavailable_with_the_sentence_the_board_shows() {
    let dir = tempfile::tempdir().unwrap();
    let root = repository(dir.path(), Mode::Folders);
    diverged(&root);
    let (env, setup) = opened(&root).await;
    assert!(merge_of(&env.last().unwrap()).available);

    *env.work.lock().unwrap() = AgentWork {
        busy: true,
        merge_job_unended: false,
    };
    let status = setup.merge_refresh(ID).await.unwrap().unwrap();
    assert!(!status.available);
    assert_eq!(status.reason.as_deref(), Some("The agent is working"));
    // The window is told, with the revision that follows.
    assert_eq!(
        merge_of(&env.last().unwrap()).reason.as_deref(),
        Some("The agent is working")
    );

    *env.work.lock().unwrap() = AgentWork::default();
    fs::write(root.join("a.txt"), "changed\n").unwrap();
    let status = setup.merge_refresh(ID).await.unwrap().unwrap();
    assert_eq!(
        status.reason.as_deref(),
        Some("Commit or stash your changes first")
    );
}

#[tokio::test]
async fn the_dialog_lists_the_branches_and_the_tracked_files_of_weaver() {
    let dir = tempfile::tempdir().unwrap();
    let root = repository(dir.path(), Mode::Folders);
    diverged(&root);
    git(&root, &["branch", "alpha"]);
    save(&root, ".weaver/model/m.md", "x\n", "a weaver file in git");
    let (_, setup) = opened(&root).await;

    let found = setup.merge_branches(ID).await.unwrap();
    assert_eq!(found.current, "main");
    assert_eq!(found.local, ["alpha", "dev"]);
    assert!(found.remote.is_empty());
    assert_eq!(found.tracked, [".weaver/model/m.md"]);
}

#[tokio::test]
async fn conflicts_start_a_merge_that_the_board_follows_and_the_job_for_them_can_start() {
    let dir = tempfile::tempdir().unwrap();
    let root = repository(dir.path(), Mode::Folders);
    conflicting(&root);
    let (env, setup) = opened(&root).await;
    let boards = *env.boards.lock().unwrap();

    let answer = setup.merge_start(ID, "dev".to_owned()).await.unwrap();
    assert_eq!(answer.notice, None);
    assert!(!answer.merge.available);
    assert_eq!(
        answer.merge.reason.as_deref(),
        Some("A merge is in progress")
    );
    assert_eq!(
        answer.merge.in_progress,
        Some(MergeProgress {
            branch: "dev".to_owned(),
            current: "main".to_owned(),
            conflicts: 1
        })
    );
    assert!(solo_merge::in_progress(&root));
    // The state is announced with `project-state`, and the Board recomputes what it shows.
    assert_eq!(env.last().unwrap().merge, Some(answer.merge.clone()));
    assert!(*env.boards.lock().unwrap() > boards);
    assert!(env.scans.lock().unwrap().is_empty());

    // `Resolve with the agent` starts the merge job with the count of the moment.
    let status = setup.resolve_merge_conflicts(ID).await.unwrap();
    assert_eq!(*env.merge_jobs.lock().unwrap(), [1]);
    assert_eq!(
        status.reason.as_deref(),
        Some("The agent is working"),
        "the job is working now"
    );
    // A second one is the jobs' to refuse.
    let refused = setup.resolve_merge_conflicts(ID).await.unwrap_err();
    assert_eq!(refused.code, ErrorCode::Refused);
    assert_eq!(
        refused.message,
        "Could not resolve the conflicts: a merge conflicts job has not ended."
    );
}

#[tokio::test]
async fn a_merge_without_conflicts_is_concluded_at_once_and_the_scan_starts_with_its_files() {
    let dir = tempfile::tempdir().unwrap();
    let root = repository(dir.path(), Mode::Folders);
    diverged(&root);
    let (env, setup) = opened(&root).await;

    let answer = setup.merge_start(ID, "dev".to_owned()).await.unwrap();
    assert_eq!(answer.notice, None);
    assert_eq!(*env.scans.lock().unwrap(), [vec!["b.txt".to_owned()]]);
    assert!(!solo_merge::in_progress(&root), "merge.json is gone");
    assert_eq!(answer.merge.in_progress, None);
    // The scan is working: Merge says so.
    assert_eq!(answer.merge.reason.as_deref(), Some("The agent is working"));
    assert!(env.notices.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_scan_that_the_jobs_refuse_after_a_merge_is_a_notice() {
    let dir = tempfile::tempdir().unwrap();
    let root = repository(dir.path(), Mode::Folders);
    diverged(&root);
    let (env, setup) = opened(&root).await;
    *env.scan_refusal.lock().unwrap() = Some("omp is not ready".to_owned());

    setup.merge_start(ID, "dev".to_owned()).await.unwrap();
    assert!(env.scans.lock().unwrap().is_empty());
    assert_eq!(
        *env.notices.lock().unwrap(),
        ["Could not scan the System after the merge of dev: omp is not ready."]
    );
}

#[tokio::test]
async fn nothing_to_merge_and_every_refusal_come_back_as_a_notice_with_nothing_changed() {
    let dir = tempfile::tempdir().unwrap();
    let root = repository(dir.path(), Mode::Folders);
    git(&root, &["branch", "same"]);
    diverged(&root);
    let (env, setup) = opened(&root).await;

    let answer = setup.merge_start(ID, "same".to_owned()).await.unwrap();
    assert_eq!(
        answer.notice,
        Some(MergeNotice::text("main is already up to date with same."))
    );
    assert!(answer.merge.available);

    let answer = setup.merge_start(ID, "missing".to_owned()).await.unwrap();
    let notice = answer.notice.unwrap();
    assert!(
        notice.text.starts_with("Could not merge missing: "),
        "{}",
        notice.text
    );
    assert!(notice.text.ends_with('.') && !notice.text.ends_with(".."));

    fs::write(root.join("a.txt"), "changed\n").unwrap();
    let answer = setup.merge_start(ID, "dev".to_owned()).await.unwrap();
    assert_eq!(
        answer.notice,
        Some(MergeNotice::text(
            "Could not merge dev: commit or stash your changes first."
        ))
    );
    assert!(!solo_merge::in_progress(&root));
    assert!(env.scans.lock().unwrap().is_empty());
}

#[tokio::test]
async fn files_solo_mode_keeps_out_of_git_stop_the_merge_and_are_named_in_the_notice() {
    let dir = tempfile::tempdir().unwrap();
    let root = repository(dir.path(), Mode::Folders);
    git(&root, &["checkout", "-q", "-b", "dev"]);
    save(
        &root,
        ".weaver/model/new.md",
        "dev\n",
        "dev tracks a weaver file",
    );
    git(&root, &["checkout", "-q", "main"]);
    save(&root, "c.txt", "main\n", "main work");
    fs::create_dir_all(root.join(".weaver/model")).unwrap();
    fs::write(root.join(".weaver/model/new.md"), "mine\n").unwrap();
    let (_, setup) = opened(&root).await;

    let answer = setup.merge_start(ID, "dev".to_owned()).await.unwrap();
    assert_eq!(
        answer.notice,
        Some(MergeNotice {
            text: "Could not merge dev: it would overwrite files that Solo mode keeps out of git."
                .to_owned(),
            paths: Some(vec![".weaver/model/new.md".to_owned()])
        })
    );
    assert_eq!(
        fs::read_to_string(root.join(".weaver/model/new.md")).unwrap(),
        "mine\n"
    );
}

#[tokio::test]
async fn a_merge_without_solo_mode_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let root = repository(dir.path(), Mode::Off);
    diverged(&root);
    let (_, setup) = {
        let env = FakeEnvironment::new(&root);
        let setup = setup(&env);
        open(&setup, &root);
        until(&env, |all| !all.is_empty()).await;
        (env, setup)
    };
    let refused = setup.merge_start(ID, "dev".to_owned()).await.unwrap_err();
    assert_eq!(refused.code, ErrorCode::Refused);
    assert!(refused.message.contains("only while Solo mode is on"));
    assert!(setup.merge_abort(ID).await.is_err());
    assert!(setup.resolve_merge_conflicts(ID).await.is_err());
    assert!(!root.join(".weaver/merge").exists());
}

#[tokio::test]
async fn an_abort_is_refused_while_the_merge_job_has_not_ended_and_then_forgets_the_merge() {
    let dir = tempfile::tempdir().unwrap();
    let root = repository(dir.path(), Mode::Folders);
    conflicting(&root);
    let (env, setup) = opened(&root).await;
    setup.merge_start(ID, "dev".to_owned()).await.unwrap();
    let start = git(&root, &["rev-parse", "HEAD"]);

    *env.work.lock().unwrap() = AgentWork {
        busy: true,
        merge_job_unended: true,
    };
    let answer = setup.merge_abort(ID).await.unwrap();
    assert_eq!(
        answer.notice,
        Some(MergeNotice::text(
            "Could not abort the merge: the agent is resolving the conflicts."
        ))
    );
    assert!(answer.merge.in_progress.is_some());
    assert!(solo_merge::in_progress(&root));

    *env.work.lock().unwrap() = AgentWork::default();
    let boards = *env.boards.lock().unwrap();
    let answer = setup.merge_abort(ID).await.unwrap();
    assert_eq!(answer.notice, None);
    assert_eq!(answer.merge.in_progress, None);
    assert!(answer.merge.available);
    assert!(!solo_merge::in_progress(&root));
    assert_eq!(git(&root, &["rev-parse", "HEAD"]), start);
    assert!(*env.boards.lock().unwrap() > boards);
    assert!(
        env.scans.lock().unwrap().is_empty(),
        "an abort scans nothing"
    );
    assert_eq!(env.last().unwrap().merge, Some(answer.merge));
}

#[tokio::test]
async fn a_failed_abort_leaves_the_merge_and_says_gits_first_line() {
    let dir = tempfile::tempdir().unwrap();
    let root = repository(dir.path(), Mode::Folders);
    conflicting(&root);
    let (_, setup) = opened(&root).await;
    setup.merge_start(ID, "dev".to_owned()).await.unwrap();
    // Committed outside Weaver, not checked yet: git has no merge to abort.
    resolve_and_commit(&root);

    let answer = setup.merge_abort(ID).await.unwrap();
    let notice = answer.notice.unwrap();
    assert!(
        notice.text.starts_with("Could not abort the merge: ")
            && notice.text.contains("no merge to abort"),
        "{}",
        notice.text
    );
}

#[tokio::test]
async fn a_merge_committed_outside_weaver_is_concluded_when_the_window_comes_back() {
    let dir = tempfile::tempdir().unwrap();
    let root = repository(dir.path(), Mode::Folders);
    conflicting(&root);
    let (env, setup) = opened(&root).await;
    setup.merge_start(ID, "dev".to_owned()).await.unwrap();
    assert!(env.scans.lock().unwrap().is_empty());

    // The developer resolves the conflicts and commits in git, then comes back to Weaver.
    resolve_and_commit(&root);
    setup.window_focused(ID);
    until(&env, |all| {
        all.last()
            .is_some_and(|status| merge_of(status).in_progress.is_none())
    })
    .await;
    assert_eq!(
        *env.scans.lock().unwrap(),
        [vec!["a.txt".to_owned(), "d.txt".to_owned()]]
    );
    assert!(!solo_merge::in_progress(&root));
    assert!(env.notices.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_merge_aborted_outside_weaver_is_forgotten_with_a_notice_and_no_scan() {
    let dir = tempfile::tempdir().unwrap();
    let root = repository(dir.path(), Mode::Folders);
    conflicting(&root);
    let (env, setup) = opened(&root).await;
    setup.merge_start(ID, "dev".to_owned()).await.unwrap();

    git(&root, &["merge", "--abort"]);
    // A job of the project ended.
    setup.merge_may_have_changed(ID);
    until(&env, |all| {
        all.last()
            .is_some_and(|status| merge_of(status).in_progress.is_none())
    })
    .await;
    assert!(env.scans.lock().unwrap().is_empty());
    assert_eq!(
        *env.notices.lock().unwrap(),
        ["The merge of dev ended without a commit: Weaver forgot it and did not scan."]
    );
    assert!(!solo_merge::in_progress(&root));
}

#[tokio::test]
async fn a_merge_still_in_progress_updates_its_count_and_survives_a_new_opening() {
    let dir = tempfile::tempdir().unwrap();
    let root = repository(dir.path(), Mode::Folders);
    conflicting(&root);
    {
        let (_, setup) = opened(&root).await;
        setup.merge_start(ID, "dev".to_owned()).await.unwrap();
    }

    // Weaver was closed and opened again: the merge is found, with its count.
    let (env, setup) = opened(&root).await;
    assert_eq!(
        merge_of(&env.last().unwrap()).in_progress,
        Some(MergeProgress {
            branch: "dev".to_owned(),
            current: "main".to_owned(),
            conflicts: 1
        })
    );

    // The developer resolves the file by hand: the count follows when the window comes back.
    fs::write(root.join("a.txt"), "resolved\n").unwrap();
    git(&root, &["add", "a.txt"]);
    setup.window_focused(ID);
    until(&env, |all| {
        all.last().is_some_and(|status| {
            merge_of(status)
                .in_progress
                .as_ref()
                .is_some_and(|progress| progress.conflicts == 0)
        })
    })
    .await;
    assert!(env.scans.lock().unwrap().is_empty());
}

#[tokio::test]
async fn solo_mode_cannot_change_while_the_merge_is_in_progress_and_merge_goes_with_it_when_off() {
    let dir = tempfile::tempdir().unwrap();
    let root = repository(dir.path(), Mode::Folders);
    conflicting(&root);
    let (env, setup) = opened(&root).await;
    setup.merge_start(ID, "dev".to_owned()).await.unwrap();
    assert!(env.last().unwrap().merge.is_some());

    let refused = setup.set_solo_mode(ID, Mode::Off).await.unwrap_err();
    assert_eq!(
        refused.message,
        "Finish or abort the merge in progress first."
    );
    assert!(setup.set_solo_mode(ID, Mode::Files).await.is_err());
    assert!(solo_merge::in_progress(&root));

    setup.merge_abort(ID).await.unwrap();
    setup.set_solo_mode(ID, Mode::Off).await.unwrap();
    until(&env, |all| {
        all.last().is_some_and(|status| status.merge.is_none())
    })
    .await;
}

#[tokio::test]
async fn the_memory_of_the_merge_goes_with_the_weaver_folder_and_nothing_is_scanned() {
    let dir = tempfile::tempdir().unwrap();
    let root = repository(dir.path(), Mode::Folders);
    conflicting(&root);
    let (env, setup) = opened(&root).await;
    setup.merge_start(ID, "dev".to_owned()).await.unwrap();
    assert!(merge_of(&env.last().unwrap()).in_progress.is_some());

    // `workspace.PROJECT_FOLDER_REMOVED`: `.weaver/` is deleted, with `merge.json` in it.
    fs::remove_dir_all(root.join(".weaver")).unwrap();
    setup.batch_applied(ID, None);
    until(&env, |all| {
        all.last().is_some_and(|status| status.merge.is_none())
    })
    .await;
    // Git's merge stays as it is, and no scan or notice came of it.
    assert!(root.join(".git/MERGE_HEAD").exists());
    assert!(env.scans.lock().unwrap().is_empty());
    assert!(env.notices.lock().unwrap().is_empty());
}
