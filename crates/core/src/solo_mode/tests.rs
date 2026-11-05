use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::testing::*;
use super::*;

const BEGIN: &str = "# >>> weaver solo mode: /work/a";
const END: &str = "# <<< weaver solo mode: /work/a";

fn block(lines: &[&str]) -> String {
    let mut text = format!("{BEGIN}\n");
    for line in lines {
        text.push_str(line);
        text.push('\n');
    }
    text.push_str(END);
    text.push('\n');
    text
}

fn rewritten(content: &str, block: Option<&str>) -> String {
    String::from_utf8(rewrite(content.as_bytes(), "/work/a", block)).unwrap()
}

fn exclude_of(repo: &Path) -> PathBuf {
    repo.join(".git/info/exclude")
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap()
}

/// A project folder with `.weaver/` inside the repository at `repo`.
fn project(repo: &Path, rel: &str) -> PathBuf {
    let root = if rel.is_empty() {
        repo.to_path_buf()
    } else {
        repo.join(rel)
    };
    fs::create_dir_all(root.join(".weaver/model")).unwrap();
    root
}

fn marker(root: &Path) -> String {
    marker_of(root)
}

// --- the block, as bytes --------------------------------------------------------------------------

#[test]
fn the_lines_of_each_mode_start_at_the_root_of_the_repository() {
    assert_eq!(Mode::Off.lines(""), Vec::<String>::new());
    assert_eq!(
        Mode::Folders.lines(""),
        ["/.weaver/", "/.agents/skills/", "/.omp/"]
    );
    assert_eq!(
        Mode::Files.lines("packages/app/"),
        [
            "/packages/app/.weaver/",
            "/packages/app/.agents/skills/weaver-*/",
            "/packages/app/.omp/agents/weaver-*.md",
            "/packages/app/.omp/WATCHDOG.yml",
        ]
    );
}

#[test]
fn characters_special_to_git_patterns_in_the_prefix_are_escaped() {
    assert_eq!(escape_prefix("a*b?c[d]e\\f/"), "a\\*b\\?c\\[d]e\\\\f/");
    assert_eq!(escape_prefix("#top/"), "\\#top/");
    assert_eq!(escape_prefix("!top/"), "\\!top/");
    assert_eq!(escape_prefix("mid#dle!/"), "mid#dle!/");
    assert_eq!(escape_prefix("end  "), "end\\ \\ ");
    assert_eq!(escape_prefix(""), "");
}

#[test]
fn a_block_is_appended_after_an_lf_when_the_file_does_not_end_with_one() {
    let new = block(&["/.weaver/"]);
    assert_eq!(rewritten("", Some(&new)), new);
    assert_eq!(
        rewritten("# mine\nfoo\n", Some(&new)),
        format!("# mine\nfoo\n{new}")
    );
    assert_eq!(
        rewritten("# mine\nfoo", Some(&new)),
        format!("# mine\nfoo\n{new}")
    );
}

#[test]
fn the_block_is_replaced_in_place_and_every_other_byte_stays() {
    let old = block(&["/old/"]);
    let new = block(&["/.weaver/", "/.omp/"]);
    let others = "# team\r\n*.log\r\n\u{e9}\n# >>> weaver solo mode: /work/b\n/x/\n# <<< weaver solo mode: /work/b\n";
    for (before, after) in [("# top\n", "\n# end"), ("", ""), ("a", "")] {
        let content = format!("{before}{others}{old}tail\n{after}");
        let want = format!("{before}{others}{new}tail\n{after}");
        assert_eq!(rewritten(&content, Some(&new)), want);
    }
    // Bytes that are not UTF-8 stay as they were.
    let mut bytes = b"\xff\xfe\n".to_vec();
    bytes.extend_from_slice(old.as_bytes());
    bytes.extend_from_slice(b"\xc3\n");
    let mut want = b"\xff\xfe\n".to_vec();
    want.extend_from_slice(new.as_bytes());
    want.extend_from_slice(b"\xc3\n");
    assert_eq!(rewrite(&bytes, "/work/a", Some(&new)), want);
}

#[test]
fn the_same_block_again_changes_nothing() {
    let new = block(&["/.weaver/"]);
    for content in [
        String::new(),
        "x".to_owned(),
        format!("x\n{new}"),
        format!("{new}\nlast"),
    ] {
        let once = rewrite(content.as_bytes(), "/work/a", Some(&new));
        let twice = rewrite(&once, "/work/a", Some(&new));
        assert_eq!(once, twice);
    }
    let content = format!("a\n{new}z");
    assert_eq!(
        rewrite(content.as_bytes(), "/work/a", Some(&new)),
        content.as_bytes()
    );
}

