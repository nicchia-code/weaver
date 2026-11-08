//! Asking omp about itself: where it is, its version and its usable models, and the other
//! commands Weaver runs (`requirements/tech#omp`, "Commands").
//!
//! | Command | Limit | Use |
//! |---|---|---|
//! | `omp --version` | 15 s | version |
//! | `omp models --json` | 15 s | `selector`, `provider`; no models means no provider |
//! | `omp update --check` | 30 s | a newer version exists |
//! | `omp update` | 5 min | the update ended or failed |
//! | `omp auth-broker logout <provider>` | 30 s | log out |
//! | `omp usage --json` | 30 s | usage limits of the connected providers (see [`super::usage`]) |

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use super::lookup::{find_omp, LookupEnv};
use super::run::{limits, Exec};
use super::version::{is_supported, parse_omp_version, Version, MIN_OMP_VERSION};
use crate::process::timed::Failure;

/// A usable model of `omp models --json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct OmpModel {
    pub provider: String,
    /// What is stored in the settings and passed to omp (`anthropic/claude-sonnet-5`).
    pub selector: String,
    pub name: Option<String>,
    /// The thinking levels omp lists for the model.
    pub thinking_levels: Vec<String>,
}

/// What a check found. The outcomes are the events `omp.READY`, `omp.MISSING`, `omp.OUTDATED`,
/// `omp.NO_PROVIDER` and, while Installing, `omp.INSTALL_FAILED`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbeResult {
    /// omp answers with the minimum version and at least one model.
    Ready {
        omp: PathBuf,
        version: String,
        models: Vec<OmpModel>,
    },
    /// omp cannot be found or does not answer; `reason` says why (`None`: it is not installed).
    Missing { reason: Option<String> },
    /// omp answers with a version below the minimum, or one that cannot be read.
    Outdated {
        omp: PathBuf,
        version: Option<String>,
    },
    /// omp answers with the minimum version and no model.
    NoProvider { omp: PathBuf, version: String },
    /// Only while Installing (`omp.INSTALL_FAILED`): omp is still not found, and the installer
    /// ended with an exit code other than 0.
    InstallFailed { exit_code: i32 },
}

/// The models in the output of `omp models --json`: `{"models": [...]}` or the array itself.
/// Entries without a `provider` and a `selector` are not usable and are skipped; `None` when the
/// text is not JSON of that shape.
pub fn parse_models(stdout: &str) -> Option<Vec<OmpModel>> {
    let value: Value = serde_json::from_str(stdout.trim()).ok()?;
    let list = match &value {
        Value::Array(list) => list,
        Value::Object(object) => object.get("models")?.as_array()?,
        _ => return None,
    };
    Some(
        list.iter()
            .filter_map(|entry| {
                let text = |key: &str| {
                    entry
                        .get(key)
                        .and_then(Value::as_str)
                        .filter(|s| !s.is_empty())
                };
                Some(OmpModel {
                    provider: text("provider")?.to_owned(),
                    selector: text("selector")?.to_owned(),
                    name: text("name").map(str::to_owned),
                    thinking_levels: entry
                        .get("thinking")
                        .and_then(Value::as_array)
                        .map(|levels| {
                            levels
                                .iter()
                                .filter_map(Value::as_str)
                                .map(str::to_owned)
                                .collect()
                        })
                        .unwrap_or_default(),
                })
            })
            .collect(),
    )
}

/// A check: the lookup, `omp --version` and `omp models --json`.
pub fn probe(setting: &str, env: &LookupEnv, exec: &dyn Exec) -> ProbeResult {
    let omp = match find_omp(setting, env) {
        Ok(omp) => omp,
        Err(not_found) => {
            return ProbeResult::Missing {
                reason: not_found.reason(),
            }
        }
    };
    probe_found(&omp, exec)
}

