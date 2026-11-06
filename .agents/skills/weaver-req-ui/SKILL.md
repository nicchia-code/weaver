---
name: weaver-req-ui
description: "Use when building or changing screens, layout, styling, components, navigation look or texts shown to users."
---

Every text of the interface is in English, whatever the language setting says; that setting only changes the language of the agent's texts. The interface only displays what the Rust backend holds and sends the developer's requests to it: it keeps no state of the project, the cards or the jobs of its own, and never reads or writes project files itself.

## General

### Theme and components
- shadcn/ui with the `zinc` base colour and a blue primary (Tailwind `blue-600` in light, `blue-500` in dark), in light and dark. The Theme setting is `auto` (follows the OS), `light` or `dark`; a change, of the setting or of the OS in `auto`, applies at once to every open window without a reload.
- Every control is a shadcn/ui component (Button, DropdownMenu, ContextMenu, Popover, Tooltip, Dialog, AlertDialog, Select, RadioGroup, Textarea, Badge, Alert, ScrollArea…); no hand-made widget where shadcn has one. The File view's editor is CodeMirror (`requirements/ui#file-tab`); the Notes view's editor is TipTap (`requirements/ui#notes-view`).
- Colours that carry a meaning (states, change kinds, label colours, the Drift and resolution marks) come from the Tailwind palette, shade 600 in light and 400 in dark. Everything else uses the theme tokens (`foreground`, `muted-foreground`, `border`, `ring`…).
- Cards use `shadow-sm`; dialogs, popovers and menus `shadow-lg`. The border of an unchecked checkbox or radio has a contrast of at least 3:1.
- Fonts: the system UI font stack at 14 px base; ids, paths and code use `ui-monospace, SFMono-Regular, Menlo, Consolas, monospace`.
- Transitions last at most 200 ms. When the OS asks for reduced motion there are no transitions, no shimmer and no rotation; scrolls jump.
- Status is never told by colour alone: every coloured dot, border or tint comes with a text, a badge or an icon, and with an accessible name.

### Window layout
- A project window has a left sidebar, 208 px wide, the full height of the window, with two groups of entries:
  - at the top, in this order: Board, Chat, Jobs, Files, Assets, Notes, Requirements, Specs, History;
  - at the bottom-left, kept apart from the top group by the free space between them: Usage, then Settings, the last entry.
- The selected entry is the view shown on the right; switching entries keeps the state of every view (focus, overlay, scroll), except the File view, which closes when another entry is chosen (`requirements/ui#file-view`), and the Assets view, which forgets its selection (`requirements/ui#asset-view`). Usage is not a view: it opens the Usage dialog (`requirements/ui#usage`) over the view shown, is never the selected entry and never carries `aria-current`.
- Below a window width of 1024 px the sidebar shows icons only, 56 px wide, and then each entry, Usage and Settings included, gets its name as tooltip.
- The Board entry shows the number of real cards in Backlog; the Jobs entry shows the number of jobs waiting for the developer, in the warning colour (amber); the Chat entry shows `1` in the same colour while Chat waits for the answers of its round. No number is shown when it is 0.
- The minimum size of a project window is the one in `requirements/tech#window` (960 × 600 px); its size and position are remembered per project.

