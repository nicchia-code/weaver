//! The layout of `.weaver/`, and which file is what.
//!
//! ```text
//! .weaver/
//!   model/                       system.md, actors/<actor>.md, <feature>/feature.md, <feature>/<machine>.md
//!   requirements/<area>/         area.md and <fragment>.md
//!   specs.yaml                   optional
//!   proposals/<id>-<slug>/       proposal.md, the files it changes, tasks/<id>-<slug>.md
//!   tasks/<id>-<slug>.md         live Tasks
//!   drifts/<id>-<slug>.md        every Drift
//!   history/<id>-<slug>.md       History entries
//!   assets/<name>                the developer's assets, flat, files only; versioned, never read as project files
//!   requests/                    local only
//!   short-numbers/assigned       local only
//!   task-images/<task id>/       local only
//!   notes/                       local only: the developer's notes, `notes.md` and its images
//!   solo/solo.json               local only: Solo mode of this working copy
//!   merge/merge.json             local only: the merge in progress (`crate::solo_merge`)
//! ```
//!
//! Paths in this crate's project API are relative to `.weaver/` and use `/`, whatever the
//! operating system. Files and folders whose name starts with `.` are never read.

use std::io;
use std::path::{Component, Path, PathBuf};

use super::cards::CardKind;
use super::ids::split_card_stem;
use super::model::is_element_id;
use super::requirements::AREA_FILE;
use super::text;

/// The project folder holding everything Weaver keeps.
pub const WEAVER_DIR: &str = ".weaver";

pub const MODEL_DIR: &str = "model";
pub const REQUIREMENTS_DIR: &str = "requirements";
pub const SPECS_FILE: &str = "specs.yaml";
pub const PROPOSALS_DIR: &str = "proposals";
pub const TASKS_DIR: &str = "tasks";
pub const DRIFTS_DIR: &str = "drifts";
pub const HISTORY_DIR: &str = "history";
pub const PROPOSAL_FILE: &str = "proposal.md";

/// The developer's assets: flat, files only, versioned with the project, created when the first
/// asset is copied in. The project image never reads them (`crate::assets` lists them).
pub const ASSETS_DIR: &str = "assets";

