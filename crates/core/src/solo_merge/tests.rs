//! Merge in Solo mode, against temporary git repositories with real git (`requirements/quality#solo-merge`).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::*;
use crate::project::clock::is_instant;
use crate::solo_mode::testing::{git, put, Sandbox};

// --- helpers ------------------------------------------------------------------------------------

/// The repository's own config, which also binds Weaver's git (it runs with the system config):
/// `core.autocrlf` off, so that a Windows git does not turn the LF files of the tests into CRLF.
fn configure(repo: &Path) {
    git(repo, &["symbolic-ref", "HEAD", "refs/heads/main"]);
    for (key, value) in [
        ("user.name", "Weaver Test"),
        ("user.email", "test@example.com"),
        ("commit.gpgsign", "false"),
        ("merge.ff", "true"),
        ("core.autocrlf", "false"),
    ] {
        git(repo, &["config", key, value]);
    }
}

fn repo(sandbox: &Sandbox, name: &str) -> PathBuf {
    let root = sandbox.repo(name);
    configure(&root);
    root
}

/// Writes `content` in `rel` and commits it (forcing the add, so that ignored files can be tracked).
fn save(repo: &Path, rel: &str, content: &str, message: &str) {
    put(repo, rel, content);
    git(repo, &["add", "-f", "--", rel]);
    git(repo, &["commit", "-q", "-m", message]);
}

fn head(repo: &Path) -> String {
    git(repo, &["rev-parse", "HEAD"]).trim().to_owned()
}

fn branch_name(repo: &Path) -> String {
    git(repo, &["rev-parse", "--abbrev-ref", "HEAD"])
        .trim()
        .to_owned()
}

fn status(repo: &Path) -> String {
    git(repo, &["status", "--porcelain=v1"])
}

/// A project folder with `.weaver/` inside `repo`, Solo mode on in `mode`.
fn project(repo: &Path, rel: &str, mode: Mode) -> PathBuf {
    let root = if rel.is_empty() {
        repo.to_path_buf()
    } else {
        repo.join(rel)
    };
    fs::create_dir_all(root.join(".weaver/model")).unwrap();
    if mode.is_on() {
        solo_mode::set_mode(&Git::system(), &root, mode).unwrap();
    }
    root
}

/// `main` with one commit of `a.txt`, and `dev` one commit further with `b.txt`; back on `main`.
fn ahead(repo: &Path) {
    save(repo, "a.txt", "base\n", "base");
    git(repo, &["checkout", "-q", "-b", "dev"]);
    save(repo, "b.txt", "dev\n", "dev work");
    git(repo, &["checkout", "-q", "main"]);
}

/// `main` and `dev` each one commit further, on different files; back on `main`.
fn diverged(repo: &Path) {
    save(repo, "a.txt", "base\n", "base");
    git(repo, &["checkout", "-q", "-b", "dev"]);
    save(repo, "b.txt", "dev\n", "dev work");
    git(repo, &["checkout", "-q", "main"]);
    save(repo, "c.txt", "main\n", "main work");
}

/// `main` and `dev` change `a.txt` differently, and `dev` adds `d.txt`; back on `main`.
fn conflicting(repo: &Path) {
    save(repo, "a.txt", "base\n", "base");
    git(repo, &["checkout", "-q", "-b", "dev"]);
    save(repo, "a.txt", "dev\n", "dev edits a");
    save(repo, "d.txt", "dev only\n", "dev adds d");
    git(repo, &["checkout", "-q", "main"]);
    save(repo, "a.txt", "main\n", "main edits a");
}

fn merge(git_runner: &Git, root: &Path, branch: &str) -> Started {
    start(git_runner, root, branch, false)
}

fn is_available(root: &Path) -> Availability {
    availability(&Git::system(), root, false)
}

fn record(start: &str, incoming: &str) -> Record {
    Record {
        branch: "dev".to_owned(),
        current: "main".to_owned(),
        start: start.to_owned(),
        incoming: incoming.to_owned(),
        started: "2026-11-05T10:00:00.000Z".to_owned(),
    }
}

// --- availability, in its order -----------------------------------------------------------------

#[test]
fn merge_is_available_on_a_branch_with_nothing_uncommitted() {
    let sandbox = Sandbox::new();
    let repo = repo(&sandbox, "a");
    ahead(&repo);
    let root = project(&repo, "", Mode::Folders);
    let answer = is_available(&root);
    assert_eq!(answer.unavailable, None);
    assert_eq!(answer.current.as_deref(), Some("main"));
}

#[test]
fn a_job_of_the_project_comes_first_even_with_a_merge_in_progress_and_no_git() {
    let sandbox = Sandbox::new();
    let repo = repo(&sandbox, "a");
    ahead(&repo);
    let root = project(&repo, "", Mode::Folders);
    write(&root, &record(&head(&repo), &head(&repo))).unwrap();
    let no_git = Git::system().with_search_path("");
    assert_eq!(
        availability(&no_git, &root, true).unavailable,
        Some(Unavailable::AgentWorking)
    );
    // Next in order: the merge of Weaver, before any call to git.
    assert_eq!(
        availability(&no_git, &root, false).unavailable,
        Some(Unavailable::MergeInProgress)
    );
    assert_eq!(
        Unavailable::AgentWorking.to_string(),
        "The agent is working"
    );
    assert_eq!(
        Unavailable::MergeInProgress.to_string(),
        "A merge is in progress"
    );
}

