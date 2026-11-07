//! System notifications (Machine `notification`, feature `app`): the pure part.
//!
//! A notification tells the developer that it is their turn: a job finished, stopped abnormally,
//! or waits for their answers or their permission. It is shown at once, and only when the window
//! of that project is not in the foreground. It names the project, the job and what happened,
//! never what the agent wrote, asked or did. Jobs the developer stopped never notify, and the Chat
//! of a project (General) raises none.
//!
//! This module decides *whether* to notify ([`in_foreground`], [`decide_end`], [`Focus`]), *what*
//! to say ([`Book`] builds the [`Notice`] with the exact texts of `requirements/ui#notification`),
//! how notifications of one project *group* and where a *click* leads ([`Book::clicked`]). It has
//! no clock, no randomness and no I/O: the ids come from the counter of the [`Book`]. The app
//! (`src-tauri/src/notifications.rs`) sends them through `tauri-plugin-notification`.

use std::collections::HashMap;

use crate::jobs::JobState;

/// The message of a notification that says a job finished.
pub const MESSAGE_FINISHED: &str = "Weaver is waiting for you.";
/// The message of a notification that says a job stopped before finishing.
pub const MESSAGE_FAILED: &str = "The agent stopped before finishing its work.";
/// The message of a notification that says a round of questions waits.
pub const MESSAGE_QUESTION: &str = "The agent is waiting for your answers.";
/// The message of a notification that says a permission waits.
pub const MESSAGE_PERMISSION: &str = "The agent is waiting for your approval.";
/// The title of the notification that says Weaver turned on System Certificates by itself.
pub const TITLE_SYSTEM_CERTIFICATES: &str = "System Certificates turned on";
/// The message of that notification (`requirements/ui#system-certificates`, Turn-on notice).
pub const MESSAGE_SYSTEM_CERTIFICATES: &str = "Weaver turned on System Certificates because Zscaler is installed: the agent and its commands now trust the certificates your system trusts.";

/// What the system says of a window at the time a notification would be sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowFacts {
    pub visible: bool,
    pub minimized: bool,
    /// `is_focused()`, or the last `WindowEvent::Focused` when the window was seen ([`Focus::resolve`]).
    pub focused: bool,
}

/// The project window is in the foreground: visible, not minimised and focused. Another window of
/// Weaver having the focus does not count (the facts are those of the project's own window).
pub fn in_foreground(window: WindowFacts) -> bool {
    window.visible && !window.minimized && window.focused
}

/// The last `WindowEvent::Focused` of each window, by label.
#[derive(Debug, Default)]
pub struct Focus {
    known: HashMap<String, bool>,
}

impl Focus {
    /// `WindowEvent::Focused(focused)` of the window `label`.
    pub fn changed(&mut self, label: &str, focused: bool) {
        self.known.insert(label.to_owned(), focused);
    }

    /// The window is gone.
    pub fn destroyed(&mut self, label: &str) {
        self.known.remove(label);
    }

    /// Whether the window has the focus: what the events said last, and what the system answers
    /// (`system`) for a window whose events were never seen.
    pub fn resolve(&self, label: &str, system: bool) -> bool {
        self.known.get(label).copied().unwrap_or(system)
    }
}

/// What a notification says happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Finished,
    Failed,
    Question,
    Permission,
    /// Weaver turned on System Certificates by itself. It belongs to no project and is always shown.
    SystemCertificates,
}

/// `job.ENDED`: what the end of a job says, if anything. A job the developer stopped never notifies.
pub fn kind_of_end(state: JobState) -> Option<Kind> {
    match state {
        JobState::Done => Some(Kind::Finished),
        JobState::Failed => Some(Kind::Failed),
        _ => None,
    }
}

/// `job.ENDED` of a job in `state`, with the project window in the foreground or not: the kind to
/// show, or nothing.
pub fn decide_end(state: JobState, foreground: bool) -> Option<Kind> {
    if foreground {
        None
    } else {
        kind_of_end(state)
    }
}

