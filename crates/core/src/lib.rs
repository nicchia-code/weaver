//! `weaver-core`: the library shared by the Weaver app and the `weaver` command line.
//!
//! It has no Tauri dependency, no async runtime and does synchronous `std` I/O only.
//! Modules extended by later work (project files, rules, mutations, skills) are added
//! next to the ones below; each module keeps one responsibility.
//!
//! | Module      | Responsibility |
//! |-------------|----------------|
//! | [`paths`]   | App identifier, config / local data / runtime folders, project hash |
//! | [`atomic`]  | Atomic file writes (temp file + rename), optional Unix mode |
//! | [`endpoint`]| CSPRNG helpers, socket / named-pipe endpoint naming |
//! | [`runtime`] | `runtime.json`: `{endpoint, secret, pid, version}` |
//! | [`protocol`]| CLI↔app messages: framing, size limit, protocol version, secret check |
//! | [`ipc`]     | Local-socket transport (`interprocess`) and one-request-per-connection serving |
//! | [`lock`]    | OS advisory project lock shared by the app and the CLI |
//! | [`process`] | The one function every child process is started with, and the time-limited runner (`process::timed`) |
//! | [`project`] | Project files under `.weaver/`: parsers, rules, changes, in-memory image |
//! | [`settings`]| Weaver's settings: typed schema, defaults, validation, `settings.json` store |
//! | [`agent_files`] | The texts installed for the agent (skills, sub-agent definitions, advisor instructions), embedded |
//! | [`job_instruction`] | The one-line instruction of each job |
//! | [`command`] | The request kind `command`: a `weaver` command run by the app, and its reply |
//! | [`jobs`]    | Jobs of the agent: kinds, the Machine `job`, admission, launch, registry record |
//! | [`process_group`] | Children that never outlive Weaver, ended as a group |
//! | [`omp`]     | omp: lookup, version, models, the Machine `omp-setup`, the system terminal |
//! | [`notification`] | System notifications (Machine `notification`): foreground test, texts, grouping, click routing |
//! | [`project_files`] | Files Weaver installs for the agent: generated Requirement skills, initialisation, sync |
//! | [`question`] | Question rounds (Machine `question-round`): request validation, the round, the queue of a project |
//! | [`system_certificates`] | System Certificates: the bundle of the certificates the system trusts, its export, the variables of omp processes, the Machine `system-certificates` |
//! | [`file_tab`] | The files open in the tabs of the File view: checked paths, reading, saving, tab rules |
//! | [`file_lock`] | The Machine `file-lock`: which files open in a tab an agent is changing |
//! | [`file_view`] | The File view's tree: listing, create / rename / move / delete, the Machine `file-view` |
//! | [`assets`] | The developer's assets in `.weaver/assets/`: the list, copy in, rename, delete, the paths of a drag out |
//! | [`notes`] | The developer's notes in `.weaver/notes/`: reading and writing the sheet, naming, copying in, reading and trashing the images, the sheet's hash and the images' names the watcher reports |
//! | [`solo_mode`] | Solo mode: the state of the working copy, the block in git's local exclusions, the tracked files it cannot hide |
//! | [`solo_merge`] | Merge in Solo mode: availability, the branches, the git steps of a merge, `merge/merge.json` and the check of a merge in progress |
//! | [`git_info`] | What a project window shows of git: the branch, git's HEAD file of the working copy, the user's name |

pub mod agent_files;
pub mod assets;
pub mod atomic;
pub mod atomic_commits;
pub mod command;
pub mod endpoint;
pub mod file_lock;
pub mod file_tab;
pub mod file_view;
pub mod git_info;
pub mod history_commit;
pub mod ipc;
pub mod job_instruction;
pub mod jobs;
pub mod lock;
pub mod notes;
pub mod notification;
pub mod omp;
pub mod paths;
pub mod process;
pub mod process_group;
pub mod project;
pub mod project_files;
pub mod protocol;
pub mod question;
pub mod runtime;
pub mod settings;
pub mod solo_merge;
pub mod solo_mode;
pub mod system_certificates;
