//! The requests of one project on disk: `.weaver/requests/<id>.md`, `<id>.images/` and
//! `.state.json`.
//!
//! Every operation reads the state of the request from the files, asks the Machine
//! ([`super::machine::step`]) what the event does and carries out the effect. The caller (the
//! app) runs the operations that write inside the project's mutation queue; nothing here talks to
//! a job: [`RequestStore::plan_send`] names the job to start and [`RequestStore::record_sent`]
//! records the job that was started.
//!
//! `.state.json` is written by the app only: `{"sent": {<id>: <job id>}, "consumed": [<id>]}`.
//! An id listed as consumed is never shown, written or sent again, even if a file with that name
//! reappears.
//!
//! A request is held by Chat (`RequestState::HeldByChat`) while a `draft` Proposal with
//! `chatWriting: true` names it in `request`; nothing is recorded for it, so it is the Proposals
//! that are read ([`RequestStore::state_of`], [`RequestStore::list`]). A held request cannot be
//! saved, sent or discarded, and the hand-over of that Proposal consumes it
//! ([`RequestStore::consume`]).

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::id::{self, is_request_id, parse_id, LocalTime};
use super::image::{self, ImageError, ImageKind, MAX_IMAGE_BYTES};
use super::machine::{step, Effect, RequestEvent, RequestState};
use super::title;
use crate::atomic;
use crate::job_instruction::Job;
use crate::paths;
use crate::project::frontmatter;
use crate::project::layout::{self, PROPOSALS_DIR, PROPOSAL_FILE, REQUESTS_DIR};
use crate::project::model::Model;
use crate::project::text;

/// The file that records the requests that were sent and the ones that were consumed.
pub const STATE_FILE: &str = ".state.json";

/// Why an operation on a request failed or was refused. The message is for the developer.
#[derive(Debug, thiserror::Error)]
pub enum RequestError {
    /// The file is gone (or never existed, or the request was consumed): nothing is ever
    /// sent from it.
    #[error("the request no longer exists")]
    Gone,
    /// The request was sent: it can be read, not changed, sent again or discarded.
    #[error("the request was sent to the agent and cannot change")]
    NotWriting,
    /// The agent of Chat is writing a Proposal bound to it: it can be read, not changed, sent or
    /// discarded.
    #[error("Chat is writing a Proposal from the request and it cannot change")]
    HeldByChat,
    /// The name is not the id of a request.
    #[error("`{0}` is not the id of a request")]
    NotAnId(String),
    #[error("{0}")]
    Image(#[from] ImageError),
    #[error("cannot read the record of sent requests (`.weaver/requests/.state.json`): {0}")]
    State(String),
    #[error("{0}")]
    Io(#[from] io::Error),
}

/// What `.weaver/requests/.state.json` holds.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestsState {
    /// Request id → the id of the job that received it.
    #[serde(default)]
    pub sent: BTreeMap<String, String>,
    #[serde(default)]
    pub consumed: Vec<String>,
}

/// An open request (being written, or sent), as the list of the editor shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestEntry {
    pub id: String,
    pub title: String,
    /// The local time of creation, `YYYY-MM-DDTHH:MM:SS`.
    pub created: String,
    /// `Writing`, `Sent` or `HeldByChat`.
    pub state: RequestState,
    /// The job that received it, when it was sent.
    pub job_id: Option<String>,
    pub empty: bool,
}

/// What sending a request comes to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SendPlan {
    /// The request is empty: nothing happens.
    Nothing,
    /// Start this job, then [`RequestStore::record_sent`] with its id.
    Start(Job),
}

/// The job a request starts: a setup job when the Model is empty, a design job otherwise; `assets`
/// are the names of the assets the request tags.
pub fn job_for(request: &str, model_is_empty: bool, assets: Vec<String>) -> Job {
    if model_is_empty {
        Job::Setup {
            request: request.to_owned(),
            assets,
        }
    } else {
        Job::Design {
            request: request.to_owned(),
            assets,
        }
    }
}

/// The Model is empty when it has no Feature and no Actor.
pub fn model_is_empty(model: &Model) -> bool {
    model.features.is_empty() && model.actors.is_empty()
}

/// The text of a request as the file holds it, without the one final newline the file adds.
fn without_final_newline(mut content: String) -> String {
    if content.ends_with('\n') {
        content.pop();
    }
    content
}

/// The state of a request in `state` once a Proposal is bound to it, `by_chat` being whether the
/// agent of Chat opened it: the Machine decides (`agent.OPEN_PROPOSAL`); the Draft is the record.
fn opened_by_chat(state: RequestState, by_chat: bool) -> RequestState {
    step(state, RequestEvent::OpenProposal { by_chat }).state
}

#[derive(Debug, Clone)]
pub struct RequestStore {
    project_root: PathBuf,
}

impl RequestStore {
    pub fn new(project_root: &Path) -> Self {
        Self {
            project_root: project_root.to_path_buf(),
        }
    }

    /// `.weaver/requests`.
    pub fn dir(&self) -> PathBuf {
        layout::weaver_dir(&self.project_root).join(REQUESTS_DIR)
    }

    fn file(&self, id: &str) -> PathBuf {
        self.dir().join(format!("{id}.md"))
    }

    fn images_dir(&self, id: &str) -> PathBuf {
        self.dir().join(format!("{id}{}", IMAGES_SUFFIX))
    }

    fn checked(&self, id: &str) -> Result<(), RequestError> {
        if is_request_id(id) {
            Ok(())
        } else {
            Err(RequestError::NotAnId(id.to_owned()))
        }
    }

    // --- the record of sent and consumed requests ---

    /// Reads `.state.json` (missing: nothing sent, nothing consumed).
    pub fn state(&self) -> Result<RequestsState, RequestError> {
        let path = self.dir().join(STATE_FILE);
        match text::read_text(&path) {
            Ok(content) => {
                serde_json::from_str(&content).map_err(|e| RequestError::State(e.to_string()))
            }
            Err(text::ReadError::Io(e)) if e.kind() == io::ErrorKind::NotFound => {
                Ok(RequestsState::default())
            }
            Err(e) => Err(RequestError::State(e.to_string())),
        }
    }

