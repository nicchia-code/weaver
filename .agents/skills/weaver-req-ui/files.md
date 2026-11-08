# Files

## file-view

- **Entry:** `Files` in the sidebar's `PROJECT` group, after Specs, icon `FolderTree`. Like the other views but Settings, it shows Get Started until the project has `.weaver/`.
- **Layout:** under the view header (`requirements/ui#General`, title `Files`, no controls), which spans both sides, the tree on the left, the tabs and the editor on the right; the tab bar stays below the view header. With no tab open the right side says `Open a file from the tree.` in muted text.
- **Tree:**
  - Rows show the icon (`Folder`, `FolderOpen` or `File`) and the name, about 16 px of indent per level, as the Import from Code tree; folders first, then files, each in name order. The root's entries show at once; every folder starts collapsed.
  - It is an ARIA `tree`: Up and Down move between rows, Right expands a folder or goes to its first child, Left collapses it or goes to its parent, Enter opens a file in a tab or toggles a folder; a click does the same.
  - A file open in a tab whose agent lock is held carries `Lock` with the accessible name and tooltip `The agent is editing this file`.
  - Entries of `.weaver/` look like the others; the actions that would change them are shown disabled, with the reason `.weaver is written by Weaver: read-only here.`
- **Actions:**
  - A shadcn `ContextMenu` on each row, and on the empty space of the tree for the root, opened by a right click, the Menu key or `Shift+F10`: `New file`, `New folder` (folders and the root only), `Rename`, `Move to…`, `Delete`.
  - `F2` renames the selected row and `Delete` (`Backspace` with `Cmd` on macOS) deletes it.
  - `New file`, `New folder` and `Rename` show a text field in the tree row, with the name selected (without its extension when renaming a file); Enter confirms, Esc or leaving the field cancels.
  - Dragging a row onto a folder, or onto the root's empty space, moves it there. The drag starts once the pointer has moved 4 px with the primary button held on a row; a press that moves less stays a click. While dragging, a small label with the row's icon and name follows the pointer, the folder under the pointer is highlighted (the tree itself for the root's empty space), and nothing is highlighted where the move would be refused. Releasing over a highlighted target moves the entry there; releasing anywhere else, `Esc` or a cancelled pointer ends the drag and nothing changes.
  - **Drag aids,** for rows and for drops from outside alike: a collapsed folder under the pointer for 1 s expands; with the pointer within 24 px of the tree's top or bottom edge the tree scrolls that way, faster nearer the edge.
  - `Move to…` opens a `Dialog` with the project's folders as a tree, read one folder at a time as it expands, and `Move` / `Cancel`.
  - **Drop from outside:** files and folders dragged from the Finder or Explorer over the tree highlight the folder under the pointer, or the tree for the root's empty space, as a row drag does. A drop outside the tree does nothing in the File view. During the copy a loading toast says `Copying <n> items…` (`Copying 1 item…`) until it ends; the rest of the view stays usable. When the copy succeeds the toast closes, the target folder expands if it was collapsed and the first copied entry, in the tree's order, is selected and scrolled into view. When the copy fails the toast becomes the error toast `Could not copy "<name>": <reason>. Nothing was copied.`
  - Refusals of a drop from outside use the error toast of refused actions, for example `"<name>" is already in the project: drag it inside the tree to move it.`, `Two dropped items are both named "<name>".` or `Another copy is still running in this window.`
  - `Delete` asks in an `AlertDialog`: `Move "<name>" to the trash?`, plus `<n> open files have unsaved changes that will be lost.` when tabs on it have unsaved changes; buttons `Move to Trash` (destructive) and `Cancel`.
  - A refused action shows an error toast with the reason, for example `A file or folder named "<name>" already exists here.`, `The agent is editing this file.` or `Could not move "<name>" to the trash: <reason>`; nothing changes.
- **Leaving:** choosing another sidebar entry, or a link or notification that shows another view, closes the File view. With unsaved changes an `AlertDialog` first asks `Save changes to <n> files?`, listing the file paths, with `Save all`, `Discard all` (destructive) and `Cancel`; Cancel keeps the File view. When a file cannot be saved, the File view stays on that file's tab.

## file-tab

- **Tabs:** a tab bar above the editor, one shadcn-styled ARIA `tab` per file in opening order: the file name (path in the tooltip, monospace), a dot with the accessible name `Unsaved changes` while it has unsaved changes, `Lock` while its lock is held, `TriangleAlert` while one of its bars waits, and an icon-only `X` (`Close`). Left and Right move between tabs.
- **Shortcuts,** only while the File view is shown: `Cmd+S` / `Ctrl+S` saves the active tab; `Cmd+W` / `Ctrl+W` closes the active tab instead of the window; `Ctrl+Tab` and `Ctrl+Shift+Tab` show the next and previous tab. Saving a tab that cannot be changed does nothing.
- **Tab limit:** when every tab has unsaved changes, opening another file shows the warning toast `Every tab has unsaved changes: save or close one to open another file.`
- **Toolbar** above the editor: the file's path relative to the project (monospace) and `Open in editor`, which opens the file at the cursor's line in the developer's editor (`requirements/ui#app`).
  - On a tab showing an image, or an SVG tab showing its drawing, the path is followed in muted text by the size `<width> × <height> px` and the weight, in `B`, `KB` or `MB` with 1 KB = 1,024 bytes and one decimal from 1 KB on (`12.4 KB`); the size appears once the image is drawn. `Open in editor` on an image opens the file without a line.
  - On an SVG tab, a shadcn `ToggleGroup` with one choice, `Code` and `Preview`, sits next to `Open in editor`; Left and Right move between its two items. The tab opens on `Preview`, and the choice holds for that tab until it closes.
- **Image:** the image is centred in the panel, scaled down to fit and never enlarged, with alt text the file name. Behind it, and only under it, a checkerboard of light and muted squares from the theme's tokens shows transparent areas in both themes. A click on the image, or `Enter` or `Space` while it has focus, switches between fitted and actual size (one image pixel per CSS pixel), where the panel scrolls both ways; the cursor is `zoom-in` when fitted and `zoom-out` at actual size, and the accessible name is `Show actual size` or `Fit to panel`. A tab starts fitted and keeps its choice when the image is read again.
- **SVG preview:** the drawing uses the same image view, from the tab's text. The read-only lines, the lock line and the bars stay above it as above the text, and `Cmd+S` / `Ctrl+S` still saves; search and replace work only on `Code`. Switching never loses the cursor, the selection or the undo history. When the text cannot be drawn, the panel says `This SVG cannot be drawn.` in muted text in place of the drawing.
- **Editor:** CodeMirror 6, in the monospace font stack, with Weaver's resolved light or dark theme, switching at once with it; syntax colours meet the contrast rules of `requirements/quality#General` in both themes. Line numbers, undo and redo, search and replace (`Cmd+F` / `Ctrl+F`), matching brackets, code folding and multiple cursors; no completion, no diagnostics, no language server.
- **Read-only and binary:**
  - A file over the size limit: the line `This file is larger than 2 MB: read-only, without colours.` above the text.
  - A file in `.weaver/`: the line `Files in .weaver are written by Weaver: read-only here.`
  - A binary file: the panel `Binary file` with `Open in editor`, and no text.
  - An image over 10 MB: the same panel, with the line `This image is larger than 10 MB: no preview.`
  - An image that cannot be drawn: the same panel, with the line `This image cannot be shown.`
- **Bars** sit at the top of the editor, as an `Alert`, and stay until resolved:
  - changed on disk with unsaved changes (warning): `This file changed on disk. Saving will ask what to keep.`;
  - lock released with unsaved changes (warning): `The agent changed this file while it had unsaved changes.` with `Take the agent's version` (destructive), `Keep mine` and `Cancel`; `Cancel` removes the bar and keeps the unsaved changes;
  - deleted (warning): `The agent deleted this file.` after a lock, `This file was deleted on disk.` otherwise, with `Save again` on a tab that can be changed.
- **Dialogs:**
  - save that finds the file changed on disk: `AlertDialog` `This file changed on disk since it was opened.` with `Reload from disk` (destructive), `Overwrite` (destructive) and `Cancel`;
  - closing a tab with unsaved changes: `Save changes to <name>?` with `Save`, `Discard` (destructive) and `Cancel`.
- **Errors:** a failed write shows the error toast `Could not save <name>.` with the reason as its description; the tab keeps its text.

## file-lock

- While the lock of a tab's file is held, its editor is read-only and a line above the text, with `Lock`, says `The agent is editing this file.`; the tab and the tree row carry `Lock`. The line is announced to assistive technologies when it appears and when it goes.
- The lock state comes from the backend; the interface never derives it from the conversations.
