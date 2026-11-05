//! The application menu: what makes `Quit` and `Close Window` reachable by the keyboard.
//!
//! Quit is `Cmd+Q` on macOS, where Tauri's default menu has it in the application menu and the
//! system raises `RunEvent::ExitRequested` like any other quit; on Windows and Linux it is
//! `Ctrl+Q` and **File › Quit**, a menu item of Weaver's own (the predefined one ends the process
//! without asking), which starts the quit path ([`super::request_quit`]). The Edit entries are
//! needed for copy and paste to work in the webview.
//!
//! **Close Window** (`Cmd+W` on macOS, `Ctrl+W` elsewhere) is also an item of Weaver's own, since a
//! menu accelerator is handled before the webview sees the key: while the File view of the window
//! has a tab ([`super::WindowManager::close_key_claimed`]) it closes that tab (`files-close-tab`),
//! otherwise it closes the window, through the same close path as its title bar button.
//!
//! The top of the File menu, on every system, is **Open Folder…** (`CmdOrCtrl+O`, a native
//! accelerator, so it works whatever has the focus), the **Open Recent** submenu and a separator;
//! they act through [`super::menu_open`]. The submenu is created once with the menu and changed in
//! place ([`refresh`]): at start, after every change of the recent projects and whenever a window
//! gets the focus.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use tauri::menu::{Menu, MenuItemBuilder, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Emitter, Manager, WebviewWindow, Wry};

use super::commands::{recent_entries_of, RecentEntry};
use super::registry::WELCOME_LABEL;
use crate::events::EventName;

/// Id of **File › Quit** (Windows and Linux).
pub const QUIT_ID: &str = "quit";
/// Id of **Close Window**.
pub const CLOSE_WINDOW_ID: &str = "close-window";
/// Id of **Open Folder…**.
pub const OPEN_FOLDER_ID: &str = "open-folder";
/// Id of the **Open Recent** submenu.
pub const OPEN_RECENT_ID: &str = "open-recent";
/// Id of **Clear Recent**.
pub const CLEAR_RECENT_ID: &str = "clear-recent";
/// Id of the disabled **No Recent Projects** item.
pub const NO_RECENT_ID: &str = "no-recent";
/// What an item of **Open Recent** has before the canonical path it opens.
pub const RECENT_PREFIX: &str = "recent:";

fn close_window_item(app: &AppHandle) -> tauri::Result<tauri::menu::MenuItem<Wry>> {
    MenuItemBuilder::with_id(CLOSE_WINDOW_ID, "Close Window")
        .accelerator(if cfg!(target_os = "macos") {
            "Cmd+W"
        } else {
            "Ctrl+W"
        })
        .build(app)
}

/// The menu of every window.
#[cfg(target_os = "macos")]
pub fn build(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    use tauri::menu::MenuItemKind;

    let menu = Menu::default(app)?;
    // The default menu closes the window itself, on `Cmd+W`: its first **Close Window** becomes
    // ours, and the items that open a project go above it, in the same File menu.
    for kind in menu.items()? {
        let MenuItemKind::Submenu(submenu) = kind else {
            continue;
        };
        for (at, item) in submenu.items()?.into_iter().enumerate() {
            let MenuItemKind::Predefined(predefined) = &item else {
                continue;
            };
            if predefined.text()? != "Close Window" {
                continue;
            }
            submenu.remove(&item)?;
            submenu.insert(&close_window_item(app)?, at)?;
            let (open_folder, recent, separator) = open_items(app)?;
            submenu.insert(&open_folder, at)?;
            submenu.insert(&recent, at + 1)?;
            submenu.insert(&separator, at + 2)?;
            return Ok(menu);
        }
    }
    Ok(menu)
}

/// The menu of every window.
#[cfg(not(target_os = "macos"))]
pub fn build(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    use tauri::menu::SubmenuBuilder;

    let quit = MenuItemBuilder::with_id(QUIT_ID, "Quit")
        .accelerator("Ctrl+Q")
        .build(app)?;
    let (open_folder, recent, separator) = open_items(app)?;
    let file = SubmenuBuilder::new(app, "File")
        .item(&open_folder)
        .item(&recent)
        .item(&separator)
        .item(&close_window_item(app)?)
        .item(&quit)
        .build()?;
    let edit = SubmenuBuilder::new(app, "Edit")
        .undo()
        .redo()
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .build()?;
    let window = SubmenuBuilder::new(app, "Window")
        .minimize()
        .maximize()
        .build()?;
    Menu::with_items(app, &[&file, &edit, &window])
}