/// Whether the `.weaver/`-relative path `rel` is the assets folder or lies inside it.
pub fn is_assets_rel(rel: &str) -> bool {
    rel == ASSETS_DIR
        || rel
            .strip_prefix(ASSETS_DIR)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// The developer's notes: `notes.md` and the images pasted in it. Local only, created when the
/// sheet or an image is first written (never at initialisation). Only the Notes view writes in
/// it; the project image never reads its files and the watcher sends their events to the view
/// alone (`crate::notes`).
pub const NOTES_DIR: &str = "notes";

/// Whether the `.weaver/`-relative path `rel` is the notes folder or lies inside it.
pub fn is_notes_rel(rel: &str) -> bool {
    rel == NOTES_DIR
        || rel
            .strip_prefix(NOTES_DIR)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// Solo mode of this working copy: `solo.json`, local only (`crate::solo_mode`).
pub const SOLO_DIR: &str = "solo";

/// The merge in progress of Solo mode: `merge.json`, local only (`crate::solo_merge`).
pub const MERGE_DIR: &str = "merge";

/// Local-only folders: each holds its own `.gitignore` containing `*`.
pub const REQUESTS_DIR: &str = "requests";
pub const SHORT_NUMBERS_DIR: &str = "short-numbers";
pub const TASK_IMAGES_DIR: &str = "task-images";
pub const LOCAL_ONLY_DIRS: [&str; 6] = [
    REQUESTS_DIR,
    SHORT_NUMBERS_DIR,
    TASK_IMAGES_DIR,
    NOTES_DIR,
    SOLO_DIR,
    MERGE_DIR,
];

/// The versioned entries of `.weaver/` that the project image reads.
pub const VERSIONED_ENTRIES: [&str; 7] = [
    MODEL_DIR,
    REQUIREMENTS_DIR,
    SPECS_FILE,
    PROPOSALS_DIR,
    TASKS_DIR,
    DRIFTS_DIR,
    HISTORY_DIR,
];

const GITIGNORE: &str = ".gitignore";

/// `<project root>/.weaver`.
pub fn weaver_dir(project_root: &Path) -> PathBuf {
    project_root.join(WEAVER_DIR)
}

/// The absolute path of a `.weaver/`-relative path (with `/` or `\` separators).
pub fn absolute(project_root: &Path, rel: &str) -> PathBuf {
    let mut path = weaver_dir(project_root);
    path.extend(
        normalize_rel(rel)
            .split('/')
            .filter(|part| !part.is_empty()),
    );
    path
}

/// Turns a path written with Windows separators into the `/` form used for keys.
pub fn normalize_rel(rel: &str) -> String {
    rel.replace('\\', "/")
}

/// The `.weaver/`-relative key of an absolute path, or `None` when it is outside `.weaver/`.
pub fn rel_key(project_root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(weaver_dir(project_root)).ok()?;
    let mut parts = Vec::new();
    for component in rel.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_string_lossy().into_owned()),
            _ => return None,
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// Whether any component of `rel` starts with `.`.
pub fn is_hidden(rel: &str) -> bool {
    rel.split('/').any(|part| part.starts_with('.'))
}

/// Makes sure the local-only folder `name` exists and holds its `.gitignore` with `*`,
/// writing the file again whenever it is missing. Returns the folder.
pub fn ensure_local_folder(project_root: &Path, name: &str) -> io::Result<PathBuf> {
    let folder = weaver_dir(project_root).join(name);
    std::fs::create_dir_all(&folder)?;
    let ignore = folder.join(GITIGNORE);
    if !ignore.exists() {
        text::write_text(&ignore, "*")?;
    }
    Ok(folder)
}

/// What the file at a `.weaver/`-relative path is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileClass {
    /// Not read: hidden, local-only, a copy inside a Proposal folder, or another file type.
    Ignored,
    System,
    Actor {
        id: String,
    },
    Feature {
        id: String,
    },
    Machine {
        feature: String,
        id: String,
    },
    AreaDoc {
        area: String,
    },
    Fragment {
        area: String,
        name: String,
    },
    Specs,
    Card {
        kind: CardKind,
        id: String,
        slug: String,
        parent: Option<String>,
    },
    /// A file in a versioned folder where nothing is expected; the reason says why.
    Misplaced(String),
}

/// Classifies the file at `rel` (a `/`-separated path relative to `.weaver/`).
pub fn classify(rel: &str) -> FileClass {
    if is_hidden(rel) {
        return FileClass::Ignored;
    }
    let parts: Vec<&str> = rel.split('/').collect();
    let md_stem = |name: &str| name.strip_suffix(".md").map(str::to_owned);
    match parts.as_slice() {
        [SPECS_FILE] => FileClass::Specs,
        [MODEL_DIR, rest @ ..] => classify_model(rest, &md_stem),
        [REQUIREMENTS_DIR, rest @ ..] => classify_requirements(rest, &md_stem),
        [TASKS_DIR, name] => card_file(CardKind::Task, name, None),
        [DRIFTS_DIR, name] => card_file(CardKind::Drift, name, None),
        [HISTORY_DIR, name] => card_file(CardKind::History, name, None),
        [TASKS_DIR | DRIFTS_DIR | HISTORY_DIR, ..] => {
            FileClass::Misplaced("folders are not allowed here".to_owned())
        }
        [PROPOSALS_DIR, folder, file] if *file == PROPOSAL_FILE => match split_card_stem(folder) {
            Some((id, slug)) => FileClass::Card {
                kind: CardKind::Proposal,
                id: id.to_owned(),
                slug: slug.to_owned(),
                parent: None,
            },
            None => FileClass::Misplaced("a Proposal folder is named `<id>-<slug>`".to_owned()),
        },
        [PROPOSALS_DIR, folder, TASKS_DIR, name] => match split_card_stem(folder) {
            Some((parent, _)) => card_file(CardKind::ProposalTask, name, Some(parent.to_owned())),
            None => FileClass::Misplaced("a Proposal folder is named `<id>-<slug>`".to_owned()),
        },
        [PROPOSALS_DIR, ..] => FileClass::Ignored,
        [NOTES_DIR, ..] => FileClass::Ignored,
        _ => FileClass::Ignored,
    }
}

fn card_file(kind: CardKind, name: &str, parent: Option<String>) -> FileClass {
    let Some(stem) = name.strip_suffix(".md") else {
        return FileClass::Ignored;
    };
    match split_card_stem(stem) {
        Some((id, slug)) => FileClass::Card {
            kind,
            id: id.to_owned(),
            slug: slug.to_owned(),
            parent,
        },
        None => FileClass::Misplaced(
            "a card file is named `<id>-<slug>.md`, the id being 8 characters of [a-z0-9]"
                .to_owned(),
        ),
    }
}

fn classify_model(rest: &[&str], md_stem: &dyn Fn(&str) -> Option<String>) -> FileClass {
    match rest {
        ["system.md"] => FileClass::System,
        ["actors", name] => match md_stem(name) {
            Some(id) => FileClass::Actor { id },
            None => FileClass::Ignored,
        },
        [feature, "feature.md"] => FileClass::Feature { id: (*feature).to_owned() },
        [feature, name] if *feature != "actors" => match md_stem(name) {
            Some(id) => FileClass::Machine { feature: (*feature).to_owned(), id },
            None => FileClass::Ignored,
        },
        [name] if name.ends_with(".md") => {
            FileClass::Misplaced("the only `.md` directly in `model/` is `system.md`; Machines live in `<feature>/<machine>.md`".to_owned())
        }
        [] | [_] => FileClass::Ignored,
        [.., last] if !last.ends_with(".md") => FileClass::Ignored,
        _ => FileClass::Misplaced("`model/` has `actors/<actor>.md` and `<feature>/<file>.md` only: no deeper folders".to_owned()),
    }
}

fn classify_requirements(rest: &[&str], md_stem: &dyn Fn(&str) -> Option<String>) -> FileClass {
    match rest {
        [name] if name.ends_with(".md") => FileClass::Misplaced(
            "a `.md` directly in `requirements/` is an error: an Area is a folder with `area.md` and fragments (`weaver project edit` converts it)".to_owned(),
        ),
        [area, name] if name.ends_with(".md") && !is_element_id(area) => {
            FileClass::Misplaced(format!("Area folder `{area}` must be kebab-case"))
        }
        [area, name] => match md_stem(name) {
            Some(stem) if stem == AREA_FILE => FileClass::AreaDoc { area: (*area).to_owned() },
            Some(stem) => FileClass::Fragment { area: (*area).to_owned(), name: stem },
            None => FileClass::Ignored,
        },
        [.., last] if !last.ends_with(".md") => FileClass::Ignored,
        [_, _, ..] => FileClass::Misplaced("an Area has no subfolders".to_owned()),
        _ => FileClass::Ignored,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(kind: CardKind, id: &str, slug: &str, parent: Option<&str>) -> FileClass {
        FileClass::Card {
            kind,
            id: id.into(),
            slug: slug.into(),
            parent: parent.map(Into::into),
        }
    }

    #[test]
    fn model_files() {
        assert_eq!(classify("model/system.md"), FileClass::System);
        assert_eq!(
            classify("model/actors/user.md"),
            FileClass::Actor { id: "user".into() }
        );
        assert_eq!(
            classify("model/shop/feature.md"),
            FileClass::Feature { id: "shop".into() }
        );
        assert_eq!(
            classify("model/shop/order.md"),
            FileClass::Machine {
                feature: "shop".into(),
                id: "order".into()
            }
        );
        assert!(matches!(
            classify("model/order.md"),
            FileClass::Misplaced(_)
        ));
        assert!(matches!(
            classify("model/shop/deep/order.md"),
            FileClass::Misplaced(_)
        ));
        assert_eq!(classify("model/shop/notes.txt"), FileClass::Ignored);
    }

    #[test]
    fn requirement_files() {
        assert_eq!(
            classify("requirements/ui/area.md"),
            FileClass::AreaDoc { area: "ui".into() }
        );
        assert_eq!(
            classify("requirements/ui/main.md"),
            FileClass::Fragment {
                area: "ui".into(),
                name: "main".into()
            }
        );
        assert!(
            matches!(classify("requirements/ui.md"), FileClass::Misplaced(m) if m.contains("weaver project edit"))
        );
        assert!(
            matches!(classify("requirements/ui/sub/x.md"), FileClass::Misplaced(m) if m.contains("no subfolders"))
        );
        assert_eq!(classify("requirements/ui/logo.png"), FileClass::Ignored);
    }

    #[test]
    fn specs_and_cards() {
        assert_eq!(classify("specs.yaml"), FileClass::Specs);
        assert_eq!(
            classify("tasks/vxdhoylk-core.md"),
            card(CardKind::Task, "vxdhoylk", "core", None)
        );
        assert_eq!(
            classify("drifts/vxdhoylk-core.md"),
            card(CardKind::Drift, "vxdhoylk", "core", None)
        );
        assert_eq!(
            classify("history/vxdhoylk-core.md"),
            card(CardKind::History, "vxdhoylk", "core", None)
        );
        assert_eq!(
            classify("proposals/qmqh1o3p-setup/proposal.md"),
            card(CardKind::Proposal, "qmqh1o3p", "setup", None)
        );
        assert_eq!(
            classify("proposals/qmqh1o3p-setup/tasks/vxdhoylk-core.md"),
            card(CardKind::ProposalTask, "vxdhoylk", "core", Some("qmqh1o3p"))
        );
        assert!(matches!(
            classify("tasks/notes.md"),
            FileClass::Misplaced(_)
        ));
        assert!(matches!(
            classify("tasks/sub/vxdhoylk-core.md"),
            FileClass::Misplaced(_)
        ));
        assert!(matches!(
            classify("proposals/bad/proposal.md"),
            FileClass::Misplaced(_)
        ));
        assert_eq!(
            classify("proposals/qmqh1o3p-setup/model/shop/order.md"),
            FileClass::Ignored
        );
        assert_eq!(
            classify("proposals/qmqh1o3p-setup/specs.yaml"),
            FileClass::Ignored
        );
    }

    #[test]
    fn hidden_and_local_files_are_never_read() {
        for rel in [
            "model/.gitkeep",
            "tasks/.gitkeep",
            "model/.hidden/order.md",
            "tasks/.vxdhoylk-core.md.0123.tmp",
            "requirements/ui/.draft.md",
            "requests/2026-11-02T07-00-00.md",
            "short-numbers/assigned",
            "task-images/vxdhoylk/a.png",
            "proposals/.cache/proposal.md",
        ] {
            assert_eq!(classify(rel), FileClass::Ignored, "{rel}");
        }
    }

    #[test]
    fn paths_with_windows_separators_and_non_ascii_names() {
        assert_eq!(
            normalize_rel("model\\shop\\order.md"),
            "model/shop/order.md"
        );
        let root = Path::new("project root ü");
        let abs = absolute(root, "model\\shop\\ordine così.md");
        assert_eq!(
            abs,
            Path::new("project root ü/.weaver/model/shop/ordine così.md")
        );
        assert_eq!(
            rel_key(root, &abs).as_deref(),
            Some("model/shop/ordine così.md")
        );
        assert_eq!(rel_key(root, Path::new("elsewhere/x.md")), None);
        assert_eq!(rel_key(root, &weaver_dir(root)), None);
    }

    #[test]
    fn local_folders_get_a_gitignore_that_comes_back_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        for name in LOCAL_ONLY_DIRS {
            let folder = ensure_local_folder(dir.path(), name).unwrap();
            assert_eq!(std::fs::read(folder.join(".gitignore")).unwrap(), b"*\n");
        }
        let ignore = weaver_dir(dir.path()).join("requests/.gitignore");
        std::fs::remove_file(&ignore).unwrap();
        ensure_local_folder(dir.path(), "requests").unwrap();
        assert_eq!(std::fs::read(&ignore).unwrap(), b"*\n");
        // Nothing else of `.weaver/` is ignored.
        assert!(!weaver_dir(dir.path()).join(".gitignore").exists());
        assert!(!weaver_dir(dir.path()).join("tasks").exists());
    }

    #[test]
    fn the_assets_folder_is_versioned_and_never_read_as_project_files() {
        assert!(!LOCAL_ONLY_DIRS.contains(&ASSETS_DIR));
        assert!(!VERSIONED_ENTRIES.contains(&ASSETS_DIR));
        assert!(is_assets_rel("assets"));
        assert!(is_assets_rel("assets/logo.png"));
        assert!(!is_assets_rel("assets-old/logo.png"));
        assert!(!is_assets_rel("tasks/assets"));
        assert_eq!(classify("assets/notes.md"), FileClass::Ignored);
    }

    #[test]
    fn the_notes_folder_is_local_only_and_never_read_as_project_files() {
        assert!(LOCAL_ONLY_DIRS.contains(&NOTES_DIR));
        assert!(!VERSIONED_ENTRIES.contains(&NOTES_DIR));
        assert!(is_notes_rel("notes"));
        assert!(is_notes_rel("notes/notes.md"));
        assert!(is_notes_rel("notes/pasted/a.png"));
        assert!(!is_notes_rel("notes-old/notes.md"));
        assert!(!is_notes_rel("tasks/notes"));
        // Whatever its name looks like, a file under `notes/` is not a project file.
        for rel in [
            "notes",
            "notes/notes.md",
            "notes/specs.yaml",
            "notes/model/system.md",
            "notes/requirements/ui/area.md",
            "notes/tasks/vxdhoylk-core.md",
            "notes/drifts/vxdhoylk-core.md",
            "notes/history/vxdhoylk-core.md",
            "notes/proposals/vxdhoylk-core/proposal.md",
            "notes/image.png",
        ] {
            assert_eq!(classify(rel), FileClass::Ignored, "{rel}");
        }
    }

    #[test]
    fn the_solo_folder_is_local_only_and_never_read_as_project_files() {
        assert!(LOCAL_ONLY_DIRS.contains(&SOLO_DIR));
        assert!(!VERSIONED_ENTRIES.contains(&SOLO_DIR));
        assert_eq!(classify("solo/solo.json"), FileClass::Ignored);
        let dir = tempfile::tempdir().unwrap();
        let folder = ensure_local_folder(dir.path(), SOLO_DIR).unwrap();
        assert_eq!(folder, weaver_dir(dir.path()).join("solo"));
        let ignore = folder.join(".gitignore");
        assert_eq!(std::fs::read(&ignore).unwrap(), b"*\n");
        std::fs::remove_file(&ignore).unwrap();
        ensure_local_folder(dir.path(), SOLO_DIR).unwrap();
        assert_eq!(std::fs::read(&ignore).unwrap(), b"*\n");
    }

    #[test]
    fn the_merge_folder_is_local_only_and_never_read_as_project_files() {
        assert!(LOCAL_ONLY_DIRS.contains(&MERGE_DIR));
        assert!(!VERSIONED_ENTRIES.contains(&MERGE_DIR));
        assert_eq!(classify("merge/merge.json"), FileClass::Ignored);
        let dir = tempfile::tempdir().unwrap();
        let folder = ensure_local_folder(dir.path(), MERGE_DIR).unwrap();
        assert_eq!(folder, weaver_dir(dir.path()).join("merge"));
        let ignore = folder.join(".gitignore");
        assert_eq!(std::fs::read(&ignore).unwrap(), b"*\n");
        std::fs::remove_file(&ignore).unwrap();
        ensure_local_folder(dir.path(), MERGE_DIR).unwrap();
        assert_eq!(std::fs::read(&ignore).unwrap(), b"*\n");
    }

    #[test]
    fn the_notes_folder_and_its_gitignore_come_with_the_first_write_and_again_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        // Nothing makes it before a write: not even the project's `.weaver/`.
        assert!(!weaver_dir(dir.path()).join(NOTES_DIR).exists());

        let folder = ensure_local_folder(dir.path(), NOTES_DIR).unwrap();
        assert_eq!(folder, weaver_dir(dir.path()).join("notes"));
        let ignore = folder.join(".gitignore");
        assert_eq!(std::fs::read(&ignore).unwrap(), b"*\n");

        std::fs::remove_file(&ignore).unwrap();
        ensure_local_folder(dir.path(), NOTES_DIR).unwrap();
        assert_eq!(std::fs::read(&ignore).unwrap(), b"*\n");
    }
}
