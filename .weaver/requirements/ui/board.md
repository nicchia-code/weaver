---
description: "Use when building or changing the Board: columns, footers, ghosts, notices, Task cards and their details dialogs."
---
# Board

## work
- Two columns of equal width share the whole view, in this order: `Backlog` (open Proposals and open Drifts in one creation order, oldest first) and `Active` (Tasks in To do and In progress, blocked ones included, oldest first). No Done column, collapsed or not, and no other column.
- Each column title is followed by the number of its real cards; ghosts never count, a resolution card counts once. Empty texts: `Nothing to decide` (Backlog), `Nothing active` (Active), shown also above ghosts.
- The Backlog grouping (which Drifts sit in a resolution, which Proposals are hidden) comes computed from the backend; the interface never regroups.
- No card can be dragged, moved, taken back or removed from the Board. Clicking a card outside its controls, or Enter on its title, opens its details dialog.
- Each column scrolls on its own; its footer stays visible at its bottom.

### Footers and header
- Bottom of Backlog, full column width: `+ Work Item`, `outline` Button with a dashed border, always enabled; it opens a new design request (see `design-request`). Tooltip `Write a work item in Markdown and send it to an agent`.
- Bottom of Active, full column width: `Run all`, primary. Tooltip `Run all the Tasks in To do`. Disabled reasons, in order: `A merge is in progress`, `Drifts are being resolved`, `A run is in progress`, `No Task is in To do`, `All the Tasks in To do are blocked`.
- Board header (the view header of `requirements/ui#General`, title `Board`), on its right: `Merge` while Solo mode is on (`requirements/ui#solo-merge`), then `Scan System` (outline), followed by a muted `· <n> done since last scan`: the Tasks finished (not closed unchanged) since the last scan job of the whole System started, asked by the developer or started after a merge, deleted and forgotten ones included; hidden when 0. It confirms first (`Compare the whole System with the code?`, `Scan System` / `Cancel`); cancelling leaves the count unchanged. It is disabled with `A merge is in progress` while a merge is in progress.
- While a merge is in progress its banner sits at the top of the Board, above the notices (`requirements/ui#solo-merge`).

### Ghosts
- A ghost sits at the bottom of its target column (Backlog for a Proposal or Drift, Active for a Task), after the real cards and before the footer, in announcement order; Weaver's own ghost of a setup or import job takes the place it would have if it had been announced when its job was created.
- Skeleton card: dashed border, 70% opacity. Top part, known and still: the kind (`Proposal`, `Task`, `Drift`), the title, a phase line, then the job's latest note in muted text on one line cut with `…` (full text as tooltip). None of these ever shimmers.
- Phase line: `Preparing` until the Draft exists, `Writing` once it exists, `Waiting for your answers` while the job waits for a round or a permission, `Being refined` for a refinement. Plain muted text, no icon, no animation.
- Under the known part, always, one block of placeholders for what the ghost does not know yet, in the order of the real card of its kind and with its shape: muted bars and blocks with no text.
  - Proposal: a short bar for the id, a bar for the files line, a bar for the Tasks line (always shown), then a row of two button-sized blocks for the controls.
  - Task: a small circle for the status dot next to a short bar for the id, then one button-sized block for `Run`.
  - Drift: a short bar for the id, then a row of two button-sized blocks for `Resolve` and `Dismiss Drift`.
- Only the placeholders shimmer: a light band sweeps across them from right to left, once every 2.4 s. Under reduced motion they are still bars.
- The whole ghost is one button that opens its job's conversation in Jobs. Accessible name `<kind> being prepared by the agent: <title>, <phase>`.
- When the card arrives, in the same picture the ghost disappears and the card takes its place in creation order.
- **Weaver's own ghost of a setup or import job:** from the moment the job is created (queued) until it ends, suspended included, whenever none of the job's announced ghosts is on the Board, Backlog shows one ghost of the job made by Weaver, without any announcement.
  - Kind `Proposal`; title `Setup` for a setup job, `Import from Code` for an import job; phase line `Waiting for your answers` while the job waits for a round or a permission, `Preparing` otherwise (never `Writing`); the job's latest note as on the other ghosts. Same skeleton, button and accessible name as every ghost (`Proposal being prepared by the agent: Setup, Preparing`); it never counts in the column's number.
  - In the same picture in which the job's first announced ghost appears, it disappears; it comes back in the same picture in which the job's last announced ghost leaves, while the job has not ended. A Proposal's ghost stays while its Draft is written, so Weaver's ghost stays away until that Proposal is handed over.
  - It disappears when the job ends, whatever the reason, in the same picture as the job's other ghosts.
  - Its identity is derived from its job, stays the same across pictures and never equals an announcement's; it is never stored in the job's record.

### Details dialogs and notices
- A details dialog follows its card: each new picture updates it in place without moving the focus. When the card leaves the Board (decided, Done, deleted, gathered into a resolution) the dialog closes by itself and the focus goes to the column the card was in.
- Notices sit at the top of the Board, above both columns, newest first, each an `Alert` with a close button, until closed or the window closes: decisions refused when applied (`Could not delete T4: the agent that filed it is still working.`) and Proposals sent back to Draft while the developer was deciding them.
- History warnings sit under the notices, above both columns: one amber `Alert` with `TriangleAlert` for each cited Task whose copy in History could not be made (`Could not record T3 in History: <reason>. The Task stays until its copy can be made.`), each with a close button (`X`, accessible name and tooltip `Close`). A closed warning stays hidden while the window stays open, even when the Board changes; it shows again at the next opening of the window if it is still true. A new warning, or one whose text changed, shows at once.

## task
- Card: status dot, id, title (at most 3 lines), badges; under them the Proposal's label badge when it has one; then `View requirements` (only when its focus would have at least one step) and `Run` (outline).
- Status dot, 8 px, never animated, with tooltip and accessible name: To do = hollow ring in `muted-foreground`, `To do`; In progress = filled amber, `In progress`, or `Agent working` while a job holds it.
- Badges: `technical` (outline), `blocked` (red), `waiting <id>` (amber); both may show. `blocked` and `waiting` are Popover buttons: `Blocked by` lists the blocking Drifts (id, title) in creation order, `Waiting for` lists the fix Task; a row closes the popover, scrolls its card (or its row in a resolution card) into view and focuses it. Tooltips: `Blocked by Drift D3, D7`, `Waiting for Task T4`.
- Left border 3 px in its label colour: the FNV-1a 32-bit hash of the label's UTF-8 bytes modulo 6 picks, in order, blue, green, amber, violet, cyan, pink. A Task outside any design uses the `border` token.
- While a job holds it: a 2 px conic border rotating blue → cyan (CSS only, one turn every 3 s), static cyan under reduced motion. No other card has it.
- `Run` disabled reasons, in order: `The agent is still finishing`, `A merge is in progress`, `Drifts are being resolved`, `A run is in progress`, `The agent is working on it`, `Blocked by Drift <ids>`, `Waiting for Task <id>`. Another Task held by the agent never disables it.
- Details dialog: id, status, badges, label, refs as chips (single-ref focus), rendered body, for a fix Task its Drift's two sides, chosen option and note; then `View requirements`, `Run`, `Ask more` (see `ask-conversation`; not on the card) and, only in To do, `Delete` (destructive), confirmed with `Delete <id> for good?` / `Delete` / `Cancel`. `Delete` disabled reasons, in order: `The agent is still finishing`, `Origin of open Drift <id>`, `Fix Task of Drift <id>, whose code is to be fixed`.
