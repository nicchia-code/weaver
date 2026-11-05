//! omp, the agent Weaver runs: finding it, asking about it, keeping the app-wide answer to "is omp
//! ready" (`omp-setup`) and opening the system terminal for its installer and login.
//!
//! Synchronous `std` code with no Tauri dependency: the app's `omp` module executes the effects of
//! the [`machine`] and tells the UI.
//!
//! | Module        | Responsibility |
//! |---------------|----------------|
//! | [`version`]   | `MIN_OMP_VERSION`, semver comparison, reading `omp --version` |
//! | [`providers`] | The fixed list of providers omp can log in to |
//! | [`failure`]   | Reasons of a failed command and the explanations of certificate and HTTP errors |
//! | [`github`]    | omp's GitHub releases: the download script of the installer's terminal and of the background update |
//! | [`installer`] | The marker where omp's installer leaves its exit code |
//! | [`run`]       | omp commands with time limits ([`run::Exec`], on `process::timed`) |
//! | [`path_env`]  | The login PATH, the Windows registry PATH, the PATH of a child |
//! | [`lookup`]    | Where omp is: `ompPath`, PATH, the installer folders |
//! | [`probe`]     | The check (`--version`, `models --json`), update check, update, logout |
//! | [`terminal`]  | Quoting and launch of the system terminal |
//! | [`usage`]     | `omp usage --json` and its parsing: limits of the connected providers |
//! | [`machine`]   | The Machine `omp-setup`: events in, effects out, no I/O and no clock |

pub mod failure;
pub mod github;
pub mod installer;
pub mod lookup;
pub mod machine;
pub mod path_env;
pub mod probe;
pub mod providers;
pub mod run;
pub mod terminal;
pub mod usage;
pub mod version;

pub use lookup::{find_omp, LookupEnv, Os};
pub use probe::{OmpModel, ProbeResult};
pub use providers::{provider, Provider, ProviderRow, PROVIDERS};
pub use version::MIN_OMP_VERSION;
