//! Ask more (`requirements/tech#ask-conversation`): a free conversation with omp about one thing the
//! developer is reading.
//!
//! It is not a job: no registry record, no `JobKind`, no admission, nothing in the working-job
//! count of a window close, no notification. The conversation itself runs on the service and the
//! actor of General, generalised to several conversations ([`crate::general`]); this module reads
//! the chosen thing and opens it.
//!
//! | Module | Responsibility |
//! |--------|----------------|
//! | [`build`] | Reads the chosen thing (files, or the round or the card of the Chat in memory) and builds the name and the instruction |
//! | [`commands`] | The Tauri commands: open, the conversation and the decisions of the developer in it |
//!
//! Opening sends nothing and starts no omp: the instruction waits in the conversation until the
//! first message goes out with it. Neither the instruction nor any prompt is written to disk or
//! logged.

pub mod build;
pub mod commands;

#[cfg(test)]
mod tests;

use std::path::Path;
use std::sync::Arc;

use crate::general::service::{AskOpening, General, Rebuild};
use crate::jobs::host::Host;
use crate::rounds::Rounds;
use build::{AskTarget, Sources};

/// What the Settings say the developer's language is, when one is set.
fn language_of(host: &dyn Host) -> Option<String> {
    Some(host.settings().language.trim().to_owned()).filter(|language| !language.is_empty())
}

/// `developer.ASK_MORE`: reads the chosen thing and opens its conversation. `None` when the thing
/// no longer exists: nothing opens. Blocks on the disk.
pub fn open(
    general: &Arc<General>,
    rounds: &Arc<Rounds>,
    project_id: &str,
    root: &Path,
    target: &AskTarget,
) -> Option<String> {
    let language = language_of(general.host().as_ref());
    let built = build::build(
        &Sources {
            project_id,
            root,
            rounds,
            cards: general.as_ref(),
            language: language.as_deref(),
        },
        target,
    )?;
    // The instruction built again, when the session of the conversation is lost and replaced: from
    // the files at that moment, and the settings of that moment.
    let rebuild: Rebuild = {
        // Weak: the conversation lives in the General that this closure is kept by.
        let (host, rounds, cards) = (
            general.host().clone(),
            rounds.clone(),
            Arc::downgrade(general),
        );
        let (project, root, target) = (project_id.to_owned(), root.to_path_buf(), target.clone());
        Arc::new(move || {
            let cards = cards.upgrade()?;
            let language = language_of(host.as_ref());
            build::build(
                &Sources {
                    project_id: &project,
                    root: &root,
                    rounds: &rounds,
                    cards: cards.as_ref(),
                    language: language.as_deref(),
                },
                &target,
            )
            .map(|built| built.instruction)
        })
    };
    Some(general.open_ask(AskOpening {
        project_id: project_id.to_owned(),
        root: root.to_path_buf(),
        name: built.name,
        kind: built.kind,
        round: built.round,
        card: built.card,
        instruction: built.instruction,
        rebuild,
    }))
}