#[test]
fn the_blocks_of_two_working_copies_never_touch_each_other() {
    let a = block(&["/a/"]);
    let b = "# >>> weaver solo mode: /work/b\n/b/\n# <<< weaver solo mode: /work/b\n";
    let content = format!("{a}{b}");
    let new = block(&["/new/"]);
    assert_eq!(rewritten(&content, Some(&new)), format!("{new}{b}"));
    assert_eq!(rewritten(&content, None), b);
    let other = rewrite(content.as_bytes(), "/work/b", None);
    assert_eq!(String::from_utf8(other).unwrap(), a);
}

#[test]
fn a_duplicate_block_is_removed_and_a_begin_line_without_an_end_goes_alone() {
    let old = block(&["/old/"]);
    let new = block(&["/new/"]);
    // The first run is replaced in place, the second one removed.
    assert_eq!(
        rewritten(&format!("1\n{old}2\n{old}3\n"), Some(&new)),
        format!("1\n{new}2\n3\n")
    );
    // An orphan begin line is removed as a single line; the block then goes at the end.
    assert_eq!(
        rewritten(&format!("1\n{BEGIN}\n/lost/\n2\n"), Some(&new)),
        format!("1\n/lost/\n2\n{new}")
    );
    // An orphan after a complete block goes too, the block stays where it was.
    assert_eq!(
        rewritten(&format!("{old}{BEGIN}\nz"), Some(&new)),
        format!("{new}z")
    );
    // An end line alone is another byte of the file.
    assert_eq!(
        rewritten(&format!("{END}\n"), Some(&new)),
        format!("{END}\n{new}")
    );
}

#[test]
fn removing_takes_every_run_and_nothing_else() {
    let old = block(&["/old/"]);
    assert_eq!(rewritten(&format!("1\n{old}2\n{old}3"), None), "1\n2\n3");
    // A file left empty stays empty.
    assert_eq!(rewritten(&old, None), "");
    // A begin line with no end is not a block: removal leaves it.
    let orphan = format!("1\n{BEGIN}\n2\n");
    assert_eq!(rewritten(&orphan, None), orphan);
    // Nothing to remove, nothing changes, final newline or not.
    assert_eq!(rewritten("a\nb", None), "a\nb");
}

// --- the state ------------------------------------------------------------------------------------

#[test]
fn no_state_file_means_off_without_a_diagnostic() {
    let sandbox = Sandbox::new();
    let root = sandbox.folder("p");
    assert_eq!(
        read_state(&root),
        State {
            mode: Mode::Off,
            diagnostic: None
        }
    );
}

#[test]
fn the_two_values_are_read_and_anything_else_is_off_with_a_diagnostic() {
    let sandbox = Sandbox::new();
    let root = sandbox.folder("p");
    for (text, mode) in [
        ("{\"mode\": \"folders\"}\n", Mode::Folders),
        ("{\"mode\":\"files\"}", Mode::Files),
        ("\u{feff}{\"mode\": \"files\"}\r\n", Mode::Files),
    ] {
        put(&root, ".weaver/solo/solo.json", text);
        assert_eq!(
            read_state(&root),
            State {
                mode,
                diagnostic: None
            },
            "{text}"
        );
    }
    for text in [
        "",
        "{",
        "[]",
        "{\"mode\": \"off\"}",
        "{\"mode\": \"all\"}",
        "{\"mode\": 1}",
        "{}",
    ] {
        put(&root, ".weaver/solo/solo.json", text);
        let state = read_state(&root);
        assert_eq!(state.mode, Mode::Off, "{text}");
        let diagnostic = state
            .diagnostic
            .unwrap_or_else(|| panic!("no diagnostic for {text:?}"));
        assert_eq!(diagnostic.path, "solo/solo.json");
        assert!(!diagnostic.message.is_empty());
    }
    // Not UTF-8.
    fs::write(root.join(".weaver/solo/solo.json"), b"\xff\xfe").unwrap();
    assert!(read_state(&root).diagnostic.is_some());
}

// --- git ------------------------------------------------------------------------------------------

