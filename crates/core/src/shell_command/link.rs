//! macOS and Linux: the `weaver` command is a symbolic link to Weaver's stable copy of the command
//! line (`<app local data>/bin/weaver`), at a place on the terminal's `PATH`.
//!
//! The link is made by the user when the folder allows it. On macOS, when `/usr/local/bin` is
//! missing or not writable, the same step runs with administrator rights through
//! `/usr/bin/osascript` ([`Osascript`]); a password request the developer cancels is not a failure.

use std::ffi::OsString;
use std::io;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use super::{ShellCommandOutcome, ShellCommandResult};
use crate::process;

/// The step to run with administrator rights.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Elevated<'a> {
    /// Create `parent` when missing, then the link `link` to `copy`.
    Link {
        parent: &'a Path,
        copy: &'a Path,
        link: &'a Path,
    },
    /// Delete the link `link`.
    Unlink { link: &'a Path },
}

/// Why an elevated step did not happen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ElevatedFailure {
    /// The developer cancelled the request for the administrator password.
    Cancelled,
    /// The step failed; the reason is the system's own words.
    Failed(String),
}

/// Runs a step with administrator rights.
pub trait Elevator {
    fn run(&self, step: Elevated<'_>) -> Result<(), ElevatedFailure>;
}

/// The elevator of macOS: `do shell script … with administrator privileges` through osascript.
pub struct Osascript;

/// The program that runs the AppleScript.
pub const OSASCRIPT: &str = "/usr/bin/osascript";

/// The AppleScript of a step. The paths are never part of the text: they are the arguments of
/// `on run argv`, each quoted with `quoted form of`.
pub fn script(step: &Elevated<'_>) -> [&'static str; 3] {
    let body = match step {
        Elevated::Link { .. } => {
            "do shell script \"/bin/mkdir -p -- \" & quoted form of (item 1 of argv) & \" && /bin/ln -s -- \" & quoted form of (item 2 of argv) & \" \" & quoted form of (item 3 of argv) with administrator privileges"
        }
        Elevated::Unlink { .. } => {
            "do shell script \"/bin/rm -- \" & quoted form of (item 1 of argv) with administrator privileges"
        }
    };
    ["on run argv", body, "end run"]
}

/// The arguments of the script of a step, in the order the script reads them.
fn script_arguments(step: &Elevated<'_>) -> Vec<OsString> {
    match step {
        Elevated::Link { parent, copy, link } => vec![
            parent.as_os_str().to_owned(),
            copy.as_os_str().to_owned(),
            link.as_os_str().to_owned(),
        ],
        Elevated::Unlink { link } => vec![link.as_os_str().to_owned()],
    }
}

/// What osascript's failure means: the AppleScript error `-128` is the developer's cancel.
pub fn classify_failure(stderr: &str) -> ElevatedFailure {
    let text = stderr.trim();
    if text.contains("(-128)") {
        return ElevatedFailure::Cancelled;
    }
    let reason = text
        .strip_prefix("execution error: ")
        .unwrap_or(text)
        .trim();
    ElevatedFailure::Failed(if reason.is_empty() {
        "the administrator step failed".to_owned()
    } else {
        reason.to_owned()
    })
}

impl Elevator for Osascript {
    fn run(&self, step: Elevated<'_>) -> Result<(), ElevatedFailure> {
        let [open, body, close] = script(&step);
        let mut command = process::command(OSASCRIPT, ["-e", open, "-e", body, "-e", close]);
        command.args(script_arguments(&step));
        let output = command
            .output()
            .map_err(|error| ElevatedFailure::Failed(format!("cannot run {OSASCRIPT}: {error}")))?;
        if output.status.success() {
            Ok(())
        } else {
            Err(classify_failure(&String::from_utf8_lossy(&output.stderr)))
        }
    }
}

/// What a place holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Found {
    /// Nothing.
    Vacant,
    /// A symbolic link that resolves to Weaver's copy.
    Ours,
    /// A file, a folder or a link that does not resolve to the copy.
    Other,
}