/// A job waits for the developer (`job.WAITING`, a round of questions or a permission): shown
/// only when the project window is not in the foreground.
pub fn decide_wait(foreground: bool) -> bool {
    !foreground
}

/// What the system shows: the text of a notification and the id that replaces or withdraws it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub id: i32,
    pub title: String,
    pub message: &'static str,
}

/// Where a click leads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Click {
    /// A notification of a project: its window comes forward showing `destination`.
    Project {
        project_id: String,
        destination: Destination,
    },
    /// The notification of the app (System Certificates turned on), which belongs to no project:
    /// the project window used most recently comes forward showing Settings, else the welcome window.
    App,
}

/// What the window of a project shows when a notification is clicked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Destination {
    /// The window only comes forward: the notification was withdrawn and the system could not remove it.
    WindowOnly,
    /// The Jobs view, in the conversation of `job_id` (none for several jobs: the view itself; a round
    /// of questions is the overlay on top of it; a permission is the card the conversation ends with).
    Jobs { job_id: Option<String> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Target {
    /// One or several jobs finished; `first` names the first while it is the only one.
    Finished {
        jobs: Vec<String>,
        first: String,
    },
    Failed {
        job: String,
        name: String,
    },
    Question {
        job: String,
        name: String,
    },
    Permission {
        job: String,
        name: String,
        request: u64,
    },
    /// System Certificates turned on: the notification of the app, never grouped nor withdrawn.
    SystemCertificates,
}

impl Target {
    fn kind(&self) -> Kind {
        match self {
            Target::Finished { .. } => Kind::Finished,
            Target::Failed { .. } => Kind::Failed,
            Target::Question { .. } => Kind::Question,
            Target::Permission { .. } => Kind::Permission,
            Target::SystemCertificates => Kind::SystemCertificates,
        }
    }

    fn job(&self) -> Option<&str> {
        match self {
            Target::Finished { jobs, .. } if jobs.len() == 1 => jobs.first().map(String::as_str),
            Target::Finished { .. } => None,
            Target::Failed { job, .. }
            | Target::Question { job, .. }
            | Target::Permission { job, .. } => Some(job),
            Target::SystemCertificates => None,
        }
    }

    /// The exact texts of `requirements/ui#notification`.
    fn text(&self, project: &str) -> (String, &'static str) {
        match self {
            Target::Finished { jobs, first } if jobs.len() == 1 => {
                (format!("{project} – {first} finished"), MESSAGE_FINISHED)
            }
            Target::Finished { jobs, .. } => (
                format!("{project} – {} jobs finished", jobs.len()),
                MESSAGE_FINISHED,
            ),
            Target::Failed { name, .. } => (format!("{project} – {name} stopped"), MESSAGE_FAILED),
            Target::Question { name, .. } => (
                format!("{project} – {name} asks a question"),
                MESSAGE_QUESTION,
            ),
            Target::Permission { name, .. } => (
                format!("{project} – {name} asks permission"),
                MESSAGE_PERMISSION,
            ),
            Target::SystemCertificates => (
                TITLE_SYSTEM_CERTIFICATES.to_owned(),
                MESSAGE_SYSTEM_CERTIFICATES,
            ),
        }
    }

    /// Whether `other` replaces this notification: the same project, job and kind (project and
    /// kind only for the finished ones, which group), and the same request for a permission.
    fn replaced_by(&self, other: &Target) -> bool {
        match (self, other) {
            (Target::Finished { .. }, Target::Finished { .. }) => true,
            (Target::Failed { job: a, .. }, Target::Failed { job: b, .. }) => a == b,
            (Target::Question { job: a, .. }, Target::Question { job: b, .. }) => a == b,
            (
                Target::Permission {
                    job: a, request: x, ..
                },
                Target::Permission {
                    job: b, request: y, ..
                },
            ) => a == b && x == y,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Shown {
    id: i32,
    /// `None` for the notification of the app, which belongs to no project.
    project_id: Option<String>,
    project: String,
    target: Target,
    /// No longer true. Where the system cannot remove a delivered notification the mapping stays
    /// bare, so that a later click only brings the window forward.
    withdrawn: bool,
}

impl Shown {
    fn of_project(&self, project_id: &str) -> bool {
        self.project_id.as_deref() == Some(project_id)
    }
}

/// The notifications shown (or believed shown) and the ids they were given: the mapping of an id
/// to its project and target that click routing needs. All of a project's entries go when its window closes.
#[derive(Debug)]
pub struct Book {
    next: i32,
    shown: Vec<Shown>,
    /// The system can remove a delivered notification (Linux); elsewhere a withdrawn one keeps a bare mapping.
    removable: bool,
}

impl Default for Book {
    fn default() -> Self {
        Self::new(false)
    }
}

impl Book {
    /// An empty book; the counter starts at 1. `removable`: the system can remove a delivered notification.
    pub fn new(removable: bool) -> Self {
        Self {
            next: 1,
            shown: Vec::new(),
            removable,
        }
    }

    /// The next id of the app-wide counter, never one in use.
    fn next_id(&mut self) -> i32 {
        loop {
            let id = self.next;
            self.next = if id == i32::MAX { 1 } else { id + 1 };
            if !self.shown.iter().any(|shown| shown.id == id) {
                return id;
            }
        }
    }

    /// Records `target`: it takes the place (and the id) of the notification it replaces, or gets a new id.
    fn post(&mut self, project_id: &str, project: &str, target: Target) -> Notice {
        let (title, message) = target.text(project);
        let position = self.shown.iter().position(|shown| {
            !shown.withdrawn && shown.of_project(project_id) && shown.target.replaced_by(&target)
        });
        let id = match position {
            Some(at) => self.shown[at].id,
            None => self.next_id(),
        };
        let entry = Shown {
            id,
            project_id: Some(project_id.to_owned()),
            project: project.to_owned(),
            target,
            withdrawn: false,
        };
        match position {
            Some(at) => self.shown[at] = entry,
            None => self.shown.push(entry),
        }
        Notice { id, title, message }
    }

    /// Weaver turned on System Certificates by itself (`system-certificates.AUTO_ENABLED`). The
    /// notification of the app: it gets an id of the counter but belongs to no project, so it is
    /// never grouped, never withdrawn by the events of a job and survives the closing of a window.
    pub fn system_certificates(&mut self) -> Notice {
        let target = Target::SystemCertificates;
        let (title, message) = target.text("");
        let id = self.next_id();
        self.shown.push(Shown {
            id,
            project_id: None,
            project: String::new(),
            target,
            withdrawn: false,
        });
        Notice { id, title, message }
    }

    /// A job finished. While a finished notification of the same project is shown it is replaced
    /// by `<n> jobs finished`: at most one per project.
    pub fn finished(&mut self, project_id: &str, project: &str, job: &str, name: &str) -> Notice {
        let mut jobs = self
            .shown
            .iter()
            .find_map(|shown| match &shown.target {
                Target::Finished { jobs, .. }
                    if !shown.withdrawn && shown.of_project(project_id) =>
                {
                    Some(jobs.clone())
                }
                _ => None,
            })
            .unwrap_or_default();
        if !jobs.iter().any(|known| known == job) {
            jobs.push(job.to_owned());
        }
        let first = if jobs.len() == 1 {
            name.to_owned()
        } else {
            String::new()
        };
        self.post(project_id, project, Target::Finished { jobs, first })
    }

    /// A job stopped before finishing (one notification per job).
    pub fn failed(&mut self, project_id: &str, project: &str, job: &str, name: &str) -> Notice {
        self.post(
            project_id,
            project,
            Target::Failed {
                job: job.to_owned(),
                name: name.to_owned(),
            },
        )
    }

    /// A round of questions of a job waits.
    pub fn question(&mut self, project_id: &str, project: &str, job: &str, name: &str) -> Notice {
        self.post(
            project_id,
            project,
            Target::Question {
                job: job.to_owned(),
                name: name.to_owned(),
            },
        )
    }

    /// A permission asked by a job waits.
    pub fn permission(
        &mut self,
        project_id: &str,
        project: &str,
        job: &str,
        name: &str,
        request: u64,
    ) -> Notice {
        self.post(
            project_id,
            project,
            Target::Permission {
                job: job.to_owned(),
                name: name.to_owned(),
                request,
            },
        )
    }

    /// Withdraws what `matches`: the system removes it where it can, elsewhere the mapping stays bare.
    /// The ids newly withdrawn.
    fn withdraw(&mut self, matches: impl Fn(&Shown) -> bool) -> Vec<i32> {
        let removable = self.removable;
        let mut ids = Vec::new();
        for shown in self
            .shown
            .iter_mut()
            .filter(|shown| !shown.withdrawn && matches(shown))
        {
            ids.push(shown.id);
            shown.withdrawn = true;
        }
        if removable {
            self.shown.retain(|shown| !shown.withdrawn);
        }
        ids
    }

    /// `question-round.CLOSED`: the round the notification announced closed.
    pub fn round_closed(&mut self, project_id: &str, job: &str) -> Vec<i32> {
        self.withdraw(|shown| {
            shown.of_project(project_id)
                && matches!(&shown.target, Target::Question { job: j, .. } if j == job)
        })
    }

    /// `developer.ANSWER_PERMISSION`: the permission the notification announced was answered.
    pub fn permission_answered(&mut self, project_id: &str, job: &str, request: u64) -> Vec<i32> {
        self.withdraw(|shown| {
            shown.of_project(project_id)
                && matches!(&shown.target, Target::Permission { job: j, request: r, .. } if j == job && *r == request)
        })
    }

    /// `job.ENDED`: the waits the job announced are no longer true. What it announced about its
    /// end (finished, stopped) stays.
    pub fn job_ended(&mut self, project_id: &str, job: &str) -> Vec<i32> {
        self.withdraw(|shown| {
            shown.of_project(project_id)
                && matches!(&shown.target, Target::Question { job: j, .. } | Target::Permission { job: j, .. } if j == job)
        })
    }

    /// The window of the project closed: every mapping of the project is dropped. The ids that were
    /// not withdrawn already.
    pub fn project_closed(&mut self, project_id: &str) -> Vec<i32> {
        let live = self
            .shown
            .iter()
            .filter(|shown| shown.of_project(project_id) && !shown.withdrawn)
            .map(|shown| shown.id)
            .collect();
        self.shown.retain(|shown| !shown.of_project(project_id));
        live
    }

    /// `desktop.NOTIFICATION_CLICKED`: where the click leads. A click on an id that is not mapped
    /// (withdrawn, clicked before, never ours) leads nowhere. A clicked notification is final: its
    /// mapping is dropped.
    pub fn clicked(&mut self, id: i32) -> Option<Click> {
        let at = self.shown.iter().position(|shown| shown.id == id)?;
        let shown = self.shown.remove(at);
        let Some(project_id) = shown.project_id else {
            return Some(Click::App);
        };
        let destination = if shown.withdrawn {
            Destination::WindowOnly
        } else {
            Destination::Jobs {
                job_id: shown.target.job().map(str::to_owned),
            }
        };
        Some(Click::Project {
            project_id,
            destination,
        })
    }

    /// The kind of the notification mapped to `id`.
    pub fn kind_of(&self, id: i32) -> Option<Kind> {
        self.shown
            .iter()
            .find(|shown| shown.id == id)
            .map(|shown| shown.target.kind())
    }

    /// How many notifications are mapped.
    pub fn len(&self) -> usize {
        self.shown.len()
    }

    pub fn is_empty(&self) -> bool {
        self.shown.is_empty()
    }

    /// The project name a notification was written with (the folder name), for the tests of grouping.
    pub fn project_of(&self, id: i32) -> Option<&str> {
        self.shown
            .iter()
            .find(|shown| shown.id == id)
            .map(|shown| shown.project.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ACME: &str = "aaaa0000aaaa0000";
    const OTHER: &str = "bbbb1111bbbb1111";

    fn window(visible: bool, minimized: bool, focused: bool) -> WindowFacts {
        WindowFacts {
            visible,
            minimized,
            focused,
        }
    }

    #[test]
    fn only_a_visible_unminimised_focused_window_is_in_the_foreground() {
        assert!(in_foreground(window(true, false, true)));
        assert!(!in_foreground(window(false, false, true)));
        assert!(!in_foreground(window(true, true, true)));
        assert!(
            !in_foreground(window(true, false, false)),
            "another window of Weaver having the focus does not count"
        );
    }

    #[test]
    fn the_last_focus_event_of_a_window_wins_over_what_the_system_answers() {
        let mut focus = Focus::default();
        assert!(
            focus.resolve("p-a", true),
            "a window whose events were never seen follows the system"
        );
        assert!(!focus.resolve("p-a", false));
        focus.changed("p-a", true);
        assert!(focus.resolve("p-a", false));
        focus.changed("p-a", false);
        focus.changed("p-b", true);
        assert!(
            !focus.resolve("p-a", true),
            "the focus moved to another window"
        );
        assert!(focus.resolve("p-b", false));
        focus.destroyed("p-b");
        assert!(!focus.resolve("p-b", false));
    }

    #[test]
    fn a_finished_or_failed_job_notifies_only_in_the_background_and_a_stopped_one_never() {
        assert_eq!(decide_end(JobState::Done, false), Some(Kind::Finished));
        assert_eq!(decide_end(JobState::Failed, false), Some(Kind::Failed));
        assert_eq!(
            decide_end(JobState::Stopped, false),
            None,
            "the developer stopped it"
        );
        assert_eq!(decide_end(JobState::Done, true), None);
        assert_eq!(decide_end(JobState::Failed, true), None);
        for state in [
            JobState::Queued,
            JobState::Running,
            JobState::Waiting,
            JobState::Suspended,
        ] {
            assert_eq!(decide_end(state, false), None, "{state:?} is not an end");
        }
        assert!(decide_wait(false));
        assert!(!decide_wait(true));
    }

    #[test]
    fn each_kind_has_its_exact_text_and_never_anything_the_agent_said() {
        let mut book = Book::new(true);
        let finished = book.finished(ACME, "Acme", "job1", "Build the cart");
        assert_eq!(
            (finished.title.as_str(), finished.message),
            (
                "Acme – Build the cart finished",
                "Weaver is waiting for you."
            )
        );
        let failed = book.failed(ACME, "Acme", "job2", "Scan");
        assert_eq!(
            (failed.title.as_str(), failed.message),
            (
                "Acme – Scan stopped",
                "The agent stopped before finishing its work."
            )
        );
        let question = book.question(ACME, "Acme", "job3", "Import");
        assert_eq!(
            (question.title.as_str(), question.message),
            (
                "Acme – Import asks a question",
                "The agent is waiting for your answers."
            )
        );
        let permission = book.permission(ACME, "Acme", "job4", "Run T3", 7);
        assert_eq!(
            (permission.title.as_str(), permission.message),
            (
                "Acme – Run T3 asks permission",
                "The agent is waiting for your approval."
            )
        );
    }

    #[test]
    fn ids_come_from_one_counter_and_are_never_reused_while_mapped() {
        let mut book = Book::new(true);
        let a = book.failed(ACME, "Acme", "job1", "A").id;
        let b = book.failed(OTHER, "Other", "job2", "B").id;
        assert_ne!(a, b);
        assert_eq!(book.len(), 2);
        book.next = i32::MAX;
        let c = book.question(ACME, "Acme", "job3", "C").id;
        let d = book.question(ACME, "Acme", "job4", "D").id;
        assert_eq!(c, i32::MAX);
        assert_eq!(d, 3, "the counter wraps and skips the ids in use");
    }

    #[test]
    fn a_job_finishing_while_a_finished_notification_is_shown_replaces_it_with_the_count() {
        let mut book = Book::new(true);
        let first = book.finished(ACME, "Acme", "job1", "Build");
        let second = book.finished(ACME, "Acme", "job2", "Scan");
        assert_eq!(second.id, first.id, "it replaces the shown one");
        assert_eq!(second.title, "Acme – 2 jobs finished");
        let third = book.finished(ACME, "Acme", "job3", "Run");
        assert_eq!(
            (third.id, third.title.as_str()),
            (first.id, "Acme – 3 jobs finished")
        );
        assert_eq!(
            book.len(),
            1,
            "at most one finished notification per project"
        );
        let elsewhere = book.finished(OTHER, "Other", "job9", "Build");
        assert_ne!(elsewhere.id, first.id, "projects never group together");
        assert_eq!(elsewhere.title, "Other – Build finished");
        let again = book.finished(ACME, "Acme", "job3", "Run");
        assert_eq!(
            again.title, "Acme – 3 jobs finished",
            "the same job is not counted twice"
        );
    }

    #[test]
    fn failed_questions_and_permissions_are_one_per_event_and_never_grouped() {
        let mut book = Book::new(true);
        book.finished(ACME, "Acme", "job1", "Build");
        let one = book.failed(ACME, "Acme", "job2", "Scan");
        let two = book.failed(ACME, "Acme", "job3", "Run");
        assert_ne!(one.id, two.id);
        let q = book.question(ACME, "Acme", "job4", "Import");
        let p1 = book.permission(ACME, "Acme", "job4", "Import", 1);
        let p2 = book.permission(ACME, "Acme", "job4", "Import", 2);
        assert_ne!(p1.id, p2.id);
        assert_ne!(q.id, p1.id);
        assert_eq!(book.len(), 6);
        let replaced = book.permission(ACME, "Acme", "job4", "Import", 2);
        assert_eq!(
            replaced.id, p2.id,
            "the same project, job, kind and request replace one another"
        );
        assert_eq!(book.len(), 6);
    }

    #[test]
    fn a_click_leads_to_the_conversation_of_the_job_or_to_the_jobs_view_for_several() {
        let mut book = Book::new(true);
        let finished = book.finished(ACME, "Acme", "job1", "Build");
        let failed = book.failed(OTHER, "Other", "job2", "Scan");
        let question = book.question(ACME, "Acme", "job3", "Import");
        let permission = book.permission(ACME, "Acme", "job4", "Run", 5);
        book.finished(ACME, "Acme", "job5", "Scan");
        let click = |book: &mut Book, id| project_click(book.clicked(id));
        let jobs = |job: Option<&str>| Destination::Jobs {
            job_id: job.map(str::to_owned),
        };
        assert_eq!(
            click(&mut book, finished.id),
            Some((ACME.into(), jobs(None))),
            "several finished: the Jobs view"
        );
        assert_eq!(
            click(&mut book, failed.id),
            Some((OTHER.into(), jobs(Some("job2"))))
        );
        assert_eq!(
            click(&mut book, question.id),
            Some((ACME.into(), jobs(Some("job3"))))
        );
        assert_eq!(
            click(&mut book, permission.id),
            Some((ACME.into(), jobs(Some("job4"))))
        );
    }

    #[test]
    fn one_finished_job_leads_to_its_conversation() {
        let mut book = Book::new(true);
        let finished = book.finished(ACME, "Acme", "job1", "Build");
        assert_eq!(
            project_click(book.clicked(finished.id)).map(|(_, destination)| destination),
            Some(Destination::Jobs {
                job_id: Some("job1".to_owned())
            })
        );
    }

    #[test]
    fn a_click_on_an_unmapped_id_or_a_second_click_leads_nowhere() {
        let mut book = Book::new(true);
        let id = book.failed(ACME, "Acme", "job1", "Scan").id;
        assert_eq!(book.clicked(id + 100), None, "never ours");
        assert!(book.clicked(id).is_some());
        assert_eq!(book.clicked(id), None, "a clicked notification is final");
        assert!(book.is_empty());
    }

    #[test]
    fn a_closed_round_an_answered_permission_and_an_ended_job_withdraw_what_they_announced() {
        let mut book = Book::new(true);
        let question = book.question(ACME, "Acme", "job1", "Import").id;
        let permission = book.permission(ACME, "Acme", "job1", "Import", 3).id;
        let other_permission = book.permission(ACME, "Acme", "job1", "Import", 4).id;
        let finished = book.finished(ACME, "Acme", "job2", "Scan").id;

        assert_eq!(
            book.round_closed(OTHER, "job1"),
            Vec::<i32>::new(),
            "another project's round is not this one"
        );
        assert_eq!(book.round_closed(ACME, "job1"), vec![question]);
        assert_eq!(book.round_closed(ACME, "job1"), Vec::<i32>::new());
        assert_eq!(book.permission_answered(ACME, "job1", 3), vec![permission]);
        assert_eq!(book.kind_of(other_permission), Some(Kind::Permission));
        assert_eq!(book.job_ended(ACME, "job1"), vec![other_permission]);
        assert_eq!(
            book.kind_of(finished),
            Some(Kind::Finished),
            "what a job announced about its end stays"
        );
        assert_eq!(
            book.clicked(permission),
            None,
            "where the system removes it, nothing is left to click"
        );
    }

    #[test]
    fn where_the_system_cannot_remove_a_notification_a_click_on_a_withdrawn_one_only_brings_the_window_forward(
    ) {
        let mut book = Book::new(false);
        let question = book.question(ACME, "Acme", "job1", "Import").id;
        let finished = book.finished(ACME, "Acme", "job2", "Scan").id;
        assert_eq!(book.round_closed(ACME, "job1"), vec![question]);
        assert_eq!(
            book.round_closed(ACME, "job1"),
            Vec::<i32>::new(),
            "withdrawn once"
        );
        let click = project_click(book.clicked(question)).expect("the mapping stays bare");
        assert_eq!((click.0.as_str(), click.1), (ACME, Destination::WindowOnly));
        assert_eq!(book.clicked(question), None);
        assert!(matches!(
            project_click(book.clicked(finished)).map(|(_, destination)| destination),
            Some(Destination::Jobs { .. })
        ));
        let later = book.question(ACME, "Acme", "job1", "Import");
        assert_ne!(
            later.id, question,
            "a withdrawn notification is not replaced"
        );
    }

    #[test]
    fn closing_the_window_drops_every_mapping_of_the_project_and_no_other() {
        let mut book = Book::new(false);
        let a = book.failed(ACME, "Acme", "job1", "Scan").id;
        let b = book.question(ACME, "Acme", "job2", "Import").id;
        let c = book.failed(OTHER, "Other", "job3", "Scan").id;
        let mut gone = book.project_closed(ACME);
        gone.sort_unstable();
        assert_eq!(gone, vec![a, b]);
        assert_eq!(book.len(), 1);
        assert_eq!(book.project_of(c), Some("Other"));
    }

    #[test]
    fn the_system_certificates_notification_has_its_exact_text_and_belongs_to_no_project() {
        let mut book = Book::new(true);
        let notice = book.system_certificates();
        assert_eq!(
            (notice.title.as_str(), notice.message),
            (
                "System Certificates turned on",
                "Weaver turned on System Certificates because Zscaler is installed: the agent and its commands now trust the certificates your system trusts."
            )
        );
        assert_eq!(book.kind_of(notice.id), Some(Kind::SystemCertificates));
        let project = book.failed(ACME, "Acme", "job1", "Scan");
        assert_ne!(notice.id, project.id, "one counter for every notification");
        let second = book.system_certificates();
        assert_ne!(second.id, notice.id, "never grouped");

        assert_eq!(book.project_closed(ACME), vec![project.id]);
        assert_eq!(book.job_ended(ACME, "job1"), Vec::<i32>::new());
        assert_eq!(
            book.len(),
            2,
            "closing a window and the events of a job leave it alone"
        );
        assert_eq!(book.clicked(notice.id), Some(Click::App));
        assert_eq!(
            book.clicked(notice.id),
            None,
            "a clicked notification is final"
        );
    }

    fn project_click(click: Option<Click>) -> Option<(String, Destination)> {
        click.map(|click| match click {
            Click::Project {
                project_id,
                destination,
            } => (project_id, destination),
            Click::App => panic!("a notification of a project led to the app"),
        })
    }
}