#[test]
fn a_git_that_fails_or_is_slow_says_why_before_anything_git_says() {
    let sandbox = Sandbox::new();
    let repo = repo(&sandbox, "a");
    ahead(&repo);
    let root = project(&repo, "", Mode::Folders);
    // A merge in git too, and a detached HEAD: the failure of git is the reason shown.
    let no_git = Git::system().with_search_path("");
    let answer = availability(&no_git, &root, false);
    assert_eq!(answer.unavailable, Some(Unavailable::Git(Reason::NoGit)));
    assert_eq!(
        answer.unavailable.unwrap().to_string(),
        "Merge needs git: git was not found"
    );

    let slow = Git::system().with_limit(Duration::ZERO);
    assert_eq!(
        availability(&slow, &root, false).unavailable,
        Some(Unavailable::Git(Reason::TimedOut(Duration::ZERO)))
    );

    let outside = sandbox.folder("plain");
    let answer = availability(&Git::system(), &outside, false);
    assert_eq!(
        answer.unavailable,
        Some(Unavailable::Git(Reason::NotARepository))
    );
}

#[test]
fn git_in_the_middle_of_an_operation_is_a_reason_before_a_detached_head() {
    let sandbox = Sandbox::new();

    // A merge with conflicts.
    let a = repo(&sandbox, "merging");
    conflicting(&a);
    let root = project(&a, "", Mode::Folders);
    let _ = Git::system().exec(&root, &["merge", "dev"], Call::Merge);
    assert_eq!(is_available(&root).unavailable, Some(Unavailable::GitBusy));

    // A cherry-pick with conflicts.
    let b = repo(&sandbox, "picking");
    conflicting(&b);
    let root = project(&b, "", Mode::Folders);
    let _ = Git::system().exec(&root, &["cherry-pick", "dev~1"], Call::Merge);
    assert_eq!(is_available(&root).unavailable, Some(Unavailable::GitBusy));

    // A rebase stopped by a conflict.
    let c = repo(&sandbox, "rebasing");
    conflicting(&c);
    let root = project(&c, "", Mode::Folders);
    let _ = Git::system().exec(&root, &["rebase", "dev"], Call::Merge);
    assert_eq!(is_available(&root).unavailable, Some(Unavailable::GitBusy));

    // A rebase folder, with HEAD detached: the operation is the reason.
    let d = repo(&sandbox, "folder");
    ahead(&d);
    let root = project(&d, "", Mode::Folders);
    git(&d, &["checkout", "-q", "--detach"]);
    fs::create_dir_all(d.join(".git/rebase-apply")).unwrap();
    assert_eq!(is_available(&root).unavailable, Some(Unavailable::GitBusy));
    fs::remove_dir_all(d.join(".git/rebase-apply")).unwrap();
    fs::create_dir_all(d.join(".git/rebase-merge")).unwrap();
    assert_eq!(is_available(&root).unavailable, Some(Unavailable::GitBusy));
}

#[test]
fn a_detached_head_comes_before_uncommitted_changes() {
    let sandbox = Sandbox::new();
    let repo = repo(&sandbox, "a");
    ahead(&repo);
    let root = project(&repo, "", Mode::Folders);
    git(&repo, &["checkout", "-q", "--detach"]);
    put(&repo, "a.txt", "changed\n");
    let answer = is_available(&root);
    assert_eq!(answer.unavailable, Some(Unavailable::Detached));
    assert_eq!(answer.unavailable.unwrap().to_string(), "Not on a branch");
}

#[test]
fn an_uncommitted_change_to_a_tracked_file_is_the_last_reason() {
    let sandbox = Sandbox::new();
    let repo = repo(&sandbox, "a");
    ahead(&repo);
    let root = project(&repo, "", Mode::Folders);
    put(&repo, "a.txt", "changed\n");
    let answer = is_available(&root);
    assert_eq!(answer.unavailable, Some(Unavailable::Uncommitted));
    assert_eq!(
        answer.unavailable.unwrap().to_string(),
        "Commit or stash your changes first"
    );
    // An untracked file is not an uncommitted change.
    put(&repo, "a.txt", "base\n");
    put(&repo, "new.txt", "x\n");
    assert_eq!(is_available(&root).unavailable, None);
}

#[test]
fn weavers_files_are_not_uncommitted_changes_in_either_mode_but_every_other_file_is() {
    let sandbox = Sandbox::new();
    for (mode, tracked, team) in [
        (
            Mode::Folders,
            [
                ".weaver/model/m.md",
                ".agents/skills/weaver-setup/SKILL.md",
                ".omp/agents/weaver-analysis.md",
            ],
            ".agents/skills/mine/SKILL.md",
        ),
        (
            Mode::Files,
            [
                ".weaver/model/m.md",
                ".agents/skills/weaver-setup/SKILL.md",
                ".omp/agents/weaver-analysis.md",
            ],
            ".agents/skills/team/SKILL.md",
        ),
    ] {
        let repo = repo(&sandbox, mode.as_str());
        ahead(&repo);
        for path in tracked {
            save(&repo, path, "one\n", "weaver file");
        }
        save(&repo, team, "one\n", "team file");
        let root = project(&repo, "", mode);
        for path in tracked {
            put(&repo, path, "two\n");
        }
        assert_eq!(is_available(&root).unavailable, None, "{mode:?}");
        if mode == Mode::Files {
            // The team's own file is not Weaver's in this mode.
            put(&repo, team, "two\n");
            assert_eq!(
                is_available(&root).unavailable,
                Some(Unavailable::Uncommitted)
            );
        } else {
            // The whole folder is Weaver's in this mode.
            put(&repo, team, "two\n");
            assert_eq!(is_available(&root).unavailable, None);
        }
        put(&repo, "a.txt", "changed\n");
        assert_eq!(
            is_available(&root).unavailable,
            Some(Unavailable::Uncommitted),
            "{mode:?}"
        );
    }

    // With Solo mode off, tracked files of Weaver count like any other.
    let repo = repo(&sandbox, "off");
    ahead(&repo);
    save(&repo, ".weaver/model/m.md", "one\n", "weaver file");
    let root = project(&repo, "", Mode::Off);
    put(&repo, ".weaver/model/m.md", "two\n");
    assert_eq!(
        is_available(&root).unavailable,
        Some(Unavailable::Uncommitted)
    );
}