/// The command at its place on a Unix system.
pub struct Link {
    /// The link: `/usr/local/bin/weaver` or `~/.local/bin/weaver`.
    pub place: PathBuf,
    /// How the place is said to the developer.
    pub display: String,
    /// Weaver's stable copy of the command line.
    pub copy: PathBuf,
    /// Who makes the link when the user cannot; none on Linux, which never asks for privileges.
    pub elevator: Option<Box<dyn Elevator>>,
    /// A folder that should be on the app's `PATH` once installed, and that `PATH`: the result
    /// carries the warning when it is not.
    pub path_check: Option<(PathBuf, Option<OsString>)>,
}

impl Link {
    /// Whether `dir` is one of the entries of `path_var`.
    pub fn dir_is_on_path(dir: &Path, path_var: Option<&OsString>) -> bool {
        path_var.is_some_and(|value| std::env::split_paths(value).any(|entry| entry == dir))
    }

    fn found(&self) -> io::Result<Found> {
        match std::fs::symlink_metadata(&self.place) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Found::Vacant),
            Err(error) => Err(error),
            Ok(meta) if meta.file_type().is_symlink() && self.points_to_copy() => Ok(Found::Ours),
            Ok(_) => Ok(Found::Other),
        }
    }

    /// Whether the link at the place resolves to the copy. A link whose target is missing (the
    /// copy is being renewed) counts when it names the copy.
    fn points_to_copy(&self) -> bool {
        if let (Ok(link), Ok(copy)) = (
            std::fs::canonicalize(&self.place),
            std::fs::canonicalize(&self.copy),
        ) {
            return link == copy;
        }
        match std::fs::read_link(&self.place) {
            Ok(target) => {
                let target = match self.place.parent() {
                    Some(parent) => parent.join(target),
                    None => target,
                };
                target == self.copy
            }
            Err(_) => false,
        }
    }

    fn result(&self, outcome: ShellCommandOutcome, path_warning: bool) -> ShellCommandResult {
        ShellCommandResult {
            installed: matches!(self.found(), Ok(Found::Ours)),
            place: self.display.clone(),
            outcome,
            path_warning,
        }
    }

    fn failed(&self, reason: impl std::fmt::Display) -> ShellCommandResult {
        self.result(
            ShellCommandOutcome::Failed {
                reason: reason.to_string(),
            },
            false,
        )
    }

    fn refused(&self) -> ShellCommandResult {
        self.result(
            ShellCommandOutcome::Refused {
                found: self.place.display().to_string(),
            },
            false,
        )
    }

    fn elevated(&self, step: Elevated<'_>) -> ShellCommandResult {
        let Some(elevator) = &self.elevator else {
            return self.failed("administrator rights are not available");
        };
        match elevator.run(step) {
            Ok(()) => self.result(ShellCommandOutcome::Done, self.warns_about_path()),
            Err(ElevatedFailure::Cancelled) => self.result(ShellCommandOutcome::Cancelled, false),
            Err(ElevatedFailure::Failed(reason)) => self.failed(reason),
        }
    }

    fn warns_about_path(&self) -> bool {
        self.path_check
            .as_ref()
            .is_some_and(|(dir, path_var)| !Self::dir_is_on_path(dir, path_var.as_ref()))
    }

    /// The current state: installed when the place is a link that resolves to the copy.
    pub fn status(&self) -> ShellCommandResult {
        self.result(ShellCommandOutcome::Done, false)
    }

    /// Makes the link. Refuses, touching nothing, when the place holds anything that is not it.
    pub fn install(&self) -> ShellCommandResult {
        match self.found() {
            Err(error) => return self.failed(error),
            Ok(Found::Other) => return self.refused(),
            Ok(Found::Ours) => return self.result(ShellCommandOutcome::Done, false),
            Ok(Found::Vacant) => {}
        }
        if !self.copy.exists() {
            return self.failed(format!(
                "Weaver's copy of the command line is missing at {}",
                self.copy.display()
            ));
        }
        let Some(parent) = self.place.parent() else {
            return self.failed("the place has no folder");
        };
        match self.link_directly(parent) {
            Ok(()) => self.result(ShellCommandOutcome::Done, self.warns_about_path()),
            Err(error) if self.elevator.is_some() && needs_rights(&error) => {
                self.elevated(Elevated::Link {
                    parent,
                    copy: &self.copy,
                    link: &self.place,
                })
            }
            Err(error) => self.failed(error),
        }
    }

    /// Creates the folder when missing and the link; when the link fails, the folders made here
    /// are taken away again.
    fn link_directly(&self, parent: &Path) -> io::Result<()> {
        let missing: Vec<&Path> = parent.ancestors().take_while(|dir| !dir.exists()).collect();
        let made = std::fs::create_dir_all(parent).and_then(|()| symlink(&self.copy, &self.place));
        if made.is_err() {
            for dir in &missing {
                // The deepest first; one that is not empty or not ours stays.
                let _ = std::fs::remove_dir(dir);
            }
        }
        made
    }

    /// Deletes the link, only while it still resolves to the copy.
    pub fn remove(&self) -> ShellCommandResult {
        match self.found() {
            Err(error) => self.failed(error),
            Ok(Found::Other) => self.refused(),
            Ok(Found::Vacant) => self.result(ShellCommandOutcome::Done, false),
            Ok(Found::Ours) => match std::fs::remove_file(&self.place) {
                Ok(()) => self.result(ShellCommandOutcome::Done, false),
                Err(error) if self.elevator.is_some() && needs_rights(&error) => {
                    self.elevated(Elevated::Unlink { link: &self.place })
                }
                Err(error) => self.failed(error),
            },
        }
    }
}

