//! What a project window shows of git (`requirements/tech#window`): the branch the folder is on, git's
//! HEAD file of the working copy, and the developer's user name.
//!
//! Every call is a read through the runner of Solo mode ([`Git`]): `GIT_OPTIONAL_LOCKS=0`, the limit
//! of 10 s, the project root as working folder. A missing git, a failure, a timeout and a folder
//! outside a repository all answer `None`: the window then shows nothing, never an error. Only the
//! user's name is read, never an email or any other identity field.

use std::path::{Path, PathBuf};

use crate::process::timed::Failure;
use crate::solo_mode::{Call, Git};

/// Runs a read and answers its trimmed, non-empty output.
fn read(git: &Git, root: &Path, args: &[&str]) -> Option<String> {
    let output = git.exec(root, args, Call::Read).ok()?;
    let text = output.stdout.trim();
    (!text.is_empty()).then(|| text.to_owned())
}

/// The branch git is on in `root` (`git symbolic-ref --short -q HEAD`); when it exits 1, because
/// HEAD is on no branch, the short hash of the commit (`git rev-parse --short HEAD`). `None` when
/// git is missing or fails, or when `root` is not in a repository.
pub fn branch(git: &Git, root: &Path) -> Option<String> {
    match git.exec(root, &["symbolic-ref", "--short", "-q", "HEAD"], Call::Read) {
        Ok(output) => {
            let name = output.stdout.trim();
            (!name.is_empty()).then(|| name.to_owned())
        }
        Err(Failure::Exit { code: Some(1), .. }) => {
            read(git, root, &["rev-parse", "--short", "HEAD"])
        }
        Err(_) => None,
    }
}

/// The path of git's HEAD file of the working copy (`git rev-parse --git-path HEAD`): the one of a
/// linked worktree and the one a `.git` file points to included. `None` outside a repository.
pub fn head_path(git: &Git, root: &Path) -> Option<PathBuf> {
    let printed = PathBuf::from(read(git, root, &["rev-parse", "--git-path", "HEAD"])?);
    // Git prints the path relative to the folder it ran in, unless it is absolute.
    Some(if printed.is_absolute() {
        printed
    } else {
        root.join(printed)
    })
}

/// The developer's user name: `git config user.name` in `root`, which falls back to the global
/// value, else `git config --global user.name` (outside a repository). `None` when there is none.
pub fn user_name(git: &Git, root: &Path) -> Option<String> {
    read(git, root, &["config", "user.name"])
        .or_else(|| read(git, root, &["config", "--global", "user.name"]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solo_mode::testing::{commit, git as run, Sandbox};

    #[test]
    fn the_branch_is_the_one_git_is_on() {
        let sandbox = Sandbox::new();
        let repo = sandbox.repo("repo");
        run(&repo, &["checkout", "-q", "-b", "feature/sidebar"]);
        assert_eq!(branch(&Git::system(), &repo).as_deref(), Some("feature/sidebar"));
    }

    #[test]
    fn a_repository_without_a_commit_already_has_its_branch() {
        let sandbox = Sandbox::new();
        let repo = sandbox.repo("repo");
        run(&repo, &["checkout", "-q", "-b", "main"]);
        assert_eq!(branch(&Git::system(), &repo).as_deref(), Some("main"));
    }

    #[test]
    fn a_detached_head_is_the_short_hash_of_the_commit() {
        let sandbox = Sandbox::new();
        let repo = sandbox.repo("repo");
        commit(&repo);
        let full = run(&repo, &["rev-parse", "HEAD"]).trim().to_owned();
        run(&repo, &["checkout", "-q", "--detach"]);
        let shown = branch(&Git::system(), &repo).expect("the short hash");
        assert!(shown.len() >= 7 && shown.len() < full.len(), "{shown}");
        assert!(full.starts_with(&shown));
    }

    #[test]
    fn a_folder_outside_a_repository_or_without_git_has_no_branch() {
        let sandbox = Sandbox::new();
        let plain = sandbox.folder("plain");
        assert_eq!(branch(&Git::system(), &plain), None);
        let repo = sandbox.repo("repo");
        assert_eq!(
            branch(&Git::system().with_search_path(""), &repo),
            None,
            "git cannot be run"
        );
    }

    #[test]
    fn the_branch_of_a_subfolder_is_the_one_of_its_repository() {
        let sandbox = Sandbox::new();
        let repo = sandbox.repo("repo");
        run(&repo, &["checkout", "-q", "-b", "inner"]);
        let inner = repo.join("packages").join("app");
        std::fs::create_dir_all(&inner).unwrap();
        assert_eq!(branch(&Git::system(), &inner).as_deref(), Some("inner"));
    }

    #[test]
    fn the_head_file_is_the_one_of_the_working_copy() {
        let sandbox = Sandbox::new();
        let repo = sandbox.repo("repo");
        let head = head_path(&Git::system(), &repo).expect("a repository");
        assert!(head.is_absolute());
        assert_eq!(head, repo.join(".git").join("HEAD"));
        assert_eq!(head_path(&Git::system(), &sandbox.folder("plain")), None);
    }

    #[test]
    fn a_linked_worktree_has_a_head_file_of_its_own() {
        let sandbox = Sandbox::new();
        let repo = sandbox.repo("repo");
        commit(&repo);
        let tree = sandbox.dir.path().join("tree");
        run(&repo, &["worktree", "add", "-q", "-b", "other", tree.to_str().unwrap()]);
        let tree = crate::paths::canonical_root(&tree).unwrap();
        let head = head_path(&Git::system(), &tree).expect("a worktree");
        assert_ne!(head, repo.join(".git").join("HEAD"));
        assert!(head.ends_with("HEAD") && head.starts_with(repo.join(".git")), "{head:?}");
        assert_eq!(branch(&Git::system(), &tree).as_deref(), Some("other"));
    }

    #[test]
    fn the_user_name_is_the_one_of_the_repository() {
        let sandbox = Sandbox::new();
        let repo = sandbox.repo("repo");
        run(&repo, &["config", "user.name", "  Luca Soda "]);
        run(&repo, &["config", "user.email", "luca@example.com"]);
        let name = user_name(&Git::system(), &repo);
        assert_eq!(name.as_deref(), Some("Luca Soda"), "trimmed, and never the email");
        assert_eq!(
            user_name(&Git::system().with_search_path(""), &repo),
            None,
            "git cannot be run"
        );
    }
}
