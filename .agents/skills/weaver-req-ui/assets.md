# Assets

## assets

### Tags in message boxes
- Four boxes recognise tags: the Chat message box, the reply box of a job's conversation in the Jobs view, the message box of an Ask more conversation, and the text of the request editor (not its `Preview`). A tag is recognised as `requirements/rules#assets` says, against the assets of the project at that moment.
- **Suggestions:** typing `@` at the start of the text or after whitespace opens a list under the caret, a shadcn `Popover` anchored to the caret that does not take the focus from the box. It lists the assets whose name contains the text typed after `@`, case-insensitively: names that start with it first, then the others, each group in name order; at most 8 rows show, the rest scroll. Each row shows the asset's kind icon (`requirements/ui#asset-view`) and its name.
  - Up and Down move the active row; Enter or Tab inserts `@<name>` followed by a space in place of what was typed after `@`, and closes the list; a click on a row does the same. While the list is open, Enter and Tab never send the message nor move the focus.
  - Esc closes the list and leaves the text as it is. The list closes by itself when no asset matches, when the caret leaves the tag, or when the box loses the focus. With no asset in the project it never opens.
  - The box announces the list to assistive technologies as a combobox: `aria-expanded`, `aria-controls` and `aria-activedescendant` on the box, `listbox` and `option` roles on the list.
- **Highlight:** while the developer writes, every recognised tag, `@` included, is drawn in the primary blue (`blue-600` light, `blue-500` dark) in the box; the rest of the text, the caret, the selection, wrapping and scrolling stay exactly as in a plain `Textarea`. A tag that names no asset stays in the normal text colour. The highlight follows every change of the text and of the assets.
- **Caret on the drawn text:** in every box, the caret and the selection sit exactly on the characters drawn, on every line, also after a soft wrap and while the suggestion list is open; a click places the caret on the character under the pointer, and each typed character appears at the caret.
- **Input methods:** in every box, Enter or Tab pressed to confirm a composition (accents, dead keys, an IME) only confirms it: it never sends the message, never picks a suggestion and never moves the focus.
- What the conversation shows of a sent message is only the text the developer wrote, tags included as he typed them, with no highlight and nothing of what Weaver added for the agent.

## asset-view

- **Entry:** `Assets` in the sidebar, after Files, icon `Paperclip`. Like the other views but Settings, it shows Get Started until the project has `.weaver/`.
- **Header:** the view header of `requirements/ui#General`, title `Assets`, no controls.
- **Grid:** the assets in name order, case-insensitively, as tiles in rows that wrap with the view's width: each tile 104 px wide, the kind icon at 40 px above the name, centred; the name on at most 2 lines, cut with an ellipsis. The tile's tooltip gives the full name and the size (`logo.png — 2.4 MB`; sizes in B, KB, MB or GB with one decimal above 1 KB, 1 KB = 1024 B).
- **Kind icons** (lucide-react, by the extension, case-insensitively; the names of the installed lucide version):

  | Kind | Extensions | Icon |
  |---|---|---|
  | Image | png, jpg, jpeg, gif, webp, svg, bmp, ico, tif, tiff, heic, avif | `FileImage` |
  | Video | mp4, mov, webm, mkv, avi, m4v | `FileVideo` |
  | Audio | mp3, wav, ogg, flac, m4a, aac | `FileAudio` |
  | Archive | zip, tar, gz, tgz, bz2, xz, 7z, rar | `FileArchive` |
  | Spreadsheet | csv, tsv, xls, xlsx, ods, numbers | `FileSpreadsheet` |
  | Document | txt, md, pdf, doc, docx, rtf, odt, pages, ppt, pptx, key | `FileText` |
  | Code and data | json, yaml, yml, toml, xml, html, css, js, ts, tsx, jsx, py, rs, go, java, c, h, cpp, sh, sql | `FileCode` |
  | Font | ttf, otf, woff, woff2 | `FileType` |
  | Anything else | | `File` |

- **Size mark:** a tile whose asset is larger than the size limit (`requirements/rules#asset-view`) carries an amber `TriangleAlert` at the icon's top right, with the accessible name and tooltip `Larger than 10 MB: it goes into the repository.`
- **Empty:** with no asset, the view says in muted text, centred: `No assets yet. Drag files here from the Finder or Explorer.` (`File Explorer` on Windows, `your file manager` on Linux).
- **Selection:** the grid is an ARIA `grid` of `gridcell`s with `aria-multiselectable`. A click selects one tile; `Cmd`+click (`Ctrl`+click elsewhere) adds or removes one; `Shift`+click selects the range from the last clicked tile in grid order; a click on the empty space clears the selection; the arrow keys move the active tile, with `Shift` extending the selection; `Cmd+A` (`Ctrl+A`) selects every asset. A double click, Enter or Space on a tile only selects it. Selected tiles use the `accent` background and a `ring` outline.
- **Actions:**
  - A shadcn `ContextMenu` on a tile, opened by a right click, the Menu key or `Shift+F10`, acting on the selection (a right click on a tile outside the selection selects that tile alone first): `Rename` (`Pencil`; only with one asset selected, otherwise shown disabled with the reason `Rename one asset at a time.`) and `Delete` (`Trash2`). No other entry.
  - `F2` renames the selected asset; `Delete` (`Backspace` with `Cmd` on macOS) deletes the selection.
  - `Rename` replaces the tile's name with a text field holding the name, selected without its extension; Enter confirms, Esc or leaving the field cancels.
  - `Delete` asks in an `AlertDialog`: `Move "<name>" to the trash?`, or `Move <n> assets to the trash?` with the names listed; buttons `Move to Trash` (destructive) and `Cancel`.
  - A refused action shows an error toast with the reason, for example `An asset named "<name>" already exists.`; nothing changes. When some assets could not go to the trash: the error toast `Could not move <n> assets to the trash: <reason>` (`Could not move "<name>" to the trash: <reason>` for one).
- **Drop from outside:** files dragged from the Finder or Explorer over the view highlight the whole grid area with a dashed `ring` border; a drop anywhere on it adds them. During the copy a loading toast says `Copying <n> files…` (`Copying 1 file…`) until it ends; the rest of the view stays usable. When it succeeds the toast closes and the copied assets are selected and scrolled into view; when it fails the toast becomes the error toast `Could not copy "<name>": <reason>. Nothing was copied.`
  - Refusals use the error toast of refused actions, for example `"<name>" is a folder: only files can be assets.`, `An asset named "<name>" already exists.`, `Two dropped files are both named "<name>".`, `Names starting with "." are not allowed.` or `Another copy is still running in this window.`
  - When copied files are larger than the size limit, a warning toast `"<name>" is larger than 10 MB: it goes into the repository.` (`<n> copied files are larger than 10 MB: they go into the repository.` for several) follows the end of the copy.
  - Dropping assets back onto their own view does nothing and shows nothing.
- **Drag out:** pressing the primary button on a tile and moving 4 px starts a drag out of Weaver, carrying the selection when the tile is selected, otherwise that tile alone; the system shows its own drag image. A press that moves less stays a click. Other apps receive copies; the grid does not change.
- **Leaving:** choosing another sidebar entry clears the selection; the grid's scroll position is kept.