/// Whether the system refused for lack of rights (or a read-only system volume), the cases
/// the administrator step is for.
fn needs_rights(error: &io::Error) -> bool {
    matches!(
        error.raw_os_error(),
        Some(libc::EACCES | libc::EPERM | libc::EROFS)
    )
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::os::unix::fs::PermissionsExt;
    use std::rc::Rc;

    use super::*;

    /// An elevator that acts as the administrator would: it opens the folder it is asked about
    /// for the step, then closes it again.
    struct Fake {
        calls: RefCell<Vec<String>>,
        answer: Result<(), ElevatedFailure>,
    }

    impl Fake {
        fn new(answer: Result<(), ElevatedFailure>) -> Rc<Self> {
            Rc::new(Self {
                calls: RefCell::new(Vec::new()),
                answer,
            })
        }
    }

    impl Elevator for Rc<Fake> {
        fn run(&self, step: Elevated<'_>) -> Result<(), ElevatedFailure> {
            match step {
                Elevated::Link { parent, copy, link } => {
                    self.calls
                        .borrow_mut()
                        .push(format!("link {}", link.display()));
                    if self.answer.is_ok() {
                        std::fs::create_dir_all(parent).unwrap();
                        set_mode(parent, 0o755);
                        symlink(copy, link).unwrap();
                        set_mode(parent, 0o555);
                    }
                }
                Elevated::Unlink { link } => {
                    self.calls
                        .borrow_mut()
                        .push(format!("unlink {}", link.display()));
                    if self.answer.is_ok() {
                        let parent = link.parent().unwrap();
                        set_mode(parent, 0o755);
                        std::fs::remove_file(link).unwrap();
                        set_mode(parent, 0o555);
                    }
                }
            }
            self.answer.clone()
        }
    }

    struct Setup {
        _dir: tempfile::TempDir,
        copy: PathBuf,
        bin: PathBuf,
    }

    fn setup() -> Setup {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let copy = root.join("data").join("bin").join("weaver");
        std::fs::create_dir_all(copy.parent().unwrap()).unwrap();
        std::fs::write(&copy, b"#!/bin/sh\n").unwrap();
        let bin = root.join("home").join(".local").join("bin");
        Setup {
            _dir: dir,
            copy,
            bin,
        }
    }

    fn user_link(setup: &Setup, path_var: Option<&str>) -> Link {
        Link {
            place: setup.bin.join("weaver"),
            display: "~/.local/bin/weaver".to_owned(),
            copy: setup.copy.clone(),
            elevator: None,
            path_check: Some((setup.bin.clone(), path_var.map(OsString::from))),
        }
    }

    fn with_elevator(setup: &Setup, fake: &Rc<Fake>) -> Link {
        Link {
            place: setup.bin.join("weaver"),
            display: "/usr/local/bin/weaver".to_owned(),
            copy: setup.copy.clone(),
            elevator: Some(Box::new(fake.clone())),
            path_check: None,
        }
    }

    fn set_mode(dir: &Path, mode: u32) {
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(mode)).unwrap();
    }

    /// The folder, read-only until dropped: the user cannot write in it.
    struct ReadOnly<'a>(&'a Path);

    impl<'a> ReadOnly<'a> {
        fn new(dir: &'a Path) -> Self {
            std::fs::create_dir_all(dir).unwrap();
            set_mode(dir, 0o555);
            Self(dir)
        }
    }

    impl Drop for ReadOnly<'_> {
        fn drop(&mut self) {
            set_mode(self.0, 0o755);
        }
    }

    /// Running as root, a folder stays writable: the cases of missing rights cannot be made.
    fn is_root() -> bool {
        unsafe { libc::geteuid() == 0 }
    }

    #[test]
    fn installing_makes_the_folder_and_a_link_that_resolves_to_the_copy() {
        let setup = setup();
        let link = user_link(&setup, Some("/usr/bin"));
        assert!(!link.status().installed);

        let result = link.install();
        assert_eq!(result.outcome, ShellCommandOutcome::Done);
        assert!(result.installed);
        assert_eq!(result.place, "~/.local/bin/weaver");
        assert!(result.path_warning, "the folder is not on the PATH");
        assert_eq!(std::fs::read_link(&link.place).unwrap(), setup.copy);
        assert!(link.status().installed);
    }

    #[test]
    fn the_path_warning_is_only_for_a_folder_that_is_not_on_the_path() {
        let setup = setup();
        let on_path = format!("/usr/bin:{}", setup.bin.display());
        let result = user_link(&setup, Some(&on_path)).install();
        assert_eq!(result.outcome, ShellCommandOutcome::Done);
        assert!(!result.path_warning);

        let other = self::setup();
        assert!(user_link(&other, None).install().path_warning);
    }

    #[test]
    fn installing_again_changes_nothing() {
        let setup = setup();
        let link = user_link(&setup, None);
        link.install();
        let again = link.install();
        assert_eq!(again.outcome, ShellCommandOutcome::Done);
        assert!(again.installed);
        assert_eq!(std::fs::read_link(&link.place).unwrap(), setup.copy);
    }

    #[test]
    fn a_link_that_survives_the_renewal_of_the_copy_stays_installed() {
        let setup = setup();
        let link = user_link(&setup, None);
        link.install();
        std::fs::remove_file(&setup.copy).unwrap();
        assert!(link.status().installed, "dangling, but it names the copy");
        std::fs::write(&setup.copy, b"new").unwrap();
        assert!(link.status().installed);
    }

    #[test]
    fn installing_refuses_a_file_a_folder_or_another_link_and_touches_nothing() {
        let setup = setup();
        std::fs::create_dir_all(&setup.bin).unwrap();
        let place = setup.bin.join("weaver");
        let link = user_link(&setup, None);

        std::fs::write(&place, b"mine").unwrap();
        let result = link.install();
        assert_eq!(
            result.outcome,
            ShellCommandOutcome::Refused {
                found: place.display().to_string()
            }
        );
        assert!(!result.installed);
        assert_eq!(std::fs::read(&place).unwrap(), b"mine");

        std::fs::remove_file(&place).unwrap();
        std::fs::create_dir(&place).unwrap();
        assert!(matches!(
            link.install().outcome,
            ShellCommandOutcome::Refused { .. }
        ));
        assert!(place.is_dir());
        std::fs::remove_dir(&place).unwrap();

        let other = setup.bin.join("other");
        std::fs::write(&other, b"x").unwrap();
        symlink(&other, &place).unwrap();
        assert!(matches!(
            link.install().outcome,
            ShellCommandOutcome::Refused { .. }
        ));
        assert_eq!(std::fs::read_link(&place).unwrap(), other);
        assert!(!link.status().installed);
    }

    #[test]
    fn removing_deletes_the_link_only_while_it_resolves_to_the_copy() {
        let setup = setup();
        let link = user_link(&setup, None);
        link.install();
        let removed = link.remove();
        assert_eq!(removed.outcome, ShellCommandOutcome::Done);
        assert!(!removed.installed);
        assert!(std::fs::symlink_metadata(&link.place).is_err());
        assert!(setup.copy.exists(), "the copy is never removed");

        // Something else took the place: nothing is touched and its path is reported.
        std::fs::write(&link.place, b"mine").unwrap();
        let refused = link.remove();
        assert_eq!(
            refused.outcome,
            ShellCommandOutcome::Refused {
                found: link.place.display().to_string()
            }
        );
        assert_eq!(std::fs::read(&link.place).unwrap(), b"mine");
    }

    #[test]
    fn removing_when_the_place_is_already_empty_is_done() {
        let setup = setup();
        let result = user_link(&setup, None).remove();
        assert_eq!(result.outcome, ShellCommandOutcome::Done);
        assert!(!result.installed);
    }

    #[test]
    fn a_missing_copy_fails_the_installation_and_leaves_nothing() {
        let setup = setup();
        std::fs::remove_file(&setup.copy).unwrap();
        let result = user_link(&setup, None).install();
        assert!(matches!(result.outcome, ShellCommandOutcome::Failed { .. }));
        assert!(!setup.bin.exists());
    }

    #[test]
    fn a_folder_that_cannot_be_made_fails_with_the_reason_and_leaves_nothing() {
        let setup = setup();
        // `home/file` is a regular file: `home/file/bin` cannot be made.
        let file = setup.bin.parent().unwrap().join("file");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, b"x").unwrap();
        let link = Link {
            place: file.join("bin").join("weaver"),
            ..user_link(&setup, None)
        };
        let result = link.install();
        assert!(matches!(result.outcome, ShellCommandOutcome::Failed { .. }));
        assert!(!result.installed);
        assert_eq!(std::fs::read(&file).unwrap(), b"x");
    }

    #[test]
    fn an_unwritable_folder_without_an_elevator_fails_with_the_reason() {
        if is_root() {
            return;
        }
        let setup = setup();
        let _locked = ReadOnly::new(&setup.bin);
        let result = user_link(&setup, None).install();
        assert!(matches!(result.outcome, ShellCommandOutcome::Failed { .. }));
        assert!(!result.installed);
    }

    #[test]
    fn an_unwritable_folder_asks_the_elevator_to_make_the_link() {
        if is_root() {
            return;
        }
        let setup = setup();
        let _locked = ReadOnly::new(&setup.bin);
        let fake = Fake::new(Ok(()));
        let link = with_elevator(&setup, &fake);
        let result = link.install();
        assert_eq!(result.outcome, ShellCommandOutcome::Done);
        assert!(result.installed);
        assert_eq!(result.place, "/usr/local/bin/weaver");
        assert!(!result.path_warning);
        assert_eq!(
            fake.calls.borrow().as_slice(),
            [format!("link {}", link.place.display())]
        );
        assert_eq!(std::fs::read_link(&link.place).unwrap(), setup.copy);
    }

    #[test]
    fn a_writable_folder_needs_no_administrator() {
        let setup = setup();
        std::fs::create_dir_all(&setup.bin).unwrap();
        let fake = Fake::new(Ok(()));
        let link = with_elevator(&setup, &fake);
        assert_eq!(link.install().outcome, ShellCommandOutcome::Done);
        assert_eq!(link.remove().outcome, ShellCommandOutcome::Done);
        assert!(fake.calls.borrow().is_empty());
    }

    #[test]
    fn a_cancelled_password_request_changes_nothing_and_is_not_a_failure() {
        if is_root() {
            return;
        }
        let setup = setup();
        let _locked = ReadOnly::new(&setup.bin);
        let fake = Fake::new(Err(ElevatedFailure::Cancelled));
        let link = with_elevator(&setup, &fake);
        let result = link.install();
        assert_eq!(result.outcome, ShellCommandOutcome::Cancelled);
        assert!(!result.installed);
        assert_eq!(fake.calls.borrow().len(), 1);
        assert!(std::fs::symlink_metadata(&link.place).is_err());
    }

    #[test]
    fn a_failure_of_the_elevated_step_carries_its_reason() {
        if is_root() {
            return;
        }
        let setup = setup();
        let _locked = ReadOnly::new(&setup.bin);
        let fake = Fake::new(Err(ElevatedFailure::Failed("disk full".into())));
        let result = with_elevator(&setup, &fake).install();
        assert_eq!(
            result.outcome,
            ShellCommandOutcome::Failed {
                reason: "disk full".into()
            }
        );
        assert!(!result.installed);
    }

    #[test]
    fn removing_from_an_unwritable_folder_asks_the_elevator() {
        if is_root() {
            return;
        }
        let setup = setup();
        user_link(&setup, None).install();
        let _locked = ReadOnly::new(&setup.bin);

        let cancelled = Fake::new(Err(ElevatedFailure::Cancelled));
        let result = with_elevator(&setup, &cancelled).remove();
        assert_eq!(result.outcome, ShellCommandOutcome::Cancelled);
        assert!(result.installed, "a cancel changes nothing");

        let failed = Fake::new(Err(ElevatedFailure::Failed("busy".into())));
        let result = with_elevator(&setup, &failed).remove();
        assert_eq!(
            result.outcome,
            ShellCommandOutcome::Failed {
                reason: "busy".into()
            }
        );
        assert!(result.installed, "a failed removal changes nothing");

        let admin = Fake::new(Ok(()));
        let link = with_elevator(&setup, &admin);
        let result = link.remove();
        assert_eq!(result.outcome, ShellCommandOutcome::Done);
        assert!(!result.installed);
        assert_eq!(
            admin.calls.borrow().as_slice(),
            [format!("unlink {}", link.place.display())]
        );
    }

    #[test]
    fn the_script_takes_every_path_as_an_argument_and_never_in_its_text() {
        let parent = Path::new("/usr/local/bin");
        let copy = Path::new("/Users/o'neil/Library/x y/bin/weaver");
        let link = Path::new("/usr/local/bin/weaver");
        let steps = [
            Elevated::Link { parent, copy, link },
            Elevated::Unlink { link },
        ];
        for step in steps {
            let text = script(&step).join("\n");
            assert!(text.starts_with("on run argv\n") && text.ends_with("\nend run"));
            assert!(text.contains("quoted form of (item 1 of argv)"));
            assert!(text.contains("with administrator privileges"));
            for path in [parent, copy, link] {
                assert!(!text.contains(&path.display().to_string()), "{text}");
            }
        }
        assert_eq!(
            script_arguments(&steps[0]),
            [parent.as_os_str(), copy.as_os_str(), link.as_os_str()]
        );
        assert_eq!(script_arguments(&steps[1]), [link.as_os_str()]);
        assert!(script(&steps[0])[1].contains("(item 3 of argv)"));
    }

    #[test]
    fn osascripts_error_128_is_a_cancel_and_any_other_error_is_a_failure() {
        assert_eq!(
            classify_failure("0:57: execution error: User canceled. (-128)\n"),
            ElevatedFailure::Cancelled
        );
        assert_eq!(
            classify_failure("execution error: ln: /usr/local/bin/weaver: File exists (1)\n"),
            ElevatedFailure::Failed("ln: /usr/local/bin/weaver: File exists (1)".into())
        );
        assert_eq!(
            classify_failure(""),
            ElevatedFailure::Failed("the administrator step failed".into())
        );
    }
}