    fn save_state(&self, state: &RequestsState) -> Result<(), RequestError> {
        layout::ensure_local_folder(&self.project_root, REQUESTS_DIR)?;
        let json =
            serde_json::to_string_pretty(state).map_err(|e| RequestError::State(e.to_string()))?;
        text::write_text(&self.dir().join(STATE_FILE), &json)?;
        Ok(())
    }

    /// The state of request `id`, and whether its file exists. A consumed id stays consumed
    /// whatever the files say.
    fn locate(&self, id: &str, state: &RequestsState) -> (RequestState, bool) {
        let exists = self.file(id).is_file();
        let located = if state.consumed.iter().any(|c| c == id) {
            RequestState::Consumed
        } else if state.sent.contains_key(id) {
            RequestState::Sent
        } else if exists {
            RequestState::Writing
        } else {
            RequestState::Discarded
        };
        (located, exists)
    }

    /// [`Self::locate`] that also tells a request held by Chat from one being written. It reads the
    /// Proposals, so an operation that does not need the difference uses `locate`.
    fn locate_held(
        &self,
        id: &str,
        state: &RequestsState,
    ) -> Result<(RequestState, bool), RequestError> {
        let (located, exists) = self.locate(id, state);
        if located == RequestState::Writing {
            let held = self.held_by_chat()?.contains(id);
            return Ok((opened_by_chat(located, held), exists));
        }
        Ok((located, exists))
    }

    /// The ids of the requests held by Chat: a `draft` Proposal with `chatWriting: true` names each in
    /// `request`. Nothing else records it (`requirements/data#design-request`), so it ends by itself
    /// when that Draft is handed over or discarded. A Proposal file that cannot be read says nothing.
    fn held_by_chat(&self) -> Result<BTreeSet<String>, RequestError> {
        let folder = layout::weaver_dir(&self.project_root).join(PROPOSALS_DIR);
        let listing = match fs::read_dir(&folder) {
            Ok(listing) => listing,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(BTreeSet::new()),
            Err(e) => return Err(e.into()),
        };
        let mut held = BTreeSet::new();
        for item in listing {
            let Ok(content) = text::read_text(&item?.path().join(PROPOSAL_FILE)) else {
                continue;
            };
            let Ok(document) = frontmatter::parse(&content) else {
                continue;
            };
            let front = &document.front;
            if frontmatter::get_str(front, "status") == Some("draft")
                && frontmatter::get_bool(front, "chatWriting") == Some(true)
            {
                if let Some(request) = frontmatter::get_str(front, "request") {
                    held.insert(request.to_owned());
                }
            }
        }
        Ok(held)
    }

    /// The state of request `id`.
    pub fn state_of(&self, id: &str) -> Result<RequestState, RequestError> {
        self.checked(id)?;
        Ok(self.locate_held(id, &self.state()?)?.0)
    }

    // --- the requests ---

