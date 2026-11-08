# Usage

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