#[test]
fn the_block_is_written_in_the_exclusions_of_the_repository_for_both_modes() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo("r");
    let git = Git::system();
    let location = locate(&git, &repo).unwrap();
    assert_eq!(location.exclude, exclude_of(&repo));
    assert_eq!(location.prefix, "");
    let original = read(&exclude_of(&repo));

    assert!(location.write_block(Mode::Folders).unwrap());
    let expected = format!(
        "# >>> weaver solo mode: {m}\n/.weaver/\n/.agents/skills/\n/.omp/\n# <<< weaver solo mode: {m}\n",
        m = marker(&repo)
    );
    let separator = if original.is_empty() || original.ends_with('\n') {
        ""
    } else {
        "\n"
    };
    assert_eq!(
        read(&exclude_of(&repo)),
        format!("{original}{separator}{expected}")
    );
    for ignored_path in [
        ".weaver/model/system.md",
        ".agents/skills/team/SKILL.md",
        ".omp/agents/mine.md",
        ".omp/settings.yml",
    ] {
        assert!(ignored(&repo, ignored_path), "{ignored_path}");
    }
    assert!(!ignored(&repo, "src/main.rs"));

    // The other mode rewrites the block in place.
    assert!(location.write_block(Mode::Files).unwrap());
    let expected = format!(
        "# >>> weaver solo mode: {m}\n/.weaver/\n/.agents/skills/weaver-*/\n/.omp/agents/weaver-*.md\n/.omp/WATCHDOG.yml\n# <<< weaver solo mode: {m}\n",
        m = marker(&repo)
    );
    assert_eq!(
        read(&exclude_of(&repo)),
        format!("{original}{separator}{expected}")
    );
    assert!(ignored(&repo, ".weaver/model/system.md"));
    assert!(ignored(&repo, ".agents/skills/weaver-design/SKILL.md"));
    assert!(ignored(&repo, ".omp/agents/weaver-analysis.md"));
    assert!(ignored(&repo, ".omp/WATCHDOG.yml"));
    assert!(!ignored(&repo, ".agents/skills/team/SKILL.md"));
    assert!(!ignored(&repo, ".omp/agents/mine.md"));
    assert!(!ignored(&repo, ".omp/settings.yml"));
}

#[test]
fn a_project_in_a_subfolder_excludes_only_its_own_files() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo("r");
    let root = project(&repo, "packages/app");
    let location = locate(&Git::system(), &root).unwrap();
    assert_eq!(location.prefix, "packages/app/");
    assert_eq!(location.exclude, exclude_of(&repo));
    location.write_block(Mode::Folders).unwrap();
    assert!(read(&exclude_of(&repo)).contains(
        "\n/packages/app/.weaver/\n/packages/app/.agents/skills/\n/packages/app/.omp/\n"
    ));
    assert!(ignored(&repo, "packages/app/.weaver/model/system.md"));
    assert!(ignored(&repo, "packages/app/.omp/x"));
    assert!(!ignored(&repo, ".weaver/model/system.md"));
    assert!(!ignored(&repo, "packages/other/.weaver/model/system.md"));
    assert!(!ignored(&repo, "packages/app/src/lib.rs"));
}

#[test]
fn a_prefix_with_special_characters_matches_only_its_own_folder() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo("r");
    let mut cases = vec![
        ("#hash", "#hasX"),
        ("!bang", "bang"),
        ("a[b]c", "abc"),
        ("back\\slash", "backslash"),
    ];
    if cfg!(unix) {
        cases.push(("star*", "starX"));
        cases.push(("q?", "qZ"));
    }
    for (name, lookalike) in cases {
        if name.contains('\\') && !cfg!(unix) {
            continue;
        }
        let root = project(&repo, name);
        let location = locate(&Git::system(), &root).unwrap();
        assert_eq!(location.prefix, format!("{name}/"));
        location.write_block(Mode::Files).unwrap();
        assert!(
            ignored(&repo, &format!("{name}/.weaver/model/system.md")),
            "{name}"
        );
        assert!(
            ignored(&repo, &format!("{name}/.omp/WATCHDOG.yml")),
            "{name}"
        );
        assert!(
            !ignored(&repo, &format!("{lookalike}/.weaver/model/system.md")),
            "{name}"
        );
        // The tracked files come back relative to the project, under the same prefix.
        put(&repo, &format!("{name}/.weaver/model/system.md"), "# S\n");
        track(&repo, &format!("{name}/.weaver/model/system.md"));
        assert_eq!(
            location.tracked_files(&Git::system(), Mode::Files).unwrap(),
            [".weaver/model/system.md"],
            "{name}"
        );
        assert!(location.remove_block().unwrap());
    }
}