#[test]
fn weavers_files_of_a_project_in_a_subfolder_are_found_with_its_prefix() {
    let sandbox = Sandbox::new();
    let repo = repo(&sandbox, "a");
    ahead(&repo);
    save(&repo, "app/.weaver/model/m.md", "one\n", "weaver file");
    save(
        &repo,
        ".weaver/model/m.md",
        "one\n",
        "another project's file",
    );
    let root = project(&repo, "app", Mode::Folders);
    put(&repo, "app/.weaver/model/m.md", "two\n");
    assert_eq!(is_available(&root).unavailable, None);
    // The block excludes only the files of this project.
    put(&repo, ".weaver/model/m.md", "two\n");
    assert_eq!(
        is_available(&root).unavailable,
        Some(Unavailable::Uncommitted)
    );
}

// --- branches -----------------------------------------------------------------------------------

#[test]
fn the_branches_are_local_then_remote_sorted_without_the_current_one_or_any_remote_head() {
    let sandbox = Sandbox::new();
    let origin = repo(&sandbox, "origin");
    save(&origin, "a.txt", "base\n", "base");
    for name in ["zeta", "dev"] {
        git(&origin, &["branch", name]);
    }
    git(
        sandbox.dir.path(),
        &["clone", "-q", origin.to_str().unwrap(), "work"],
    );
    let work = sandbox.folder("work");
    configure(&work);
    for name in ["feature/b", "alpha"] {
        git(&work, &["branch", name]);
    }
    let root = project(&work, "", Mode::Folders);
    save(&work, ".weaver/model/m.md", "x\n", "weaver file tracked");
    save(&work, ".weaver/model/n.md", "x\n", "weaver file tracked");

    let found = branches(&Git::system(), &root).unwrap();
    assert_eq!(found.current.as_deref(), Some("main"));
    assert_eq!(found.local, ["alpha", "feature/b"]);
    assert_eq!(found.remote, ["origin/dev", "origin/main", "origin/zeta"]);
    assert_eq!(found.tracked, [".weaver/model/m.md", ".weaver/model/n.md"]);

    // Nothing was fetched: a branch that appears in origin later is not known.
    git(&origin, &["branch", "later"]);
    assert!(!branches(&Git::system(), &root)
        .unwrap()
        .remote
        .contains(&"origin/later".to_owned()));
}

#[test]
fn with_no_other_branch_the_lists_are_empty_and_without_git_the_reason_comes_back() {
    let sandbox = Sandbox::new();
    let repo = repo(&sandbox, "a");
    save(&repo, "a.txt", "base\n", "base");
    let root = project(&repo, "", Mode::Folders);
    let found = branches(&Git::system(), &root).unwrap();
    assert!(found.local.is_empty() && found.remote.is_empty() && found.tracked.is_empty());
    assert_eq!(
        branches(&Git::system().with_search_path(""), &root),
        Err(Reason::NoGit)
    );
}

// --- start --------------------------------------------------------------------------------------

#[test]
fn a_branch_that_brings_nothing_new_starts_nothing() {
    let sandbox = Sandbox::new();
    let repo = repo(&sandbox, "a");
    ahead(&repo);
    git(&repo, &["branch", "same"]);
    let root = project(&repo, "", Mode::Folders);
    let before = head(&repo);
    assert_eq!(
        merge(&Git::system(), &root, "same"),
        Started::UpToDate {
            current: "main".to_owned(),
            branch: "same".to_owned()
        }
    );
    assert!(!in_progress(&root));
    assert_eq!(head(&repo), before);
}

#[test]
fn a_fast_forward_is_concluded_at_once_with_the_incoming_commit_as_head() {
    let sandbox = Sandbox::new();
    let repo = repo(&sandbox, "a");
    ahead(&repo);
    let root = project(&repo, "", Mode::Folders);
    let incoming = git(&repo, &["rev-parse", "dev"]).trim().to_owned();
    let started = merge(&Git::system(), &root, "dev");
    assert_eq!(
        started,
        Started::Concluded(Concluded {
            branch: "dev".to_owned(),
            files: vec!["b.txt".to_owned()]
        })
    );
    assert_eq!(head(&repo), incoming);
    assert!(
        !in_progress(&root),
        "merge.json is deleted when the merge ends"
    );
    assert!(root.join(".weaver/merge/.gitignore").exists());
}

