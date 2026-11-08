---
name: weaver-req-ui
description: "Use when building or changing screens, layout, styling, components, navigation look or texts shown to users."
---

Every text of the interface is in English, whatever the language setting says; that setting only changes the language of the agent's texts. The interface only displays what the Rust backend holds and sends the developer's requests to it: it keeps no state of the project, the cards or the jobs of its own, and never reads or writes project files itself.

## General

### Theme and components
- shadcn/ui components, restyled with Weaver's own tokens below, in light and dark. The Theme setting is `auto` (follows the OS), `light` or `dark`; a change, of the setting or of the OS in `auto`, applies at once to every open window without a reload.
- Every control is a shadcn/ui component (Button, DropdownMenu, ContextMenu, Popover, Tooltip, Dialog, AlertDialog, Select, RadioGroup, Textarea, Badge, Alert, ScrollArea…); no hand-made widget where shadcn has one. The File view's editor is CodeMirror (`requirements/ui#file-tab`); the Notes view's editor is TipTap (`requirements/ui#notes-view`).
- Every surface, Settings, Files, Assets, Notes, dialogs, toasts and banners included, takes these tokens, shapes and fonts.

### Ground and ink
- Light values: Paper `#F6F5F1`, the window's ground around the work surface and behind the sidebar; Surface `#FFFFFF`, the work surface, cards, dialogs, popovers and menus; Lane `#FAF9F6`, boxes set inside the surface (Board columns, list panes, collapsed blocks); Line `#E7E5DF`, rings and control borders (`#E0DDD5` on secondary buttons and inputs, `#EFEDE7` for separators inside the surface); Ink `#18181B`, text; Ink 2 `#52525B`, secondary text and the icons of ghost buttons; Muted `#6B6B73`, muted text.
- Ink is the only colour of actions: primary buttons are Ink with white text; secondary buttons are Surface with a 1 px `#E0DDD5` border and Ink text; ghost buttons have no background and Ink 2 text; destructive buttons are Surface with Rose ink text and a 1 px `#EBCBD4` border. The focus ring is Ink. No blue primary remains.
- Dark values keep the same roles, derived from the light ones: Paper and Surface are dark warm greys, Surface the lighter of the two; Line a low-contrast warm grey; Ink near white, so primary buttons are light with dark text; each thread keeps its hue with a darker tint and a lighter ink. In both themes the contrasts of `requirements/quality#General` hold.

### Threads: the colours of state
- Five colours from the logo, each with a base (dots, bars, stripes), a tint (backgrounds of badges and boxes) and an ink (text and icons on the tint):

| Thread | Base | Tint | Ink |
|---|---|---|---|
| Sky | `#96BBE0` | `#E6EFF8` | `#2E5F8F` |
| Sage | `#9DC09A` | `#E9F2E7` | `#3F6B3C` |
| Peach | `#EDB08F` | `#FBEDE4` | `#9A4A22` |
| Lavender | `#A996D6` | `#EFEAF8` | `#5B4A99` |
| Rose | `#E3A3B5` | `#F9E8ED` | `#9B3B57` |

- Which state takes which thread: Sky for queued, running and refining, and for whatever an agent is doing now (a job holding a Task, a reading in progress); Peach for waiting for the developer, stale, blocked, almost used up, a Drift and warnings; Lavender for a Proposal, a question round and the agent's choices; Sage for done, validated, connected and success; Rose for failed, stopped, used up, errors and destructive controls; neutral (Ink 2 on Lane) for suspended and every other state.
- Colour tells only a state: everything else is ink on paper. Labels, kinds of file and entries have no colour of their own; a label shows as text in an outline badge.
- Wherever another fragment of this Area names a colour for one of the states above, that state's thread applies. A colour named for anything else follows its name: blue and cyan are Sky, green Sage, amber, yellow and the warning colour Peach, violet Lavender, red and the error colour Rose, the success colour Sage. The primary blue is Ink, except for links in text and tags in message boxes, which use the Sky ink.

### Shapes
- Radii: 6 px for ids, chips and inline code; 9 px for buttons, inputs, menu items and sidebar entries; 12 px for cards, boxes and list rows; 14 px for the work surface, Board columns and dialogs. Badges and pills are fully rounded.
- Cards and boxes carry a 1 px ring in Line (a box shadow), not a border; a card that stands on a lane adds `0 2px 6px rgba(24,24,27,0.05)`; dialogs, popovers and menus a larger soft shadow. The border of an unchecked checkbox or radio has a contrast of at least 3:1.

### Type and motion
- Fonts: Geist at 14 px base, weights 400, 500 and 600; ids, paths, commands and code in Geist Mono. Both are bundled in the app and never fetched from the network. Titles use a slightly negative letter spacing (−0.01 to −0.02 em).
- Transitions last at most 200 ms. When the OS asks for reduced motion there are no transitions, no shimmer and no rotation; scrolls jump.
- Status is never told by colour alone: every coloured dot, border or tint comes with a text, a badge or an icon, and with an accessible name.

