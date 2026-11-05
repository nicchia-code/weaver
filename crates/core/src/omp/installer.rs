//! The exit code of omp's installer (`requirements/tech#omp-setup`, "Installer", "Re-checks").
//!
//! The installer runs in the system terminal. When it ends it writes the exit code of the last step
//! that ran (the official installer, or after its failure the GitHub download of
//! [`super::github`]; `0` when the official installer succeeded) as decimal text to
//! `<app local data>/omp-install/<nonce>.code`. Each
//! `INSTALL_OMP` has a new nonce and deletes the markers of the earlier ones; a terminal closed
//! before the end leaves none. While Installing, a check that finds no omp reads the marker of
//! the current nonce: a code other than `0` is `omp.INSTALL_FAILED`; `0`, no marker or an
//! unreadable one is `omp.MISSING`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The extension of a marker.
pub const MARKER_EXTENSION: &str = "code";

/// A new nonce: 16 random bytes as 32 hexadecimal digits.
pub fn new_nonce() -> String {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).expect("the OS random number generator is unavailable");
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Whether `nonce` is one [`new_nonce`] could give: only lower-case hexadecimal digits, so that
/// it cannot name anything but a marker in the folder.
pub fn is_nonce(nonce: &str) -> bool {
    !nonce.is_empty()
        && nonce
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// `<dir>/<nonce>.code`; `None` when `nonce` is not a nonce.
pub fn marker_path(dir: &Path, nonce: &str) -> Option<PathBuf> {
    is_nonce(nonce).then(|| dir.join(format!("{nonce}.{MARKER_EXTENSION}")))
}

/// Deletes every marker in `dir`. A folder that does not exist has none.
pub fn clear(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == MARKER_EXTENSION) {
            let _ = fs::remove_file(path);
        }
    }
}

/// Starts a new installer: deletes the markers of the earlier ones and makes sure the folder the
/// installer writes into exists.
pub fn prepare(dir: &Path) -> io::Result<()> {
    clear(dir);
    fs::create_dir_all(dir)
}

/// The exit code in the text of a marker: decimal, with blanks around it (PowerShell ends the
/// line). `None` when it is anything else.
pub fn parse_exit_code(text: &str) -> Option<i32> {
    text.trim_start_matches('\u{feff}').trim().parse().ok()
}

/// The exit code the installer of `nonce` left; `None` for no marker or an unreadable one.
pub fn read_exit_code(dir: &Path, nonce: &str) -> Option<i32> {
    let text = fs::read_to_string(marker_path(dir, nonce)?).ok()?;
    parse_exit_code(&text)
}

/// The exit code of a failed installer: what [`read_exit_code`] reads when it is not `0`. `None`
/// is `omp.MISSING` (not finished, finished well, no marker, an unreadable one).
pub fn failed_exit_code(dir: &Path, nonce: &str) -> Option<i32> {
    read_exit_code(dir, nonce).filter(|code| *code != 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_nonce_is_32_hex_digits_and_a_new_one_each_time() {
        let a = new_nonce();
        let b = new_nonce();
        assert_eq!(a.len(), 32);
        assert!(is_nonce(&a));
        assert_ne!(a, b);
    }

    #[test]
    fn only_a_nonce_names_a_marker() {
        let dir = Path::new("/data/omp-install");
        assert_eq!(
            marker_path(dir, "0a1b"),
            Some(PathBuf::from("/data/omp-install/0a1b.code"))
        );
        for bad in ["", "../x", "A1", "a/b", "a.code", "g1"] {
            assert_eq!(marker_path(dir, bad), None, "{bad:?}");
        }
    }

    #[test]
    fn the_marker_text_is_a_decimal_code() {
        assert_eq!(parse_exit_code("0"), Some(0));
        assert_eq!(parse_exit_code("22\n"), Some(22));
        assert_eq!(parse_exit_code("\u{feff}7\r\n"), Some(7));
        assert_eq!(parse_exit_code("-1"), Some(-1));
        assert_eq!(parse_exit_code(""), None);
        assert_eq!(parse_exit_code("curl failed"), None);
        assert_eq!(parse_exit_code("1 2"), None);
    }

    #[test]
    fn only_a_code_other_than_zero_is_a_failure() {
        let dir = tempfile::tempdir().unwrap();
        let dir = dir.path();
        assert_eq!(failed_exit_code(dir, "aa"), None, "no marker");
        fs::write(dir.join("aa.code"), "0\n").unwrap();
        assert_eq!(failed_exit_code(dir, "aa"), None, "finished well");
        assert_eq!(read_exit_code(dir, "aa"), Some(0));
        fs::write(dir.join("bb.code"), "56").unwrap();
        assert_eq!(failed_exit_code(dir, "bb"), Some(56));
        fs::write(dir.join("cc.code"), "oops").unwrap();
        assert_eq!(failed_exit_code(dir, "cc"), None, "unreadable");
        assert_eq!(failed_exit_code(dir, "../bb"), None, "not a nonce");
    }

    #[test]
    fn a_new_installer_forgets_the_earlier_markers_and_makes_the_folder() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("omp-install");
        prepare(&dir).unwrap();
        fs::write(dir.join("aa.code"), "1").unwrap();
        fs::write(dir.join("keep.txt"), "x").unwrap();
        prepare(&dir).unwrap();
        assert_eq!(read_exit_code(&dir, "aa"), None);
        assert!(dir.join("keep.txt").exists());
        clear(&root.path().join("missing"));
    }
}