#[test]
fn a_merge_without_conflicts_makes_its_own_commit_with_gits_message() {
    let sandbox = Sandbox::new();
    let repo = repo(&sandbox, "a");
    diverged(&repo);
    let root = project(&repo, "", Mode::Folders);
    let before = head(&repo);
    let started = merge(&Git::system(), &root, "dev");
    assert_eq!(
        started,
        Started::Concluded(Concluded {
            branch: "dev".to_owned(),
            files: vec!["b.txt".to_owned()]
        })
    );
    let parents = git(&repo, &["rev-list", "--parents", "-n", "1", "HEAD"]);
    assert_eq!(
        parents.split_whitespace().count(),
        3,
        "a merge commit: {parents}"
    );
    assert_eq!(parents.split_whitespace().nth(1).unwrap(), before);
    assert_eq!(
        git(&repo, &["log", "-1", "--format=%s"]).trim(),
        "Merge branch 'dev'"
    );
    assert!(!in_progress(&root));
    assert_eq!(status(&repo), "");
}

#[test]
fn a_remote_branch_merges_like_a_local_one() {
    let sandbox = Sandbox::new();
    let origin = repo(&sandbox, "origin");
    save(&origin, "a.txt", "base\n", "base");
    git(&origin, &["checkout", "-q", "-b", "dev"]);
    save(&origin, "b.txt", "dev\n", "dev work");
    git(&origin, &["checkout", "-q", "main"]);
    git(
        sandbox.dir.path(),
        &["clone", "-q", origin.to_str().unwrap(), "work"],
    );
    let work = sandbox.folder("work");
    configure(&work);
    let root = project(&work, "", Mode::Folders);
    assert!(matches!(
        merge(&Git::system(), &root, "origin/dev"),
        Started::Concluded(_)
    ));
    assert!(work.join("b.txt").exists());
}

#[test]
fn conflicts_lead_to_merging_with_their_count_and_the_merge_is_remembered() {
    let sandbox = Sandbox::new();
    let repo = repo(&sandbox, "a");
    conflicting(&repo);
    let root = project(&repo, "", Mode::Folders);
    let start_hash = head(&repo);
    let incoming = git(&repo, &["rev-parse", "dev"]).trim().to_owned();

    let started = merge(&Git::system(), &root, "dev");
    assert_eq!(
        started,
        Started::Merging(InProgress {
            branch: "dev".to_owned(),
            current: "main".to_owned(),
            conflicts: Some(1)
        })
    );
    // The merge stays in git, uncommitted and nothing staged by Weaver beyond git's own work.
    assert_eq!(head(&repo), start_hash);
    assert!(repo.join(".git/MERGE_HEAD").exists());

    let Stored::Found(stored) = read(&root) else {
        panic!("merge.json missing");
    };
    assert_eq!(stored.branch, "dev");
    assert_eq!(stored.current, "main");
    assert_eq!(stored.start, start_hash);
    assert_eq!(stored.incoming, incoming);
    assert!(is_instant(&stored.started), "{}", stored.started);

    let text = fs::read_to_string(record_path(&root)).unwrap();
    assert!(text.ends_with('\n') && !text.contains('\r'));
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    let mut keys: Vec<&str> = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(keys, ["branch", "current", "incoming", "start", "started"]);
    assert_eq!(
        fs::read_to_string(root.join(".weaver/merge/.gitignore")).unwrap(),
        "*\n"
    );
    // The file is never in the repository's view.
    assert!(!status(&repo).contains("merge.json"));

    // While merging, Merge is not available and says why.
    assert_eq!(
        is_available(&root).unavailable,
        Some(Unavailable::MergeInProgress)
    );
}

#[test]
fn a_start_is_refused_when_a_condition_holds_again_and_nothing_is_written() {
    let sandbox = Sandbox::new();
    let repo = repo(&sandbox, "a");
    ahead(&repo);
    let root = project(&repo, "", Mode::Folders);
    assert_eq!(
        start(&Git::system(), &root, "dev", true),
        Started::Refused(Refusal::Unavailable(Unavailable::AgentWorking))
    );
    put(&repo, "a.txt", "changed\n");
    assert_eq!(
        merge(&Git::system(), &root, "dev"),
        Started::Refused(Refusal::Unavailable(Unavailable::Uncommitted))
    );
    assert!(!in_progress(&root));
    assert!(!root.join(".weaver/merge").exists());
}

#[test]
fn git_refuses_a_merge_that_would_overwrite_local_changes_and_says_its_first_line() {
    let sandbox = Sandbox::new();
    let repo = repo(&sandbox, "a");
    save(&repo, ".weaver/model/m.md", "one\n", "weaver file");
    git(&repo, &["checkout", "-q", "-b", "dev"]);
    save(
        &repo,
        ".weaver/model/m.md",
        "dev\n",
        "dev edits the weaver file",
    );
    git(&repo, &["checkout", "-q", "main"]);
    save(&repo, "other.txt", "x\n", "main work");
    let root = project(&repo, "", Mode::Folders);
    // Weaver's own tracked file, changed locally: not an uncommitted change for Merge, but git
    // would lose it.
    put(&repo, ".weaver/model/m.md", "mine\n");
    assert_eq!(is_available(&root).unavailable, None);
    let before = head(&repo);

    let started = merge(&Git::system(), &root, "dev");
    let Started::Refused(Refusal::Git(line)) = started else {
        panic!("{started:?}");
    };
    assert!(
        line.contains("Your local changes to the following files would be overwritten by merge"),
        "{line}"
    );
    assert_eq!(head(&repo), before);
    assert_eq!(
        fs::read_to_string(repo.join(".weaver/model/m.md")).unwrap(),
        "mine\n"
    );
    assert!(!in_progress(&root), "merge.json goes with a refused merge");
}

