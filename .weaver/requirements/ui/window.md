---
description: "Use when building the project window's frame, sidebar, project switcher, palette or user row, the welcome window, the File menu or Weaver's logo."
---
# Window

## window

### Frame
- The window's ground is Paper (`requirements/ui#General`). On the left the sidebar stands on Paper; on the right the work surface, a Surface panel inset 8 px from the window's edges and from the sidebar, radius 14 px, with a 1 px Line ring, holds the selected view.
- The banners of the window (startup, update, System Certificates, agent files) sit at the top of the work surface, full width of it, above the view's header; the view's own banners and notices stay inside the view.
- The minimized bar of a question round (`requirements/ui#question-round`) sits at the bottom of the work surface, full width of it, below the selected view, which gets shorter to make room for it.
- The selected entry is the view shown on the right; switching entries keeps the state of every view (focus, overlay, scroll), except the File view, which closes when another entry is chosen (`requirements/ui#file-view`), and the Assets view, which forgets its selection (`requirements/ui#asset-view`).
- The minimum size of a project window is the one in `requirements/tech#window` (960 × 600 px); its size and position are remembered per project.

### Sidebar
- 248 px wide, the full height of the window, padding 14 px 12 px 12 px, from top to bottom:
  1. the project switcher, then the `Search or run…` button;
  2. `Board`, `Chat`, `Jobs`;
  3. a group headed `PROJECT` (11 px, medium, uppercase, Muted): `Requirements`, `Specs`, `Files`, `Assets`, `Notes`, `History`;
  4. the free space;
  5. the summary of Usage (`requirements/ui#usage`);
  6. the user row, ending with the `Settings` button.
- Entries are 34 px high, radius 9 px, icon and name in Ink 2; the selected entry stands on Surface with a 1 px Line ring, its name in Ink and medium weight, and carries `aria-current`. `Settings` is a view too: while it is shown, the gear button of the user row carries `aria-current` and the Surface look.
- `Search or run…` is a 34 px Surface button with a Line border, `Search` icon, the text in Muted and, at its right end, the key hint `⌘K` on macOS, `Ctrl K` elsewhere, in Geist Mono on Paper; it opens the palette.
- Numbers on entries, right-aligned, none when 0: `Board` shows the number of real cards in Backlog, drafts of requests included, in Muted; `Chat` shows `1` in Peach while Chat waits for the answers of its round; `Jobs` shows the number of jobs waiting for the developer in Peach, or, when none waits and some are Running, a Sky pill with a running dot and `<n> running`.
- Below a window width of 1024 px the sidebar shows icons only, 56 px wide: the switcher shows only the logo and opens the same menu; `Search or run…` becomes an icon button with the tooltip `Search or run… (⌘K)` (`Ctrl+K` elsewhere); entries show their icon with their name as tooltip and their number as a small badge on the icon; the `PROJECT` heading becomes a separator; the summary of Usage becomes a `Gauge` button whose tooltip and accessible name give the summary; the user row hides its avatar and name and keeps the gear.

### Project switcher
- A button at the top of the sidebar: the logo at 30 px; the project folder's name, 14 px semibold, cut with `…`; under it the branch in 12 px Muted; then `ChevronsUpDown`. Accessible name `Project <name>, branch <branch>`.
- The branch line shows the branch git is on in the project folder, the short hash of the commit when git is on no branch, and nothing when the folder is not in a git repository or git cannot be run. It changes as soon as the backend reads a new value (`requirements/tech#window`).
- It opens a Popover under it, 320 px wide, with, in order:
  - a `Search projects` field, focused, filtering both lists below by folder name, case-insensitively, as typed;
  - `Open`: every project with an open window, the background ones included, current first with `Check`, then by most recent use; each row has the folder name, the path in Muted cut in the middle, and, when it has working jobs, a Sky dot with `<n> working`;
  - `Recent`: the recent projects without an open window, most recent first; a folder that no longer exists is a disabled row `<name> — Not found` with an icon-only `X` (`Remove from recent projects`) that removes it from the list;
  - a separator and `Open Folder…` (`FolderOpen`), which opens the system folder picker modal to this window.
- Choosing the current project closes the menu; choosing another one is `developer.OPEN_PROJECT`: its window opens or comes to the front and this window stays as it was. A failed opening shows the error toast of the File menu (`Could not open <name>.` with the reason). Empty lists are hidden; with no match, `No projects match.` in Muted.
- Arrows move through the rows and the field, Enter chooses, Esc closes and gives the focus back to the switcher. No keyboard shortcut opens it.

