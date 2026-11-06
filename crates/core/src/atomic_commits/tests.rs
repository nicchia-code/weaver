use std::fs;
use std::path::{Path, PathBuf};

use super::*;
use crate::solo_mode::testing::Sandbox;

/// A project folder, with `.weaver/` inside, in the folder `name` of the sandbox.
fn project_in(path: PathBuf) -> PathBuf {
    fs::create_dir_all(path.join(".weaver/model")).unwrap();
    path
}

fn commits_dir(root: &Path) -> PathBuf {
    root.join(".weaver/commits")
}

fn no_git() -> Git {
    Git::system().with_search_path("")
}

#[test]
fn without_a_file_it_is_off_and_says_nothing() {
    let sandbox = Sandbox::new();
    let root = project_in(sandbox.repo("a"));
    let state = read_state(&root);
    assert!(!state.enabled);
    assert_eq!(state.diagnostic, None);
    assert!(!is_on(&root));
}

#[test]
fn turning_on_writes_the_state_with_its_own_gitignore_and_reads_back() {
    let sandbox = Sandbox::new();
    let root = project_in(sandbox.repo("a"));
    let inspection = set_enabled(&Git::system(), &root, true).unwrap();
    assert!(inspection.enabled);
    assert_eq!(inspection.unavailable, None);
    assert_eq!(inspection.diagnostic, None);
    assert_eq!(
        fs::read_to_string(state_path(&root)).unwrap(),
        "{\"enabled\": true}\n"
    );
    assert_eq!(
        fs::read(commits_dir(&root).join(".gitignore")).unwrap(),
        b"*\n"
    );
    assert!(is_on(&root));
}

#[test]
fn turning_off_deletes_the_whole_folder_and_needs_no_git() {
    let sandbox = Sandbox::new();
    let root = project_in(sandbox.repo("a"));
    set_enabled(&Git::system(), &root, true).unwrap();
    let inspection = set_enabled(&no_git(), &root, false).unwrap();
    assert!(!inspection.enabled);
    assert_eq!(inspection.unavailable, Some(Reason::NoGit));
    assert!(!commits_dir(&root).exists());
    assert!(!is_on(&root));
    // Off again, with nothing there: nothing happens.
    assert!(!set_enabled(&no_git(), &root, false).unwrap().enabled);
}

#[test]
fn it_cannot_be_turned_on_outside_a_repository_or_without_git_and_nothing_changes() {
    let sandbox = Sandbox::new();
    let root = project_in(sandbox.folder("plain"));
    assert_eq!(
        set_enabled(&Git::system(), &root, true),
        Err(Reason::NotARepository)
    );
    assert!(!commits_dir(&root).exists());

    let repo = project_in(sandbox.repo("a"));
    let error = set_enabled(&no_git(), &repo, true).unwrap_err();
    assert_eq!(error, Reason::NoGit);
    assert_eq!(error.to_string(), "git was not found");
    assert!(!commits_dir(&repo).exists());
}

#[test]
fn it_is_refused_in_a_folder_that_is_not_a_project() {
    let sandbox = Sandbox::new();
    let root = sandbox.repo("a");
    assert_eq!(
        set_enabled(&Git::system(), &root, true),
        Err(Reason::NotAProject)
    );
}

#[test]
fn a_file_that_cannot_be_read_or_holds_another_value_is_off_and_reported_and_not_rewritten() {
    let sandbox = Sandbox::new();
    let root = project_in(sandbox.repo("a"));
    for text in ["not json", "{\"enabled\": false}", "{\"enabled\": \"yes\"}", "{}"] {
        fs::create_dir_all(commits_dir(&root)).unwrap();
        fs::write(state_path(&root), text).unwrap();
        let state = read_state(&root);
        assert!(!state.enabled, "{text}");
        let diagnostic = state.diagnostic.unwrap_or_else(|| panic!("{text}"));
        assert_eq!(diagnostic.path, "commits/commits.json");
        assert!(!diagnostic.message.is_empty());
        assert!(!is_on(&root));
        assert_eq!(fs::read_to_string(state_path(&root)).unwrap(), text);
        // Inspecting never rewrites it either.
        assert!(inspect(&Git::system(), &root).diagnostic.is_some());
        assert_eq!(fs::read_to_string(state_path(&root)).unwrap(), text);
    }
}

#[test]
fn turning_on_repairs_a_file_reported_as_a_problem() {
    let sandbox = Sandbox::new();
    let root = project_in(sandbox.repo("a"));
    fs::create_dir_all(commits_dir(&root)).unwrap();
    fs::write(state_path(&root), "{\"enabled\": 3}").unwrap();
    let inspection = set_enabled(&Git::system(), &root, true).unwrap();
    assert!(inspection.enabled);
    assert_eq!(inspection.diagnostic, None);
    assert_eq!(
        fs::read_to_string(state_path(&root)).unwrap(),
        "{\"enabled\": true}\n"
    );
}

#[test]
fn it_is_never_part_of_the_settings() {
    let schema = serde_json::to_string(&crate::settings::Settings::default()).unwrap();
    assert!(!schema.to_lowercase().contains("atomic"));
    assert!(!schema.contains("commits"));
}

#[test]
fn another_clone_of_the_repository_starts_off() {
    let sandbox = Sandbox::new();
    let first = project_in(sandbox.repo("a"));
    let second = project_in(sandbox.repo("b"));
    set_enabled(&Git::system(), &first, true).unwrap();
    assert!(is_on(&first));
    assert!(!is_on(&second));
}
