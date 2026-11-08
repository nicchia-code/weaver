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
- Change colours, each as ink on its tint (`requirements/ui#General`): added Sage, changed Peach, removed Rose. Every change also has a text label (`added`, `changed`, `removed`) and an accessible name.

### Focus
- Requirements and Specs each have their own focus, opened by `View requirements` (a Proposal's scope or a Task's refs) or by a chip (a single ref). Steps: sections, Areas or fields in page order.
- A chip opens the first of Requirements and Specs where its focus has at least one step. The focus of a Model element on Requirements holds the sections titled with that element or with an element containing it, up to its Feature. A chip whose focus has no step in either view is plain text, not a control.
- Focus bar, full width under the view's header: what it is about (`Proposal P3 – <title>`, `Task T4 – <title>`, or the ref), the step name (section, Area or field), `<n> of <N>`, `Previous` / `Next` (icon-only; disabled at the ends), `<M> not found` (muted, tooltip lists the missing refs one per line), on Requirements an `Only these` toggle (off at start), and `Clear`. With one step there is no counter nor arrows.
- Focused sections and fields get a Sky (or change-colour) left border and tint; going to a step scrolls it to the top of the view.
- When files change, the focus is recomputed and stays on the same step if it still exists. Esc clears the focus. `Clear` and Esc also end the overlay the focus drew (see Overlay of a Proposal). Step changes are announced to assistive technologies.

### Requirements view
- Header (the view header of `requirements/ui#General`, title `Requirements`, subtitle `What the app must do, in plain words. Agents build from this.`), on its right: a 36 px search field `Search requirements` (`Search` icon, about 260 px), `Ask more` for the Requirements as a whole (see `ask-conversation`), then `Ask for a change` (primary, `MessageSquare`).
- `Ask for a change` shows Chat with its message box focused and holding `Change to the Requirements: ` with the cursor at the end; text already in the box stays, and the prefix goes on a new line after it. Nothing is sent until the developer sends it.
- Below the header, two columns: on the left, a navigation about 220 px wide that scrolls on its own; on the right, the Requirements, at most 900 px wide.
- **Navigation:** for each Area, in folder-name order, its title as a heading (11 px, medium, uppercase, Muted), then one entry per fragment in file-name order: its title and, right-aligned in Muted, its number of sections. A fragment the overlaid Proposal adds, removes or changes in any section carries a Lavender pill `changed`. Choosing an entry scrolls its fragment to the top; the entry of the fragment at the top of the view stands on Paper in Ink, the others in Ink 2. Under a search, the numbers count only the matching sections and entries without any are hidden.
- **Content:** Areas in folder-name order, each with its title and prose; inside, fragments in file-name order: the fragment's title (22 px semibold), its path relative to the Area (`data/stored.md`) as a monospace tag on Paper, and `Open in editor` (ghost) at its right; then its sections, `General` first, then in file order, in one box with a 1 px Line ring, radius 12 px, one row per section separated by `#F1EFEA`: the section's title in Geist Mono 12.5 px medium Ink 2, in a column about 140 px wide, and its rendered text beside it (stacked under it when the row is narrower than about 480 px), with `Open in editor` at the heading's line shown on hover and focus. No section carries a reference chip or `Ask more`, nor do Area and fragment headings.
- **Search:** typing filters the content in place, case-insensitively: only the sections whose title or text contains the words remain, with their matches marked; a fragment or Area with none left is hidden; with no match at all, `No sections match.` in Muted. Esc in the field, or emptying it, shows everything again. The search combines with the focus and its `Only these`: a section shows only when both keep it. It never searches anything but the Requirements.
- Overlay labels: `new fragment`, `fragment removed`, `added`, `removed`, `changed`, `moved from <fragment>`. A removed section is shown struck through in muted text; a changed one shows the new text with a `Show before` toggle that shows the old text.

### Specs view
- Header (the view header of `requirements/ui#General`, title `Specs`, subtitle `The stack the agents build with · specs.yaml`, the file name in Geist Mono), on its right a pair of pill toggles: `All <n>` (pressed by default) and `Agent's choice <n>` (`Sparkles`), which shows only the entries the agent chose; the counts are of entries.
- Every field that has entries or text, in the order of the Specs format (languages, frameworks, data, testing, CI, repository, deployment, cloud, infrastructure, notes); empty fields are hidden.
- Each field: its title (15 px semibold), its number of entries in Muted, a thin rule filling the row, and `Open in editor` (ghost, `SquareArrowOutUpRight`), which opens the Specs file at the field's line in the editor chosen in the Settings.
- Entries in a grid of columns at least 250 px wide, at most 1180 px in all; each a card with the Line ring, radius 12 px: a 26 px neutral tile (Ink 2 on Paper) with the name's initial, the name (14 px semibold), the version as a monospace tag on Paper, the note in Ink 2, and, at the bottom, for an entry the agent chose, a Lavender pill `Agent's choice` with `Sparkles` whose tooltip is its reason.
- The notes field shows its rendered Markdown in one box with the Line ring.
- Overlay: entries labelled `added`, `changed`, `removed` as on Requirements.

### Errors and the editor
- An invalid Model or Requirements shows, at the top of the Requirements and Specs, a red `Alert` `The Model has <n> errors` with an expandable list: each error with its message and file, its line when known, and `Open in editor`. Unreadable files are listed the same way.
- `Open in editor` is shown wherever a file and a line are known and opens them in the editor chosen in the Settings; when it fails, an error toast `Could not open the file in the editor.` gives the reason as its description.

## usage
- **Summary:** a box near the bottom of the sidebar (`requirements/ui#window`), Surface with a 1 px Line ring, radius 12 px, padding 12 px; the whole box is one button that opens the Usage dialog, accessible name `Usage: <the summary's text>`.
  - First line: `Usage` (12 px, medium) and, right-aligned, the used percentage of the limit closest to being used up (`requirements/tech#usage`); under it a 6 px bar of that fraction, neutral (Ink 2 on `#EFEDE7`), Peach when almost used up, Rose when used up; under it, in 12 px Muted, the limit's name and its reset (`Claude 5 Hour · Resets in 2 h 10 min`).
  - Without limits read yet: `Reading usage…` while a reading runs, otherwise `Open to read usage`; when no connected provider has a limit: `No usage limits`; when the agent is not ready: `Usage needs the agent`; when the last reading failed and no limits were ever read: `Could not read usage`. Limits read earlier stay shown in every case, the reading included.
  - It shows the limits read last app-wide and changes in every window as soon as a reading ends.
- **Dialog:** a shadcn `Dialog`, max width 480 px, title `Usage`, opened by the summary in the sidebar or by `Usage` in the palette; Esc, `X` or a click outside close it and the focus goes back to where it was. No Refresh button: the limits are read at each opening and every five minutes (`requirements/tech#usage`).
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
