//! Git's HEAD file of a working copy, watched on its own (`requirements/tech#project`): a change of
//! it means git changed branch, also outside Weaver, and the window reads its branch again. Its
//! events never touch the picture of the project and are never mixed with the batches of
//! [`super::watch`].
//!
//! Git replaces HEAD by renaming `HEAD.lock` over it, which a watch on the file itself would lose
//! with the old inode: the folder that holds it is watched, without recursion, and only the events
//! that name the HEAD file count. A burst of events is one call, after a quiet time.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::Arc;
use std::time::Duration;

use notify::{EventKind, RecursiveMode, Watcher};

use crate::log_warn;

/// Quiet time after the last event of HEAD before the change is announced.
pub const QUIET: Duration = Duration::from_millis(100);

/// Called, on the watcher's own thread, each time HEAD changed.
pub type OnChange = Arc<dyn Fn() + Send + Sync>;

enum Message {
    Event(notify::Result<notify::Event>),
    Stop,
}

/// A running watch; dropping it stops it.
pub struct HeadWatch(mpsc::Sender<Message>);

impl Drop for HeadWatch {
    fn drop(&mut self) {
        let _ = self.0.send(Message::Stop);
    }
}

/// Whether an event of `kind` on `paths` concerns the file `head`: anything but a read of it.
fn concerns(kind: &EventKind, paths: &[PathBuf], head: &Path) -> bool {
    !matches!(kind, EventKind::Access(_)) && paths.iter().any(|path| path == head)
}

/// Watches the HEAD file `head` (absolute) and calls `on_change` each time it changes.
pub fn start(head: &Path, on_change: OnChange) -> Result<HeadWatch, String> {
    let folder = head
        .parent()
        .ok_or_else(|| "git's HEAD file has no folder".to_owned())?;
    let (sender, receiver) = mpsc::channel::<Message>();
    let events = sender.clone();
    let mut watcher = notify::recommended_watcher(move |event| {
        let _ = events.send(Message::Event(event));
    })
    .map_err(|error| format!("cannot start the watch of git's HEAD: {error}"))?;
    watcher
        .watch(folder, RecursiveMode::NonRecursive)
        .map_err(|error| format!("cannot watch git's HEAD: {error}"))?;
    let head = head.to_path_buf();
    std::thread::Builder::new()
        .name("git-head-watcher".to_owned())
        .spawn(move || run(watcher, receiver, &head, &on_change))
        .map_err(|error| format!("cannot start the watch of git's HEAD: {error}"))?;
    Ok(HeadWatch(sender))
}

fn run(
    watcher: notify::RecommendedWatcher,
    receiver: mpsc::Receiver<Message>,
    head: &Path,
    on_change: &OnChange,
) {
    // The watcher stops when it is dropped, with the thread.
    let _watcher = watcher;
    let mut changed = false;
    loop {
        let message = if changed {
            receiver.recv_timeout(QUIET)
        } else {
            receiver.recv().map_err(|_| RecvTimeoutError::Disconnected)
        };
        match message {
            Ok(Message::Stop) | Err(RecvTimeoutError::Disconnected) => return,
            Ok(Message::Event(Ok(event))) => {
                if concerns(&event.kind, &event.paths, head) {
                    changed = true;
                }
            }
            Ok(Message::Event(Err(error))) => {
                // Events may have been lost: the branch is read again.
                log_warn!("the watch of git's HEAD reported an error: {error}");
                changed = true;
            }
            Err(RecvTimeoutError::Timeout) => {
                changed = false;
                on_change();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{AccessKind, CreateKind, ModifyKind};

    #[test]
    fn only_a_change_of_the_head_file_counts() {
        let head = Path::new("/repo/.git/HEAD");
        let on = |kind: EventKind, path: &str| concerns(&kind, &[PathBuf::from(path)], head);
        assert!(on(EventKind::Modify(ModifyKind::Any), "/repo/.git/HEAD"));
        assert!(on(EventKind::Create(CreateKind::File), "/repo/.git/HEAD"));
        // The lock file, other files of git and reads of HEAD itself are not changes of the branch.
        assert!(!on(EventKind::Create(CreateKind::File), "/repo/.git/HEAD.lock"));
        assert!(!on(EventKind::Modify(ModifyKind::Any), "/repo/.git/index"));
        assert!(!on(EventKind::Access(AccessKind::Any), "/repo/.git/HEAD"));
    }

    #[test]
    fn a_change_of_the_head_file_is_announced_once_and_a_replacement_too() {
        let folder = tempfile::tempdir().unwrap();
        let folder = weaver_core::paths::canonical_root(folder.path()).unwrap();
        let head = folder.join("HEAD");
        std::fs::write(&head, "ref: refs/heads/main\n").unwrap();
        let (sender, announced) = mpsc::channel::<()>();
        let sender = std::sync::Mutex::new(sender);
        let watch = start(
            &head,
            Arc::new(move || {
                let _ = sender.lock().unwrap().send(());
            }),
        )
        .unwrap();
        // The backend of the platform needs a moment to be listening.
        std::thread::sleep(Duration::from_millis(300));

        // Another file of the folder changes: nothing is announced.
        std::fs::write(folder.join("index"), "x").unwrap();
        assert!(announced.recv_timeout(Duration::from_millis(600)).is_err());

        // Git switches branch by writing a lock file and renaming it over HEAD.
        std::fs::write(folder.join("HEAD.lock"), "ref: refs/heads/other\n").unwrap();
        std::fs::rename(folder.join("HEAD.lock"), &head).unwrap();
        assert!(announced.recv_timeout(Duration::from_secs(10)).is_ok());

        // Once the watch is dropped nothing more is announced.
        drop(watch);
        std::thread::sleep(Duration::from_millis(300));
        while announced.try_recv().is_ok() {}
        std::fs::write(&head, "ref: refs/heads/third\n").unwrap();
        assert!(announced.recv_timeout(Duration::from_millis(800)).is_err());
    }
}
