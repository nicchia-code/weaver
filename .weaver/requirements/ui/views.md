---
description: "Use when building or changing the Requirements, Specs, History, focus, overlay, welcome window or the Usage dialog."
---
# Views

## app

### Overlay of a Proposal
- There is no overlay picker. The overlay is the Proposal of the last focus opened, while that focus lasts: `View requirements` on a Proposal that can be overlaid (open and not attached to a Drift; never a Draft, a Proposal being refined or an attached Proposal) draws it in both the Requirements and the Specs of the window. `View requirements` on any other Proposal opens its focus on the Current Model.
- Opening a focus on another subject (a Task, a chip, a Proposal that cannot be overlaid), in either view, or ending the focus that drew the Proposal (`Clear` or Esc) returns both views to the Current Model.
- When the drawn Proposal stops being open, the views go back to the Current Model and a notice says why; the focus stays, recomputed on the Current Model.
- The overlay shows the Requirements and the Specs with the Proposal applied on top of the Proposals it requires.
- Change colours (Tailwind 600 light / 400 dark): added `green`, changed `amber`, removed `red`. Every change also has a text label (`added`, `changed`, `removed`) and an accessible name.

### Focus
- Requirements and Specs each have their own focus, opened by `View requirements` (a Proposal's scope or a Task's refs) or by a chip (a single ref). Steps: sections, Areas or fields in page order.
- A chip opens the first of Requirements and Specs where its focus has at least one step. The focus of a Model element on Requirements holds the sections titled with that element or with an element containing it, up to its Feature. A chip whose focus has no step in either view is plain text, not a control.
- Focus bar, full width under the view's header: what it is about (`Proposal P3 – <title>`, `Task T4 – <title>`, or the ref), the step name (section, Area or field), `<n> of <N>`, `Previous` / `Next` (icon-only; disabled at the ends), `<M> not found` (muted, tooltip lists the missing refs one per line), on Requirements an `Only these` toggle (off at start), and `Clear`. With one step there is no counter nor arrows.
- Focused sections and fields get a cyan (or change-colour) left border and tint; going to a step scrolls it to the top of the view.
- When files change, the focus is recomputed and stays on the same step if it still exists. Esc clears the focus. `Clear` and Esc also end the overlay the focus drew (see Overlay of a Proposal). Step changes are announced to assistive technologies.

### Requirements view
- Header (the view header of `requirements/ui#General`, title `Requirements`), on its right: only `Ask more` for the Requirements as a whole (see `ask-conversation`). The Specs view's header (title `Specs`) has no controls.
- Areas in folder-name order, each with its title and prose; inside, fragments in file-name order (`<fragment>.md` in muted text next to the title); inside, sections with `General` first, then in file order.
- Each section heading shows its title and `Open in editor` at the heading's line; it carries no reference chip and no `Ask more`. Area and fragment headings have no `Ask more` either.
- Overlay labels: `new fragment`, `fragment removed`, `added`, `removed`, `changed`, `moved from <fragment>`. A removed section is shown struck through in muted text; a changed one shows the new text with a `Show before` toggle that shows the old text.

### Specs view
- Fields in this fixed order: languages, frameworks, data, testing, CI, repository, deployment, infrastructure, notes. Empty fields are hidden.
- Max width 1100 px; entries in a grid of columns at least 300 px wide. An entry shows name, version and note; one the agent chose carries the `Sparkles` mark `agent's choice` whose tooltip is its reason.
- Overlay: entries labelled `added`, `changed`, `removed` as on Requirements.

### Errors and the editor
- An invalid Model or Requirements shows, at the top of the Requirements and Specs, a red `Alert` `The Model has <n> errors` with an expandable list: each error with its message and file, its line when known, and `Open in editor`. Unreadable files are listed the same way.
- `Open in editor` is shown wherever a file and a line are known and opens them in the editor chosen in the Settings; when it fails, an error toast `Could not open the file in the editor.` gives the reason as its description.

## history-entry
- The History view lists entries newest first by their date (written or last updated), grouped by local day. Group headers: `Today · <n>`, `Yesterday · <n>`, then the long date (`Monday, 2 November 2026 · <n>`).
- An entry row: title, one-line summary (cut with `…`, full text as tooltip), the time `HH:MM` in Today and Yesterday, the number of Tasks.
- Period filter, a Select on the right of the view header (`requirements/ui#General`, title `History`): `Today`, `Last 7 days` (default at each opening of the window), `Last 30 days`, `All`. A period has both bounds: from local midnight of its first day (today, 6 days ago, 29 days ago) to now; an entry dated in the future is shown only under `All`. Empty: `Nothing done in this period`.
- Clicking an entry opens a Dialog: title, date, summary, rendered text, then its Tasks, from the copies kept in the entry, in the order cited: id, title, label badge, finished time; a row expands to show the Task's rendered body and refs, read-only. The Dialog's footer holds `Ask more` on the whole entry (see `ask-conversation`).

## window
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

## usage
- **Dialog:** a shadcn `Dialog`, max width 480 px, title `Usage`, opened by the sidebar entry `Usage`; Esc, `X` or a click outside close it and the focus goes back to the entry. No Refresh button: the limits are read at each opening.
- **Content:** one block per connected provider of the fixed list (`requirements/tech#omp-setup`), in its order, headed by the provider's bold short name. Providers not connected are not shown. No email, account, organisation or plan identifier is ever shown; when omp reports several accounts for one provider, their limits follow one another in the same block.
- **Limit row:** the limit's name as omp gives it (`Claude 5 Hour`, `Premium Requests`), a shadcn `Progress` bar of the used fraction, the value and the reset; the bar's accessible name is `<name>: <value>`.
  - Value by unit: percent `17% used`; requests `3,437 / 100,000 requests`; tokens `<used> / <limit> tokens`; credits `<used> / <limit> credits`; minutes `<used> / <limit> min`; USD `$0.50 / $1,000.00`. Without a limit, only the used amount; with no known used fraction, no bar and the value `Usage unknown`.
  - Reset: `Resets in <duration>` when it is less than 24 h away (durations as in `requirements/ui#General`), otherwise `Resets on <date>` (date written out as in `requirements/ui#General`); nothing when omp gives no reset time.
- **Status:** a limit almost used up has its bar and value in `amber` with the text `Almost used up`; a limit used up in `red` with `Used up` (shade 600 in light, 400 in dark); the others stay neutral.
- **Provider without limits:** one secondary line under its name: `No usage data` when omp reports the account without usage, `Login expired` when omp turned its credential off, `Not reported by omp` when omp says nothing about it.
- **No limits at all:** `No usage limits to show.` in secondary text above the provider blocks.
- **Time of the data:** a muted line under the title, `Updated at HH:MM`, the oldest time at which omp read one of the shown providers.
- **Reading:** with earlier limits, they stay shown and the time line gets a `Loader` and `Updating…`; without them, a centred `Loader` with `Reading usage…`. The content has `aria-busy="true"` while reading, and `Loader` is static under reduced motion.
- **Failed:** a destructive `Alert` `Could not read usage: <reason>.` (reasons as in `requirements/tech#omp`) with an `outline` small `Retry` button (`RotateCw` icon); the earlier limits, when there are some, stay below it with their time line.
- **Waiting for omp:** the text of the omp status box for the current omp state (`requirements/ui#omp-setup`) and an `Open Settings` button, which closes the dialog and shows Settings. When omp becomes ready, the dialog switches to Reading by itself.
