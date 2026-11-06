---
description: "Use when building or changing Get Started, import exclusions, omp status and login, the request editor or Settings."
---
# Setup and settings

## project
- **Not initialised:** every view of the window except Settings shows Get Started: a centred column (~560 px) with the title `Set up Weaver in this folder`, the text `Weaver will create a .weaver folder that describes this project. It holds no secrets and is meant to be committed, like the agent files it installs.` (another text with Solo mode, `requirements/ui#solo-mode`), the list of agent files it installs (relative paths, monospace, from the same list the backend installs), the Solo mode choice, and the primary button `Initialise`.
- **Overwrite conflict:** when agent files exist with different content, an AlertDialog `Some agent files already exist` lists each relative path (monospace, scrolling past 8 lines) and offers `Overwrite and initialise` (destructive) and `Cancel` (focus). Cancel changes nothing and shows nothing.
- A failed initialisation shows `Could not initialise: <reason>.` in an error box under the button; the button stays for a retry.
- **Empty Model:** the Board shows above its empty columns two side-by-side cards (stacked below ~640 px), each a button: `Start from Scratch` (`Sparkles`) `For an application still to be built: you describe it, an agent designs it.`; `Import from Code` (`FolderCode`) `For a project that already has code: an agent describes it as Proposals.` When the folder holds names not starting with a dot besides `.weaver`, a warning line under the cards: `Start from Scratch ignores the files already in this folder unless your request mentions them.` The cards hide while a setup or import job works or a Proposal is open. While omp is not ready, the omp status box shows above the cards.
- Start from Scratch opens a new setup request in the request editor (`design-request`).
- **Import from Code:** a panel with the folder tree and the primary button `Start import`.
  - The tree lists the project folder, leaving out names starting with a dot and everything git ignores; first level shown, folders collapsed, children read when opened.
  - Each entry: checkbox (indeterminate for a partly excluded folder), `Folder`/`FolderOpen` or `File` icon, name truncated with the full path in the tooltip; ~16 px indent per level. Unchecking a folder excludes all inside it.
  - Above the tree: `<N> excluded` or `Everything is included`.
  - `Start import` is disabled while nothing is included, tooltip `Include at least one file or folder.`
  - Exclusions live only in the panel until `Start import` and are never saved; leaving the panel forgets them.
  - Keyboard: ARIA tree; arrows move and open/close, `Space` toggles.
- **Update skills** (in Settings, group `Project`): button `Update skills` (`RefreshCw`, spinning while it runs, disabled meanwhile). Its report stays under the button until the next run: `Everything was already up to date.`, or `Updated <n> files:` with one line per file (relative path, monospace, and `created`, `rewritten` or `removed`), or `Could not update the agent files: <reason>.` in the error colour. It renders the same structured result the command line reports.
- **On opening:** when the background check rewrote files, a toast `Updated the agent files to this version of Weaver.`; when it failed, a banner at the top of the window `Could not update the agent files: <reason>.` with `Retry` (runs Update skills), shown at every opening until it succeeds.

## omp-setup
- **Status box:** a bordered `aria-live="polite"` region, the same component in Settings, Get Started and the welcome window:

| State | Icon | Text | Action |
|---|---|---|---|
| Checking | `Loader` | `Checking omp…`; at the app's start, while System Certificates exports, with the secondary line `Exporting system certificates first…` | — |
| Missing | `TriangleAlert` (warning) | `omp is not installed.` or the reason: `omp not found at <path>`, `omp did not answer within 15 s`, `omp exited with code <N>`, a proxy explanation; after a failed update of a too-old omp: `omp <version> is older than <minimum> and could not be updated (<reason>).`; after a failed installer, its text (`requirements/ui#system-certificates`) | `Install omp` |
| Installing | `Loader` | `Installing omp in the terminal…` | `Open installer again` |
| Updating | `Loader` | `Updating omp to <version>…` (`Updating omp…` without a known version) | — |
| Not connected | `TriangleAlert` (warning) | `omp <version> has no model provider. Log in to one below.` | provider rows |
| Logging in | `Loader` | `Waiting for the login to <provider> in the terminal…` | — |
| Ready | `CircleCheck` (success) | `omp <version> is ready with <N> models.`; after a failed update, warning colour: `Could not update omp (<reason>). omp <version> is ready with <N> models.` | — |

- While Installing or Logging in, a secondary line under the text: `Finish it in the terminal window. Weaver checks again every few seconds.`
- When Weaver cannot open the system terminal: `Could not open a terminal: <reason>.` in the error colour, with the command in monospace and a `Copy` button.
- `Loader` is static under reduced motion. The state is always text, never colour only.
- **Providers:** shown when omp answers (Not connected, Logging in, Ready), below the status box, subtitle `Providers`. One row per provider of the fixed list, in its order, thin separators: bold short name, plan in secondary text, state, buttons right (below the name under ~480 px).
- **State:** `CircleCheck` (success) + `Connected`; `Not connected` in secondary text; Unknown shows no state and leaves no gap.
- **Log in:** `outline` small button, `LogIn` icon: `Log in`, or `Log in again` when Connected; tooltip `Open a terminal that logs omp in to <name>`. The Cursor row is the same, login only.
- **Log out:** only on Connected rows, `ghost` small button, `LogOut` icon. AlertDialog `Log out of <name>?` / `omp forgets the accounts of <name> it stored. Credentials in environment variables, .env files or models.yml stay.`, `Log out` (destructive) and `Cancel` (focus). While it runs the icon becomes a spinning `Loader`, the row's buttons are disabled and the row has `aria-busy="true"`; afterwards focus returns to the row's `Log in`.
- Under the row afterwards: failure `Could not log out of <name>: <reason>.` (error colour); still connected `<name> is still connected through another credential (environment variable, .env file or models.yml). Remove it there to disconnect.` Nothing is shown when the row turns `Not connected`.