#[test]
fn files_git_would_overwrite_that_are_not_solo_modes_give_gits_first_line() {
    let sandbox = Sandbox::new();
    let repo = repo(&sandbox, "a");
    save(&repo, "a.txt", "base\n", "base");
    git(&repo, &["checkout", "-q", "-b", "dev"]);
    save(&repo, "plain.txt", "dev\n", "dev adds plain");
    git(&repo, &["checkout", "-q", "main"]);
    save(&repo, "c.txt", "main\n", "main work");
    let root = project(&repo, "", Mode::Folders);
    put(&repo, "plain.txt", "mine\n");

    let started = merge(&Git::system(), &root, "dev");
    let Started::Refused(Refusal::Git(line)) = started else {
        panic!("{started:?}");
    };
    assert!(
        line.contains("untracked working tree files would be overwritten by merge"),
        "{line}"
    );
    assert_eq!(
        fs::read_to_string(repo.join("plain.txt")).unwrap(),
        "mine\n"
    );
    assert!(!in_progress(&root));
}

#[test]
fn files_solo_mode_keeps_out_of_git_are_never_overwritten_and_are_named() {
    let sandbox = Sandbox::new();
    for rel in ["", "app"] {
        let name = if rel.is_empty() { "root" } else { rel };
        let repo = repo(&sandbox, name);
        let prefix = if rel.is_empty() {
            String::new()
        } else {
            format!("{rel}/")
        };
        save(&repo, &format!("{prefix}a.txt"), "base\n", "base");
        git(&repo, &["checkout", "-q", "-b", "dev"]);
        save(
            &repo,
            &format!("{prefix}.weaver/model/new.md"),
            "dev\n",
            "dev tracks a weaver file",
        );
        git(&repo, &["checkout", "-q", "main"]);
        save(&repo, &format!("{prefix}c.txt"), "main\n", "main work");
        let root = project(&repo, rel, Mode::Folders);
        put(&root, ".weaver/model/new.md", "mine, kept out of git\n");
        let before = head(&repo);

        let started = merge(&Git::system(), &root, "dev");
        assert_eq!(
            started,
            Started::Refused(Refusal::KeptOutOfGit(vec![
                ".weaver/model/new.md".to_owned()
            ])),
            "project at `{rel}`"
        );
        assert_eq!(head(&repo), before);
        assert_eq!(
            fs::read_to_string(root.join(".weaver/model/new.md")).unwrap(),
            "mine, kept out of git\n"
        );
        assert!(!in_progress(&root));
    }
}

/// A `git` that runs the real one, then waits past any limit after `git merge` (the merge itself
/// stops on its conflicts at once, so that is how the limit is reached with `MERGE_HEAD` in place), or
/// waits before it does anything.
#[cfg(unix)]
fn slow_git(sandbox: &Sandbox, name: &str, before: bool) -> Git {
    use std::os::unix::fs::PermissionsExt;
    let real = std::process::Command::new("sh")
        .args(["-c", "command -v git"])
        .output()
        .unwrap();
    let real = String::from_utf8(real.stdout).unwrap().trim().to_owned();
    let folder = sandbox.folder(name);
    let script = if before {
        format!(
            "#!/bin/sh\nif [ \"$1\" = merge ] && [ \"$2\" != --abort ]; then exec /bin/sleep 30; fi\nexec {real} \"$@\"\n"
        )
    } else {
        format!(
            "#!/bin/sh\nif [ \"$1\" = merge ] && [ \"$2\" != --abort ]; then {real} \"$@\"; exec /bin/sleep 30; fi\nexec {real} \"$@\"\n"
        )
    };
    let program = folder.join("git");
    fs::write(&program, script).unwrap();
    fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).unwrap();
    Git::system()
        .with_search_path(folder.into_os_string())
        .with_merge_limit(Duration::from_millis(600))
}

#[cfg(unix)]
#[test]
fn a_merge_that_exceeds_its_limit_is_aborted_and_the_branch_stays_as_it_was() {
    let sandbox = Sandbox::new();
    let repo = repo(&sandbox, "a");
    conflicting(&repo);
    let root = project(&repo, "", Mode::Folders);
    let before = head(&repo);
    let slow = slow_git(&sandbox, "slow-after", false);

    let started = merge(&slow, &root, "dev");
    assert_eq!(started, Started::Refused(Refusal::TimedOut));
    assert_eq!(head(&repo), before);
    assert!(
        !repo.join(".git/MERGE_HEAD").exists(),
        "git merge --abort ran"
    );
    assert_eq!(status(&repo), "", "the working copy is as it was");
    assert!(!in_progress(&root));
}

#[cfg(unix)]
#[test]
fn a_merge_killed_before_it_changed_anything_leaves_nothing_behind() {
    let sandbox = Sandbox::new();
    let repo = repo(&sandbox, "a");
    conflicting(&repo);
    let root = project(&repo, "", Mode::Folders);
    let before = head(&repo);
    let slow = slow_git(&sandbox, "slow-before", true);

    assert_eq!(
        merge(&slow, &root, "dev"),
        Started::Refused(Refusal::TimedOut)
    );
    assert_eq!(head(&repo), before);
    assert_eq!(status(&repo), "");
    assert!(!in_progress(&root));
}