/// A check of an omp already found (the lookup is not repeated).
pub fn probe_found(omp: &Path, exec: &dyn Exec) -> ProbeResult {
    let missing = |failure: Failure| ProbeResult::Missing {
        reason: Some(failure.status_text_for(exec.system_certificates_on())),
    };
    let version_output = match exec.run(omp, &["--version"], limits::VERSION) {
        Ok(output) => output,
        Err(failure) => return missing(failure),
    };
    let version = parse_omp_version(&version_output.stdout);
    if !is_supported(version.as_ref()) {
        return ProbeResult::Outdated {
            omp: omp.to_path_buf(),
            version: version.map(|v| v.to_string()),
        };
    }
    let version = version.expect("a supported version was read").to_string();
    let models_output = match exec.run(omp, &["models", "--json"], limits::MODELS) {
        Ok(output) => output,
        Err(failure) => return missing(failure),
    };
    match parse_models(&models_output.stdout) {
        None => ProbeResult::Missing {
            reason: Some("the agent answered with a list of models Weaver cannot read".to_owned()),
        },
        Some(models) if models.is_empty() => ProbeResult::NoProvider {
            omp: omp.to_path_buf(),
            version,
        },
        Some(models) => ProbeResult::Ready {
            omp: omp.to_path_buf(),
            version,
            models,
        },
    }
}

/// The text of `Missing` after a failed update of an omp that is still too old.
pub fn too_old_text(version: Option<&str>, reason: &str) -> String {
    let reason = reason.trim_end_matches('.');
    match version {
        Some(version) => format!(
            "the agent {version} is older than {MIN_OMP_VERSION} and could not be updated ({reason})."
        ),
        None => format!("the agent is older than {MIN_OMP_VERSION} and could not be updated ({reason})."),
    }
}

/// The version `omp update --check` announces: the line `New version available: <version>` of its
/// output (colour codes ignored). `None` when omp is up to date or the output is not understood;
/// an update that cannot be recognised is never started.
pub fn parse_update_check(stdout: &str) -> Option<String> {
    const MARKER: &str = "new version available:";
    let clean = strip_ansi(stdout);
    let current = clean
        .lines()
        .find_map(|line| {
            line.trim()
                .strip_prefix("Current version:")
                .map(|v| v.trim().to_owned())
        })
        .and_then(|v| Version::parse(&v));
    clean.lines().find_map(|line| {
        let line = line.trim();
        let at = line.to_ascii_lowercase().find(MARKER)?;
        let version = Version::parse(line[at + MARKER.len()..].split_whitespace().next()?)?;
        // An announced version that is not newer is no update.
        current
            .as_ref()
            .is_none_or(|current| version > *current)
            .then(|| version.to_string())
    })
}

fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            for next in chars.by_ref() {
                if next.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// `omp update --check`: the newer version, if there is one.
pub fn check_for_update(omp: &Path, exec: &dyn Exec) -> Result<Option<String>, Failure> {
    exec.run(omp, &["update", "--check"], limits::UPDATE_CHECK)
        .map(|output| parse_update_check(&output.stdout))
}

/// `omp update`. On failure, its reason: the explanation of a proxy problem, else the last line
/// of stderr (`requirements/tech#omp`, "Failure reasons"), chosen with the setting as it is now.
pub fn run_update(omp: &Path, exec: &dyn Exec) -> Result<(), String> {
    exec.run(omp, &["update"], limits::UPDATE)
        .map(|_| ())
        .map_err(|failure| failure.reason_for(exec.system_certificates_on()))
}

/// `omp auth-broker logout <provider>`; `provider` is an id of the fixed list. On failure, its
/// reason, as for [`run_update`].
pub fn logout(
    omp: &Path,
    provider: &super::providers::Provider,
    exec: &dyn Exec,
) -> Result<(), String> {
    exec.run(omp, &["auth-broker", "logout", provider.id], limits::LOGOUT)
        .map(|_| ())
        .map_err(|failure| failure.reason_for(exec.system_certificates_on()))
}

#[cfg(test)]
pub(crate) mod fake {
    //! A scripted [`Exec`]: answers by the arguments it is called with and records the calls.

    use std::sync::Mutex;
    use std::time::Duration;

    use super::*;
    use crate::process::timed::Output;

    type Answer = Result<Output, Failure>;

    #[derive(Default)]
    pub struct FakeOmp {
        answers: Mutex<Vec<(Vec<String>, Answer)>>,
        pub calls: Mutex<Vec<(Vec<String>, Duration)>>,
        certificates_on: bool,
    }

    impl FakeOmp {
        pub fn new() -> Self {
            Self::default()
        }

        /// System Certificates is on in this fake.
        pub fn certificates_on(mut self) -> Self {
            self.certificates_on = true;
            self
        }

        pub fn on(self, args: &[&str], answer: Answer) -> Self {
            self.answers
                .lock()
                .unwrap()
                .push((args.iter().map(|a| a.to_string()).collect(), answer));
            self
        }

        pub fn ok(stdout: &str) -> Answer {
            Ok(Output {
                stdout: stdout.to_owned(),
                stderr: String::new(),
            })
        }
    }

    impl Exec for FakeOmp {
        fn run(&self, _program: &Path, args: &[&str], limit: Duration) -> Answer {
            let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
            self.calls.lock().unwrap().push((args.clone(), limit));
            self.answers
                .lock()
                .unwrap()
                .iter()
                .find(|(expected, _)| *expected == args)
                .map(|(_, answer)| answer.clone())
                .unwrap_or(Err(Failure::NotStarted))
        }

        fn system_certificates_on(&self) -> bool {
            self.certificates_on
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::fake::FakeOmp;
    use super::*;
    use crate::omp::lookup::Os;
    use crate::omp::providers::provider;
    use crate::process::timed::Output;

    const MODELS: &str = r#"{"models":[
        {"provider":"anthropic","kind":"chat","id":"a","selector":"anthropic/a","name":"Model A","thinking":["low","high"]},
        {"provider":"openai-codex","selector":"openai-codex/b"},
        {"provider":"no-selector"},
        {"selector":"no-provider"}
    ]}"#;

    fn env_with_omp(dir: &Path) -> LookupEnv {
        LookupEnv {
            os: Os::Linux,
            path: dir.as_os_str().to_owned(),
            pi_install_dir: None,
            bun_install: None,
            home: None,
            local_app_data: None,
        }
    }

    fn omp_in(dir: &Path) -> PathBuf {
        let path = dir.join("omp");
        std::fs::write(&path, "").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        path
    }

    #[test]
    fn models_keep_selector_provider_name_and_thinking_levels() {
        let models = parse_models(MODELS).unwrap();
        assert_eq!(
            models,
            [
                OmpModel {
                    provider: "anthropic".into(),
                    selector: "anthropic/a".into(),
                    name: Some("Model A".into()),
                    thinking_levels: vec!["low".into(), "high".into()],
                },
                OmpModel {
                    provider: "openai-codex".into(),
                    selector: "openai-codex/b".into(),
                    name: None,
                    thinking_levels: vec![]
                },
            ]
        );
        assert_eq!(
            parse_models(r#"[{"provider":"p","selector":"p/m"}]"#)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(parse_models(r#"{"models":[]}"#), Some(vec![]));
        assert_eq!(parse_models("not json"), None);
        assert_eq!(parse_models(r#"{"other":1}"#), None);
        assert_eq!(parse_models("7"), None);
    }

    #[test]
    fn a_missing_omp_is_reported_with_the_path_that_was_set() {
        let dir = tempfile::tempdir().unwrap();
        let env = env_with_omp(dir.path());
        let fake = FakeOmp::new();
        assert_eq!(
            probe("", &env, &fake),
            ProbeResult::Missing { reason: None }
        );
        let gone = dir.path().join("gone");
        assert_eq!(
            probe(gone.to_str().unwrap(), &env, &fake),
            ProbeResult::Missing {
                reason: Some(format!("the agent not found at {}", gone.display()))
            }
        );
        assert!(fake.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn omp_that_does_not_answer_is_missing_with_the_reason() {
        let dir = tempfile::tempdir().unwrap();
        let omp = omp_in(dir.path());
        let env = env_with_omp(dir.path());
        let timed_out = FakeOmp::new().on(
            &["--version"],
            Err(Failure::TimedOut {
                limit: Duration::from_secs(15),
            }),
        );
        assert_eq!(
            probe("", &env, &timed_out),
            ProbeResult::Missing {
                reason: Some("the agent did not answer within 15 s".into())
            }
        );
        let crashed = FakeOmp::new().on(
            &["--version"],
            Err(Failure::Exit {
                code: Some(9),
                stderr: String::new(),
            }),
        );
        assert_eq!(
            probe("", &env, &crashed),
            ProbeResult::Missing {
                reason: Some("the agent exited with code 9".into())
            }
        );
        let no_models = FakeOmp::new()
            .on(&["--version"], FakeOmp::ok("omp/18.15.0\n"))
            .on(
                &["models", "--json"],
                Err(Failure::Exit {
                    code: Some(1),
                    stderr: "auth broken\n".into(),
                }),
            );
        assert_eq!(
            probe("", &env, &no_models),
            ProbeResult::Missing {
                reason: Some("auth broken".into())
            }
        );
        let garbled = FakeOmp::new()
            .on(&["--version"], FakeOmp::ok("omp/18.15.0"))
            .on(&["models", "--json"], FakeOmp::ok("???"));
        assert!(matches!(
            probe("", &env, &garbled),
            ProbeResult::Missing { reason: Some(_) }
        ));
        let _ = omp;
    }

    #[test]
    fn an_old_or_unreadable_version_is_outdated_and_models_are_not_asked() {
        let dir = tempfile::tempdir().unwrap();
        let omp = omp_in(dir.path());
        let env = env_with_omp(dir.path());
        let old = FakeOmp::new().on(&["--version"], FakeOmp::ok("omp/18.14.9"));
        assert_eq!(
            probe("", &env, &old),
            ProbeResult::Outdated {
                omp: omp.clone(),
                version: Some("18.14.9".into())
            }
        );
        let unreadable = FakeOmp::new().on(&["--version"], FakeOmp::ok("hello"));
        assert_eq!(
            probe("", &env, &unreadable),
            ProbeResult::Outdated { omp, version: None }
        );
        assert_eq!(old.calls.lock().unwrap().len(), 1);
    }

    #[test]
    fn no_model_means_no_provider_and_models_mean_ready() {
        let dir = tempfile::tempdir().unwrap();
        let omp = omp_in(dir.path());
        let env = env_with_omp(dir.path());
        let none = FakeOmp::new()
            .on(&["--version"], FakeOmp::ok("omp/18.15.0"))
            .on(&["models", "--json"], FakeOmp::ok(r#"{"models":[]}"#));
        assert_eq!(
            probe("", &env, &none),
            ProbeResult::NoProvider {
                omp: omp.clone(),
                version: "18.15.0".into()
            }
        );
        let some = FakeOmp::new()
            .on(&["--version"], FakeOmp::ok("omp/18.16.0"))
            .on(&["models", "--json"], FakeOmp::ok(MODELS));
        let ProbeResult::Ready {
            version, models, ..
        } = probe("", &env, &some)
        else {
            panic!("ready expected")
        };
        assert_eq!(version, "18.16.0");
        assert_eq!(models.len(), 2);
    }

    #[test]
    fn commands_run_with_their_limits() {
        let dir = tempfile::tempdir().unwrap();
        let omp = omp_in(dir.path());
        let fake = FakeOmp::new()
            .on(&["--version"], FakeOmp::ok("omp/18.15.0"))
            .on(&["models", "--json"], FakeOmp::ok(MODELS))
            .on(
                &["update", "--check"],
                FakeOmp::ok("Current version: 18.15.0\nAlready up to date"),
            )
            .on(&["update"], FakeOmp::ok(""))
            .on(&["auth-broker", "logout", "anthropic"], FakeOmp::ok(""));
        let _ = probe_found(&omp, &fake);
        check_for_update(&omp, &fake).unwrap();
        run_update(&omp, &fake).unwrap();
        logout(&omp, provider("anthropic").unwrap(), &fake).unwrap();
        let limits: Vec<u64> = fake
            .calls
            .lock()
            .unwrap()
            .iter()
            .map(|(_, limit)| limit.as_secs())
            .collect();
        assert_eq!(limits, [15, 15, 30, 300, 30]);
    }

    #[test]
    fn the_update_check_reads_the_announced_version() {
        assert_eq!(
            parse_update_check(
                "Current version: 18.15.0\n\u{1b}[36mNew version available: 18.16.0\u{1b}[39m\n"
            ),
            Some("18.16.0".into())
        );
        assert_eq!(
            parse_update_check("Current version: 18.15.0\nAlready up to date"),
            None
        );
        assert_eq!(
            parse_update_check("Current version: 18.15.0\nNew version available: 18.15.0"),
            None
        );
        assert_eq!(
            parse_update_check("Current version: 18.15.0\nNew version available: 18.9.0"),
            None
        );
        assert_eq!(parse_update_check("something else"), None);
        assert_eq!(parse_update_check("New version available: soon"), None);
    }

    #[test]
    fn the_text_for_an_omp_that_stays_too_old() {
        assert_eq!(
            too_old_text(Some("18.10.0"), "exit code 1"),
            "the agent 18.10.0 is older than 18.15.0 and could not be updated (exit code 1)."
        );
        assert_eq!(
            too_old_text(None, "timed out."),
            "the agent is older than 18.15.0 and could not be updated (timed out)."
        );
    }

    fn certificate_failure() -> Result<Output, Failure> {
        Err(Failure::Exit {
            code: Some(1),
            stderr: "fetching models\nunable to get local issuer certificate\nclosing\n".into(),
        })
    }

    #[test]
    fn a_certificate_error_replaces_the_reason_with_the_setting_as_it_is_at_the_failure() {
        let dir = tempfile::tempdir().unwrap();
        let omp = omp_in(dir.path());
        let off = FakeOmp::new().on(&["--version"], certificate_failure());
        let ProbeResult::Missing { reason: Some(text) } = probe_found(&omp, &off) else {
            panic!("missing expected")
        };
        assert!(text.contains("turning on System Certificates"), "{text}");
        assert!(!text.ends_with('.'), "{text}");
        let on = FakeOmp::new()
            .certificates_on()
            .on(&["--version"], certificate_failure());
        let ProbeResult::Missing { reason: Some(text) } = probe_found(&omp, &on) else {
            panic!("missing expected")
        };
        assert!(text.contains("even with System Certificates on"), "{text}");
    }

    #[test]
    fn a_failed_update_or_logout_carries_the_explained_reason() {
        let dir = tempfile::tempdir().unwrap();
        let omp = omp_in(dir.path());
        let http = || {
            Err(Failure::Exit {
                code: Some(1),
                stderr: "request failed: HTTP 407\nbye\n".into(),
            })
        };
        let fake = FakeOmp::new()
            .on(&["update"], http())
            .on(&["auth-broker", "logout", "anthropic"], http());
        let expected = "the agent received an HTTP 407 error; a corporate proxy that inspects HTTPS traffic may be blocking the request, and turning on System Certificates in Weaver's settings may fix it";
        assert_eq!(run_update(&omp, &fake), Err(expected.to_owned()));
        assert_eq!(
            logout(&omp, provider("anthropic").unwrap(), &fake),
            Err(expected.to_owned())
        );
        let plain = FakeOmp::new().on(
            &["update"],
            Err(Failure::Exit {
                code: Some(2),
                stderr: "no space left\n".into(),
            }),
        );
        assert_eq!(run_update(&omp, &plain), Err("no space left".to_owned()));
    }
}