    /// The open requests (writing, sent or held by Chat), newest first. A consumed id is never
    /// listed, and neither is a sent one whose file is gone.
    pub fn list(&self) -> Result<Vec<RequestEntry>, RequestError> {
        let state = self.state()?;
        let held = self.held_by_chat()?;
        let listing = match fs::read_dir(self.dir()) {
            Ok(listing) => listing,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e.into()),
        };
        let mut entries = Vec::new();
        for item in listing {
            let item = item?;
            let name = item.file_name().to_string_lossy().into_owned();
            let Some(request) = name.strip_suffix(".md") else {
                continue;
            };
            let Some((created, _)) = parse_id(request) else {
                continue;
            };
            if !item.path().is_file() || state.consumed.iter().any(|c| c == request) {
                continue;
            }
            let content = match text::read_text(&item.path()) {
                Ok(content) => without_final_newline(content),
                Err(e) => {
                    return Err(RequestError::Io(io::Error::other(format!(
                        "cannot read `{name}`: {e}"
                    ))))
                }
            };
            let job_id = state.sent.get(request).cloned();
            let listed = if job_id.is_some() {
                RequestState::Sent
            } else {
                opened_by_chat(RequestState::Writing, held.contains(request))
            };
            entries.push(RequestEntry {
                id: request.to_owned(),
                title: title::title_of(&content, request),
                created: created.iso(),
                state: listed,
                job_id,
                empty: title::is_empty(&content),
            });
        }
        entries.sort_by(|a, b| id::compare(&b.id, &a.id));
        Ok(entries)
    }

    /// The text of an open request, `Gone` when it does not exist (or was consumed).
    pub fn read(&self, id: &str) -> Result<String, RequestError> {
        self.checked(id)?;
        let state = self.state()?;
        if self.locate(id, &state).0 == RequestState::Consumed {
            return Err(RequestError::Gone);
        }
        match text::read_text(&self.file(id)) {
            Ok(content) => Ok(without_final_newline(content)),
            Err(text::ReadError::Io(e)) if e.kind() == io::ErrorKind::NotFound => {
                Err(RequestError::Gone)
            }
            Err(e) => Err(RequestError::Io(io::Error::other(e.to_string()))),
        }
    }

    /// `developer.NEW_REQUEST`: creates an empty request named after `now` and returns its id.
    pub fn create(&self, now: &LocalTime) -> Result<String, RequestError> {
        let state = self.state()?;
        layout::ensure_local_folder(&self.project_root, REQUESTS_DIR)?;
        let taken = |candidate: &str| {
            self.file(candidate).exists()
                || self.images_dir(candidate).exists()
                || state.consumed.iter().any(|c| c == candidate)
                || state.sent.contains_key(candidate)
        };
        let id = id::new_id(now, &taken);
        let outcome = step(RequestState::NotWritten, RequestEvent::NewRequest);
        debug_assert_eq!(outcome.effect, Effect::CreateFile);
        text::write_text(&self.file(&id), "")?;
        Ok(id)
    }

    /// Writes the text of a request being written. The file is the text of the developer with
    /// LF line endings and a final newline.
    pub fn save(&self, id: &str, content: &str) -> Result<(), RequestError> {
        self.checked(id)?;
        self.require_writing(id)?;
        let path = self.file(id);
        let wanted = text::file_content(content);
        if text::read_text(&path).is_ok_and(|current| current == wanted) {
            return Ok(());
        }
        atomic::write(&path, wanted.as_bytes(), None)?;
        Ok(())
    }

    /// The request must be in `Writing`: a file that vanished meanwhile is `REQUEST_DELETED`
    /// (and is recorded as such), a sent request cannot change.
    fn require_writing(&self, id: &str) -> Result<(), RequestError> {
        let state = self.state()?;
        let (located, exists) = self.locate_held(id, &state)?;
        match located {
            RequestState::Writing => Ok(()),
            RequestState::HeldByChat => Err(RequestError::HeldByChat),
            RequestState::Sent if exists => Err(RequestError::NotWriting),
            RequestState::Sent => {
                self.apply_deleted(id)?;
                Err(RequestError::Gone)
            }
            _ => Err(RequestError::Gone),
        }
    }

    /// `workspace.REQUEST_DELETED`: the file is gone; forget what was recorded about it.
    fn apply_deleted(&self, id: &str) -> Result<(), RequestError> {
        let mut state = self.state()?;
        let (located, _) = self.locate(id, &state);
        let outcome = step(located, RequestEvent::RequestDeleted);
        if outcome.effect == Effect::ForgetRemains {
            if state.sent.remove(id).is_some() {
                self.save_state(&state)?;
            }
            remove_dir_if_present(&self.images_dir(id))?;
        }
        Ok(())
    }

    /// `developer.SEND_REQUEST`: what sending request `id` comes to. An empty request is not
    /// sent; a request that was sent, or whose file is gone, is refused.
    pub fn plan_send(&self, id: &str, model_is_empty: bool) -> Result<SendPlan, RequestError> {
        self.plan_send_tagged(id, model_is_empty, &|_| Vec::new())
    }

    /// [`Self::plan_send`] for a project with assets: `tagged` gives the names of the assets the
    /// text of the request tags, from the one read of the file. The request file is never changed.
    pub fn plan_send_tagged(
        &self,
        id: &str,
        model_is_empty: bool,
        tagged: &dyn Fn(&str) -> Vec<String>,
    ) -> Result<SendPlan, RequestError> {
        self.checked(id)?;
        self.require_writing(id)?;
        let content = self.read(id)?;
        let outcome = step(
            RequestState::Writing,
            RequestEvent::SendRequest {
                empty: title::is_empty(&content),
            },
        );
        Ok(match outcome.effect {
            Effect::StartJob => SendPlan::Start(job_for(id, model_is_empty, tagged(&content))),
            _ => SendPlan::Nothing,
        })
    }

    /// Records that the job `job_id` received request `id`: it is `Sent` from now on.
    pub fn record_sent(&self, id: &str, job_id: &str) -> Result<(), RequestError> {
        self.checked(id)?;
        self.require_writing(id)?;
        let mut state = self.state()?;
        state.sent.insert(id.to_owned(), job_id.to_owned());
        self.save_state(&state)
    }

    /// `developer.DISCARD_REQUEST`: deletes the file and the images of a request being written.
    /// A request that is already gone stays gone.
    pub fn discard(&self, id: &str) -> Result<(), RequestError> {
        self.checked(id)?;
        let state = self.state()?;
        let (located, exists) = self.locate_held(id, &state)?;
        match located {
            RequestState::Writing => {
                let outcome = step(located, RequestEvent::DiscardRequest);
                debug_assert_eq!(outcome.effect, Effect::DeleteFiles);
                self.delete_files(id)?;
                Ok(())
            }
            RequestState::HeldByChat => Err(RequestError::HeldByChat),
            RequestState::Sent if exists => Err(RequestError::NotWriting),
            RequestState::Sent => self.apply_deleted(id),
            RequestState::Consumed | RequestState::Discarded | RequestState::NotWritten => Ok(()),
        }
    }

    /// `proposal.SUBMITTED` of a Proposal bound to request `id`: the first one consumes the
    /// request (files deleted, id recorded as consumed). Returns whether this call consumed it.
    ///
    /// `from_chat` is whether the Proposal was a Draft the agent of Chat was writing (`chatWriting`)
    /// before the hand-over: that Draft is what held the request, and it is a Draft no longer when
    /// this is called.
    pub fn consume(&self, id: &str, from_chat: bool) -> Result<bool, RequestError> {
        self.checked(id)?;
        let mut state = self.state()?;
        let (mut located, _) = self.locate(id, &state);
        located = opened_by_chat(located, from_chat);
        let outcome = step(located, RequestEvent::ProposalSubmitted);
        if outcome.state != RequestState::Consumed || outcome.effect != Effect::DeleteFiles {
            return Ok(false);
        }
        // Recorded first: whatever happens to the files, the id is never shown or sent again.
        state.sent.remove(id);
        state.consumed.push(id.to_owned());
        self.save_state(&state)?;
        self.delete_files(id)?;
        Ok(true)
    }

    /// `job.ENDED` of job `job_id`: every request it received that is still `Sent` goes back to
    /// `Writing` when no Proposal bound to it was handed over. Returns their ids (a request
    /// whose file was deleted meanwhile is discarded instead and not returned).
    pub fn job_ended(
        &self,
        job_id: &str,
        proposal_handed_over: bool,
    ) -> Result<Vec<String>, RequestError> {
        let mut state = self.state()?;
        let received: Vec<String> = state
            .sent
            .iter()
            .filter(|(_, job)| job.as_str() == job_id)
            .map(|(id, _)| id.clone())
            .collect();
        let mut returned = Vec::new();
        let mut changed = false;
        for id in received {
            let (located, exists) = self.locate(&id, &state);
            if !exists {
                let outcome = step(located, RequestEvent::RequestDeleted);
                if outcome.effect == Effect::ForgetRemains {
                    state.sent.remove(&id);
                    remove_dir_if_present(&self.images_dir(&id))?;
                    changed = true;
                }
                continue;
            }
            let outcome = step(
                located,
                RequestEvent::JobEnded {
                    proposal_handed_over,
                },
            );
            if outcome.effect == Effect::ReturnToWriting {
                state.sent.remove(&id);
                returned.push(id);
                changed = true;
            }
        }
        if changed {
            self.save_state(&state)?;
        }
        Ok(returned)
    }

    /// `workspace.REQUEST_DELETED`: finds the requests whose file is gone and forgets what was
    /// recorded about them (a request being written needs nothing: it just no longer exists).
    /// Returns the ids of the sent requests that were forgotten.
    pub fn reconcile(&self) -> Result<Vec<String>, RequestError> {
        let state = self.state()?;
        let gone: Vec<String> = state
            .sent
            .keys()
            .filter(|id| !self.file(id).is_file())
            .cloned()
            .collect();
        for id in &gone {
            self.apply_deleted(id)?;
        }
        Ok(gone)
    }

    /// Deletes `<id>.images/` and then `<id>.md`.
    fn delete_files(&self, id: &str) -> io::Result<()> {
        remove_dir_if_present(&self.images_dir(id))?;
        match fs::remove_file(self.file(id)) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }

    // --- images ---

    /// Saves a pasted or dropped image as `<id>.images/image-<n>.<ext>` (next free number) and
    /// returns the path to link, relative to the request file.
    pub fn add_image(&self, id: &str, bytes: &[u8]) -> Result<String, RequestError> {
        self.checked(id)?;
        self.require_writing(id)?;
        let kind = image::check(bytes)?;
        let folder = self.images_dir(id);
        fs::create_dir_all(&folder)?;
        let existing: Vec<String> = fs::read_dir(&folder)?
            .filter_map(|item| item.ok())
            .map(|item| item.file_name().to_string_lossy().into_owned())
            .collect();
        let name = image::image_name(
            image::next_number(existing.iter().map(String::as_str)),
            kind,
        );
        atomic::write(&folder.join(&name), bytes, None)?;
        Ok(format!("{id}{IMAGES_SUFFIX}/{name}"))
    }

    /// Links an image file that is on the disk (a file dropped on the editor): an image already
    /// inside the project is linked where it is, any other is copied like a pasted one. Returns
    /// the path to link, relative to the request file.
    pub fn import_image(&self, id: &str, source: &Path) -> Result<String, RequestError> {
        self.checked(id)?;
        self.require_writing(id)?;
        let accepted = source
            .extension()
            .and_then(|e| e.to_str())
            .and_then(ImageKind::from_extension)
            .is_some();
        let too_big = fs::metadata(source)?.len() > MAX_IMAGE_BYTES as u64;
        if !accepted || too_big {
            return Err(ImageError::NotAccepted.into());
        }
        let bytes = fs::read(source)?;
        image::check(&bytes)?;
        match self.link_inside_project(id, source) {
            Some(link) => Ok(link),
            None => self.add_image(id, &bytes),
        }
    }

    /// The link, relative to `.weaver/requests/`, of a file inside the project.
    fn link_inside_project(&self, id: &str, source: &Path) -> Option<String> {
        let root = paths::canonical_root(&self.project_root).ok()?;
        let source = paths::canonical_root(source).ok()?;
        let inside = source.strip_prefix(&root).ok()?;
        let parts: Vec<String> = inside
            .components()
            .map(|c| match c {
                Component::Normal(part) => Some(part.to_string_lossy().into_owned()),
                _ => None,
            })
            .collect::<Option<_>>()?;
        let own_images = format!("{id}{IMAGES_SUFFIX}");
        let own = [layout::WEAVER_DIR, REQUESTS_DIR, own_images.as_str()];
        let link = if parts.len() == own.len() + 1
            && parts[..own.len()].iter().zip(own).all(|(a, b)| a == b)
        {
            format!("{id}{IMAGES_SUFFIX}/{}", parts[own.len()])
        } else {
            format!("../../{}", parts.join("/"))
        };
        Some(image::link_escape(&link))
    }

    /// The type and bytes of an image of a request, from its link (`<id>.images/<file>`). Only
    /// files of the request's own images folder are served.
    pub fn read_image(&self, id: &str, link: &str) -> Result<(ImageKind, Vec<u8>), RequestError> {
        self.checked(id)?;
        let prefix = format!("{id}{IMAGES_SUFFIX}/");
        let name = link
            .strip_prefix(&prefix)
            .filter(|n| !n.is_empty() && !n.contains(['/', '\\']) && !n.starts_with('.'))
            .ok_or(RequestError::Gone)?;
        let bytes = match fs::read(self.images_dir(id).join(name)) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Err(RequestError::Gone),
            Err(e) => return Err(e.into()),
        };
        let kind = image::check(&bytes)?;
        Ok((kind, bytes))
    }
}

