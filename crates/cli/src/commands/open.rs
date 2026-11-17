//! `weaver open [<folder>]`: the developer's terminal asks the running Weaver to open a folder or to
//! show its welcome window. The registry holds its help and its schemas; running it is
//! [`crate::open`], which sends the request kind `open` and never goes through the handlers of the
//! registry. It is the one command with no verb, and agents cannot run it, so it is never in the
//! command reference of the installed skills.

use super::local;
use crate::spec::*;

pub fn commands() -> Vec<Command> {
    vec![Command::new(
        "open",
        "",
        "Opens a folder in the running Weaver, or shows its welcome window.",
        not_for_the_registry,
    )
    .notes(&[
        "Two forms: `weaver open <folder>` opens the folder as a project (its window comes to the front when it is open already), and `weaver open` with no folder shows the welcome window.",
        "Weaver must already be running: the command never starts it. A folder with no `.weaver/` is accepted; its window opens on Get Started. After `open`, every word is a folder, also one named like an object. It takes no input and no root.",
        "It prints nothing on success; with `--json`, an empty object.",
    ])
    .arg(Arg::Folder)
    .access(Access::Developer)
    .place(local)
    .example("weaver open ~/projects/shop")]
}

/// `open` is not run by the handlers of the registry: it is sent to the app by [`crate::open`].
fn not_for_the_registry(_call: &Call) -> Result<weaver_core::command::CommandReply, Failure> {
    Err(Failure::refused(weaver_core::open::AGENT_REFUSAL))
}
