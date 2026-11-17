//! The `weaver` command in the developer's own terminal (Machine `shell-command`,
//! `requirements/tech#shell-command`).
//!
//! What is made reachable is always Weaver's stable copy of the command line,
//! `<app local data>/bin/weaver` (`weaver.exe` on Windows), never a file inside the app bundle: an
//! update of Weaver keeps an installed command working. Installing and removing happen only when
//! the developer asks, from the settings.
//!
//! | System | What installing does |
//! |--------|----------------------|
//! | macOS  | a symbolic link `/usr/local/bin/weaver` to the copy, made with administrator rights through `osascript` when the folder does not allow the user ([`link`]) |
//! | Linux  | a symbolic link `~/.local/bin/weaver` to the copy; never asks for privileges ([`link`]) |
//! | Windows | the folder of the copy added to the user's `Path` in `HKEY_CURRENT_USER\Environment` ([`user_path`], `registry`) |
//!
//! [`status`], [`install`] and [`remove`] each return one [`ShellCommandResult`]. They block: call
//! them from the blocking pool.

use std::path::Path;

use serde::Serialize;
use ts_rs::TS;

pub mod user_path;

#[cfg(unix)]
pub mod link;
#[cfg(windows)]
mod registry;

/// How an action ended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(rename = "ShellCommandOutcome")]
pub enum ShellCommandOutcome {
    /// It happened (or there was nothing to do). A plain reading of the state also ends so.
    Done,
    /// Something that is not Weaver's command takes the place: nothing was touched. `found` is
    /// its path.
    Refused { found: String },
    /// The developer cancelled the system's request for an administrator password: nothing
    /// changed and nothing is shown.
    Cancelled,
    /// It failed; nothing is left behind. `reason` is the system's own words.
    Failed { reason: String },
}

/// The state and the outcome of one reading or action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename = "ShellCommandResult")]
pub struct ShellCommandResult {
    /// Whether the terminal reaches Weaver's command line through Weaver's installation, read
    /// after the action.
    pub installed: bool,
    /// The place, as the developer is told: `/usr/local/bin/weaver`, `~/.local/bin/weaver`, or
    /// the folder added to the user's `Path` on Windows.
    pub place: String,
    pub outcome: ShellCommandOutcome,
    /// Linux, after installing: `~/.local/bin` is not in the app's own `PATH`.
    pub path_warning: bool,
}

/// The place of the link on macOS.
#[cfg(target_os = "macos")]
const MACOS_LINK: &str = "/usr/local/bin/weaver";

/// What the terminal finds now: installed or not, and where.
pub fn status(copy: &Path) -> ShellCommandResult {
    platform::status(copy)
}

/// Makes the copy reachable from the terminals opened afterwards.
pub fn install(copy: &Path) -> ShellCommandResult {
    platform::install(copy)
}

/// Undoes what [`install`] made.
pub fn remove(copy: &Path) -> ShellCommandResult {
    platform::remove(copy)
}

#[cfg(unix)]
mod platform {
    use std::path::Path;

    use super::link::Link;
    use super::{ShellCommandOutcome, ShellCommandResult};

    #[cfg(target_os = "macos")]
    fn link_for(copy: &Path) -> Result<Link, String> {
        Ok(Link {
            place: std::path::PathBuf::from(super::MACOS_LINK),
            display: super::MACOS_LINK.to_owned(),
            copy: copy.to_owned(),
            elevator: Some(Box::new(super::link::Osascript)),
            path_check: None,
        })
    }

    #[cfg(not(target_os = "macos"))]
    fn link_for(copy: &Path) -> Result<Link, String> {
        let home = dirs::home_dir().ok_or_else(|| "the home folder was not found".to_owned())?;
        let dir = home.join(".local").join("bin");
        Ok(Link {
            place: dir.join("weaver"),
            display: "~/.local/bin/weaver".to_owned(),
            copy: copy.to_owned(),
            elevator: None,
            path_check: Some((dir, std::env::var_os("PATH"))),
        })
    }

    fn unavailable(reason: String) -> ShellCommandResult {
        ShellCommandResult {
            installed: false,
            place: "~/.local/bin/weaver".to_owned(),
            outcome: ShellCommandOutcome::Failed { reason },
            path_warning: false,
        }
    }

    pub fn status(copy: &Path) -> ShellCommandResult {
        link_for(copy).map_or_else(unavailable, |link| link.status())
    }

    pub fn install(copy: &Path) -> ShellCommandResult {
        link_for(copy).map_or_else(unavailable, |link| link.install())
    }

    pub fn remove(copy: &Path) -> ShellCommandResult {
        link_for(copy).map_or_else(unavailable, |link| link.remove())
    }
}

#[cfg(windows)]
mod platform {
    use std::path::Path;

    use super::registry;
    use super::ShellCommandResult;

    pub fn status(copy: &Path) -> ShellCommandResult {
        registry::status(copy)
    }

    pub fn install(copy: &Path) -> ShellCommandResult {
        registry::install(copy)
    }

    pub fn remove(copy: &Path) -> ShellCommandResult {
        registry::remove(copy)
    }
}