#[test]
fn a_linked_worktree_writes_in_the_exclusions_of_the_main_repository() {
    let sandbox = Sandbox::new();
    let main = sandbox.repo("main");
    commit(&main);
    let linked = sandbox.dir.path().join("linked");
    git(&main, &["worktree", "add", "-q", linked.to_str().unwrap()]);
    let linked = crate::paths::canonical_root(&linked).unwrap();

    let git_ = Git::system();
    let here = locate(&git_, &main).unwrap();
    let there = locate(&git_, &linked).unwrap();
    assert_eq!(there.exclude, here.exclude);
    assert_eq!(there.exclude, exclude_of(&main));
    assert_eq!(there.prefix, "");

    // Two working copies, two blocks: each one writes and removes its own.
    here.write_block(Mode::Folders).unwrap();
    there.write_block(Mode::Files).unwrap();
    let text = read(&exclude_of(&main));
    assert!(text.contains(&format!("# >>> weaver solo mode: {}\n", marker(&main))));
    assert!(text.contains(&format!("# >>> weaver solo mode: {}\n", marker(&linked))));
    assert!(ignored(&linked, ".omp/WATCHDOG.yml"));
    assert!(ignored(&main, ".omp/anything"));

    assert!(there.remove_block().unwrap());
    let text = read(&exclude_of(&main));
    assert!(text.contains(&format!("# >>> weaver solo mode: {}\n", marker(&main))));
    assert!(!text.contains(&marker(&linked)));
    assert!(ignored(&main, ".omp/anything"));
    // Still excluded in the linked worktree: the shared file keeps a block that excludes it.
    assert!(ignored(&linked, ".omp/anything"));
}

#[test]
fn writing_the_same_block_again_does_not_touch_the_file() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo("r");
    let location = locate(&Git::system(), &repo).unwrap();
    assert!(location.write_block(Mode::Folders).unwrap());
    assert!(location.block_in_place(Mode::Folders).unwrap());
    assert!(!location.block_in_place(Mode::Files).unwrap());
    let before = fs::metadata(&location.exclude).unwrap().modified().unwrap();
    let text = read(&location.exclude);
    std::thread::sleep(Duration::from_millis(30));
    assert!(!location.write_block(Mode::Folders).unwrap());
    assert_eq!(read(&location.exclude), text);
    assert_eq!(
        fs::metadata(&location.exclude).unwrap().modified().unwrap(),
        before
    );
}

#[test]
fn a_missing_exclusions_file_and_its_info_folder_are_created_and_removal_leaves_an_empty_file() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo("r");
    fs::remove_dir_all(repo.join(".git/info")).unwrap();
    let location = locate(&Git::system(), &repo).unwrap();
    assert!(!location.block_in_place(Mode::Folders).unwrap());
    // Removing from a file that is not there creates nothing.
    assert!(!location.remove_block().unwrap());
    assert!(!location.exclude.exists());

    assert!(location.write_block(Mode::Folders).unwrap());
    assert!(location.exclude.is_file());
    assert!(location.remove_block().unwrap());
    assert_eq!(read(&location.exclude), "");
}

#[test]
fn a_file_with_other_lines_and_no_final_newline_keeps_them_byte_for_byte() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo("r");
    fs::write(exclude_of(&repo), "# mine\r\n*.log\nlast").unwrap();
    let location = locate(&Git::system(), &repo).unwrap();
    location.write_block(Mode::Folders).unwrap();
    let text = read(&exclude_of(&repo));
    assert!(text.starts_with("# mine\r\n*.log\nlast\n# >>> weaver solo mode: "));
    location.remove_block().unwrap();
    assert_eq!(read(&exclude_of(&repo)), "# mine\r\n*.log\nlast\n");
}

#[test]
fn without_git_the_reason_is_that_git_was_not_found() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo("r");
    let no_git = Git::system().with_search_path("");
    assert_eq!(locate(&no_git, &repo).unwrap_err(), Reason::NoGit);
    assert_eq!(Reason::NoGit.to_string(), "git was not found");
}

#[test]
fn a_folder_outside_a_repository_is_not_in_a_git_repository() {
    let sandbox = Sandbox::new();
    let plain = sandbox.folder("plain");
    let reason = locate(&Git::system(), &plain).unwrap_err();
    assert_eq!(reason, Reason::NotARepository);
    assert_eq!(reason.to_string(), "this folder is not in a git repository");
}

#[test]
fn a_search_path_without_git_starts_no_process_whatever_the_system_has() {
    let sandbox = Sandbox::new();
    let empty = sandbox.folder("empty-bin");
    assert_eq!(find_program("git", OsStr::new("")), None);
    assert_eq!(find_program("git", empty.as_os_str()), None);
    let repo = sandbox.repo("r");
    let no_git = Git::system().with_search_path(empty.as_os_str());
    assert_eq!(locate(&no_git, &repo).unwrap_err(), Reason::NoGit);
}

