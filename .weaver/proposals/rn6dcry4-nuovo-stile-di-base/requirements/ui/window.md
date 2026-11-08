---
description: "Use when building the project window's frame and sidebar, the welcome window, the File menu or Weaver's logo."
---
# Window

## window

### Frame
- The window's ground is Paper (`requirements/ui#General`). On the left the sidebar stands on Paper; on the right the work surface, a Surface panel inset 8 px from the window's edges and from the sidebar, radius 14 px, with a 1 px Line ring, holds the selected view.
- The banners of the window (startup, update, System Certificates, agent files) sit at the top of the work surface, full width of it, above the view's header; the view's own banners and notices stay inside the view.
- A project window has a left sidebar, 208 px wide, the full height of the window, with two groups of entries:
  - at the top, in this order: Board, Chat, Jobs, Files, Assets, Notes, Requirements, Specs, History;
  - at the bottom-left, kept apart from the top group by the free space between them: Usage, then Settings, the last entry.
- Sidebar entries are 34 px high, radius 9 px, icon and name in Ink 2; the selected entry stands on Surface with a 1 px Line ring, its name in Ink and medium weight.
- The selected entry is the view shown on the right; switching entries keeps the state of every view (focus, overlay, scroll), except the File view, which closes when another entry is chosen (`requirements/ui#file-view`), and the Assets view, which forgets its selection (`requirements/ui#asset-view`). Usage is not a view: it opens the Usage dialog (`requirements/ui#usage`) over the view shown, is never the selected entry and never carries `aria-current`.
- Below a window width of 1024 px the sidebar shows icons only, 56 px wide, and then each entry, Usage and Settings included, gets its name as tooltip.
- The Board entry shows the number of real cards in Backlog; the Jobs entry shows the number of jobs waiting for the developer, in Peach; the Chat entry shows `1` in the same colour while Chat waits for the answers of its round. No number is shown when it is 0.
- The minimum size of a project window is the one in `requirements/tech#window` (960 × 600 px); its size and position are remembered per project.

### Logo
- Weaver's logo is a woven mark: a cream (`#FBF6EE`) rounded square with a Line ring, crossed by a horizontal Sky thread and by four diagonal threads, Peach, Lavender, Rose and Sage, that weave a "W". It is one vector drawing, the same as the master of the app icon (`requirements/release#General`), decorative and hidden from assistive technologies next to the name it accompanies.
- The interface shows it wherever it shows Weaver itself, never a lucide mark in its place: the welcome window shows it above the app name.

### Welcome window and File menu
- With no project window open, Weaver shows the welcome window, 640 × 480 px, not resizable: the app name and version, the recent projects (up to 10, most recent first; folder name, full path muted; a folder that no longer exists is marked `Not found` and can be removed from the list), `Open Folder` (system folder picker), and the omp status line linking to the omp setup (see `omp-setup`).
- Clicking a recent project or choosing a folder opens it as described by `developer.OPEN_PROJECT`; the welcome window closes once a project window is open.
- The welcome window's list follows every change of the recent projects at once, also those made from the File menu (`Clear Recent`, an opening from `Open Recent`), not only when the window gets the focus.
- **File menu,** the same in every window, project and welcome, on macOS, Windows and Linux; at its top, before the items it already has: `Open Folder…`, `Open Recent`, then a separator, then the existing items (`Close Window`, and `Quit` on Windows and Linux).
  - `Open Folder…`, shortcut `Cmd+O` on macOS and `Ctrl+O` on Windows and Linux, works whatever has the focus, also the File view's editor, the Notes or a text field. It opens the system folder picker, as the welcome window's `Open Folder`, modal to the window it was chosen from (a sheet on macOS); with no window focused the picker stands alone. Cancelling it does nothing. Choosing `Open Folder…` again while a picker is open does nothing.
  - `Open Recent` is a submenu with the same projects as the welcome window, in the same order, also those already open; each item shows the folder name, and clicking it opens that project as `developer.OPEN_PROJECT`. A project whose folder no longer exists is a disabled item `<name> — Not found`. After the projects, a separator and `Clear Recent`, which empties the list without confirmation. With an empty list the submenu holds only the disabled item `No Recent Projects`, with no separator and no `Clear Recent`.
  - An opening from the File menu that fails shows its reason in the window it was chosen from: in a project window an error toast `Could not open <name>.` (`Could not open the folder.` when the picker itself fails) with the reason as its description; in the welcome window its red `Alert` with the texts it already uses (`<name> could not open: <reason>`, `The folder could not be opened: <reason>`). When no window had the focus, the welcome window opens showing that `Alert`.
- A project window's title is the project folder's name.
- The sidebar has no entry that sends the window to the background: only `Keep working in background` of the close confirmation does (`requirements/tech#window`). When the window comes back it shows the same view, with every view's state as it was.
- When the window cannot leave the screen after `Keep working in background`, it stays as it was and shows an error toast `Could not hide the window.` with the reason as its description.