#[test]
fn git_that_cannot_be_started_refuses_and_leaves_no_merge_json() {
    let sandbox = Sandbox::new();
    let repo = repo(&sandbox, "a");
    ahead(&repo);
    let root = project(&repo, "", Mode::Folders);
    let started = merge(&Git::system().with_search_path(""), &root, "dev");
    assert_eq!(
        started,
        Started::Refused(Refusal::Unavailable(Unavailable::Git(Reason::NoGit)))
    );
    assert!(!in_progress(&root));
}

#[test]
fn an_unknown_branch_or_an_option_is_refused_with_gits_first_line() {
    let sandbox = Sandbox::new();
    let repo = repo(&sandbox, "a");
    ahead(&repo);
    let root = project(&repo, "", Mode::Folders);
    for name in ["missing", "--abort", ""] {
        assert!(
            matches!(
                merge(&Git::system(), &root, name),
                Started::Refused(Refusal::Git(_))
            ),
            "{name:?}"
        );
    }
    assert!(!in_progress(&root));
}

// --- the check of a merge in progress -----------------------------------------------------------

/// A merge with conflicts in progress; returns the project root.
fn merging(sandbox: &Sandbox, name: &str) -> (PathBuf, PathBuf) {
    let repo = repo(sandbox, name);
    conflicting(&repo);
    let root = project(&repo, "", Mode::Folders);
    assert!(matches!(
        merge(&Git::system(), &root, "dev"),
        Started::Merging(_)
    ));
    (repo, root)
}

fn resolve_and_commit(repo: &Path) {
    put(repo, "a.txt", "resolved\n");
    git(repo, &["add", "a.txt", "d.txt"]);
    git(repo, &["commit", "-q", "--no-edit"]);
}

#[test]
fn a_merge_still_in_progress_is_still_merging_with_the_count_computed_again() {
    let sandbox = Sandbox::new();
    let (repo, root) = merging(&sandbox, "a");
    let progress = |count| {
        Check::Merging(InProgress {
            branch: "dev".to_owned(),
            current: "main".to_owned(),
            conflicts: Some(count),
        })
    };
    assert_eq!(check(&Git::system(), &root), progress(1));
    put(&repo, "a.txt", "resolved\n");
    git(&repo, &["add", "a.txt"]);
    assert_eq!(check(&Git::system(), &root), progress(0));
    assert!(
        in_progress(&root),
        "merge.json stays while the merge is in progress"
    );
}

#[test]
fn a_merge_committed_outside_weaver_is_concluded_with_the_files_it_brought() {
    let sandbox = Sandbox::new();
    let (repo, root) = merging(&sandbox, "a");
    resolve_and_commit(&repo);
    let checked = check(&Git::system(), &root);
    assert_eq!(
        checked,
        Check::Concluded(Concluded {
            branch: "dev".to_owned(),
            files: vec!["a.txt".to_owned(), "d.txt".to_owned()]
        })
    );
    assert!(!in_progress(&root), "merge.json is deleted at the end");
    assert_eq!(check(&Git::system(), &root), Check::NoMerge);
}

#[test]
fn a_merge_committed_outside_weaver_with_commits_on_top_is_still_the_same_merge() {
    let sandbox = Sandbox::new();
    let (repo, root) = merging(&sandbox, "a");
    resolve_and_commit(&repo);
    save(&repo, "later.txt", "later\n", "a later commit");
    save(&repo, "b.txt", "later\n", "another later commit");
    let Check::Concluded(concluded) = check(&Git::system(), &root) else {
        panic!("not concluded");
    };
    // The files are those of the merge's own commit, not of what came after.
    assert_eq!(concluded.files, ["a.txt", "d.txt"]);
}

#[test]
fn a_merge_aborted_outside_weaver_is_forgotten_without_a_scan() {
    let sandbox = Sandbox::new();
    let (repo, root) = merging(&sandbox, "a");
    git(&repo, &["merge", "--abort"]);
    assert_eq!(
        check(&Git::system(), &root),
        Check::Forgotten {
            branch: Some("dev".to_owned()),
            diagnostic: None
        }
    );
    assert!(!in_progress(&root));
}

#[test]
fn a_merge_on_another_branch_or_reset_away_is_forgotten() {
    let sandbox = Sandbox::new();

    let (repo, root) = merging(&sandbox, "another-branch");
    resolve_and_commit(&repo);
    git(&repo, &["checkout", "-q", "dev"]);
    assert_eq!(
        check(&Git::system(), &root),
        Check::Forgotten {
            branch: Some("dev".to_owned()),
            diagnostic: None
        }
    );
    assert!(!in_progress(&root));

    let (repo, root) = merging(&sandbox, "reset");
    let Stored::Found(stored) = read(&root) else {
        panic!("merge.json missing");
    };
    resolve_and_commit(&repo);
    git(&repo, &["reset", "-q", "--hard", &stored.start]);
    assert!(matches!(
        check(&Git::system(), &root),
        Check::Forgotten { .. }
    ));
    assert!(!in_progress(&root));

    // A detached HEAD is another branch too.
    let (repo, root) = merging(&sandbox, "detached");
    resolve_and_commit(&repo);
    git(&repo, &["checkout", "-q", "--detach"]);
    assert!(matches!(
        check(&Git::system(), &root),
        Check::Forgotten { .. }
    ));
}

