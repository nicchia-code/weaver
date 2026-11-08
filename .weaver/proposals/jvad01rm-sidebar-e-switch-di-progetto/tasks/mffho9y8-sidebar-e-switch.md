---
refs:
  - window
  - developer.OPEN_PROJECT
  - requirements/ui#window
  - requirements/tech#window
  - requirements/tech#project
  - requirements/ui#file-view
---
# Sidebar e switch di progetto

La finestra di un progetto ha la sidebar del redesign, con lo switch di progetto in alto e la riga utente in basso; resta una finestra per progetto.

Quando la Task è finita:
- La sidebar è larga 248 px e contiene, dall'alto: lo switch di progetto, il pulsante `Search or run…` con l'indicazione `⌘K` (`Ctrl K` fuori da macOS), le voci `Board`, `Chat`, `Jobs`, il gruppo `PROJECT` con `Requirements`, `Specs`, `Files`, `Assets`, `Notes`, `History`, lo spazio libero, il riepilogo di Usage (fornito dalla Task sul riepilogo di Usage: qui solo il suo posto) e la riga utente che termina col pulsante `Settings`. Usage non è più una voce; `Settings` resta una vista, aperta dall'ingranaggio, che ne porta lo stato di voce scelta.
- Numeri sulle voci, nessuno quando è 0: `Board` le card reali del Backlog in Muted; `Chat` un `1` Peach mentre aspetta le risposte del suo round; `Jobs` i job in attesa dello sviluppatore in Peach oppure, se nessuno attende e alcuni sono in Running, la pill Sky `<n> running` con il punto.
- Sotto i 1024 px la sidebar è larga 56 px e a sole icone: lo switch mostra solo il logo e apre lo stesso menu, la ricerca è un'icona con tooltip, le voci hanno il nome come tooltip e il numero come piccolo badge, l'intestazione `PROJECT` diventa un separatore, il riepilogo di Usage diventa un pulsante `Gauge`, la riga utente tiene solo l'ingranaggio.
- Lo switch mostra logo, nome della cartella del progetto e, sotto, il branch su cui è git nella cartella; l'hash breve del commit quando git non è su un branch; niente quando la cartella non è in un repository o git non si può eseguire. Il branch si rilegge all'apertura della finestra, quando torna in primo piano e quando git cambia branch, anche fuori da Weaver, osservando il solo file HEAD di git della copia di lavoro senza toccare la fotografia del progetto.
- Il menu dello switch ha un campo `Search projects` che filtra per nome; i progetti con una finestra aperta, anche in background, il corrente per primo con il segno di spunta, ognuno con il numero dei job che lavorano; i recenti senza finestra, al massimo 10, quelli non più trovati disattivati e rimovibili dall'elenco; `Open Folder…`. Scegliere un altro progetto lo apre nella sua finestra o la porta in primo piano, lasciando questa com'è; scegliere il corrente chiude il menu; un'apertura fallita mostra il toast d'errore del menu File. Nessuna scorciatoia apre lo switch.
- La riga utente mostra un avatar rotondo con le iniziali e il nome utente di git del progetto (altrimenti quello globale); senza nome mostra solo l'ingranaggio. Si legge solo il nome, mai l'email.
- La finestra di benvenuto ha il nuovo stile, con il logo sopra il nome dell'app; il menu File con `Open Folder…`, `Open Recent` e `Clear Recent` resta com'è.
- Il titolo della finestra resta il nome della cartella; i test unitari della sidebar e i test end-to-end sono aggiornati (le voci restano trovabili per nome accessibile, anche a sole icone).

Riferimento grafico negli Assets: `redesign-sidebar.png` e `redesign-sidebar.html` (misure e colori esatti); il menu dello switch non è disegnato e segue le Foundations.

## Code notes

Commit: 450785e1b59d7321510ae229ca9cc4280a0dc799

- Sidebar: `ui/src/components/sidebar.tsx` (`Sidebar({selected,onSelect,counts,actions})`, due `<ul>` da `VIEWS` con `group` top/bottom, `Item` con `aria-current`, `data-sidebar-action`; larghezze scritte a mano; `countOf()` 18-30 con `TONE_TEXT.amber`). Viste in `ui/src/views.ts` (`VIEWS`, `USAGE_ENTRY`, `ShellCounts` 36-45). Breakpoint in `hooks/use-media-query.ts:16-21` (`COMPACT_SIDEBAR_BELOW = 1024`). Test: `sidebar.test.tsx` con `setViewportWidth`.
- Conteggi calcolati nella UI: `project-content.tsx:33-37` (`useBoard().snapshot?.backlog.length`, `useJobs().waiting`, `useChatWaiting`); per i running filtrare `jobs` per `state==="running"` in `jobs/use-jobs.tsx:68`.
- Usage oggi è un'azione della sidebar in `project-window.tsx` (`setUsageOpen`, `focusUsageEntry` col selettore `data-sidebar-action`): da cambiare quando diventa il riepilogo.
- Aperture: `src-tauri/src/windows/commands.rs` (`welcome_state`, `open_project`, `open_folder` senza finestra genitrice, `remove_recent`), registrati in `lib.rs:172-176`. Stato: `windows/local_state.rs` (`StateStore`, `snapshot()` bloccante via `blocking::disk_named`, `RECENT_LIMIT = 10`). Progetti aperti: `WindowManager::open_projects()` (`manager.rs:439`, `registry.rs:17-25`), `focus_order()`, `is_background`; job che lavorano: `Jobs::working_count(project_id)` (`jobs/registry.rs:420`). Nessun comando o evento espone oggi i progetti aperti alla UI; `jobs-changed` va già a tutte le finestre (`jobs/host.rs:164-168`); `recent-changed` solo a `WELCOME_LABEL` (`menu.rs:300-303`). Picker con genitore e `PickerLatch` nel menu (`windows/menu_open.rs`).
- Git: runner in `crates/core/src/solo_mode.rs:217-323` (`Git::system()`, `exec(..., Call::Read)` è `pub(crate)`: serve un wrapper pubblico in `weaver_core`); `current_branch()` privato in `solo_merge.rs:169-180` (`git symbolic-ref -q HEAD`, exit 1 = detached), `rev_parse` a 193. Nessun codice di produzione legge `git config user.name`.
- Focus: `windows/mod.rs` `on_run_event` → `manager.on_focused(label)` → `ProjectHooks::window_focused` (`windows/project.rs:22`); `ProjectSetup::window_focused` in `project_setup/merge.rs:526` è il punto dove agganciare la rilettura.
- Watcher: `windows/watch.rs:135-260` (`NotifyWatch`, `route()` scarta tutto fuori da `.weaver`, `Batcher` alimenta solo `Pictures::rebuild`): serve un watch separato sul percorso di `git rev-parse --git-path HEAD` con un suo output.
- Benvenuto: `components/welcome-window.tsx` (chiamate a 74-124, `focus` a 86).