### View header
- Every view (Board, Chat, Jobs, Files, Assets, Notes, Requirements, Specs, History, Settings) starts with one header row, full width of the view: the view's name as title on the left, under it an optional one-line subtitle in 13 px Muted where the view gives one, the view's own controls, when it has any, on the right; the free space sits between them. One bottom border, `#EFEDE7`, separates it from the content.
- Every header has the same fixed height, 76 px plus its 1 px bottom border, whether or not it holds controls or a subtitle, so the border does not move when switching views. Side padding 28 px; controls are 36 px high Buttons and Selects, vertically centred.
- The title is the sidebar entry's name, text only (no icon), 19 px semibold. It is the view's main heading for assistive technologies (the heading the view's region is labelled by); no other hidden heading repeats the view's name.
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
| New Request | `Plus` |
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
- A card is shown by its short number (`P3`, `T3`, `D3`) when it has one, otherwise by its 8-character permanent id; never both. Monospace, on a Paper tag.
- The accessible name and tooltip say `Short number (temporary) P3` or `Permanent ID qmqh1o3p`.

### Dialogs and confirmations
- Details and forms open in a shadcn `Dialog` (max width 720 px, the body scrolls inside); confirmations in an `AlertDialog` whose destructive button uses the `destructive` variant and whose Cancel has the initial focus.
- A confirmation with three choices (`Save` / `Discard` / `Cancel` and the like) is an `AlertDialog` too: the choices that lose text use the `destructive` variant, the one that keeps it the default variant, and Cancel, the last button, has the initial focus.
- Esc, the `X` button or a click outside close a Dialog; an AlertDialog closes only with its buttons or Esc. Closing puts the focus back on the control that opened it.
- Every Dialog and AlertDialog, the question round included, is centred in the window by layout (for example a full-window flex or grid container), never by shifting it by a percentage of its own size; its top-left corner always sits on whole CSS pixels, whatever its content height and the window size (odd sizes included), so its text is never drawn between pixels.
- They open with a 150 ms fade only, without zoom, scale or any other transform or filter; under reduced motion they appear at once. Popovers, menus, selects and tooltips keep their own opening animation.
- A cancelled confirmation changes nothing.

### Toasts
- Transient messages are toasts, from the shadcn `Sonner` toaster mounted once per window, at the bottom right, at most 3 visible at once. While the minimized bar of a question round shows (`requirements/ui#question-round`), they sit above it, never over it.
- Three kinds: info, warning and error. Info toasts keep the neutral popover look with no icon; warning toasts use Peach with `TriangleAlert`, error toasts Rose with `CircleX`, in the border, the icon and the title.
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

## Chat

Use when building or changing the Chat view: its conversation, message box, header and the cards it shows.

Sections: `general-chat`

File: `chat.md`

## Jobs and conversations

Use when building or changing the Jobs list, a job's or a sub-agent's conversation, or General.

Sections: `job`

File: `conversations.md`

## Files

Use when building or changing the File view: its tree, tabs, editor, image and SVG previews, bars and dialogs.

Sections: `file-view`, `file-tab`, `file-lock`

File: `files.md`

## History

Use when building or changing the History view: its periods, totals, entry rows and entry dialog.

Sections: `history-entry`

File: `history.md`

## Look and feel

Use when building any screen or control: theme, colours, fonts, shapes, icons, tooltips, ids, Markdown, motion.

Sections: `General`

File: `look.md`

## Notes

Use when building or changing the Notes view: the sheet's editor, its Markdown shortcuts, checklists, images and save status.

Sections: `notes-view`

File: `notes.md`

## Requests

Use when building or changing design requests: their draft cards on the Board, the request editor, sending and discarding, and opening a sent request from its job.

Sections: `design-request`

File: `requests.md`

## Question rounds and notifications

Use when building or changing question rounds or system notifications.

Sections: `question-round`, `notification`

File: `rounds.md`

## Setup and settings

Use when building or changing Get Started, import exclusions, omp status and login, or Settings.

Sections: `project`, `omp-setup`, `developer.CHANGE_SETTINGS`

File: `setup.md`

## Solo mode and Atomic commits

Use when building the Solo mode control in Settings and Get Started, its notes, warnings and the list of tracked files; Merge on the Board with its button, Dialog, merge banner and notices; or the Atomic commits switch in Settings and its notices.

Sections: `solo-mode`, `solo-merge`, `atomic-commits`

File: `solo-mode.md`

## System Certificates

Use when building the System Certificates switch, its status line, its banners and notification, or the proxy explanations of omp's failures.

Sections: `system-certificates`

File: `system-certificates.md`

## Usage

Use when building or changing the Usage summary in the sidebar or the Usage dialog.

Sections: `usage`

File: `usage.md`

## Views

Use when building or changing the Requirements, Specs, History, focus, overlay or welcome window.

Sections: `app`

File: `views.md`

## Window

Use when building the project window's frame, sidebar, project switcher, palette or user row, the welcome window, the File menu or Weaver's logo.

Sections: `window`

File: `window.md`