#[test]
fn a_crash_after_merge_json_was_written_is_found_at_the_next_opening() {
    let sandbox = Sandbox::new();
    let repo = repo(&sandbox, "a");
    conflicting(&repo);
    let root = project(&repo, "", Mode::Folders);
    let incoming = git(&repo, &["rev-parse", "dev"]).trim().to_owned();

    // Weaver died after writing merge.json and before git ran: nothing happened in git.
    write(&root, &record(&head(&repo), &incoming)).unwrap();
    assert_eq!(
        check(&Git::system(), &root),
        Check::Forgotten {
            branch: Some("dev".to_owned()),
            diagnostic: None
        }
    );
    assert!(!in_progress(&root));

    // Weaver died while git was stopped on the conflicts: the merge is found again.
    write(&root, &record(&head(&repo), &incoming)).unwrap();
    let _ = Git::system().exec(&root, &["merge", "--no-edit", "dev"], Call::Merge);
    assert!(matches!(
        check(&Git::system(), &root),
        Check::Merging(InProgress {
            conflicts: Some(1),
            ..
        })
    ));
}

#[test]
fn a_merge_json_that_cannot_be_read_is_reported_forgotten_and_deleted() {
    let sandbox = Sandbox::new();
    let repo = repo(&sandbox, "a");
    ahead(&repo);
    let root = project(&repo, "", Mode::Folders);
    ensure_local_folder(&root, MERGE_DIR).unwrap();

    fs::write(record_path(&root), "this is not json").unwrap();
    let Check::Forgotten { branch, diagnostic } = check(&Git::system(), &root) else {
        panic!("not forgotten");
    };
    let diagnostic = diagnostic.expect("a diagnostic");
    assert_eq!(branch, None);
    assert_eq!(diagnostic.path, "merge/merge.json");
    assert!(
        diagnostic.message.contains("cannot read the file"),
        "{}",
        diagnostic.message
    );
    assert!(!in_progress(&root));

    fs::write(
        record_path(&root),
        "{\"branch\": \"dev\", \"current\": \"main\"}",
    )
    .unwrap();
    let Check::Forgotten { branch, diagnostic } = check(&Git::system(), &root) else {
        panic!("not forgotten");
    };
    assert_eq!(branch.as_deref(), Some("dev"));
    assert!(diagnostic.unwrap().message.contains("missing field"));
    assert!(!in_progress(&root));
}

#[test]
fn a_git_call_that_fails_during_the_check_leaves_the_merge_in_progress() {
    let sandbox = Sandbox::new();
    let (_, root) = merging(&sandbox, "a");
    let checked = check(&Git::system().with_search_path(""), &root);
    let Check::Failed { progress, reason } = checked else {
        panic!("{checked:?}");
    };
    assert_eq!(progress.branch, "dev");
    assert_eq!(progress.conflicts, None);
    assert_eq!(reason, Reason::NoGit);
    assert!(in_progress(&root));
}

#[test]
fn the_check_of_a_missing_merge_does_nothing() {
    let sandbox = Sandbox::new();
    let repo = repo(&sandbox, "a");
    ahead(&repo);
    let root = project(&repo, "", Mode::Folders);
    assert_eq!(check(&Git::system(), &root), Check::NoMerge);
    let refreshed = refresh(&Git::system(), &root, false);
    assert_eq!(refreshed.unavailable, None);
    assert_eq!(refreshed.current.as_deref(), Some("main"));
    assert_eq!(refreshed.in_progress, None);
    assert_eq!(refreshed.ended, None);
}

#[test]
fn a_refresh_ends_the_merge_and_then_works_out_the_availability_again() {
    let sandbox = Sandbox::new();
    let (repo, root) = merging(&sandbox, "a");
    let first = refresh(&Git::system(), &root, false);
    assert_eq!(first.unavailable, Some(Unavailable::MergeInProgress));
    assert_eq!(first.in_progress.as_ref().unwrap().conflicts, Some(1));

    resolve_and_commit(&repo);
    let second = refresh(&Git::system(), &root, false);
    assert!(matches!(second.ended, Some(Ended::Concluded(_))));
    assert_eq!(second.in_progress, None);
    // The merge ended in this very step: Merge is available again.
    assert_eq!(second.unavailable, None);
}

// --- abort --------------------------------------------------------------------------------------

#[test]
fn an_abort_leaves_the_branch_as_it_was_forgets_the_merge_and_scans_nothing() {
    let sandbox = Sandbox::new();
    let (repo, root) = merging(&sandbox, "a");
    let Stored::Found(stored) = read(&root) else {
        panic!("merge.json missing");
    };
    assert_eq!(abort(&Git::system(), &root, false), Aborted::Done);
    assert_eq!(head(&repo), stored.start);
    assert_eq!(status(&repo), "");
    assert_eq!(branch_name(&repo), "main");
    assert!(!in_progress(&root));
    assert_eq!(abort(&Git::system(), &root, false), Aborted::NoMerge);
}

#[test]
fn an_abort_is_refused_while_the_merge_conflicts_job_has_not_ended() {
    let sandbox = Sandbox::new();
    let (repo, root) = merging(&sandbox, "a");
    assert_eq!(abort(&Git::system(), &root, true), Aborted::JobNotEnded);
    assert!(in_progress(&root));
    assert!(repo.join(".git/MERGE_HEAD").exists());
}