#[test]
fn a_program_is_found_in_the_first_folder_of_the_search_path_that_holds_it() {
    let sandbox = Sandbox::new();
    let empty = sandbox.folder("empty-bin");
    let first = sandbox.folder("first-bin");
    let second = sandbox.folder("second-bin");
    let name = if cfg!(windows) { "git.exe" } else { "git" };
    fs::write(first.join(name), "").unwrap();
    fs::write(second.join(name), "").unwrap();
    let path = std::env::join_paths([empty.clone(), first.clone(), second]).unwrap();
    let found = find_program("git", &path).expect("git is found in the search path");
    // On Windows the file name takes its extension from `PATHEXT` (`git.EXE`), and the file system
    // ignores the case.
    if cfg!(windows) {
        assert_eq!(
            found.to_string_lossy().to_lowercase(),
            first.join(name).to_string_lossy().to_lowercase()
        );
    } else {
        assert_eq!(found, first.join(name));
    }
}

// The fake `git` is a shell script, which Windows cannot start as `git`: Unix only.
#[cfg(unix)]
#[test]
fn a_git_call_over_its_limit_is_killed_and_says_so() {
    use std::os::unix::fs::PermissionsExt;

    let sandbox = Sandbox::new();
    let repo = sandbox.repo("r");
    let fake = sandbox.folder("fake-bin");
    let script = fake.join("git");
    fs::write(&script, "#!/bin/sh\nexec /bin/sleep 30\n").unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();

    let slow = Git::system()
        .with_search_path(fake.as_os_str())
        .with_limit(Duration::from_secs(1));
    let started = std::time::Instant::now();
    let reason = locate(&slow, &repo).unwrap_err();
    assert_eq!(reason, Reason::TimedOut(Duration::from_secs(1)));
    assert_eq!(reason.to_string(), "git did not answer within 1 s");
    assert!(started.elapsed() < Duration::from_secs(20));
    assert_eq!(
        Reason::TimedOut(GIT_LIMIT).to_string(),
        "git did not answer within 10 s"
    );
}

// The fake `git` is a shell script, which Windows cannot start as `git`: Unix only.
#[cfg(unix)]
#[test]
fn another_git_failure_gives_the_first_line_of_its_error_output() {
    use std::os::unix::fs::PermissionsExt;

    let sandbox = Sandbox::new();
    let repo = sandbox.repo("r");
    let fake = sandbox.folder("fake-bin");
    let script = fake.join("git");
    fs::write(
        &script,
        "#!/bin/sh\necho 'fatal: first problem' >&2\necho second >&2\nexit 3\n",
    )
    .unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    let reason = locate(&Git::system().with_search_path(fake.as_os_str()), &repo).unwrap_err();
    assert_eq!(reason.to_string(), "git failed: fatal: first problem");
}

#[test]
fn the_reasons_read_as_the_texts_of_the_requirements() {
    assert_eq!(
        Reason::CouldNotWrite {
            path: "/r/.git/info/exclude".into(),
            error: "Permission denied (os error 13)".into()
        }
        .to_string(),
        "could not write /r/.git/info/exclude: Permission denied (os error 13)"
    );
    assert!(Reason::NoGit.is_unavailable() && Reason::NotARepository.is_unavailable());
    assert!(!Reason::CouldNotWrite {
        path: String::new(),
        error: String::new()
    }
    .is_unavailable());
}

#[test]
fn the_tracked_files_are_the_ones_the_block_matches_for_each_mode() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo("r");
    let root = project(&repo, "");
    let tracked = [
        ".weaver/model/system.md",
        ".agents/skills/weaver-design/SKILL.md",
        ".agents/skills/team/SKILL.md",
        ".omp/agents/weaver-analysis.md",
        ".omp/agents/mine.md",
        ".omp/WATCHDOG.yml",
        ".omp/settings.yml",
        "src/main.rs",
        "notes.md",
    ];
    for rel in tracked {
        put(&root, rel, "x\n");
        track(&repo, rel);
    }
    let git_ = Git::system();
    let location = locate(&git_, &root).unwrap();

    assert_eq!(
        location.tracked_files(&git_, Mode::Off).unwrap(),
        Vec::<String>::new()
    );
    let mut folders: Vec<String> = tracked[..7].iter().map(|s| (*s).to_owned()).collect();
    folders.sort();
    assert_eq!(
        location.tracked_files(&git_, Mode::Folders).unwrap(),
        folders
    );
    // Only what Weaver writes in the folders of the agent: the team's files there are left out.
    let mut files: Vec<String> = [tracked[0], tracked[1], tracked[3], tracked[5]]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
    files.sort();
    assert_eq!(location.tracked_files(&git_, Mode::Files).unwrap(), files);
    assert!(!files
        .iter()
        .any(|f| f.contains("team") || f.contains("mine")));
}

