//! When the installed files are checked: at most one check per project at a time, and requests
//! made meanwhile coalesce into one more run (`requirements/tech#project`).
//!
//! The repair of Solo mode's block at the opening of a project rides on the same runs: it is
//! coalesced like the check of the installed files and happens in the same run when both are due
//! (`requirements/tech#solo-mode`). So does the recomputation of Merge's state
//! (`requirements/tech#solo-merge`): one computation runs at a time per project, and the requests
//! made meanwhile coalesce into one more.
//!
//! [`Runs`] is the pure rule; the app owns one per open project and starts the work it names.

/// What a run does to the installed files, from the least to the most.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Want {
    /// None: the run is only the repair of Solo mode (a project with an empty Model, at its opening).
    NoFiles,
    /// The Requirement skills only (an approval, a change of the Requirements).
    RequirementSkills,
    /// Every installed file and the Requirement skills (the opening check).
    OpeningCheck,
    /// The opening check and the starting Areas when the Requirements are missing (`Update skills`).
    UpdateSkills,
}

/// One run: what it does to the installed files, whether it also repairs Solo mode's block and
/// whether it also works out Merge's state again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Run {
    pub want: Want,
    pub solo: bool,
    pub merge: bool,
}

impl Run {
    /// A run of `want` that also repairs Solo mode's block when `solo`.
    pub fn new(want: Want, solo: bool) -> Self {
        Self {
            want,
            solo,
            merge: false,
        }
    }

    /// The same run, working out Merge's state too.
    pub fn with_merge(mut self) -> Self {
        self.merge = true;
        self
    }

    /// A run that only works out Merge's state again.
    pub fn merge_only() -> Self {
        Run::new(Want::NoFiles, false).with_merge()
    }

    /// The one run that covers both.
    fn merged(self, other: Run) -> Run {
        Run {
            want: self.want.max(other.want),
            solo: self.solo || other.solo,
            merge: self.merge || other.merge,
        }
    }
}

/// The runs of one project.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Runs {
    running: bool,
    queued: Option<Run>,
}

impl Runs {
    /// A run is asked for. `Some(run)` says it starts now; `None` says one is running and this
    /// request will be served by one more run when it ends.
    pub fn request(&mut self, wanted: Run) -> Option<Run> {
        if self.running {
            self.queued = Some(self.queued.map_or(wanted, |queued| queued.merged(wanted)));
            None
        } else {
            self.running = true;
            Some(wanted)
        }
    }

    /// The running one ended. `Some(run)` is the one run that serves every request made meanwhile
    /// and starts now.
    pub fn finished(&mut self) -> Option<Run> {
        match self.queued.take() {
            Some(next) => Some(next),
            None => {
                self.running = false;
                None
            }
        }
    }

    pub fn is_running(&self) -> bool {
        self.running
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files(want: Want) -> Run {
        Run::new(want, false)
    }

    #[test]
    fn a_request_with_nothing_running_starts_at_once() {
        let mut runs = Runs::default();
        assert_eq!(
            runs.request(files(Want::OpeningCheck)),
            Some(files(Want::OpeningCheck))
        );
        assert!(runs.is_running());
        assert_eq!(runs.finished(), None);
        assert!(!runs.is_running());
    }

    #[test]
    fn requests_made_while_one_runs_coalesce_into_one_more_run_of_the_widest_kind() {
        let mut runs = Runs::default();
        assert_eq!(
            runs.request(files(Want::RequirementSkills)),
            Some(files(Want::RequirementSkills))
        );
        assert_eq!(runs.request(files(Want::RequirementSkills)), None);
        assert_eq!(runs.request(files(Want::UpdateSkills)), None);
        assert_eq!(runs.request(files(Want::OpeningCheck)), None);
        assert_eq!(runs.request(files(Want::RequirementSkills)), None);
        assert_eq!(runs.finished(), Some(files(Want::UpdateSkills)));
        // The extra run is the one running now; nothing else is waiting.
        assert!(runs.is_running());
        assert_eq!(runs.finished(), None);
        assert_eq!(
            runs.request(files(Want::RequirementSkills)),
            Some(files(Want::RequirementSkills))
        );
    }

    #[test]
    fn the_runs_of_two_projects_never_meet() {
        let (mut one, mut two) = (Runs::default(), Runs::default());
        assert!(one.request(files(Want::OpeningCheck)).is_some());
        assert!(two.request(files(Want::OpeningCheck)).is_some());
    }

    #[test]
    fn the_repair_of_solo_mode_rides_on_the_run_that_is_due_and_is_never_lost() {
        let mut runs = Runs::default();
        // Alone it is a run of its own, with no installed file touched.
        assert_eq!(
            runs.request(Run::new(Want::NoFiles, true)),
            Some(Run::new(Want::NoFiles, true))
        );
        // Asked while it runs, together with a wider run: one more run does both.
        assert_eq!(runs.request(files(Want::RequirementSkills)), None);
        assert_eq!(runs.request(Run::new(Want::NoFiles, true)), None);
        assert_eq!(
            runs.finished(),
            Some(Run::new(Want::RequirementSkills, true))
        );
        assert_eq!(runs.finished(), None);

        // The opening check with the repair is one run; requests made meanwhile add only their own wish.
        assert_eq!(
            runs.request(Run::new(Want::OpeningCheck, true)),
            Some(Run::new(Want::OpeningCheck, true))
        );
        assert_eq!(runs.request(files(Want::UpdateSkills)), None);
        assert_eq!(runs.request(files(Want::RequirementSkills)), None);
        assert_eq!(runs.finished(), Some(files(Want::UpdateSkills)));
        assert_eq!(runs.finished(), None);
    }

    #[test]
    fn the_state_of_merge_rides_on_the_runs_and_is_never_lost() {
        let mut runs = Runs::default();
        // Alone it is a run of its own, with no installed file touched.
        assert_eq!(runs.request(Run::merge_only()), Some(Run::merge_only()));
        // Asked again while it runs, with a wider run: one more run does both.
        assert_eq!(runs.request(Run::merge_only()), None);
        assert_eq!(runs.request(files(Want::OpeningCheck)), None);
        assert_eq!(
            runs.finished(),
            Some(Run::new(Want::OpeningCheck, false).with_merge())
        );
        assert_eq!(runs.finished(), None);

        // A run that already includes it takes the requests made meanwhile as they are.
        let opening = Run::new(Want::OpeningCheck, true).with_merge();
        assert_eq!(runs.request(opening), Some(opening));
        assert_eq!(runs.request(files(Want::RequirementSkills)), None);
        assert_eq!(runs.finished(), Some(files(Want::RequirementSkills)));
        assert_eq!(runs.finished(), None);
    }
}