/// The suffix of a request's images folder.
pub const IMAGES_SUFFIX: &str = ".images";

fn remove_dir_if_present(path: &Path) -> io::Result<()> {
    match fs::remove_dir_all(path) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: [u8; 12] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 0];

    fn at(second: u8) -> LocalTime {
        LocalTime {
            year: 2026,
            month: 11,
            day: 2,
            hour: 7,
            minute: 5,
            second,
        }
    }

    fn store() -> (tempfile::TempDir, RequestStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = RequestStore::new(dir.path());
        (dir, store)
    }

    /// A request written (and saved) with `content`.
    fn written(store: &RequestStore, second: u8, content: &str) -> String {
        let id = store.create(&at(second)).unwrap();
        store.save(&id, content).unwrap();
        id
    }

    #[test]
    fn a_new_request_is_an_empty_file_in_a_folder_git_ignores() {
        let (dir, store) = store();
        let id = store.create(&at(9)).unwrap();
        assert_eq!(id, "2026-11-02T07-05-09");
        let folder = dir.path().join(".weaver/requests");
        assert!(folder.join(format!("{id}.md")).is_file());
        assert_eq!(fs::read(folder.join(".gitignore")).unwrap(), b"*\n");
        assert_eq!(store.state_of(&id).unwrap(), RequestState::Writing);
        assert_eq!(store.read(&id).unwrap(), "");
        // Nothing else of `.weaver/` is created or ignored.
        assert!(!dir.path().join(".weaver/.gitignore").exists());
    }

    #[test]
    fn two_requests_in_the_same_second_get_a_counter() {
        let (_dir, store) = store();
        let ids: Vec<String> = (0..3).map(|_| store.create(&at(9)).unwrap()).collect();
        assert_eq!(
            ids,
            [
                "2026-11-02T07-05-09",
                "2026-11-02T07-05-09-2",
                "2026-11-02T07-05-09-3"
            ]
        );
    }

    #[test]
    fn a_consumed_id_is_not_given_out_again() {
        let (_dir, store) = store();
        let id = written(&store, 9, "x");
        store.record_sent(&id, "jjjjjjjj").unwrap();
        assert!(store.consume(&id, false).unwrap());
        assert_eq!(store.create(&at(9)).unwrap(), "2026-11-02T07-05-09-2");
    }

    #[test]
    fn what_is_saved_is_what_is_read_back() {
        let (dir, store) = store();
        let id = store.create(&at(9)).unwrap();
        for text in [
            "",
            "one line",
            "# Title\r\n\r\nbody\r\n",
            "with trailing newline\n",
            "ünïcödé — 日本語",
        ] {
            store.save(&id, text).unwrap();
            let expected = text.replace("\r\n", "\n");
            let expected = expected.strip_suffix('\n').unwrap_or(&expected);
            assert_eq!(store.read(&id).unwrap(), expected);
        }
        // LF only, one final newline, no BOM.
        store.save(&id, "a\r\nb").unwrap();
        assert_eq!(
            fs::read(dir.path().join(format!(".weaver/requests/{id}.md"))).unwrap(),
            b"a\nb\n"
        );
    }

    #[test]
    fn the_list_shows_open_requests_newest_first_with_title_state_and_time() {
        let (_dir, store) = store();
        let old = written(&store, 1, "# Old\n\ntext");
        let sent = written(&store, 2, "Make it faster");
        let blank = store.create(&at(3)).unwrap();
        store.record_sent(&sent, "jjjjjjjj").unwrap();
        let list = store.list().unwrap();
        assert_eq!(
            list.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(),
            [blank.as_str(), sent.as_str(), old.as_str()]
        );
        assert_eq!(list[0].title, blank);
        assert!(list[0].empty);
        assert_eq!(
            (
                list[1].title.as_str(),
                list[1].state,
                list[1].job_id.as_deref()
            ),
            ("Make it faster", RequestState::Sent, Some("jjjjjjjj"))
        );
        assert_eq!(
            (
                list[2].title.as_str(),
                list[2].state,
                list[2].created.as_str()
            ),
            ("Old", RequestState::Writing, "2026-11-02T07:05:01")
        );
    }

    #[test]
    fn files_that_are_not_requests_are_not_listed() {
        let (dir, store) = store();
        let id = written(&store, 1, "x");
        let folder = dir.path().join(".weaver/requests");
        fs::write(folder.join("notes.md"), "x").unwrap();
        fs::write(folder.join("2026-11-02T07-05-01.txt"), "x").unwrap();
        fs::create_dir_all(folder.join("2026-11-02T07-05-02.md")).unwrap();
        assert_eq!(
            store
                .list()
                .unwrap()
                .iter()
                .map(|e| e.id.clone())
                .collect::<Vec<_>>(),
            [id]
        );
    }

    #[test]
    fn sending_an_empty_request_does_nothing() {
        let (_dir, store) = store();
        let id = store.create(&at(9)).unwrap();
        assert_eq!(store.plan_send(&id, false).unwrap(), SendPlan::Nothing);
        store.save(&id, "  \n\t\n").unwrap();
        assert_eq!(store.plan_send(&id, false).unwrap(), SendPlan::Nothing);
        assert_eq!(store.state_of(&id).unwrap(), RequestState::Writing);
        // A request of images only is not empty.
        store
            .save(&id, "![](2026-11-02T07-05-09.images/image-1.png)")
            .unwrap();
        assert!(matches!(
            store.plan_send(&id, false).unwrap(),
            SendPlan::Start(_)
        ));
    }

    #[test]
    fn sending_starts_a_setup_job_on_an_empty_model_and_a_design_job_otherwise() {
        let (_dir, store) = store();
        let id = written(&store, 9, "Build a shop");
        assert_eq!(
            store.plan_send(&id, true).unwrap(),
            SendPlan::Start(Job::Setup {
                request: id.clone(),
                assets: vec![],
            })
        );
        assert_eq!(
            store.plan_send(&id, false).unwrap(),
            SendPlan::Start(Job::Design {
                request: id.clone(),
                assets: vec![],
            })
        );
        // Planning does not send: only recording the job does.
        assert_eq!(store.state_of(&id).unwrap(), RequestState::Writing);
        store.record_sent(&id, "jjjjjjjj").unwrap();
        assert_eq!(store.state_of(&id).unwrap(), RequestState::Sent);
        assert_eq!(
            store.state().unwrap().sent.get(&id).map(String::as_str),
            Some("jjjjjjjj")
        );
    }

    #[test]
    fn the_assets_the_text_tags_go_with_the_job_and_the_file_is_not_changed() {
        use crate::assets::tags;
        let (_dir, store) = store();
        let id = written(
            &store,
            9,
            "Use @logo.png and @missing.png and @logo.png again",
        );
        let before = store.read(&id).unwrap();
        let assets = ["logo.png".to_owned(), "spec.pdf".to_owned()];
        let plan = store
            .plan_send_tagged(&id, false, &|text| tags::tagged(text, &assets))
            .unwrap();
        assert_eq!(
            plan,
            SendPlan::Start(Job::Design {
                request: id.clone(),
                assets: vec!["logo.png".to_owned()],
            })
        );
        assert_eq!(store.read(&id).unwrap(), before);
    }

    #[test]
    fn the_model_is_empty_without_features_and_actors() {
        use crate::project::image::ProjectImage;
        use crate::project::testing::put;
        let dir = tempfile::tempdir().unwrap();
        put(dir.path(), "model/system.md", "# Shop\n\nA shop.\n");
        assert!(model_is_empty(ProjectImage::load(dir.path()).model()));
        put(
            dir.path(),
            "model/actors/user.md",
            crate::project::model::USER_ACTOR,
        );
        assert!(!model_is_empty(ProjectImage::load(dir.path()).model()));
    }

    #[test]
    fn a_request_that_was_sent_cannot_be_sent_changed_or_discarded() {
        let (_dir, store) = store();
        let id = written(&store, 9, "x");
        store.record_sent(&id, "jjjjjjjj").unwrap();
        assert!(matches!(
            store.plan_send(&id, false),
            Err(RequestError::NotWriting)
        ));
        assert!(matches!(
            store.save(&id, "changed"),
            Err(RequestError::NotWriting)
        ));
        assert!(matches!(store.discard(&id), Err(RequestError::NotWriting)));
        assert!(matches!(
            store.add_image(&id, &PNG),
            Err(RequestError::NotWriting)
        ));
        assert_eq!(store.read(&id).unwrap(), "x", "it can still be read");
    }

    #[test]
    fn only_the_first_proposal_bound_to_a_request_consumes_it() {
        let (dir, store) = store();
        let id = written(&store, 9, "Add orders");
        store.add_image(&id, &PNG).unwrap();
        store.record_sent(&id, "jjjjjjjj").unwrap();
        let folder = dir.path().join(".weaver/requests");

        assert!(store.consume(&id, false).unwrap());
        assert!(!folder.join(format!("{id}.md")).exists());
        assert!(!folder.join(format!("{id}.images")).exists());
        assert_eq!(store.state_of(&id).unwrap(), RequestState::Consumed);
        let state = store.state().unwrap();
        assert_eq!(
            (state.sent.len(), state.consumed.as_slice()),
            (0, [id.clone()].as_slice())
        );

        // The second Proposal bound to it finds nothing to consume.
        assert!(!store.consume(&id, false).unwrap());
        assert_eq!(store.state().unwrap().consumed.len(), 1);
    }

    #[test]
    fn a_request_being_written_is_not_consumed_by_a_proposal() {
        let (_dir, store) = store();
        let id = written(&store, 9, "x");
        assert!(!store.consume(&id, false).unwrap());
        assert_eq!(store.state_of(&id).unwrap(), RequestState::Writing);
        assert!(!store.consume("2026-11-02T07-05-10", false).unwrap());
    }

    /// A `proposal.md` with `status`, `chatWriting` as given and `request` bound to it.
    fn proposal(dir: &Path, folder: &str, status: &str, chat_writing: Option<bool>, request: &str) {
        let path = dir.join(".weaver/proposals").join(folder);
        fs::create_dir_all(&path).unwrap();
        let writing = chat_writing.map_or(String::new(), |on| format!("chatWriting: {on}\n"));
        fs::write(
            path.join("proposal.md"),
            format!("---\nstatus: {status}\nkind: change\nrequest: {request}\n{writing}---\n\nBody\n"),
        )
        .unwrap();
    }

    #[test]
    fn a_draft_of_chat_holds_the_request_it_names_and_only_while_it_is_a_draft() {
        let (dir, store) = store();
        let held = written(&store, 9, "# Held");
        let free = written(&store, 10, "# Free");
        let other = written(&store, 11, "# Other");
        // Chat writes one for `held`; a job's Draft (no `chatWriting`) and a Proposal that is
        // not a Draft do not hold anything.
        proposal(dir.path(), "aaaaaaaa-held", "draft", Some(true), &held);
        proposal(dir.path(), "bbbbbbbb-job", "draft", None, &free);
        proposal(dir.path(), "cccccccc-open", "open", Some(true), &other);

        assert_eq!(store.state_of(&held).unwrap(), RequestState::HeldByChat);
        assert_eq!(store.state_of(&free).unwrap(), RequestState::Writing);
        assert_eq!(store.state_of(&other).unwrap(), RequestState::Writing);
        let states: Vec<(String, RequestState)> = store
            .list()
            .unwrap()
            .into_iter()
            .map(|entry| (entry.id, entry.state))
            .collect();
        assert_eq!(
            states,
            [
                (other, RequestState::Writing),
                (free, RequestState::Writing),
                (held, RequestState::HeldByChat),
            ]
        );
        // Nothing is recorded: the app writes no `.state.json` for it.
        assert!(!dir.path().join(".weaver/requests/.state.json").exists());
    }

    #[test]
    fn a_request_held_by_chat_can_be_read_but_not_saved_sent_discarded_or_given_images() {
        let (dir, store) = store();
        let id = written(&store, 9, "Add orders");
        proposal(dir.path(), "aaaaaaaa-held", "draft", Some(true), &id);

        assert!(matches!(
            store.save(&id, "changed"),
            Err(RequestError::HeldByChat)
        ));
        assert!(matches!(
            store.plan_send(&id, false),
            Err(RequestError::HeldByChat)
        ));
        assert!(matches!(
            store.record_sent(&id, "jjjjjjjj"),
            Err(RequestError::HeldByChat)
        ));
        assert!(matches!(
            store.discard(&id),
            Err(RequestError::HeldByChat)
        ));
        assert!(matches!(
            store.add_image(&id, &PNG),
            Err(RequestError::HeldByChat)
        ));
        assert_eq!(store.read(&id).unwrap(), "Add orders");
        assert!(dir
            .path()
            .join(format!(".weaver/requests/{id}.md"))
            .is_file());
        assert!(store.state().unwrap().sent.is_empty());
    }

    #[test]
    fn discarding_the_draft_of_chat_gives_the_request_back() {
        let (dir, store) = store();
        let id = written(&store, 9, "Add orders");
        proposal(dir.path(), "aaaaaaaa-held", "draft", Some(true), &id);
        assert_eq!(store.state_of(&id).unwrap(), RequestState::HeldByChat);

        proposal(dir.path(), "aaaaaaaa-held", "discarded", Some(true), &id);
        assert_eq!(store.state_of(&id).unwrap(), RequestState::Writing);
        store.save(&id, "Add orders and carts").unwrap();
        assert!(matches!(
            store.plan_send(&id, false).unwrap(),
            SendPlan::Start(_)
        ));
    }

    #[test]
    fn the_hand_over_of_the_draft_of_chat_consumes_the_request_it_held() {
        let (dir, store) = store();
        let id = written(&store, 9, "Add orders");
        store.add_image(&id, &PNG).unwrap();
        proposal(dir.path(), "aaaaaaaa-held", "draft", Some(true), &id);
        let folder = dir.path().join(".weaver/requests");

        // The hand-over made it an Open Proposal without `chatWriting`: the app says it came from Chat.
        proposal(dir.path(), "aaaaaaaa-held", "open", None, &id);
        assert!(store.consume(&id, true).unwrap());
        assert!(!folder.join(format!("{id}.md")).exists());
        assert!(!folder.join(format!("{id}.images")).exists());
        assert_eq!(store.state_of(&id).unwrap(), RequestState::Consumed);
        assert_eq!(store.state().unwrap().consumed, std::slice::from_ref(&id));
        assert!(!store.consume(&id, true).unwrap(), "only once");
    }

    #[test]
    fn a_held_request_whose_file_is_deleted_from_outside_is_discarded() {
        let (dir, store) = store();
        let id = written(&store, 9, "x");
        proposal(dir.path(), "aaaaaaaa-held", "draft", Some(true), &id);
        fs::remove_file(dir.path().join(format!(".weaver/requests/{id}.md"))).unwrap();
        assert_eq!(store.state_of(&id).unwrap(), RequestState::Discarded);
        assert!(store.list().unwrap().is_empty());
        assert!(matches!(store.read(&id), Err(RequestError::Gone)));
        assert!(!store.consume(&id, false).unwrap());
    }

    #[test]
    fn a_job_that_ends_without_a_proposal_returns_the_request_to_writing() {
        let (_dir, store) = store();
        let id = written(&store, 9, "x");
        let other = written(&store, 10, "y");
        store.record_sent(&id, "jjjjjjjj").unwrap();
        store.record_sent(&other, "kkkkkkkk").unwrap();

        assert_eq!(
            store.job_ended("jjjjjjjj", false).unwrap(),
            std::slice::from_ref(&id)
        );
        assert_eq!(store.state_of(&id).unwrap(), RequestState::Writing);
        assert_eq!(
            store.state_of(&other).unwrap(),
            RequestState::Sent,
            "another job's request stays sent"
        );
        // It can be changed and sent again.
        store.save(&id, "x, better").unwrap();
        assert!(matches!(
            store.plan_send(&id, false).unwrap(),
            SendPlan::Start(_)
        ));
        // The job ending again changes nothing.
        assert!(store.job_ended("jjjjjjjj", false).unwrap().is_empty());
    }

    #[test]
    fn a_job_that_handed_over_a_proposal_leaves_the_request_to_its_consumption() {
        let (_dir, store) = store();
        let id = written(&store, 9, "x");
        store.record_sent(&id, "jjjjjjjj").unwrap();
        assert!(store.job_ended("jjjjjjjj", true).unwrap().is_empty());
        assert_eq!(store.state_of(&id).unwrap(), RequestState::Sent);
        // A job that ends after the request was consumed finds nothing.
        assert!(store.consume(&id, false).unwrap());
        assert!(store.job_ended("jjjjjjjj", false).unwrap().is_empty());
        assert_eq!(store.state_of(&id).unwrap(), RequestState::Consumed);
    }

    #[test]
    fn a_file_deleted_from_outside_is_never_sent_afterwards() {
        let (dir, store) = store();
        let id = written(&store, 9, "x");
        fs::remove_file(dir.path().join(format!(".weaver/requests/{id}.md"))).unwrap();
        // Weaver still showed it: Send, Save and image paste all find it gone and write nothing.
        assert!(matches!(
            store.plan_send(&id, false),
            Err(RequestError::Gone)
        ));
        assert!(matches!(store.save(&id, "again"), Err(RequestError::Gone)));
        assert!(matches!(
            store.add_image(&id, &PNG),
            Err(RequestError::Gone)
        ));
        assert!(matches!(
            store.record_sent(&id, "jjjjjjjj"),
            Err(RequestError::Gone)
        ));
        assert!(!dir
            .path()
            .join(format!(".weaver/requests/{id}.md"))
            .exists());
        assert!(store.state().unwrap().sent.is_empty());
        assert!(store.list().unwrap().is_empty());
    }

    #[test]
    fn a_sent_request_deleted_from_outside_is_forgotten() {
        let (dir, store) = store();
        let id = written(&store, 9, "x");
        store.record_sent(&id, "jjjjjjjj").unwrap();
        fs::remove_file(dir.path().join(format!(".weaver/requests/{id}.md"))).unwrap();
        assert_eq!(store.reconcile().unwrap(), std::slice::from_ref(&id));
        assert!(store.state().unwrap().sent.is_empty());
        assert!(store.list().unwrap().is_empty());
        // Its job ending afterwards returns nothing to writing.
        assert!(store.job_ended("jjjjjjjj", false).unwrap().is_empty());
        assert_eq!(store.state_of(&id).unwrap(), RequestState::Discarded);
        // Nothing is consumed either: it was discarded, not consumed.
        assert!(!store.consume(&id, false).unwrap());
        assert!(store.state().unwrap().consumed.is_empty());
    }

    #[test]
    fn a_consumed_id_is_ignored_even_if_its_file_reappears() {
        let (dir, store) = store();
        let id = written(&store, 9, "x");
        store.record_sent(&id, "jjjjjjjj").unwrap();
        assert!(store.consume(&id, false).unwrap());
        // An editor saves the file again.
        fs::write(
            dir.path().join(format!(".weaver/requests/{id}.md")),
            "back\n",
        )
        .unwrap();
        assert!(store.list().unwrap().is_empty(), "never shown");
        assert!(matches!(store.read(&id), Err(RequestError::Gone)));
        assert!(matches!(
            store.plan_send(&id, false),
            Err(RequestError::Gone)
        ));
        assert!(matches!(store.save(&id, "more"), Err(RequestError::Gone)));
        assert_eq!(store.state_of(&id).unwrap(), RequestState::Consumed);
        assert!(!store.consume(&id, false).unwrap());
    }

    #[test]
    fn discarding_deletes_the_file_and_the_images_in_one_step() {
        let (dir, store) = store();
        let id = written(&store, 9, "x");
        let link = store.add_image(&id, &PNG).unwrap();
        assert_eq!(link, format!("{id}.images/image-1.png"));
        let folder = dir.path().join(".weaver/requests");
        assert!(folder.join(&link).is_file());
        store.discard(&id).unwrap();
        assert!(!folder.join(format!("{id}.md")).exists());
        assert!(!folder.join(format!("{id}.images")).exists());
        assert_eq!(store.state_of(&id).unwrap(), RequestState::Discarded);
        store.discard(&id).unwrap();
    }

    #[test]
    fn images_are_saved_with_the_next_free_number() {
        let (_dir, store) = store();
        let id = written(&store, 9, "x");
        let mut jpeg = vec![0xFF, 0xD8, 0xFF, 0xE0];
        jpeg.resize(10, 0);
        assert_eq!(
            store.add_image(&id, &PNG).unwrap(),
            format!("{id}.images/image-1.png")
        );
        assert_eq!(
            store.add_image(&id, &jpeg).unwrap(),
            format!("{id}.images/image-2.jpg")
        );
        assert_eq!(
            store.add_image(&id, &PNG).unwrap(),
            format!("{id}.images/image-3.png")
        );
        let (kind, bytes) = store
            .read_image(&id, &format!("{id}.images/image-2.jpg"))
            .unwrap();
        assert_eq!((kind, bytes), (ImageKind::Jpeg, jpeg));
    }

    #[test]
    fn an_image_that_is_not_accepted_is_refused_and_leaves_no_folder() {
        let (dir, store) = store();
        let id = written(&store, 9, "x");
        assert!(matches!(
            store.add_image(&id, b"<svg/>"),
            Err(RequestError::Image(ImageError::NotAccepted))
        ));
        let mut big = PNG.to_vec();
        big.resize(MAX_IMAGE_BYTES + 1, 0);
        assert!(matches!(
            store.add_image(&id, &big),
            Err(RequestError::Image(ImageError::NotAccepted))
        ));
        assert!(!dir
            .path()
            .join(format!(".weaver/requests/{id}.images"))
            .exists());
    }

    #[test]
    fn only_the_images_folder_of_the_request_is_served() {
        let (dir, store) = store();
        let id = written(&store, 9, "x");
        store.add_image(&id, &PNG).unwrap();
        fs::write(dir.path().join(".weaver/requests/secret.png"), PNG).unwrap();
        for link in [
            "secret.png",
            "../secret.png",
            &format!("{id}.images/../secret.png"),
            &format!("{id}.images/"),
            &format!("{id}.images/.hidden"),
            &format!("{id}.images\\image-1.png"),
            "2026-11-02T07-05-10.images/image-1.png",
            &format!("{id}.images/missing.png"),
        ] {
            assert!(store.read_image(&id, link).is_err(), "{link}");
        }
        assert!(store
            .read_image(&id, &format!("{id}.images/image-1.png"))
            .is_ok());
    }

    #[test]
    fn a_dropped_file_inside_the_project_is_linked_where_it_is_and_one_outside_is_copied() {
        let (dir, store) = store();
        let id = written(&store, 9, "x");
        let inside = dir.path().join("docs/my shots");
        fs::create_dir_all(&inside).unwrap();
        fs::write(inside.join("a (1).png"), PNG).unwrap();
        assert_eq!(
            store.import_image(&id, &inside.join("a (1).png")).unwrap(),
            "../../docs/my%20shots/a%20%281%29.png"
        );
        assert!(
            !dir.path()
                .join(format!(".weaver/requests/{id}.images"))
                .exists(),
            "linked, not copied"
        );

        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("b.PNG"), PNG).unwrap();
        assert_eq!(
            store
                .import_image(&id, &outside.path().join("b.PNG"))
                .unwrap(),
            format!("{id}.images/image-1.png")
        );

        // A file already in the request's images folder keeps its link.
        let own = dir
            .path()
            .join(format!(".weaver/requests/{id}.images/image-1.png"));
        assert_eq!(
            store.import_image(&id, &own).unwrap(),
            format!("{id}.images/image-1.png")
        );

        // Not an image, wrong extension.
        fs::write(outside.path().join("c.png"), b"not an image").unwrap();
        fs::write(outside.path().join("d.svg"), PNG).unwrap();
        for name in ["c.png", "d.svg"] {
            assert!(
                matches!(
                    store.import_image(&id, &outside.path().join(name)),
                    Err(RequestError::Image(_))
                ),
                "{name}"
            );
        }
    }

    #[test]
    fn a_name_that_is_not_a_request_id_never_reaches_the_disk() {
        let (dir, store) = store();
        fs::create_dir_all(dir.path().join(".weaver")).unwrap();
        for bad in ["../x", "", "x/y", ".state"] {
            assert!(
                matches!(store.read(bad), Err(RequestError::NotAnId(_))),
                "{bad}"
            );
            assert!(
                matches!(store.save(bad, "x"), Err(RequestError::NotAnId(_))),
                "{bad}"
            );
            assert!(
                matches!(store.discard(bad), Err(RequestError::NotAnId(_))),
                "{bad}"
            );
            assert!(
                matches!(store.state_of(bad), Err(RequestError::NotAnId(_))),
                "{bad}"
            );
        }
    }

    #[test]
    fn an_unreadable_record_is_reported_and_never_overwritten() {
        let (dir, store) = store();
        let id = written(&store, 9, "x");
        let state = dir.path().join(".weaver/requests/.state.json");
        fs::write(&state, "{ not json").unwrap();
        assert!(matches!(store.list(), Err(RequestError::State(_))));
        assert!(matches!(
            store.record_sent(&id, "jjjjjjjj"),
            Err(RequestError::State(_))
        ));
        assert_eq!(fs::read(&state).unwrap(), b"{ not json");
    }

    #[test]
    fn the_record_has_the_documented_shape() {
        let (dir, store) = store();
        let id = written(&store, 9, "x");
        store.record_sent(&id, "jjjjjjjj").unwrap();
        let json: serde_json::Value = serde_json::from_slice(
            &fs::read(dir.path().join(".weaver/requests/.state.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "sent": { id.clone(): "jjjjjjjj" }, "consumed": [] })
        );
        // A record written by hand with a missing key still reads.
        fs::write(
            dir.path().join(".weaver/requests/.state.json"),
            "{\"consumed\": [\"2026-11-02T07-05-10\"]}",
        )
        .unwrap();
        assert_eq!(store.state().unwrap().consumed, ["2026-11-02T07-05-10"]);
    }
}
