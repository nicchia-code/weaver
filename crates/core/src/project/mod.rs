//! Project files: the one code that reads and writes `.weaver/`.
//!
//! Everything here is a pure, tested function over text and paths, plus the in-memory
//! [`image::ProjectImage`]. The app and the `weaver` command line both go through it; the
//! interface never touches a file. Time and randomness are injected ([`clock`], [`random`]).
//!
//! | Module | Responsibility |
//! |--------|----------------|
//! | [`text`] | The one read function (BOM and CRLF removed), hashes, LF writes |
//! | [`clock`] | Injected clock and UTC ISO 8601 instants with milliseconds |
//! | [`random`] | Injected random source (CSPRNG in production) |
//! | [`ids`] | Identities `^[a-z0-9]{8}$`, slugs, `<id>-<slug>` names |
//! | [`frontmatter`] | YAML front matter: ordered keys, unknown keys kept |
//! | [`diagnostic`] | A file or rule that failed: path, severity, reason |
//! | [`layout`] | The layout of `.weaver/`, what each path is, local-only folders |
//! | [`model`] | System, Actors, Features, Machines (`FORMAT.md`) |
//! | [`requirements`] | Areas, fragments and sections |
//! | [`requests`] | Design requests: the Machine `design-request`, their files and `.state.json` |
//! | [`specs`] | `specs.yaml` |
//! | [`refs`] | Element, Requirement and Specs references, and their resolution |
//! | [`resolve`] | Naming a card on the command line: identity, unique prefix or short number |
//! | [`check`] | Cross-file rules of `weaver model check` |
//! | [`cards`] | What every card file shares |
//! | [`order`] | Creation order of cards |
//! | [`shortnum`] | Short numbers `P`/`T`/`D` and their local cache |
//! | [`changes`] | All-or-nothing multi-file changes |
//! | [`image`] | The incremental in-memory image of a project |
//! | [`concerns`] | "Concerns the same thing": the one function of blocking, slots, groups and Drift lookups |
//! | [`board`] | The Board: Backlog and Active columns, blocking of Tasks, Run all reasons, Delete |
//! | [`rank`] | The order of the Board's two columns: actionable first, what unblocks first, the place of the ghosts |
//! | [`history`] | History entries: writing them, citing Tasks, the copy kept when a Task is recorded |
//! | [`browse`] | Requirement sections that apply to an element, text search of the Model |
//! | [`init`] | What a new project holds: the System and the starting Areas |
//! | [`proposal`] | Proposals: scope, coverage, staleness, reasons, hand-over, approval, rejection, refinement |
//! | [`drift`] | Drifts: the fingerprint, the prepared question, new and edit, the decisions and their reasons |
//! | [`model_view`] | The text and JSON of `weaver model show`, the text a fingerprint is taken from |
//! | [`tasks`] | Tasks: blocking, take/release/finish, slots, groups, Run all, creation, forgetting |
//! | [`view`] | What the Requirements and Specs views show, with the overlay changes |

pub mod board;
pub mod browse;
pub mod cards;
pub mod changes;
pub mod check;
pub mod clock;
pub mod concerns;
pub mod diagnostic;
pub mod drift;
pub mod frontmatter;
pub mod history;
pub mod ids;
pub mod image;
pub mod init;
pub mod layout;
pub(crate) mod md;
pub mod model;
pub mod model_view;
pub mod order;
pub mod proposal;
pub mod random;
pub mod rank;
pub mod refs;
pub mod requests;
pub mod requirements;
pub mod resolve;
pub mod shortnum;
pub mod specs;
pub mod tasks;
#[cfg(test)]
pub(crate) mod testing;
pub mod text;
pub mod view;
