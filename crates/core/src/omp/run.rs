//! Runs an omp command without a shell, with a time limit: the process is killed on timeout.
//!
//! [`Exec`] is the seam the probe, the update and the logout go through, so their tests use a
//! fake and never start a real omp.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use crate::process::timed::{run_command, Failure, Output};
use crate::system_certificates::Variables;

/// Limits of the omp commands (`requirements/tech#omp`).
pub mod limits {
    use std::time::Duration;

    pub const VERSION: Duration = Duration::from_secs(15);
    pub const MODELS: Duration = Duration::from_secs(15);
    pub const UPDATE_CHECK: Duration = Duration::from_secs(30);
    pub const UPDATE: Duration = Duration::from_secs(5 * 60);
    /// The download of omp from its GitHub release, run in the background after `omp update`
    /// failed.
    pub const GITHUB_UPDATE: Duration = Duration::from_secs(15 * 60);
    pub const LOGOUT: Duration = Duration::from_secs(30);
    pub const USAGE: Duration = Duration::from_secs(30);
    /// The read of the login PATH at start.
    pub const LOGIN_PATH: Duration = Duration::from_secs(5);
}

/// What System Certificates gives an omp process right now: whether the setting is on, and the
/// variables to set (empty with the setting off or without a bundle).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Certificates {
    pub enabled: bool,
    pub variables: Variables,
}

/// Reads [`Certificates`] at every call: the setting and the bundle of the moment, never once.
pub type CertificateSource = Arc<dyn Fn() -> Certificates + Send + Sync>;

/// Runs `program` with `args` and waits at most `limit`.
pub trait Exec: Send + Sync {
    fn run(&self, program: &Path, args: &[&str], limit: Duration) -> Result<Output, Failure>;

    /// Whether System Certificates is on now. Asked right after a failure, so that its
    /// explanation is chosen with the setting as it is then. Commands that are not omp's say
    /// `false`.
    fn system_certificates_on(&self) -> bool {
        false
    }
}

/// The operating system. For omp (a source of [`Certificates`] is given) it sets the environment
/// of an omp process: `PATH` is `<app local data>/bin`, omp's folder and bun's bin folder, then
/// the base PATH, and the System Certificates variables over the inherited ones. Without a source
/// (the login shell and `reg.exe`, which only read `PATH`) no certificate variable is set.
#[derive(Clone)]
pub struct SystemExec {
    local_bin: PathBuf,
    bun_bin: Option<PathBuf>,
    base_path: OsString,
    certificates: Option<CertificateSource>,
}

impl SystemExec {
    pub fn new(local_bin: PathBuf, bun_bin: Option<PathBuf>, base_path: OsString) -> Self {
        Self {
            local_bin,
            bun_bin,
            base_path,
            certificates: None,
        }
    }

    /// The same, for omp's own commands: they get the certificates of the moment.
    pub fn with_certificates(mut self, certificates: CertificateSource) -> Self {
        self.certificates = Some(certificates);
        self
    }

    /// The `PATH` of a child that runs `program`.
    pub fn child_path(&self, program: &Path) -> OsString {
        super::path_env::child_path(
            &self.local_bin,
            program.parent(),
            self.bun_bin.as_deref(),
            &self.base_path,
        )
    }
}

impl Exec for SystemExec {
    fn run(&self, program: &Path, args: &[&str], limit: Duration) -> Result<Output, Failure> {
        let mut command = crate::process::command(program, args);
        command.env("PATH", self.child_path(program));
        if let Some(source) = &self.certificates {
            crate::system_certificates::apply(&mut command, &source().variables);
        }
        run_command(command, limit)
    }

    fn system_certificates_on(&self) -> bool {
        self.certificates
            .as_ref()
            .is_some_and(|source| source().enabled)
    }
}

/// Whether `name` (a program) is found in the folders of `path`; the first match wins.
pub fn which(name: &str, path: &OsStr) -> Option<PathBuf> {
    std::env::split_paths(path)
        .map(|dir| dir.join(name))
        .find(|candidate| super::lookup::is_executable(candidate))
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn the_child_path_starts_with_weavers_folder_then_omps_and_buns() {
        let exec = SystemExec::new(
            "/data/bin".into(),
            Some("/home/u/.bun/bin".into()),
            "/usr/bin:/bin".into(),
        );
        let path = exec.child_path(Path::new("/opt/omp/bin/omp"));
        let folders: Vec<PathBuf> = std::env::split_paths(&path).collect();
        assert_eq!(
            folders,
            [
                PathBuf::from("/data/bin"),
                PathBuf::from("/opt/omp/bin"),
                PathBuf::from("/home/u/.bun/bin"),
                PathBuf::from("/usr/bin"),
                PathBuf::from("/bin"),
            ]
        );
    }
}

#[cfg(all(test, unix))]
mod certificate_tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::*;
    use crate::system_certificates::variables;

    const PRINT: [&str; 2] = [
        "-c",
        "printf '%s|%s|%s' \"$NODE_EXTRA_CA_CERTS\" \"$PI_REQUEST_COMPRESSION\" \"$SSL_CERT_FILE\"",
    ];

    fn exec(source: Option<CertificateSource>) -> SystemExec {
        let exec = SystemExec::new(PathBuf::new(), None, "/usr/bin:/bin".into());
        match source {
            Some(source) => exec.with_certificates(source),
            None => exec,
        }
    }

    #[test]
    fn an_omp_command_gets_the_variables_of_the_moment_over_the_inherited_ones() {
        let dir = tempfile::tempdir().unwrap();
        let bundle = dir.path().join("system-certificates.pem");
        std::fs::write(&bundle, "x").unwrap();
        let on = Arc::new(AtomicBool::new(true));
        let source: CertificateSource = {
            let (on, bundle) = (Arc::clone(&on), bundle.clone());
            Arc::new(move || {
                let enabled = on.load(Ordering::SeqCst);
                Certificates {
                    enabled,
                    variables: variables(enabled, &bundle),
                }
            })
        };
        let exec = exec(Some(source));
        let run = |exec: &SystemExec| {
            exec.run(Path::new("/bin/sh"), &PRINT, Duration::from_secs(10))
                .unwrap()
                .stdout
        };
        let bundle_text = bundle.to_str().unwrap();
        assert_eq!(
            run(&exec),
            format!("{bundle_text}|0|{bundle_text}"),
            "the setting is on"
        );
        assert!(exec.system_certificates_on());

        // Read at each start: turned off, the next command gets the inherited environment as it is
        // (the runner may set `SSL_CERT_FILE` itself).
        on.store(false, Ordering::SeqCst);
        let inherited = [
            "NODE_EXTRA_CA_CERTS",
            "PI_REQUEST_COMPRESSION",
            "SSL_CERT_FILE",
        ]
        .map(|name| std::env::var(name).unwrap_or_default())
        .join("|");
        assert_eq!(run(&exec), inherited);
        assert!(!exec.system_certificates_on());
    }

    #[test]
    fn the_commands_that_are_not_omps_get_no_certificate_variable() {
        let exec = exec(None);
        let out = exec
            .run(Path::new("/bin/sh"), &PRINT, Duration::from_secs(10))
            .unwrap();
        assert_eq!(out.stdout.matches('|').count(), 2);
        assert!(!exec.system_certificates_on());
    }
}