## design-request
- **Request editor:** a pane with the open requests (writing or sent) listed on the left, newest first, each with its title (first heading or first line, else `Untitled request`), created time and a `Draft` or `Sent` badge; the selected request on the right. It opens from the Board's new-request button and from Start from Scratch, focused in the text. The header reads `Setup request` or `Design request`.
- **Editing:** a monospace textarea with soft wrap filling the pane. A `Preview` toggle shows the rendered Markdown (shared renderer) side by side from ~900 px of pane width, in place of the text below it; the preview updates within 150 ms of typing.
- **Autosave:** the file is written 500 ms after the last keystroke and on blur, view switch and window close. Status text in the header: `Saving…`, `Saved`, or `Could not save: <reason>.` (error colour; the text stays and the next change retries). Typing starts no process.
- An outside change to the file reloads the editor when nothing is pending; otherwise Weaver's version is written and a warning toast says `The request changed on disk; Weaver kept your version.` A request deleted outside Weaver leaves the editor with the warning toast `The request was deleted outside Weaver.`
- **Send:** primary `Send` (`SendHorizontal`), also `Ctrl/Cmd+Enter`; disabled while the text is empty after trimming, tooltip `Write the request first.`
- **Discard:** `Discard` (`Trash2`), AlertDialog `Discard this request?` / `Its file and images are deleted.`, `Discard` (destructive) and `Cancel` (focus).
- **Sent:** read-only rendered view with a banner `Sent to the agent · <job name>` and `Open conversation`. Back in writing after its job ended without a Proposal: banner `The job ended without a Proposal. Change the request and send it again.`
- **Held by Chat:** listed with a `Chat` badge in place of `Draft`; read-only rendered view with a banner `Chat is writing a Proposal from this request` and `Open Chat`, without `Send` or `Discard`. When that Draft is discarded, the request is editable again as before.
- **Images:** PNG, JPEG, GIF or WebP pasted or dropped into the text are saved in the request's images folder next to it as `image-<n>.<ext>` (next free number), and `![](<images folder>/image-<n>.<ext>)` is inserted at the cursor. At most 10 MB per image; anything else is refused with an error toast `Only PNG, JPEG, GIF or WebP images up to 10 MB.` The preview shows images only from that folder.

## developer.CHANGE_SETTINGS
- **Settings view:** the view header (`requirements/ui#General`, title `Settings`, no controls) across the whole view, then one centred column, ~720 px wide, that scrolls under it; groups in this order, each a heading with its settings below. Group headings are 14 px semibold, never larger than the header's title:
  1. `omp`: the status box, `Providers`, `omp path` (text; empty = the official installer's places), `Permissions` (`Run without asking` = yolo, default; `Ask before acting` = ask), `System Certificates` (`requirements/ui#system-certificates`).
  2. `Models`: `Analysis`, `Scan`, `Execution`, with the bold advice `Your most capable model.`, `A fast, light model.`, `A mid-range model.`
  3. `Agents`: `Max parallel sub-agents` (whole number ≥ 1, default 20), `Language of the agent's texts` (empty = English).
  4. `Editor`: preset Select (`VS Code`, `Cursor`, `Zed`, `JetBrains`, `Sublime Text`, `Custom command`); with `Custom command`, a text field that must contain `{file}` (`{line}` optional).
  5. `Appearance`: `Theme` as a segmented control `Auto` / `Light` / `Dark`, applied live to every window.
  6. `Project`: `Solo mode` (`requirements/ui#solo-mode`), then `Atomic commits` (`requirements/ui#atomic-commits`), then `Update skills` (`project`).
- **Model roles:** a Combobox each: free typing, suggestions filtered while typing, the models omp lists (omp's order, `<selector>` with the readable name when different), plus `omp default` (empty value). A thinking-level Select (`Default` + the levels omp lists for that model) sits right of the Combobox only when omp lists levels; the field shows the model only, the value is stored as `<model>:<level>`, split on the last `:` only when the level is one omp lists. Model names are never validated. When the list is unavailable, a secondary line says why: `omp not found at <path>`, `omp did not answer within 15 s`, `omp exited with code <N>`, `omp did not return a valid list`.
- **Saving:** at once, no confirmation: selects on choice, text and number fields on blur or `Enter`. An invalid value is not saved; the field keeps the typed text and says why under it (`Enter a whole number of at least 1.`, `The command must contain {file}.`). A failed write shows `Could not save <title>: <reason>.` under the field and puts back the stored value.
- A change from another window shows everywhere at once, except in a field that has focus, which takes it on blur if the developer did not change it.
- Titles, descriptions (≤160 characters), defaults and validation come from the one settings schema the backend also uses; setting keys are never shown.