#[test]
fn the_tracked_files_of_a_subfolder_project_are_relative_to_it() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo("r");
    let root = project(&repo, "pkg/app");
    for rel in [
        "pkg/app/.weaver/model/system.md",
        "pkg/app/.omp/WATCHDOG.yml",
        "pkg/other/.weaver/model/system.md",
        ".weaver/model/system.md",
    ] {
        put(&repo, rel, "x\n");
        track(&repo, rel);
    }
    let git_ = Git::system();
    let location = locate(&git_, &root).unwrap();
    assert_eq!(
        location.tracked_files(&git_, Mode::Folders).unwrap(),
        [".omp/WATCHDOG.yml", ".weaver/model/system.md"]
    );
}

// --- the operations -------------------------------------------------------------------------------

#[test]
fn turning_on_writes_the_block_then_the_state_and_turning_off_removes_both() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo("r");
    let root = project(&repo, "");
    fs::write(exclude_of(&repo), "mine\n").unwrap();
    let git_ = Git::system();

    let on = set_mode(&git_, &root, Mode::Folders).unwrap();
    assert_eq!(on.mode, Mode::Folders);
    assert_eq!((on.unavailable.clone(), on.problem.clone()), (None, None));
    assert!(on.tracked.is_empty());
    assert_eq!(
        read(&root.join(".weaver/solo/solo.json")),
        "{\"mode\": \"folders\"}\n"
    );
    assert_eq!(read(&root.join(".weaver/solo/.gitignore")), "*\n");
    assert!(read(&exclude_of(&repo)).starts_with("mine\n# >>> weaver solo mode: "));
    assert_eq!(read_state(&root).mode, Mode::Folders);

    // The other mode rewrites the block, then the state.
    let other = set_mode(&git_, &root, Mode::Files).unwrap();
    assert_eq!(other.mode, Mode::Files);
    assert_eq!(
        read(&root.join(".weaver/solo/solo.json")),
        "{\"mode\": \"files\"}\n"
    );
    assert!(read(&exclude_of(&repo)).contains("weaver-*"));

    let off = set_mode(&git_, &root, Mode::Off).unwrap();
    assert_eq!(off.mode, Mode::Off);
    assert!(!root.join(".weaver/solo").exists());
    assert_eq!(read(&exclude_of(&repo)), "mine\n");
    assert!(root.join(".weaver/model").is_dir());
    // Turning off what is off changes nothing and never fails, whatever git does.
    let again = set_mode(&Git::system().with_search_path(""), &root, Mode::Off);
    assert_eq!(again.unwrap().mode, Mode::Off);
}

#[test]
fn turning_on_with_tracked_files_succeeds_and_lists_them() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo("r");
    let root = project(&repo, "");
    put(&root, ".weaver/model/system.md", "# S\n");
    track(&repo, ".weaver/model/system.md");
    put(&root, ".omp/agents/mine.md", "x\n");
    track(&repo, ".omp/agents/mine.md");
    let index_before = fs::read(repo.join(".git/index")).unwrap();

    let git_ = Git::system();
    let on = set_mode(&git_, &root, Mode::Files).unwrap();
    assert_eq!(on.tracked, [".weaver/model/system.md"]);
    assert_eq!(
        tracked_files(&git_, &root).unwrap(),
        [".weaver/model/system.md"]
    );
    // The index is never touched.
    assert_eq!(fs::read(repo.join(".git/index")).unwrap(), index_before);

    let more = set_mode(&git_, &root, Mode::Folders).unwrap();
    assert_eq!(
        more.tracked,
        [".omp/agents/mine.md", ".weaver/model/system.md"]
    );
}

#[test]
fn a_change_that_fails_changes_nothing_and_says_why() {
    let sandbox = Sandbox::new();
    let git_ = Git::system();

    // Outside a repository.
    let plain = project(&sandbox.folder("plain"), "");
    let reason = set_mode(&git_, &plain, Mode::Folders).unwrap_err();
    assert_eq!(reason, Reason::NotARepository);
    assert!(!plain.join(".weaver/solo").exists());

    // Without git.
    let repo = sandbox.repo("r");
    let root = project(&repo, "");
    let before = read(&exclude_of(&repo));
    let reason = set_mode(&Git::system().with_search_path(""), &root, Mode::Files).unwrap_err();
    assert_eq!(reason, Reason::NoGit);
    assert!(!root.join(".weaver/solo").exists());
    assert_eq!(read(&exclude_of(&repo)), before);

    // The exclusions cannot be written: a folder is in their place.
    fs::remove_file(exclude_of(&repo)).unwrap();
    fs::create_dir(exclude_of(&repo)).unwrap();
    let reason = set_mode(&git_, &root, Mode::Folders).unwrap_err();
    assert!(matches!(reason, Reason::CouldNotWrite { .. }), "{reason:?}");
    assert!(reason.to_string().starts_with("could not write "));
    assert!(!root.join(".weaver/solo").exists());

    // Not a project: nothing is created in the folder.
    let bare = sandbox.repo("bare");
    assert_eq!(
        set_mode(&git_, &bare, Mode::Folders).unwrap_err(),
        Reason::NotAProject
    );
    assert!(!bare.join(".weaver").exists());
}

