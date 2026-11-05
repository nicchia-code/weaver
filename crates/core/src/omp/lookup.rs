//! Finding omp (`requirements/tech#omp`, "Lookup"): the first executable found wins.
//!
//! 1. the `ompPath` setting, trimmed. When set it is used alone, with no fallback;
//! 2. PATH;
//! 3. the installer folders: on macOS and Linux `$PI_INSTALL_DIR`, `~/.local/bin`, then
//!    `$BUN_INSTALL/bin` or `~/.bun/bin`; on Windows `%PI_INSTALL_DIR%`, `%LOCALAPPDATA%\omp`,
//!    `%USERPROFILE%\.bun\bin`. The program is `omp.exe` on Windows.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

/// The systems Weaver runs on. Tests pass each of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    MacOs,
    Linux,
    Windows,
}

impl Os {
    pub fn current() -> Self {
        if cfg!(windows) {
            Os::Windows
        } else if cfg!(target_os = "macos") {
            Os::MacOs
        } else {
            Os::Linux
        }
    }

    /// The file name of omp.
    pub fn omp_file_name(self) -> &'static str {
        if self == Os::Windows {
            "omp.exe"
        } else {
            "omp"
        }
    }
}

/// What the lookup reads from the environment, passed in so that tests use temporary folders.
#[derive(Debug, Clone)]
pub struct LookupEnv {
    pub os: Os,
    /// The PATH to search (the login PATH on macOS and Linux, the registry's on Windows).
    pub path: OsString,
    /// `PI_INSTALL_DIR`.
    pub pi_install_dir: Option<PathBuf>,
    /// `BUN_INSTALL`.
    pub bun_install: Option<PathBuf>,
    /// The home folder (`~`; `%USERPROFILE%` on Windows).
    pub home: Option<PathBuf>,
    /// `%LOCALAPPDATA%` (Windows only).
    pub local_app_data: Option<PathBuf>,
}

impl LookupEnv {
    /// The environment of this process, with `path` as the PATH to search.
    pub fn from_process(path: OsString) -> Self {
        let var = |name: &str| {
            std::env::var_os(name)
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
        };
        Self {
            os: Os::current(),
            path,
            pi_install_dir: var("PI_INSTALL_DIR"),
            bun_install: var("BUN_INSTALL"),
            home: dirs::home_dir(),
            local_app_data: var("LOCALAPPDATA"),
        }
    }

    /// The folders of the official installer, in the order they are searched.
    pub fn installer_dirs(&self) -> Vec<PathBuf> {
        let mut folders = self.install_dirs();
        folders.extend(self.bun_bin());
        folders
    }

    /// Where the official installer puts omp: `PI_INSTALL_DIR`, then its default folder
    /// (`%LOCALAPPDATA%\omp`, `~/.local/bin`). Not bun's folder: the GitHub download only replaces
    /// an omp found here.
    pub fn install_dirs(&self) -> Vec<PathBuf> {
        let mut folders = Vec::new();
        folders.extend(self.pi_install_dir.clone());
        match self.os {
            Os::Windows => folders.extend(self.local_app_data.as_ref().map(|d| d.join("omp"))),
            Os::MacOs | Os::Linux => {
                folders.extend(self.home.as_ref().map(|h| h.join(".local").join("bin")));
            }
        }
        folders
    }

    /// Bun's bin folder: `$BUN_INSTALL/bin`, else `~/.bun/bin`.
    pub fn bun_bin(&self) -> Option<PathBuf> {
        match &self.bun_install {
            Some(install) => Some(install.join("bin")),
            None => self.home.as_ref().map(|h| h.join(".bun").join("bin")),
        }
    }
}

/// omp was not found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotFound {
    /// The `ompPath` setting (trimmed) when it was set, else `None`.
    pub configured: Option<String>,
}

impl NotFound {
    /// `omp not found at <path>` when a path was set, `None` when omp is simply not installed.
    pub fn reason(&self) -> Option<String> {
        self.configured
            .as_ref()
            .map(|path| format!("omp not found at {path}"))
    }
}

