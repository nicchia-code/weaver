//! Design requests: the Markdown document in which the developer asks for a design
//! (`requirements/data#design-request`), and the Machine `design-request` that governs it.
//!
//! | Module | Responsibility |
//! |--------|----------------|
//! | [`machine`] | The states and events of the Machine, as one pure function |
//! | [`id`] | The id of a request: the local time of its creation |
//! | [`title`] | Empty or not, and the title of a text |
//! | [`image`] | Accepted images, their names, base64 for the interface |
//! | [`store`] | The files of the requests of a project and the operations on them |
//!
//! A request is local to the machine of the developer: `.weaver/requests/` is never versioned and
//! the project image never reads it. The app runs the operations that write inside the project's
//! mutation queue.

pub mod id;
pub mod image;
pub mod machine;
pub mod store;
pub mod title;

pub use id::LocalTime;
pub use machine::{step, Effect, RequestEvent, RequestState, Step};
pub use store::{
    job_for, model_is_empty, AgentSend, RequestEntry, RequestError, RequestStore, RequestsState,
    SendPlan,
};
