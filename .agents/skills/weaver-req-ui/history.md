# History

## history-entry

- **Header:** the view header of `requirements/ui#General`, title `History`, subtitle `Everything that shipped, grouped as it was delivered.`; on its right the period, a Select: `Today`, `Last 7 days` (default at each opening of the window), `Last 30 days`, `All time`. A period has both bounds: from local midnight of its first day (today, 6 days ago, 29 days ago) to now; an entry dated in the future is shown only under `All time`.
- Content at most 1040 px wide.
- **Totals:** a row of three boxes (Lane, inset Line ring, radius 12 px; stacked when narrow), computed on the entries of the period: `Delivered`, the number of entries; `Tasks done`, the number of Tasks they cite; `Last delivery`, the date of the newest one (`Today, HH:MM`, `Yesterday, HH:MM`, otherwise the date written out). Label 12 px Muted, value 24 px semibold with tabular figures; `—` when the period has no entry.
- The entries, newest first by their date (written or last updated), grouped by local day. Each group's heading: `Today`, `Yesterday` or the weekday, semibold 13 px, followed in Muted by the date written out and `· <n> entries` (`· 1 entry`).
- Each group is one list with the Line ring, radius 12 px, one row per entry separated by `#F1EFEA`: a 4 px neutral stripe (Line) on the left, the title (14 px semibold) and under it the summary in Ink 2 on at most two lines, cut with `…` (full text as tooltip); on the right the time `HH:MM` in Geist Mono Muted and a Sage pill `<n> tasks` (`1 task`).
- Empty: `Nothing done in this period`.
- Clicking an entry opens a Dialog: title, date, summary, rendered text, then its Tasks, from the copies kept in the entry, in the order cited: id, title, label as text in an outline badge, finished time; a row expands to show the Task's rendered body and refs, read-only. The Dialog's footer holds `Ask more` on the whole entry (see `ask-conversation`).