#[test]
fn a_change_that_fails_while_on_keeps_the_earlier_state() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo("r");
    let root = project(&repo, "");
    let git_ = Git::system();
    set_mode(&git_, &root, Mode::Folders).unwrap();
    let state = read(&root.join(".weaver/solo/solo.json"));
    let exclude = read(&exclude_of(&repo));

    assert_eq!(
        set_mode(&Git::system().with_search_path(""), &root, Mode::Off).unwrap_err(),
        Reason::NoGit
    );
    assert_eq!(
        set_mode(&Git::system().with_search_path(""), &root, Mode::Files).unwrap_err(),
        Reason::NoGit
    );
    assert_eq!(read(&root.join(".weaver/solo/solo.json")), state);
    assert_eq!(read(&exclude_of(&repo)), exclude);
    assert_eq!(read_state(&root).mode, Mode::Folders);
}

#[test]
fn inspect_reports_the_state_the_tracked_files_and_why_the_block_is_not_in_place() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo("r");
    let root = project(&repo, "");
    let git_ = Git::system();

    let off = inspect(&git_, &root);
    assert_eq!(
        off,
        Inspection {
            mode: Mode::Off,
            unavailable: None,
            problem: None,
            tracked: vec![],
            diagnostic: None
        }
    );

    set_mode(&git_, &root, Mode::Folders).unwrap();
    let on = inspect(&git_, &root);
    assert_eq!(
        (on.mode, on.unavailable, on.problem),
        (Mode::Folders, None, None)
    );

    // The block is gone: the command line says so and never repairs it.
    let location = locate(&git_, &root).unwrap();
    location.remove_block().unwrap();
    let broken = inspect(&git_, &root);
    assert!(
        matches!(broken.problem, Some(Reason::BlockMissing { .. })),
        "{:?}",
        broken.problem
    );
    assert!(!location.block_in_place(Mode::Folders).unwrap());

    // No git: unavailable, and the problem is the same reason while it is on.
    let none = inspect(&Git::system().with_search_path(""), &root);
    assert_eq!(none.unavailable, Some(Reason::NoGit));
    assert_eq!(none.problem, Some(Reason::NoGit));
    assert_eq!(none.mode, Mode::Folders);
}

#[test]
fn a_state_file_that_cannot_be_read_counts_as_off_and_is_rewritten_by_the_next_change() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo("r");
    let root = project(&repo, "");
    put(&root, ".weaver/solo/solo.json", "garbage");
    let git_ = Git::system();
    let seen = inspect(&git_, &root);
    assert_eq!(seen.mode, Mode::Off);
    assert!(seen.diagnostic.is_some());
    assert_eq!(read(&root.join(".weaver/solo/solo.json")), "garbage");

    set_mode(&git_, &root, Mode::Files).unwrap();
    assert_eq!(
        read(&root.join(".weaver/solo/solo.json")),
        "{\"mode\": \"files\"}\n"
    );
    assert!(inspect(&git_, &root).diagnostic.is_none());
}

#[test]
fn opening_writes_a_missing_or_different_block_silently_and_only_then() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo("r");
    let root = project(&repo, "");
    let git_ = Git::system();
    set_mode(&git_, &root, Mode::Files).unwrap();
    let location = locate(&git_, &root).unwrap();

    let calm = open(&git_, &root);
    assert_eq!(
        calm,
        Opening {
            unavailable: None,
            problem: None,
            rewritten: false
        }
    );

    location.remove_block().unwrap();
    let repaired = open(&git_, &root);
    assert_eq!(
        repaired,
        Opening {
            unavailable: None,
            problem: None,
            rewritten: true
        }
    );
    assert!(location.block_in_place(Mode::Files).unwrap());

    // A different block (the other mode's) is written again for the remembered mode.
    location.write_block(Mode::Folders).unwrap();
    assert!(open(&git_, &root).rewritten);
    assert!(location.block_in_place(Mode::Files).unwrap());

    // A duplicate of the block too.
    let text = read(&location.exclude);
    let block = &text[text.find("# >>> weaver solo mode").unwrap()..];
    fs::write(&location.exclude, format!("{text}{block}")).unwrap();
    assert!(open(&git_, &root).rewritten);
    assert_eq!(read(&location.exclude), text);
}