/// A menu item was chosen.
pub fn on_event(app: &AppHandle, id: &str) {
    match id {
        QUIT_ID => super::request_quit(app),
        CLOSE_WINDOW_ID => close_window(app),
        OPEN_FOLDER_ID => super::menu_open::open_folder(app),
        CLEAR_RECENT_ID => super::menu_open::clear_recent(app),
        _ => {
            if let Some(path) = id.strip_prefix(RECENT_PREFIX) {
                super::menu_open::open_recent(app, path);
            }
        }
    }
}

/// The Weaver window that has the focus, which is the window a menu item is chosen from; `None`
/// when no window has it (on macOS with no window, or with every project window in the background).
pub fn focused_window(app: &AppHandle) -> Option<WebviewWindow> {
    app.webview_windows()
        .into_values()
        .find(|window| window.is_focused().unwrap_or(false))
}

/// **Close Window** in the window that has the focus: a tab of its File view, or the window.
fn close_window(app: &AppHandle) {
    let Some(window) = focused_window(app) else {
        return;
    };
    if let (Some(project_id), Some(manager)) = (
        crate::pictures::project_of_label(window.label()),
        super::manager_of(app),
    ) {
        if manager.close_key_claimed(project_id) {
            super::unsaved::emit_close_tab(app, project_id);
            return;
        }
    }
    if let Err(error) = window.close() {
        crate::log_warn!("the window {} could not be closed: {error}", window.label());
    }
}

/// What the `Open Recent` submenu holds, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecentItem {
    /// A recent project: opens `path` (its canonical path); disabled when its folder is gone.
    Project {
        path: String,
        text: String,
        enabled: bool,
    },
    Separator,
    /// `Clear Recent`.
    Clear,
    /// `No Recent Projects`, disabled.
    Empty,
}

/// The items of `Open Recent` for `recent`, the same projects as the welcome window in the same
/// order: each project, a separator and `Clear Recent`; with none, only the disabled
/// `No Recent Projects`.
pub fn recent_items(recent: &[RecentEntry]) -> Vec<RecentItem> {
    if recent.is_empty() {
        return vec![RecentItem::Empty];
    }
    let mut items: Vec<RecentItem> = recent
        .iter()
        .map(|entry| RecentItem::Project {
            path: entry.path.clone(),
            text: if entry.exists {
                entry.name.clone()
            } else {
                format!("{} — Not found", entry.name)
            },
            enabled: entry.exists,
        })
        .collect();
    items.push(RecentItem::Separator);
    items.push(RecentItem::Clear);
    items
}

/// A text as a menu label: `&` marks a mnemonic in menus, so a folder name has it doubled.
fn label(text: &str) -> String {
    text.replace('&', "&&")
}

/// The `Open Recent` submenu, created once with the menu and changed in place from then on.
pub struct RecentMenu {
    submenu: Submenu<Wry>,
    /// Rebuilds started; each takes the next number before it reads the recent projects.
    issued: AtomicU64,
    /// The number of the rebuild last applied: an older one that finishes later is dropped.
    applied: AtomicU64,
}

impl RecentMenu {
    fn new(submenu: Submenu<Wry>) -> Self {
        Self {
            submenu,
            issued: AtomicU64::new(0),
            applied: AtomicU64::new(0),
        }
    }
}

/// Replaces the items of `submenu` with `items`.
fn fill(submenu: &Submenu<Wry>, app: &AppHandle, items: &[RecentItem]) -> tauri::Result<()> {
    for item in submenu.items()? {
        submenu.remove(&item)?;
    }
    for item in items {
        match item {
            RecentItem::Project {
                path,
                text,
                enabled,
            } => submenu.append(
                &MenuItemBuilder::with_id(format!("{RECENT_PREFIX}{path}"), label(text))
                    .enabled(*enabled)
                    .build(app)?,
            )?,
            RecentItem::Separator => submenu.append(&PredefinedMenuItem::separator(app)?)?,
            RecentItem::Clear => submenu
                .append(&MenuItemBuilder::with_id(CLEAR_RECENT_ID, "Clear Recent").build(app)?)?,
            RecentItem::Empty => submenu.append(
                &MenuItemBuilder::with_id(NO_RECENT_ID, "No Recent Projects")
                    .enabled(false)
                    .build(app)?,
            )?,
        }
    }
    Ok(())
}