#[test]
fn a_failed_abort_leaves_the_merge_in_progress_with_gits_first_line() {
    let sandbox = Sandbox::new();
    let (repo, root) = merging(&sandbox, "a");
    // Committed outside Weaver but not checked yet: git has no merge to abort.
    resolve_and_commit(&repo);
    let Aborted::Failed(line) = abort(&Git::system(), &root, false) else {
        panic!("not failed");
    };
    assert!(line.contains("no merge to abort"), "{line}");
    assert!(in_progress(&root));

    // Without git, the reason says so.
    assert_eq!(
        abort(&Git::system().with_search_path(""), &root, false),
        Aborted::Failed("git was not found".to_owned())
    );
    assert!(in_progress(&root));
}

// --- the files of the merge ---------------------------------------------------------------------

#[test]
fn the_merged_files_of_a_project_in_a_subfolder_are_relative_to_it_and_leave_out_the_rest() {
    let sandbox = Sandbox::new();
    let repo = repo(&sandbox, "a");
    save(&repo, "app/a.txt", "base\n", "base");
    git(&repo, &["checkout", "-q", "-b", "dev"]);
    save(&repo, "app/src/x.txt", "dev\n", "dev work in the project");
    save(&repo, "app/y.txt", "dev\n", "dev work in the project");
    save(
        &repo,
        "elsewhere/z.txt",
        "dev\n",
        "dev work outside the project",
    );
    git(&repo, &["checkout", "-q", "main"]);
    save(&repo, "app/c.txt", "main\n", "main work");
    let root = project(&repo, "app", Mode::Folders);

    let started = merge(&Git::system(), &root, "dev");
    assert_eq!(
        started,
        Started::Concluded(Concluded {
            branch: "dev".to_owned(),
            files: vec!["src/x.txt".to_owned(), "y.txt".to_owned()]
        })
    );
}

#[test]
fn the_merged_files_of_a_fast_forward_a_merge_commit_and_resolved_conflicts_in_the_repositorys_root(
) {
    let sandbox = Sandbox::new();

    let a = repo(&sandbox, "ff");
    ahead(&a);
    let root = project(&a, "", Mode::Folders);
    let start_hash = head(&a);
    assert!(matches!(
        merge(&Git::system(), &root, "dev"),
        Started::Concluded(_)
    ));
    assert_eq!(
        merged_files(&Git::system(), &root, &start_hash, &head(&a)).unwrap(),
        ["b.txt"]
    );

    let b = repo(&sandbox, "merge-commit");
    diverged(&b);
    let root = project(&b, "", Mode::Folders);
    let start_hash = head(&b);
    assert!(matches!(
        merge(&Git::system(), &root, "dev"),
        Started::Concluded(_)
    ));
    assert_eq!(
        merged_files(&Git::system(), &root, &start_hash, &head(&b)).unwrap(),
        ["b.txt"]
    );

    let (c, root) = merging(&sandbox, "resolved");
    let Stored::Found(stored) = read(&root) else {
        panic!("merge.json missing");
    };
    resolve_and_commit(&c);
    assert_eq!(
        merged_files(&Git::system(), &root, &stored.start, &head(&c)).unwrap(),
        ["a.txt", "d.txt"]
    );
}

#[test]
fn weaver_never_commits_stages_or_fetches() {
    let sandbox = Sandbox::new();
    let (repo, root) = merging(&sandbox, "a");
    // The conflicting file is exactly as git left it: unmerged, not marked as resolved.
    assert!(git(&repo, &["diff", "--name-only", "--diff-filter=U"]).contains("a.txt"));
    assert!(matches!(
        check(&Git::system(), &root),
        Check::Merging(InProgress {
            conflicts: Some(1),
            ..
        })
    ));
    assert_eq!(
        git(&repo, &["rev-list", "--count", "HEAD"]).trim(),
        "2",
        "no commit was made"
    );
    assert!(
        !repo.join(".git/FETCH_HEAD").exists(),
        "git fetch never ran"
    );
}

#[test]
fn the_untracked_files_git_says_it_would_overwrite_are_read_from_its_error() {
    let modern = "error: The following untracked working tree files would be overwritten by merge:\n\t.weaver/model/a.md\n\tdir/b.txt\nPlease move or remove them before you merge.\nAborting\n";
    assert_eq!(
        overwritten_untracked(modern),
        [".weaver/model/a.md", "dir/b.txt"]
    );
    let single = "error: Untracked working tree file 'x/y.md' would be overwritten by merge.\n";
    assert_eq!(overwritten_untracked(single), ["x/y.md"]);
    assert!(overwritten_untracked("error: Your local changes would be overwritten\n").is_empty());
}

#[test]
fn a_fast_forward_that_would_overwrite_a_kept_out_file_is_refused_too() {
    let sandbox = Sandbox::new();
    let repo = repo(&sandbox, "a");
    save(&repo, "a.txt", "base\n", "base");
    git(&repo, &["checkout", "-q", "-b", "dev"]);
    save(
        &repo,
        ".weaver/model/new.md",
        "dev\n",
        "dev tracks a weaver file",
    );
    git(&repo, &["checkout", "-q", "main"]);
    let root = project(&repo, "", Mode::Files);
    put(&root, ".weaver/model/new.md", "mine\n");
    assert_eq!(
        merge(&Git::system(), &root, "dev"),
        Started::Refused(Refusal::KeptOutOfGit(vec![
            ".weaver/model/new.md".to_owned()
        ]))
    );
    assert_eq!(
        fs::read_to_string(root.join(".weaver/model/new.md")).unwrap(),
        "mine\n"
    );
    assert!(!in_progress(&root));
}
