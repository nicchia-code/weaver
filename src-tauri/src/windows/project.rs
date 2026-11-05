//! The hook between the windows and the Machine `project` (task "Progetto e skill installate").
//!
//! The window manager knows nothing about the files Weaver installs for the agent. It tells this
//! trait that a project window opened, that a batch of changes was applied to the picture and that
//! the window closed; [`crate::project_setup::ProjectSetup`] implements it and installs itself with
//! [`WindowManager::set_project`](super::WindowManager::set_project). Until then [`NoProject`]
//! answers, doing nothing. Every method returns quickly: the work goes on in the background.

use std::path::Path;

use crate::pictures::Revision;

pub trait ProjectHooks: Send + Sync {
    /// `window.OPENED`: the picture is ready and the window exists.
    fn window_opened(&self, project_id: &str, root: &Path);

    /// One batch of changes of the project was applied to its picture (`revision` is the picture
    /// it published, `None` when no file differed): `workspace.FILES_CHANGED`, `PROJECT_FOLDER_APPEARED`
    /// and `PROJECT_FOLDER_REMOVED` all arrive here.
    fn batch_applied(&self, project_id: &str, revision: Option<&Revision>);

    /// `desktop.WINDOW_FOCUSED`: the project's window came back to the foreground.
    fn window_focused(&self, _project_id: &str) {}

    /// `window.CLOSED`: nothing is kept for the project.
    fn window_closed(&self, project_id: &str);
}

/// No project hooks.
pub struct NoProject;

impl ProjectHooks for NoProject {
    fn window_opened(&self, _project_id: &str, _root: &Path) {}

    fn batch_applied(&self, _project_id: &str, _revision: Option<&Revision>) {}

    fn window_closed(&self, _project_id: &str) {}
}