### Palette
- `Cmd+K` on macOS and `Ctrl+K` on Windows and Linux open it from anywhere in a project window, whatever has the focus, the File view's editor, the Notes and text fields included; so does `Search or run…`. Pressed again while it is open, it closes it.
- A Dialog at the top third of the window, 560 px wide, without title bar: a search field with `Search` and the placeholder `Search or run…`, then the results as a `Command` list, the same component as the Merge dialog's list. Arrows move, Enter chooses, Esc closes and gives the focus back to where it was.
- Results come in groups, in this order, each under its heading: `Views` (every sidebar entry and `Settings`), `Actions` (`Scan System`, `Run all`, `New Request`, `Open Folder…`, `Usage`), `Projects` (as the switcher's lists), `Cards` (the Proposals, Tasks and Drifts of the Board: short number and title; the drafts of requests: `Request` in place of the short number, and the title), `Requirements` (sections: title, with the Area and fragment in Muted), `History` (entries: title and date). With the field empty only `Views`, `Actions` and `Projects` show.
- Matching is case-insensitive on names, titles and short numbers; names that start with the text come first; at most 8 rows per group.
- Choosing a view shows it; an action runs as its own control does, with the same confirmation, and a disabled action shows its reasons in Muted and cannot be chosen; a project opens as from the switcher; a card shows the Board and opens its details dialog, a draft of a request shows the Board and opens it in the request editor (`requirements/ui#design-request`); a section shows the Requirements scrolled to it; a History entry shows the History, on `All` when the period hides it, and opens the entry. The palette closes on every choice.
- It never searches the project's files, assets or notes.

### User row
- At the bottom of the sidebar: a 28 px round avatar on Paper with the initials of the name in Ink 2, 11 px semibold; the name, 13 px Ink 2, cut with `…`; at the right end the icon-only `Settings` button (`Settings` icon, accessible name and tooltip `Settings`).
- The name is git's user name for the project (`requirements/tech#window`). Without one, the row shows only the `Settings` button, at its right end.

### Logo
- Weaver's logo is a woven mark: a cream (`#FBF6EE`) rounded square with a Line ring, crossed by a horizontal Sky thread and by four diagonal threads, Peach, Lavender, Rose and Sage, that weave a "W". It is one vector drawing, the same as the master of the app icon (`requirements/release#General`), decorative and hidden from assistive technologies next to the name it accompanies.
- The interface shows it wherever it shows Weaver itself, never a lucide mark in its place: the project switcher and the welcome window.

### Welcome window and File menu
- With no project window open, Weaver shows the welcome window, 640 × 480 px, not resizable, on Paper with a Surface panel as the project window: the logo and the app name and version, the recent projects (up to 10, most recent first; folder name, full path muted; a folder that no longer exists is marked `Not found` and can be removed from the list), `Open Folder` (system folder picker), and the omp status line linking to the omp setup (see `omp-setup`).
- Clicking a recent project or choosing a folder opens it as described by `developer.OPEN_PROJECT`; the welcome window closes once a project window is open.
- The welcome window's list follows every change of the recent projects at once, also those made from the File menu (`Clear Recent`, an opening from `Open Recent`), not only when the window gets the focus.
- **File menu,** the same in every window, project and welcome, on macOS, Windows and Linux; at its top, before the items it already has: `Open Folder…`, `Open Recent`, then a separator, then the existing items (`Close Window`, and `Quit` on Windows and Linux).
  - `Open Folder…`, shortcut `Cmd+O` on macOS and `Ctrl+O` on Windows and Linux, works whatever has the focus, also the File view's editor, the Notes or a text field. It opens the system folder picker, as the welcome window's `Open Folder`, modal to the window it was chosen from (a sheet on macOS); with no window focused the picker stands alone. Cancelling it does nothing. Choosing `Open Folder…` again while a picker is open does nothing.
  - `Open Recent` is a submenu with the same projects as the welcome window, in the same order, also those already open; each item shows the folder name, and clicking it opens that project as `developer.OPEN_PROJECT`. A project whose folder no longer exists is a disabled item `<name> — Not found`. After the projects, a separator and `Clear Recent`, which empties the list without confirmation. With an empty list the submenu holds only the disabled item `No Recent Projects`, with no separator and no `Clear Recent`.
  - An opening from the File menu that fails shows its reason in the window it was chosen from: in a project window an error toast `Could not open <name>.` (`Could not open the folder.` when the picker itself fails) with the reason as its description; in the welcome window its red `Alert` with the texts it already uses (`<name> could not open: <reason>`, `The folder could not be opened: <reason>`). When no window had the focus, the welcome window opens showing that `Alert`.
- A project window's title is the project folder's name.
- The sidebar has no entry that sends the window to the background: only `Keep working in background` of the close confirmation does (`requirements/tech#window`). When the window comes back it shows the same view, with every view's state as it was.
- When the window cannot leave the screen after `Keep working in background`, it stays as it was and shows an error toast `Could not hide the window.` with the reason as its description.