/// `Open Folder…`, `Open Recent` and a separator: the top of the File menu on every system.
fn open_items(
    app: &AppHandle,
) -> tauri::Result<(
    tauri::menu::MenuItem<Wry>,
    Submenu<Wry>,
    tauri::menu::PredefinedMenuItem<Wry>,
)> {
    let open_folder = MenuItemBuilder::with_id(OPEN_FOLDER_ID, "Open Folder…")
        .accelerator("CmdOrCtrl+O")
        .build(app)?;
    let recent = Submenu::with_id(app, OPEN_RECENT_ID, "Open Recent", true)?;
    fill(&recent, app, &recent_items(&[]))?;
    app.manage(RecentMenu::new(recent.clone()));
    Ok((open_folder, recent, PredefinedMenuItem::separator(app)?))
}

/// Rebuilds `Open Recent` and tells the welcome window the list, when it is open. It runs at
/// start, after every change of the recent projects and whenever a window gets the focus: the
/// folders are checked with `Path::is_dir` off the main thread, then the submenu is changed on it.
pub fn refresh(app: &AppHandle) {
    let Some(menu) = app.try_state::<RecentMenu>() else {
        return;
    };
    let ticket = menu.issued.fetch_add(1, Ordering::SeqCst) + 1;
    let Some(manager) = super::manager_of(app) else {
        return;
    };
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let recent = recent_entries_of(&manager.recent(), Path::is_dir);
        let on_main = app.clone();
        if let Err(error) = on_main.run_on_main_thread(move || apply(&app, ticket, &recent)) {
            crate::log_warn!("the Open Recent menu was not updated: {error}");
        }
    });
}

/// Applies the rebuild `ticket` on the main thread, unless a newer one was applied already.
fn apply(app: &AppHandle, ticket: u64, recent: &[RecentEntry]) {
    let Some(menu) = app.try_state::<RecentMenu>() else {
        return;
    };
    if menu.applied.load(Ordering::SeqCst) > ticket {
        return;
    }
    menu.applied.store(ticket, Ordering::SeqCst);
    if app.get_webview_window(WELCOME_LABEL).is_some() {
        if let Err(error) = app.emit_to(WELCOME_LABEL, EventName::RecentChanged.as_str(), recent) {
            crate::log_warn!("the welcome window was not told the recent projects: {error}");
        }
    }
    if let Err(error) = fill(&menu.submenu, app, &recent_items(recent)) {
        crate::log_warn!("the Open Recent menu was not updated: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, exists: bool) -> RecentEntry {
        RecentEntry {
            path: format!("/work/{name}"),
            name: name.to_owned(),
            opened: "2026-11-02T07:00:00.000Z".to_owned(),
            exists,
        }
    }

    #[test]
    fn open_recent_lists_the_projects_in_order_then_a_separator_and_clear_recent() {
        let items = recent_items(&[entry("alpha", true), entry("beta", true)]);
        assert_eq!(
            items,
            [
                RecentItem::Project {
                    path: "/work/alpha".into(),
                    text: "alpha".into(),
                    enabled: true
                },
                RecentItem::Project {
                    path: "/work/beta".into(),
                    text: "beta".into(),
                    enabled: true
                },
                RecentItem::Separator,
                RecentItem::Clear,
            ]
        );
    }

    #[test]
    fn a_folder_that_is_gone_is_a_disabled_item_marked_not_found() {
        let items = recent_items(&[entry("gone", false)]);
        assert_eq!(
            items[0],
            RecentItem::Project {
                path: "/work/gone".into(),
                text: "gone — Not found".into(),
                enabled: false
            }
        );
    }

    #[test]
    fn an_empty_list_holds_only_the_disabled_no_recent_projects_item() {
        assert_eq!(recent_items(&[]), [RecentItem::Empty]);
    }

    #[test]
    fn an_ampersand_in_a_folder_name_is_not_a_mnemonic() {
        assert_eq!(label("R&D"), "R&&D");
        assert_eq!(label("plain"), "plain");
    }
}