### View header
- Every view (Board, Chat, Jobs, Files, Assets, Notes, Requirements, Specs, History, Settings) starts with one header row, full width of the view: the view's name as title on the left, the view's own controls, when it has any, on the right; the free space sits between them. One bottom border separates it from the content.
- Every header has the same fixed height, 56 px plus its 1 px bottom border, whether or not it holds controls, so the border does not move when switching views. Side padding 16 px; controls are `size="sm"` Buttons and Selects, vertically centred.
- The title is the sidebar entry's name, text only (no icon), 14 px semibold. It is the view's main heading for assistive technologies (the heading the view's region is labelled by); no other hidden heading repeats the view's name.
- The header stays at the top while the content below it scrolls.
- It shows in every state of the view (loading, load error, empty), but not in place of Get Started: where a view shows Get Started (`requirements/ui#project`), Get Started takes the whole view, header included.

### Icons
lucide-react, 16 px, in the colour of the text, decorative (hidden from assistive technologies), always to the left of the text. Icon-only buttons have an accessible name and a tooltip.

| Use | Icon |
|---|---|
| Board / Chat / Jobs / Files / Assets / Notes | `SquareKanban` / `MessageSquare` / `Bot` / `FolderTree` / `Paperclip` / `NotebookPen` |
| Requirements / Specs / History | `ListChecks` / `Package` / `History` |
| Usage / Settings | `Gauge` / `Settings` |
| Run, Run all | `Play` |
| + Work Item | `Plus` |
| Scan System | `ScanSearch` |
| Resolve | `WandSparkles` |
| View requirements | `ListChecks` |
| Approve | `Check` |
| Refine | `Sparkles` |
| Reject, Close, Clear | `X` |
| Dismiss Drift | `Ban` |
| Fix the code / Accept Proposal | `Wrench` / `CheckCheck` |
| Delete | `Trash2` |
| Model badge / model control of a Proposal | `Cpu` / `Settings` |
| Open in editor | `SquareArrowOutUpRight` |
| Open Folder | `FolderOpen` |
| Folder / open folder / file (trees) | `Folder` / `FolderOpen` / `File` |
| Asset kinds (Assets view) | as listed in `requirements/ui#asset-view` |
| Asset larger than the size limit | `TriangleAlert` |
| Image that cannot be shown (Notes) | `ImageOff` |
| New file / New folder | `FilePlus` / `FolderPlus` |
| Rename / Move to… | `Pencil` / `FolderInput` |
| Agent editing a file | `Lock` |
| Agent's choice (Specs) | `Sparkles` |
| Split-button arrow | `ChevronDown` |
| Warning toast / Error toast | `TriangleAlert` / `CircleX` |
| Previous / Next | `ChevronLeft` / `ChevronRight` |

### Tooltips
- At most two short sentences, about 160 characters. They open on hover after 400 ms and at once on keyboard focus, and close on Esc, blur or pointer leave.
- Disabled controls still show theirs: they use `aria-disabled` instead of `disabled`, stay focusable, ignore activation, and carry their reasons in the tooltip and in the accessible description.
- Sidebar entries (when their names are visible) and items of menus have no tooltip.

### Disabled controls
- A disabled control lists every reason that applies, one per line, never only the first, in the fixed order given for that control in this Area. `The agent is still finishing` (the job that handed the card over is still working) always comes first on a card's controls.
- A control re-enables itself as soon as the backend's state allows it, without moving the focus.
- A decision is checked again by the backend when applied; when it is refused there, a notice at the top of the view says what failed and why, and nothing changes.

### Ids
- A card is shown by its short number (`P3`, `T3`, `D3`) when it has one, otherwise by its 8-character permanent id; never both. Monospace.
- The accessible name and tooltip say `Short number (temporary) P3` or `Permanent ID qmqh1o3p`.

### Dialogs and confirmations
- Details and forms open in a shadcn `Dialog` (max width 720 px, the body scrolls inside); confirmations in an `AlertDialog` whose destructive button uses the `destructive` variant and whose Cancel has the initial focus.
- A confirmation with three choices (`Save` / `Discard` / `Cancel` and the like) is an `AlertDialog` too: the choices that lose text use the `destructive` variant, the one that keeps it the default variant, and Cancel, the last button, has the initial focus.
- Esc, the `X` button or a click outside close a Dialog; an AlertDialog closes only with its buttons or Esc. Closing puts the focus back on the control that opened it.
- Every Dialog and AlertDialog, the question round included, is centred in the window by layout (for example a full-window flex or grid container), never by shifting it by a percentage of its own size; its top-left corner always sits on whole CSS pixels, whatever its content height and the window size (odd sizes included), so its text is never drawn between pixels.
- They open with a 150 ms fade only, without zoom, scale or any other transform or filter; under reduced motion they appear at once. Popovers, menus, selects and tooltips keep their own opening animation.
- A cancelled confirmation changes nothing.

### Toasts
- Transient messages are toasts, from the shadcn `Sonner` toaster mounted once per window, at the bottom right, at most 3 visible at once.
- Three kinds: info, warning and error. Info toasts keep the neutral popover look with no icon; warning toasts use `amber` with `TriangleAlert`, error toasts `red` with `CircleX` (shade 600 in light, 400 in dark, as the other meaning colours), in the border, the icon and the title.
- Every toast, of any kind, disappears by itself 4 s after it appears.
- Every toast, of any kind, has an icon-only close button `X` (accessible name and tooltip `Close`) that removes it at once. The button is reachable from the keyboard: `Alt+T` moves the focus to the toasts, then `Tab` reaches each close button. Esc never closes a toast; swiping a toast away still closes it.

### Markdown
- Rendered with `react-markdown` + `remark-gfm` into React elements; raw HTML is never interpreted and shows as text; nothing uses `dangerouslySetInnerHTML`.
- Supported: GFM tables, strikethrough, task lists (read-only checkboxes), autolinks, fenced code.
- These rules are for rendered Markdown. The Notes view's editor is not rendered Markdown: `requirements/ui#notes-view` gives its own, its headings and clickable checkboxes included.
- Links open only when their scheme is `http` or `https`, in the system browser, never inside Weaver; other links render as plain text. When the system browser cannot be opened, an error toast `Could not open the link.`
- Images with a path inside the project are shown; remote images are never fetched and show as a link.
- A heading inside rendered text is never larger than the heading of the element that holds the text.
- Nothing ever scrolls sideways: long lines wrap; tables and code blocks scroll horizontally inside their own box.
- `mermaid` code blocks: the mermaid library is loaded on first use, with `securityLevel: 'strict'` and the current light or dark theme; when a diagram fails to render it is shown as a plain code block with the error under it in muted text.

### Times and durations
- Times are local, `HH:MM` (24 h); dates are written out in English (`Monday, 2 November 2026`).
- Durations: `less than a minute`, `<n> min`, `<h> h <m> min`.

## Ask more

Use when building or changing Ask more: its buttons, its conversations in the Jobs view and their texts.

Sections: `ask-conversation`

File: `ask-more.md`

## Assets

Use when building the Assets view or `@` asset tags.

Sections: `assets`, `asset-view`

File: `assets.md`

## Board

Use when building or changing the Board: columns, footers, ghosts, notices, Task cards and their details dialogs.

Sections: `work`, `task`

File: `board.md`

## Board decision cards

Use when building or changing the Proposal and Drift cards on the Board: badges, Approve, Refine, Reject, decision controls and their details dialogs.

Sections: `proposal`, `drift`

File: `board-cards.md`

## Jobs and conversations

Use when building or changing the Jobs list, a job's or a sub-agent's conversation, or General.

Sections: `job`, `general-chat`

File: `conversations.md`

## Files

Use when building or changing the File view: its tree, tabs, editor, image and SVG previews, bars and dialogs.

Sections: `file-view`, `file-tab`, `file-lock`

File: `files.md`

## Look and feel

Use when building any screen or control: theme, colours, icons, tooltips, ids, Markdown, motion, sidebar.

Sections: `General`

File: `look.md`

## Notes

Use when building or changing the Notes view: the sheet's editor, its Markdown shortcuts, checklists, images and save status.

Sections: `notes-view`

File: `notes.md`

## Question rounds and notifications

Use when building or changing question rounds or system notifications.

Sections: `question-round`, `notification`

File: `rounds.md`

## Setup and settings

Use when building or changing Get Started, import exclusions, omp status and login, the request editor or Settings.

Sections: `project`, `omp-setup`, `design-request`, `developer.CHANGE_SETTINGS`

File: `setup.md`

## Solo mode and Atomic commits

Use when building the Solo mode control in Settings and Get Started, its notes, warnings and the list of tracked files; Merge on the Board with its button, Dialog, merge banner and notices; or the Atomic commits switch in Settings and its notices.

Sections: `solo-mode`, `solo-merge`, `atomic-commits`

File: `solo-mode.md`

## System Certificates

Use when building the System Certificates switch, its status line, its banners and notification, or the proxy explanations of omp's failures.

Sections: `system-certificates`

File: `system-certificates.md`

## Views

Use when building or changing the Requirements, Specs, History, focus, overlay, welcome window or the Usage dialog.

Sections: `app`, `history-entry`, `window`, `usage`

File: `views.md`