#[test]
fn opening_with_solo_mode_off_writes_nothing_and_says_whether_it_can_change() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo("r");
    let root = project(&repo, "");
    let before = read(&exclude_of(&repo));
    assert_eq!(
        open(&Git::system(), &root),
        Opening {
            unavailable: None,
            problem: None,
            rewritten: false
        }
    );
    assert_eq!(read(&exclude_of(&repo)), before);

    let plain = sandbox.folder("plain");
    let opening = open(&Git::system(), &plain);
    assert_eq!(opening.unavailable, Some(Reason::NotARepository));
    assert_eq!(opening.problem, None);
}

#[test]
fn opening_without_git_keeps_solo_mode_on_and_tries_again_the_next_time() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo("r");
    let root = project(&repo, "");
    set_mode(&Git::system(), &root, Mode::Folders).unwrap();
    let location = locate(&Git::system(), &root).unwrap();
    location.remove_block().unwrap();

    let opening = open(&Git::system().with_search_path(""), &root);
    assert_eq!(opening.unavailable, Some(Reason::NoGit));
    assert_eq!(opening.problem, Some(Reason::NoGit));
    assert_eq!(read_state(&root).mode, Mode::Folders);
    assert!(!location.block_in_place(Mode::Folders).unwrap());

    assert!(open(&Git::system(), &root).rewritten);
    assert!(location.block_in_place(Mode::Folders).unwrap());
}

#[test]
fn opening_that_cannot_write_the_block_reports_the_problem_and_keeps_the_mode() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo("r");
    let root = project(&repo, "");
    set_mode(&Git::system(), &root, Mode::Folders).unwrap();
    fs::remove_file(exclude_of(&repo)).unwrap();
    fs::create_dir(exclude_of(&repo)).unwrap();
    let opening = open(&Git::system(), &root);
    assert_eq!(opening.unavailable, None);
    assert!(matches!(
        opening.problem,
        Some(Reason::CouldNotWrite { .. })
    ));
    assert_eq!(read_state(&root).mode, Mode::Folders);
}

#[test]
fn the_block_of_a_vanished_project_is_removed_by_its_markers_alone() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo("r");
    let root = project(&repo, "");
    fs::write(exclude_of(&repo), "mine\n").unwrap();
    set_mode(&Git::system(), &root, Mode::Files).unwrap();
    fs::remove_dir_all(root.join(".weaver")).unwrap();

    assert!(remove_block_of(&Git::system(), &root).unwrap());
    assert_eq!(read(&exclude_of(&repo)), "mine\n");
    assert!(!remove_block_of(&Git::system(), &root).unwrap());
}

#[test]
fn placing_a_block_before_initialising_can_be_undone() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo("r");
    let before = read(&exclude_of(&repo));
    let location = place_block(&Git::system(), &repo, Mode::Folders).unwrap();
    assert!(location.block_in_place(Mode::Folders).unwrap());
    location.remove_block().unwrap();
    assert_eq!(read(&exclude_of(&repo)), before);
    assert!(!repo.join(".weaver").exists());
}

// --- while a merge is in progress ---------------------------------------------------------------

#[test]
fn solo_mode_cannot_change_while_a_merge_is_in_progress_and_nothing_is_touched() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo("a");
    let root = project(&repo, "");
    let git = Git::system();
    set_mode(&git, &root, Mode::Folders).unwrap();
    let before = read(&exclude_of(&repo));
    let state = read(&state_path(&root));

    fs::create_dir_all(root.join(".weaver/merge")).unwrap();
    fs::write(root.join(".weaver/merge/merge.json"), "{}\n").unwrap();
    for mode in [Mode::Off, Mode::Files] {
        assert_eq!(
            set_mode(&git, &root, mode).unwrap_err(),
            Reason::MergeInProgress,
            "{mode:?}"
        );
        assert_eq!(read(&exclude_of(&repo)), before, "{mode:?}");
        assert_eq!(read(&state_path(&root)), state, "{mode:?}");
    }
    assert_eq!(
        Reason::MergeInProgress.to_string(),
        "Finish or abort the merge in progress first."
    );
    // It is checked before git is called: without git the reason is still the merge's.
    assert_eq!(
        set_mode(&git.clone().with_search_path(""), &root, Mode::Off).unwrap_err(),
        Reason::MergeInProgress
    );
    // The mode already chosen is no change.
    assert_eq!(
        set_mode(&git, &root, Mode::Folders).unwrap().mode,
        Mode::Folders
    );

    fs::remove_file(root.join(".weaver/merge/merge.json")).unwrap();
    assert_eq!(set_mode(&git, &root, Mode::Off).unwrap().mode, Mode::Off);
    assert!(!read(&exclude_of(&repo)).contains("weaver solo mode"));
}
