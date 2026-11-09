//! How a decision of the developer (Delete, and later Run, Approve, ...) is applied.
//!
//! Every decision happens whole or changes nothing, and its conditions are checked again at the
//! moment it is applied, never trusted from when its button was enabled. [`decide`] therefore
//! runs the decision in the project's mutation queue, under the project lock, and hands it the
//! project folder and the facts about the agent's work *of that moment*. The decision reads the
//! project again, checks, then writes; a refusal (`Refused`) writes nothing and comes back as
//! an [`AppError`] with [`ErrorCode::Refused`], whose message is the notice the Board shows.
//!
//! On success the picture the decision got back is published inside the same turn of the queue,
//! so pictures follow the order of the decisions, and the one `project-changed` event is sent.

use std::path::Path;
use std::sync::Arc;

use tauri::AppHandle;
use weaver_core::project::board::{AgentFacts, Refused};
use weaver_core::project::changes::MutationError;
use weaver_core::project::image::ProjectImage;

use crate::error::{AppError, ErrorCode};
use crate::events;
use crate::pictures::{Pictures, Revision};
use crate::state::AppState;

/// What a refused or failed decision becomes for the UI.
pub fn app_error(error: MutationError<Refused>) -> AppError {
    match error {
        MutationError::Refused(refused) => AppError::new(ErrorCode::Refused, refused.notice),
        MutationError::Lock(lock) => AppError::from(lock),
        other => AppError::internal(other),
    }
}

/// Applies one decision of `project_id` and publishes the picture it produced. Returns the new
/// revision, `None` when no file ended up different.
///
/// `decision` runs on the blocking pool under the project lock: it receives the project folder
/// and the agent facts read at that moment, and returns the image after its write.
pub async fn decide<F>(
    state: &AppState,
    project_id: &str,
    decision: F,
) -> Result<Option<Revision>, AppError>
where
    F: FnOnce(&Path, &AgentFacts) -> Result<ProjectImage, MutationError<Refused>> + Send + 'static,
{
    let root = state.pictures.get(project_id)?.root;
    let pictures: Arc<Pictures> = state.pictures.clone();
    let work = state.agent_work.get();
    let id = project_id.to_owned();
    state
        .mutations
        .run(project_id, move || {
            let facts = work.facts(&id);
            let image = decision(&root, &facts).map_err(app_error)?;
            pictures.publish(&id, image)
        })
        .await
}

/// [`decide`], then the `project-changed` event of the new picture.
pub async fn decide_and_announce<F>(
    app: &AppHandle,
    state: &AppState,
    project_id: &str,
    decision: F,
) -> Result<(), AppError>
where
    F: FnOnce(&Path, &AgentFacts) -> Result<ProjectImage, MutationError<Refused>> + Send + 'static,
{
    if let Some(revision) = decide(state, project_id, decision).await? {
        events::emit_project_changed(app, &revision.event())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use weaver_core::paths::AppDirs;
    use weaver_core::project::board::delete_task;

    use super::*;
    use crate::agent_work::{AgentWork, Ghost};

    const TASK: &str = "---\ncolumn: todo\ntechnical: false\nrefs: []\njob: jjjjjjjj\ncreated: 2026-11-02T07:00:01.000Z\n---\n# A task\n";

    /// Jobs that work, as the app would know them.
    struct Working(AtomicBool);

    impl AgentWork for Working {
        fn ghosts(&self, _: &str, _: &weaver_core::project::image::ProjectImage) -> Vec<Ghost> {
            Vec::new()
        }
        fn facts(&self, _: &str) -> AgentFacts {
            let mut facts = AgentFacts::default();
            if self.0.load(Ordering::SeqCst) {
                facts.working_jobs.insert("jjjjjjjj".to_owned());
            }
            facts
        }
    }

    fn state_with_task(base: &Path, project: &Path) -> AppState {
        let file = project.join(".weaver/tasks/aaaaaaaa-a-task.md");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, TASK).unwrap();
        AppState::new(AppDirs::from_bases(base, base, None))
    }

    #[tokio::test]
    async fn a_decision_refused_when_applied_writes_nothing_and_says_why() {
        let (base, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let state = state_with_task(base.path(), project.path());
        let id = state
            .pictures
            .open(project.path().to_path_buf())
            .await
            .unwrap();
        // The button was enabled when the developer saw the Board; the agent is working now.
        state
            .agent_work
            .set(Arc::new(Working(AtomicBool::new(true))));

        let error = decide(&state, &id, |root, facts| {
            delete_task(root, facts, "aaaaaaaa")
        })
        .await
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::Refused);
        assert_eq!(
            error.message,
            "Could not delete T1: the agent that filed it is still working."
        );
        assert!(project
            .path()
            .join(".weaver/tasks/aaaaaaaa-a-task.md")
            .exists());
        assert!(
            !project.path().join(".weaver/short-numbers").exists(),
            "a refusal leaves not even the cache"
        );
        assert_eq!(
            state.pictures.get(&id).unwrap().revision,
            1,
            "no new picture"
        );
    }

    #[tokio::test]
    async fn an_applied_decision_publishes_the_picture_it_wrote() {
        let (base, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let state = state_with_task(base.path(), project.path());
        let id = state
            .pictures
            .open(project.path().to_path_buf())
            .await
            .unwrap();
        state
            .agent_work
            .set(Arc::new(Working(AtomicBool::new(false))));

        let revision = decide(&state, &id, |root, facts| {
            delete_task(root, facts, "aaaaaaaa")
        })
        .await
        .unwrap();
        assert_eq!(
            revision,
            Some(Revision {
                project_id: id.clone(),
                revision: 2,
                slices: vec!["board".into()]
            })
        );
        assert!(!project
            .path()
            .join(".weaver/tasks/aaaaaaaa-a-task.md")
            .exists());
        assert!(state.pictures.get(&id).unwrap().image.cards().is_empty());
    }

    #[tokio::test]
    async fn a_project_that_is_not_open_is_refused_before_anything_runs() {
        let base = tempfile::tempdir().unwrap();
        let state = AppState::new(AppDirs::from_bases(base.path(), base.path(), None));
        let error = decide(&state, "0123456789abcdef", |_, _| {
            unreachable!("never runs")
        })
        .await
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidInput);
    }
}