/// Whether `path` is a file the system can run.
pub fn is_executable(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// Finds omp. The result is an absolute path.
pub fn find_omp(setting: &str, env: &LookupEnv) -> Result<PathBuf, NotFound> {
    let setting = setting.trim();
    if !setting.is_empty() {
        let path = PathBuf::from(setting);
        return if is_executable(&path) {
            Ok(absolute(&path))
        } else {
            Err(NotFound {
                configured: Some(setting.to_owned()),
            })
        };
    }
    let name = env.os.omp_file_name();
    let on_path = std::env::split_paths(&env.path as &OsStr).map(|dir| dir.join(name));
    let installer = env.installer_dirs().into_iter().map(|dir| dir.join(name));
    on_path
        .chain(installer)
        .find(|candidate| candidate.is_absolute() && is_executable(candidate))
        .map(|found| absolute(&found))
        .ok_or(NotFound { configured: None })
}

fn absolute(path: &Path) -> PathBuf {
    std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(dir: &Path, name: &str) -> PathBuf {
        std::fs::create_dir_all(dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, "#!/bin/sh\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        path
    }

    fn env(os: Os, root: &Path, path: &[&Path]) -> LookupEnv {
        LookupEnv {
            os,
            path: std::env::join_paths(path).unwrap(),
            pi_install_dir: None,
            bun_install: None,
            home: Some(root.join("home")),
            local_app_data: Some(root.join("local")),
        }
    }

    #[test]
    fn the_setting_is_used_alone_with_no_fallback() {
        let root = tempfile::tempdir().unwrap();
        let on_path = file(&root.path().join("bin"), "omp");
        let env = env(Os::Linux, root.path(), &[root.path().join("bin").as_path()]);
        // Set and missing: not found, though omp is on PATH.
        let missing = root.path().join("nowhere").join("omp");
        let error = find_omp(&format!("  {}  ", missing.display()), &env).unwrap_err();
        assert_eq!(
            error.reason(),
            Some(format!("omp not found at {}", missing.display()))
        );
        // Set and present: it wins over PATH.
        let chosen = file(&root.path().join("chosen"), "omp");
        assert_eq!(find_omp(chosen.to_str().unwrap(), &env), Ok(chosen));
        // Not set: PATH.
        assert_eq!(find_omp("   ", &env), Ok(on_path));
    }

    #[test]
    fn path_comes_before_the_installer_folders_and_the_first_folder_wins() {
        let root = tempfile::tempdir().unwrap();
        let first = file(&root.path().join("a"), "omp");
        file(&root.path().join("b"), "omp");
        file(&root.path().join("home/.local/bin"), "omp");
        let env = env(
            Os::Linux,
            root.path(),
            &[&root.path().join("a"), &root.path().join("b")],
        );
        assert_eq!(find_omp("", &env), Ok(first));
    }

    #[test]
    fn installer_folders_are_searched_in_order_on_macos_and_linux() {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("home");
        let pi = root.path().join("pi");
        let bun = root.path().join("bun");
        let mut env = env(Os::MacOs, root.path(), &[]);
        env.pi_install_dir = Some(pi.clone());
        env.bun_install = Some(bun.clone());
        assert_eq!(
            env.installer_dirs(),
            [pi.clone(), home.join(".local").join("bin"), bun.join("bin")]
        );

        let in_bun = file(&bun.join("bin"), "omp");
        assert_eq!(find_omp("", &env), Ok(in_bun));
        let in_local = file(&home.join(".local").join("bin"), "omp");
        assert_eq!(find_omp("", &env), Ok(in_local));
        let in_pi = file(&pi, "omp");
        assert_eq!(find_omp("", &env), Ok(in_pi));
    }

    #[test]
    fn bun_falls_back_to_the_home_folder() {
        let root = tempfile::tempdir().unwrap();
        let env = env(Os::Linux, root.path(), &[]);
        assert_eq!(
            env.bun_bin(),
            Some(root.path().join("home").join(".bun").join("bin"))
        );
    }

    #[test]
    fn windows_searches_omp_exe_in_its_own_folders() {
        let root = tempfile::tempdir().unwrap();
        let mut env = env(Os::Windows, root.path(), &[]);
        env.pi_install_dir = Some(root.path().join("pi"));
        assert_eq!(
            env.installer_dirs(),
            [
                root.path().join("pi"),
                root.path().join("local").join("omp"),
                root.path().join("home").join(".bun").join("bin"),
            ]
        );
        // `omp` without the extension is not omp on Windows.
        file(&root.path().join("local").join("omp"), "omp");
        assert!(find_omp("", &env).is_err());
        let found = file(&root.path().join("local").join("omp"), "omp.exe");
        assert_eq!(find_omp("", &env), Ok(found));
    }

    #[cfg(unix)]
    #[test]
    fn a_file_that_is_not_executable_or_a_folder_is_not_omp() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let plain = file(&root.path().join("bin"), "omp");
        std::fs::set_permissions(&plain, std::fs::Permissions::from_mode(0o644)).unwrap();
        let env = env(Os::Linux, root.path(), &[&root.path().join("bin")]);
        assert!(find_omp("", &env).is_err());
        std::fs::create_dir_all(root.path().join("dir").join("omp")).unwrap();
        assert!(find_omp(root.path().join("dir").join("omp").to_str().unwrap(), &env).is_err());
    }

    #[test]
    fn nothing_found_and_nothing_set_means_not_installed() {
        let root = tempfile::tempdir().unwrap();
        let error = find_omp("", &env(Os::Linux, root.path(), &[])).unwrap_err();
        assert_eq!(error.reason(), None);
    }
}
