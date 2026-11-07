---
created: 2026-11-07T11:20:27.137Z
updated: 2026-11-07T23:09:26.148Z
summary: 'Nuova identità grafica: colori di stato a cinque fili, cornice con logo, token e font, sidebar con Usage e switch di progetto, palette dei comandi e viste ridisegnate.'
tasks:
- 6kbjv04d
- 7fi2ycgb
- t1qhoana
- on67l9rz
- mffho9y8
- qhdhe3ub
- 23ssdtz8
- 52yce2ft
- ca432wxp
- fidz5hya
- 2pdjdnkw
- aqw1ayao
- xmrfhnck
- yby4sopj
- 26macxqz
cleared:
  6kbjv04d:
    title: Colori di stato
    refs:
    - task
    - proposal
    - drift
    - app
    - requirements/ui#General
    - requirements/ui#task
    - requirements/ui#proposal
    - requirements/ui#drift
    - requirements/ui#app
    technical: false
    proposal: rn6dcry4
    label: Redesign
    finished: 2026-11-07T10:29:08.926Z
    body: |-
      In tutta l'interfaccia il colore dice solo uno stato, con i cinque fili del logo, ognuno con base, tint e ink: Sky (`#96BBE0` / `#E6EFF8` / `#2E5F8F`), Sage (`#9DC09A` / `#E9F2E7` / `#3F6B3C`), Peach (`#EDB08F` / `#FBEDE4` / `#9A4A22`), Lavender (`#A996D6` / `#EFEAF8` / `#5B4A99`), Rose (`#E3A3B5` / `#F9E8ED` / `#9B3B57`).

      Quando la Task è finita:
      - Sky indica in coda, in corso, in rifinitura e ciò che un agente sta facendo ora (un job che tiene una Task, una lettura in corso); Peach indica in attesa dello sviluppatore, stale, blocked, quasi esaurito, una Drift e gli avvisi; Lavender una Proposal, un question round e le scelte dell'agente; Sage fatto, validato, connesso e successo; Rose fallito, fermato, esaurito, errori e controlli distruttivi; sospeso e ogni altro stato restano neutri (Ink 2 su Lane).
      - Dove un altro Requirement nomina un colore per uno di questi stati vale il filo dello stato; altrimenti blu e ciano diventano Sky, verde Sage, ambra, giallo e il colore di avviso Peach, viola Lavender, rosso e il colore di errore Rose. Il primario blu diventa Ink, tranne i link nel testo e i tag `@` nei campi di testo, che usano l'ink di Sky. I toast di avviso sono Peach, quelli di errore Rose.
      - Le etichette non hanno più colore: sparisce il bordo sinistro colorato delle card delle Task e il badge dell'etichetta (su Task, Proposal e voci di History) è un badge outline col solo testo. Il calcolo del colore dall'etichetta non esiste più.
      - Board: il punto di una Task In progress è Sky; i badge `blocked` e `waiting` sono Peach; il bordo rotante della Task tenuta da un job passa per la base e l'ink di Sky (statico Sky con movimento ridotto). Proposal: `stale` e `blocked` Peach, `refining` Sky; la card di una resolution ha bordo sinistro e tint Lavender; `Refine` è un pulsante primario (Ink) con `Sparkles`, non più giallo. Drift: bordo sinistro da 3 px Peach.
      - Requirements e Specs: sovrapposizione di una Proposal con aggiunto Sage, cambiato Peach, rimosso Rose (ink sulla sua tint), sempre con l'etichetta testuale; le sezioni a fuoco hanno bordo e tint Sky.
      - Nessuno stato è detto dal solo colore: ogni punto, bordo o tint ha testo, badge o icona e un nome accessibile.
      - I test che verificano colori o etichette colorate sono aggiornati o rimossi.

      Riferimento grafico negli Assets: `redesign-foundations.png`/`.html` (fili e loro significato) e `redesign-board.png`/`.html`, `redesign-jobs.png`/`.html` per gli stati in uso.

      ## Code notes

      Commit: 450785e1b59d7321510ae229ca9cc4280a0dc799

      - Il posto naturale della mappa è `ui/src/lib/tones.ts` (`TONES`, `TONE_TEXT`, `TONE_BORDER` scritti per esteso per Tailwind, `SEMANTIC_TONE`). Consumatori: `board/kind-mark.tsx:8-13`, `sidebar.tsx:69`, `omp-status.tsx:137,182,194`, `usage-dialog.tsx:27-28`, `notes/notes-view.tsx:19`, `project/empty-model.tsx:80`.
      - Circa 25 file scrivono classi `*-600 dark:*-400` direttamente: `components/ui/alert.tsx:12-14`, `badge.tsx:16-19`, `button.tsx:24` (variante `refine` gialla), `input.tsx:11`, `sonner.tsx:19-20`, `assets/tagged-textarea.tsx:167`, `settings-view.tsx`, `ask/ask-conversation.tsx:81,89`, `general/general-view.tsx:112,120`, `jobs/jobs-view.tsx:63-71` (mappa `STATE` dei badge dei job), `project/*`, `questions/*`, `sub-agents/sub-agent-ui.tsx:28`, `update-banner.tsx:57`, `views/change-badge.tsx:24-28`, `views/focus-bar.tsx:26-27`, `files/file-tabs.tsx:47`, `assets/asset-grid.tsx:314`, `board/drift-card.tsx:97`, `board/proposal-card.tsx:476`, `board/task-card.tsx:45`.
      - Etichette: `lib/label-colour.ts` e il duplicato `lib/label-tone.ts`; consumatori `board/task-card.tsx:17,141-146,158` (`LabelBadge`, `leftBorderClass`, riusato da `proposal-card.tsx:124,581`) e `history/history-view.tsx:28-41` (seconda copia di `LabelBadge`). Test da rivedere: `lib/label-colour.test.ts`, `board/task-card.test.tsx:13,154-155`, `history/history-view.test.tsx:14,137-139`.
      - Bordo rotante: `.task-held` in `ui/src/index.css:144-162` con `#2563eb`/`#06b6d4` scritti a mano.
  7fi2ycgb:
    title: Cornice e logo
    refs:
    - window
    - requirements/ui#window
    - requirements/release#General
    technical: false
    proposal: rn6dcry4
    label: Redesign
    finished: 2026-11-07T10:32:10.843Z
    body: |-
      La finestra di un progetto ha la cornice del redesign e Weaver ha un nuovo logo, che è anche l'icona dell'app.

      Quando la Task è finita:
      - Il fondo della finestra è Paper. A sinistra la sidebar sta sul fondo; a destra la superficie di lavoro, un pannello bianco staccato di 8 px dai bordi della finestra e dalla sidebar, raggio 14 px e anello da 1 px in Line, contiene la vista scelta.
      - I banner della finestra (avvio, aggiornamento, certificati di sistema, file dell'agente) stanno in cima alla superficie di lavoro, a tutta larghezza, sopra l'header della vista; banner e avvisi propri di una vista restano dentro la vista.
      - La sidebar mantiene in questa Proposal voci, ordine, larghezze e badge di oggi; le voci sono alte 34 px con raggio 9 px, icona e nome in Ink 2; la voce scelta sta su Surface con un anello in Line, nome in Ink e peso medio. Il numero dei job in attesa e il `1` della Chat sono Peach.
      - Il logo di Weaver è il marchio intrecciato del redesign: un quadrato arrotondato color crema (`#FBF6EE`) con un anello in Line, attraversato da un filo orizzontale Sky e da quattro fili diagonali Peach, Lavender, Rose e Sage che intrecciano una «W». È un unico disegno vettoriale, decorativo e nascosto alle tecnologie assistive accanto al nome che accompagna.
      - La finestra di benvenuto mostra il logo sopra il nome dell'app, al posto del marchio lucide.
      - Il sorgente dell'icona dell'app è quel disegno vettoriale, `src-tauri/icons/source.svg`: tela quadrata, quadrato arrotondato pari a 896/1024 di lato, centrato, tutto il resto trasparente. Ogni file di `bundle.icon` e `src-tauri/icons/icon.png` è rigenerato da lì con `tauri icon`; `source.png` non esiste più. L'interfaccia usa lo stesso SVG, mai una copia raster. Dock, barra delle applicazioni, icona nella tray, notifiche e anteprima del trascinamento degli asset mostrano il nuovo logo.
      - Il titolo della finestra resta il nome della cartella del progetto; i test end-to-end continuano a trovare la finestra e le voci della sidebar.

      Riferimento grafico negli Assets: `redesign-sidebar.png`/`.html` (logo nell'SVG del pulsante in alto) e `redesign-board.png`/`.html` (cornice e superficie di lavoro).

      ## Code notes

      Commit: 450785e1b59d7321510ae229ca9cc4280a0dc799

      - Layout: `ui/src/components/project-window.tsx:130-155` (`flex h-full`: `Sidebar`, poi colonna con `StartupBanner`, `UpdateBanner`, `SystemCertificatesBanners`, la prop `banners` e `<main>`; ogni vista è una `section` nascosta quando non scelta). La prop `banners` arriva da `project-content.tsx:104-108` (`OpeningCheckToast`, `SkillsBanner` in `project/update-skills.tsx:116`). Oggi non esiste uno strato carta/superficie.
      - Sidebar: `components/sidebar.tsx` (larghezze `w-[208px]`/`w-[56px]` scritte a mano, test in `sidebar.test.tsx`).
      - Benvenuto: `components/welcome-window.tsx` usa lucide `Blocks` (righe 1 e 133) e impila gli stessi tre banner a 127-131.
      - Icone: `src-tauri/icons/` (`source.png` master 1024 px, `icon.png`, `32x32.png`, `128x128.png`, `128x128@2x.png`, `icon.icns`, `icon.ico`), elencate in `src-tauri/tauri.conf.json:24-30`; usi in `src-tauri/src/windows/tray.rs:92` (`default_window_icon()`) e `src-tauri/src/assets/drag.rs:17` (`include_bytes!("../../icons/32x32.png")`). Nessuno script le genera: si usa `tauri icon` a mano.
      - Il disegno del logo è l'SVG 30×30 della tavola sidebar (`.weaver/assets/redesign-sidebar.html`): rettangolo `#FBF6EE` con bordo `#E7E5DF`, filo orizzontale `#96BBE0`, diagonali `#EDB08F`, `#A996D6`, `#E3A3B5`, `#9DC09A`.
      - e2e: `e2e/lib.mjs` `findProjectWindow`/`nativeWindowTitle` richiedono che il titolo nativo resti il nome della cartella.
  fidz5hya:
    title: Testi senza omp
    refs:
    - requirements/quality#General
    technical: true
    proposal: rn6dcry4
    label: Redesign
    finished: 2026-11-07T10:47:19.690Z
    body: |-
      Fuori dai Settings l'interfaccia non nomina omp: Weaver non nasconde di usarlo, ma non lo ripete ovunque.

      Quando la Task è finita:
      - Il nome `omp` compare solo nella vista Settings (gruppo `omp`, riquadro di stato, provider, elenco dei modelli) e in ciò che omp o l'agente scrivono da sé (messaggi, titoli dei tool, righe d'errore sotto `Show details`).
      - Altrove l'agente si chiama `Weaver` quando parla o agisce (per esempio `Message Weaver…`, `Weaver asks permission`, `Starting Weaver…`) e `the agent` in stati e motivi (per esempio `The agent is not installed.`, `the agent did not answer within 30 s`, `the agent exited with code <N>`).
      - I controlli che agiscono su omp fuori dai Settings restano, con un nome generico (`Install the agent`, `Log in`): la finestra di benvenuto, Get Started, la Board con Model vuoto, la Chat, le conversazioni Ask more e la finestra di Usage non mostrano più il nome.
      - I motivi calcolati dal backend e mostrati sia nei Settings sia altrove (fallimenti di avvio, uscita o risposta, motivi del proxy, `Not delivered: <reason>.`, lettura di Usage non valida, notifica dei certificati di sistema) sono scritti con `the agent`.
      - Un testo dato da un altro Requirement che nomina omp fuori dai Settings si mostra con questa regola applicata.
      - I test unitari che verificano questi testi sono aggiornati.

      ## Code notes

      Commit: 450785e1b59d7321510ae229ca9cc4280a0dc799

      - Riquadro di stato condiviso: `ui/src/components/omp-status.tsx` (`boxFace()` 50-116, `aria-label="omp status"` 159, 192, 235, 259, 274); fuori dai Settings lo usano `usage-dialog.tsx:7,137`, `welcome-window.tsx:7` (`OmpSetup`) e `project/empty-model.tsx:8,56`. Serve una variante di testi per i luoghi fuori dai Settings.
      - Letterali: `general/general-view.tsx` (EMPTY 20, 100, 113-115, 126-127), `ask/ask-conversation.tsx:76,82-84`, `jobs/jobs-view.tsx:234-235,286-287,428`, `usage-dialog.tsx:23,176`, `system-certificates-banners.tsx:11` (`TURN_ON_TEXT`), `lib/system-certificates.ts:37`.
      - Backend (`crates/core`): `notification.rs:33`, `jobs/general.rs:154`, `omp/failure.rs` (66, 69, 100-104, 123-131, 135-136, 280, 285, 312, 315, 316), `omp/lookup.rs:118`, `omp/probe.rs:136,155-157`, `omp/machine.rs:587`, `omp/usage.rs:20`, `jobs/measure.rs:233,236`, `omp/terminal.rs:64,258`. In `src-tauri/src`: `jobs/proc.rs:122,144,280-281`, `jobs/rpc.rs:53,258`, `jobs/session.rs`, `jobs/actor.rs:205-212,492`, `general/actor.rs:317,320,560`, `subagents/reader.rs:83`, `omp.rs:398,871`.
      - Test con questi testi: `general-view.test.tsx:86-155`, `ask.test.tsx:209-214`, `jobs-view.test.tsx:101,111`, `usage-dialog.test.tsx:74-82,192,276-277`, `system-certificates-banners.test.tsx:86,195`, `lib/system-certificates.test.ts:57`, `omp-status.test.tsx:96-131`, `welcome-window.test.tsx:241`.
  t1qhoana:
    title: Token, font e forme
    refs:
    - requirements/ui#General
    - specs#frameworks
    technical: true
    proposal: rn6dcry4
    label: Redesign
    finished: 2026-11-07T10:41:03.390Z
    body: |-
      L'interfaccia di Weaver adotta le fondamenta visive del redesign in tutte le sue superfici, in tema chiaro e scuro, mantenendo i componenti shadcn/ui ridisegnati (nessuna copia al pixel di stili inline).

      Quando la Task è finita:
      - I neutri sono quelli dei Requirements: Paper `#F6F5F1` come fondo della finestra, Surface `#FFFFFF` per la superficie di lavoro, card, dialog, popover e menu, Lane `#FAF9F6` per i riquadri interni, Line `#E7E5DF` per anelli e bordi dei controlli (`#E0DDD5` su pulsanti secondari e input, `#EFEDE7` per i separatori interni), Ink `#18181B`, Ink 2 `#52525B`, Muted `#6B6B73`.
      - Il nero (Ink) è l'unico colore delle azioni: pulsanti primari Ink con testo bianco, secondari bianchi con bordo da 1 px, ghost senza fondo in Ink 2, distruttivi bianchi con testo e bordo Rose. L'anello del focus è Ink. Non resta nessun primario blu.
      - Il tema scuro esiste ancora (Auto, Light, Dark, applicato subito a ogni finestra) e ha gli stessi ruoli, ricavati dai valori chiari: grigi caldi scuri per Paper e Surface (Surface il più chiaro), Line grigio caldo a basso contrasto, Ink quasi bianco con pulsanti primari chiari e testo scuro; i fili mantengono la tinta con tint più scure e ink più chiari. In entrambi i temi valgono i contrasti dei Requirements di qualità (4,5:1 per il testo, 3:1 per icone, bordi e focus).
      - Raggi 6 px per id, chip e codice inline, 9 px per pulsanti, input, voci di menu e della sidebar, 12 px per card, riquadri e righe di lista, 14 px per superficie di lavoro, colonne della Board e dialog; badge e pill completamente arrotondati. Card e riquadri hanno un anello da 1 px in Line al posto del bordo; la card appoggiata su una corsia aggiunge un'ombra leggera; dialog, popover e menu un'ombra più ampia.
      - Il font è Geist a 14 px (pesi 400, 500, 600) e Geist Mono per id, percorsi, comandi e codice, compreso l'editor della vista Files; entrambi inclusi nell'app e mai caricati dalla rete. I titoli hanno una spaziatura leggermente negativa.
      - L'header di ogni vista è alto 76 px più il bordo da 1 px (`#EFEDE7`), con 28 px di margine laterale, titolo a 19 px semibold, sottotitolo facoltativo a 13 px Muted e controlli alti 36 px.
      - Gli id delle card sono in monospace su un'etichetta Paper.
      - Le superfici non disegnate nel redesign (Settings, Files, Assets, Notes, dialog, toast, banner) mantengono contenuti e disposizione e prendono questi token, forme e font.
      - I test unitari e i test end-to-end che dipendono da altezze, classi o testi cambiati sono aggiornati; i test end-to-end continuano a trovare le voci della sidebar per nome accessibile.

      Riferimento grafico negli Assets del progetto: `redesign-foundations.png` e `redesign-foundations.html` (valori esatti), più le altre tavole `redesign-*.png`/`.html` per vedere i token applicati.

      ## Code notes

      Commit: 450785e1b59d7321510ae229ca9cc4280a0dc799

      - Token: `ui/src/index.css` (`:root` 6-29, `.dark` 31-49, `@theme inline` 51-77 con `--font-sans`/`--font-mono` 71-72 e `--radius` 73-76; `body` `font-sans text-sm` a 87). Nessun file di config Tailwind: Tailwind v4 legge `@theme inline`.
      - Primario blu consumato da `components/ui/button.tsx:15`, `badge.tsx:13`, `checkbox.tsx:15`, `progress.tsx:13`, `radio-group.tsx:23`, `switch.tsx:14`, `toggle-group.tsx:26`, e da `markdown.tsx:72`, `files/file-tabs.tsx:56`, `notes/notes-view.tsx:114`, `questions/question-index.tsx`, `question-pane.tsx`, `assets/tagged-textarea.tsx:198`.
      - Font: nessun `@fontsource` in `ui/package.json`; la CSP in `src-tauri/tauri.conf.json:16-17` ha già `font-src 'self'` (niente `data:`). CodeMirror ha font e palette proprie in `components/files/editor-theme.ts:29-56,88`; `editor-theme.test.ts` controlla il contrasto.
      - Tema: `components/theme-provider.tsx:45-47` mette `.dark` su `<html>`; mermaid segue il tema in `mermaid-block.tsx:34`.
      - Header: `components/view-header.tsx:11` `VIEW_HEADER_CLASS` (`h-14`, `px-4`), usato anche direttamente in `jobs/jobs-view.tsx:481`; test che fissano `h-14`: `view-header.test.tsx:31`, `jobs/jobs-view.test.tsx:69`, `settings-view.test.tsx:234`.
      - e2e: `e2e/specs/smoke.e2e.mjs` trova le voci con `entry(name)` per testo o `aria-label`; non dipende da colori o classi.
  mffho9y8:
    title: Sidebar e switch di progetto
    refs:
    - window
    - developer.OPEN_PROJECT
    - requirements/ui#window
    - requirements/tech#window
    - requirements/tech#project
    - requirements/ui#file-view
    technical: false
    proposal: jvad01rm
    label: Redesign
    finished: 2026-11-07T10:51:20.981Z
    body: |-
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
  on67l9rz:
    title: Usage nella sidebar
    refs:
    - usage
    - time
    - time.USAGE_REFRESH_DUE
    - developer.OPEN_USAGE
    - project-setup
    - requirements/ui#usage
    - requirements/tech#usage
    technical: false
    proposal: jvad01rm
    label: Redesign
    finished: 2026-11-07T10:34:31.788Z
    body: |-
      Ogni finestra di progetto tiene in vista il limite d'uso più vicino all'esaurimento, e Weaver rilegge i limiti da solo ogni cinque minuti.

      Quando la Task è finita:
      - Weaver legge i limiti quando lo sviluppatore apre Usage, a ogni `Retry` e da solo con un unico timer per tutta l'app: subito, appena omp è pronto senza letture dall'avvio, poi 5 minuti dopo l'inizio dell'ultima lettura, qualunque l'abbia avviata. Il tick si salta, senza ritardarlo, quando omp non è pronto o nessuna finestra di Weaver è sullo schermo (tutte in background o ridotte a icona). Una lettura alla volta per tutta l'app: apertura, `Retry` o tick durante una lettura aspettano quella. Non si chiama mai un modello e ci si appoggia alla cache di omp, mai ai provider direttamente.
      - Con Usage chiusa, la lettura da sola aggiorna il riepilogo; con Usage aperta su limiti mostrati o su un errore, il tick riporta Usage in lettura tenendo visibili i limiti precedenti, segnati come in aggiornamento.
      - Il riepilogo nella sidebar è un riquadro bianco con anello, tutto un pulsante che apre Usage: `Usage` e la percentuale usata del limite più vicino all'esaurimento, una barra da 6 px neutra, Peach se quasi esaurito, Rose se esaurito, e sotto nome del limite e reset (`Claude 5 Hour · Resets in 2 h 10 min`). Il limite scelto è quello con lo stato peggiore (esaurito, poi quasi esaurito, poi gli altri) e, a parità, la frazione usata più alta, poi il reset più vicino, poi l'ordine di provider e limiti; i limiti con frazione sconosciuta contano solo se non ce ne sono altri, e allora niente percentuale né barra. Il calcolo sta nel core.
      - Senza limiti letti il riepilogo dice `Reading usage…` durante una lettura, altrimenti `Open to read usage`; senza limiti nei provider connessi `No usage limits`; con l'agente non pronto `Usage needs the agent`; dopo un fallimento senza limiti mai letti `Could not read usage`. I limiti letti prima restano sempre visibili. Ogni finestra lo aggiorna appena una lettura finisce, perché l'ultimo risultato è uno solo per tutta l'app.
      - Usage si apre dal riepilogo o da `Usage` nella palette; alla chiusura il focus torna dove era. A sole icone il riepilogo è un pulsante `Gauge` con tooltip e nome accessibile che riportano il suo testo.
      - Il test che vietava le letture da sole è sostituito da test sul timer (tick saltati senza omp pronto o senza finestre visibili, una lettura alla volta) e sul calcolo del riepilogo.

      Riferimento grafico negli Assets: `redesign-sidebar.png`/`.html` (riquadro «Usage this week»: qui mostra il limite più vicino all'esaurimento invece di un totale settimanale, che omp non fornisce).

      ## Code notes

      Commit: 450785e1b59d7321510ae229ca9cc4280a0dc799

      - Lettura: `Ops::usage` (30 s) → `weaver_core::omp::usage::read_usage` (`src-tauri/src/omp.rs:639`); stato in `src-tauri/src/usage.rs` (`UsageState`, `last: Option<(omp_path, Usage)>`, `UsageStatus{revision, phase, failure, usage}`); `UsageState::begin(omp_path)` si unisce a una lettura in corso; `OmpSetup::begin_usage()` (`omp.rs:300`) restituisce `None` se omp non è Ready; evento `usage-changed` a tutte le finestre (`events.rs:203`).
      - Divieto attuale: test `nothing_is_read_while_omp_is_not_ready_or_by_itself` (`omp.rs:1602-1615`) e il commento di `ui/src/hooks/use-usage.ts`.
      - Timer: `Scheduler::repeat(first, period, tick)` con `AbortOnDrop` e `tokio::time::sleep` (`omp.rs:694-721`), `FakeScheduler.live_timer(period)` per i test; altro esempio `updates.rs:489` `run_schedule`.
      - Finestre sullo schermo: il `Registry` sa quali sono in background (`has_background`, `is_background`); per «ridotta a icona» `notifications.rs` legge già `is_visible`/`is_minimized` di ogni finestra.
      - UI: `components/usage-dialog.tsx` (`STATUS_BAR` 28, testi 23 e 176), apertura e ritorno del focus in `project-window.tsx` (`setUsageOpen`, `focusUsageEntry`).
  qhdhe3ub:
    title: Palette dei comandi
    refs:
    - app
    technical: false
    proposal: jvad01rm
    label: Redesign
    finished: 2026-11-07T10:55:25.067Z
    body: |-
      Da qualunque punto della finestra di un progetto una palette trova una vista, un'azione, un progetto, una card, una sezione dei Requirements o una voce di History e ci va o la esegue esattamente come il suo controllo, con le stesse conferme e gli stessi motivi di rifiuto; non cerca mai nei file del progetto, negli asset o nelle note.

      Quando la Task è finita:
      - `Cmd+K` su macOS e `Ctrl+K` su Windows e Linux aprono la palette ovunque nella finestra, anche con il focus nell'editor della vista Files, nelle Notes o in un campo di testo; anche il pulsante `Search or run…` della sidebar la apre. Premuto di nuovo, la chiude.
      - È un Dialog senza barra del titolo, largo 560 px, nel terzo superiore della finestra: un campo di ricerca con il segnaposto `Search or run…` e i risultati nella stessa lista con ricerca del dialog di Merge. Le frecce muovono, Invio sceglie, Esc chiude e restituisce il focus a dove era.
      - I risultati sono a gruppi, in quest'ordine: `Views` (ogni voce della sidebar e `Settings`), `Actions` (`Scan System`, `Run all`, `New Work Item`, `Open Folder…`, `Usage`), `Projects` (come gli elenchi dello switch di progetto), `Cards` (Proposal, Task e Drift della Board, numero breve e titolo), `Requirements` (sezioni, con Area e frammento in Muted), `History` (voci, titolo e data). Con il campo vuoto si vedono solo `Views`, `Actions` e `Projects`.
      - La ricerca non distingue maiuscole e minuscole e guarda nomi, titoli e numeri brevi; prima ciò che inizia con il testo; al massimo 8 righe per gruppo.
      - Scegliere una vista la mostra; un'azione si esegue come il suo controllo, con la stessa conferma (per esempio `Scan System`), mentre un'azione disattivata mostra i suoi motivi in Muted e non si può scegliere; un progetto si apre come dallo switch; una card mostra la Board e apre il suo dialog di dettaglio; una sezione mostra i Requirements scorsi fino a lei; una voce di History mostra la History, sul periodo `All` se quello scelto la nasconde, e apre la voce. La palette si chiude a ogni scelta.
      - Test unitari coprono apertura da tastiera anche dentro l'editor, ordine dei gruppi, filtro e scelta di un'azione disattivata.

      Riferimento grafico negli Assets: `redesign-sidebar.png`/`.html` (pulsante `Search or run…`); la palette non è disegnata e segue le Foundations.

      ## Code notes

      Commit: 450785e1b59d7321510ae229ca9cc4280a0dc799

      - Nessun binding `Mod-k` in `ui/src`; CodeMirror (`files/editor-state.ts:57`) lega `Shift-Mod-k`, non `Mod-k`. TipTap usa `Mod-Enter` (`notes-extensions.tsx:239`); `isModifier(event)` in `notes/notes-model.ts:97`. Un listener `keydown` in cattura su `ProjectWindow` arriva prima degli editor. Attenzione: `files/file-tabs-context.tsx:643-667` ignora i tasti mentre è aperto un `[role="dialog"]`.
      - Lista: `components/ui/command.tsx` (lista con ricerca fatta in casa, usata dal dialog di Merge).
      - Dati già caricati: card da `useBoard().snapshot` (`board/board-state.tsx`, `BoardCard.head{id, shortNumber, title}`; Task in `active`, Proposal e Drift in `backlog`); Requirements da `useViews().snapshot.data.requirements` (`views/views-state.tsx`, disponibile solo dentro `around`); History da `useHistory(projectId)` (`history/use-history.ts`, hook locale, non contesto); job da `useJobs()`.
      - Navigazione e azioni: `useProject()` (`showView`, `openJob`) in `project-context.tsx`; Scan e Run all in `jobs/board-actions.tsx` (`scan_system`, `run_all`); New Work Item da `requests/requests-layer.tsx` (`useRequests().newRequest`); apertura del dettaglio di una card con `useBoardUi().openDetails`.
      - Progetti: usare lo stesso comando ed evento dello switch di progetto (Task della stessa Proposal).
  23ssdtz8:
    title: History ridisegnata
    refs:
    - history-entry
    - requirements/ui#history-entry
    technical: false
    proposal: e8oe9509
    label: Redesign
    finished: 2026-11-07T10:24:55.672Z
    body: |-
      La vista History prende il disegno del redesign, con i totali del periodo scelto.

      Quando la Task è finita:
      - L'header ha il sottotitolo `Everything that shipped, grouped as it was delivered.` e a destra il periodo: `Today`, `Last 7 days` (predefinito a ogni apertura della finestra), `Last 30 days`, `All time` (al posto di `All`), con i limiti di oggi.
      - Sotto, tre riquadri calcolati sulle voci del periodo: `Delivered` (numero di voci), `Tasks done` (Task citate), `Last delivery` (`Today, HH:MM`, `Yesterday, HH:MM` o la data scritta per esteso); `—` quando il periodo è vuoto.
      - Le voci, dalla più recente, raggruppate per giorno con intestazione `Today`, `Yesterday` o il giorno della settimana, seguita in Muted da data e `· <n> entries` (`· 1 entry`). Ogni gruppo è un elenco con anello: striscia neutra a sinistra, titolo, riassunto su al massimo due righe con il testo intero nel tooltip, a destra l'ora in monospace e una pill Sage `<n> tasks` (`1 task`).
      - Vuoto: `Nothing done in this period`. Il dialog della voce resta com'è, con l'etichetta delle Task come testo in un badge outline.
      - I test della vista History sono aggiornati.

      Riferimento grafico negli Assets: `redesign-history.png` e `redesign-history.html` (le strisce colorate della tavola diventano neutre).

      ## Code notes

      Commit: 450785e1b59d7321510ae229ca9cc4280a0dc799

      - Vista: `ui/src/components/history/history-view.tsx` (`HistoryView({now})`, `Select` del periodo nell'header, `DayList`/`EntryRow`, `EntryDialog` con `TaskRow` e la sua copia di `LabelBadge` a 28-41).
      - Modello: `history/history-model.ts` (`PERIODS`, con «All» da rinominare `All time`; `DEFAULT_PERIOD = "last-7-days"`; `groupByDay(entries, period, now)` → `DayGroup` con `header` tipo `Today · 2`; `inPeriod`, `entryTime` su `updated`; `plainSummary`).
      - Dati: `history/use-history.ts` `useHistory(projectId)`; `ui/src/bindings/HistoryEntry.ts` (`tasks: HistoryTask[]`): i totali si calcolano nella UI dalle voci filtrate (numero di voci, somma di `tasks.length`, massimo di `updated`).
      - Test: `history/history-view.test.tsx` (anche `labelTone` a 137-139).
  2pdjdnkw:
    title: Jobs ridisegnati
    refs:
    - job
    - requirements/ui#job
    technical: false
    proposal: e8oe9509
    label: Redesign
    finished: 2026-11-07T10:31:07.885Z
    body: |-
      La vista Jobs prende il disegno del redesign: elenco filtrabile a sinistra, conversazione completa del job a destra.

      Quando la Task è finita:
      - In cima all'elenco un controllo a tre parti: `All <n>` (predefinito a ogni apertura della finestra), `Active <n>` (job in coda, in corso, in attesa o sospesi, e conversazioni Ask more il cui agente lavora o attende) e `Ended <n>` (job fatti, falliti o fermati). Le conversazioni Ask more inattive compaiono solo in `All`. Un filtro vuoto dice `No jobs here`; il job selezionato resta selezionato anche se il filtro ne nasconde la riga.
      - L'elenco sta su Lane; ogni riga mostra nome, tipo in Muted, pill di stato a destra, l'ultima nota su al massimo due righe, l'ora di avvio e il tempo di lavoro; la riga selezionata sta su Surface con anello e ombra leggera. L'albero dei sotto-agenti sotto la riga selezionata resta com'è oggi.
      - Le pill di stato: `queued` e `suspended` neutre, `running` Sky con il punto, `waiting` Peach, `done` Sage, `failed` e `stopped` Rose, sempre con testo e icona.
      - A destra, l'header del job: nome, pill di stato con il tempo di lavoro per un job che lavora, `Started today at HH:MM` (o la data) e `Stop job` (distruttivo) finché il job è in coda, in corso, in attesa o sospeso, con la conferma di oggi. Lo `Stop` accanto a `Send` non c'è più.
      - La conversazione è quella completa di oggi, ridisegnata: la prima voce è l'istruzione di Weaver in un riquadro comprimibile `Instructions from Weaver`, chiuso all'inizio; messaggi dello sviluppatore in bolle su Paper; messaggi dell'agente, `Thinking`, righe dei tool, sotto-agenti, permessi, note, messaggi in coda e riga di fine con le regole di oggi. Nessuna percentuale di avanzamento, nessun conteggio di file cambiati o di controlli, nessuna riga sul batch.
      - Il campo per scrivere al job è un riquadro con anello, ombra e raggio 14 px, segnaposto `Steer the agent while it runs…`, invio con un pulsante primario a icona.
      - I test della vista Jobs sono aggiornati, compresi filtro e posizione di `Stop job`.

      Riferimento grafico negli Assets: `redesign-jobs.png` e `redesign-jobs.html` (la timeline riassunta, la percentuale del sotto-agente, «3 files changed so far», «checked 5 times» e la riga «batch» della tavola non vanno implementati).

      ## Code notes

      Commit: 450785e1b59d7321510ae229ca9cc4280a0dc799

      - Vista: `ui/src/components/jobs/jobs-view.tsx` (`JobsView`, `<nav aria-label="Jobs">` da 300 px, `<section aria-label="Conversation">`; `Row`, `StateBadge` esportato, mappa `BADGES` 63-71; `Conversation` con `useConversation(projectId, jobId)` da `jobs/use-jobs.tsx`, `FollowScroller`; `EntryView` esportato e condiviso con Chat e Ask more: cambiarne l'aspetto tocca anche loro; `MessageBox` con `TaggedTextarea`, Stop con `ConfirmDialog` → `job_stop`; `DisabledMessageBox`).
      - Ordine delle righe: `mergeRows(jobs, asks)` in `ask/ask-model.ts`; righe Ask more `AskListRow`. Nessun filtro esiste oggi.
      - L'istruzione arriva come `Entry {kind:"instruction", text}` (`ui/src/bindings/Entry.ts`), oggi resa come riquadro etichettato `Weaver` nel caso `instruction` di `EntryView`.
      - Sotto-agenti: `sub-agents/sub-agent-ui.tsx` (`SubAgentList`, `SubAgentLine`, `AgentState`), `sub-agents/api.ts`.
      - Test: `jobs/jobs-view.test.tsx` (fissa anche `h-14` a 69).
  52yce2ft:
    title: Requirements e Specs ridisegnati
    refs:
    - app
    - requirements/ui#app
    technical: false
    proposal: e8oe9509
    label: Redesign
    finished: 2026-11-07T10:39:53.453Z
    body: |-
      Le viste Requirements e Specs prendono il disegno del redesign, con navigazione e ricerca nei Requirements e nessun colore che non dica uno stato.

      Quando la Task è finita:
      - Requirements: l'header ha il sottotitolo `What the app must do, in plain words. Agents build from this.` e a destra il campo `Search requirements`, `Ask more` e `Ask for a change` (primario). `Ask for a change` mostra la Chat con il campo di testo a fuoco che contiene `Change to the Requirements: ` col cursore in fondo; il testo già presente resta e il prefisso va a capo dopo di esso; nulla parte finché lo sviluppatore non invia.
      - A sinistra una navigazione di circa 220 px: per ogni Area il titolo come intestazione maiuscola in Muted, poi un'entrata per frammento con il numero delle sue sezioni; una pill Lavender `changed` sui frammenti che la Proposal in sovrapposizione aggiunge, rimuove o cambia; la scelta di un'entrata scorre il frammento in cima, e l'entrata del frammento in cima è evidenziata.
      - A destra, larghe al massimo 900 px, le Aree e i frammenti: titolo del frammento a 22 px, percorso in un'etichetta monospace e `Open in editor`; le sezioni in un riquadro con anello, una riga per sezione con il titolo in Geist Mono in una colonna di circa 140 px e il testo accanto (sotto, quando la riga è stretta), `Open in editor` visibile al passaggio e al focus. Nessun colore per Area o sezione.
      - La ricerca filtra sul posto, senza distinguere maiuscole e minuscole: restano solo le sezioni il cui titolo o testo contiene le parole, con le corrispondenze evidenziate; frammenti e Aree vuoti spariscono, i conteggi della navigazione contano solo ciò che resta; senza risultati `No sections match.`; Esc o campo vuoto mostrano tutto. Si combina con il focus e `Only these`. Non cerca altro che i Requirements.
      - Barra di focus, sovrapposizione, etichette dei cambiamenti e `Show before` restano, nel nuovo stile.
      - Specs: sottotitolo `The stack the agents build with · specs.yaml`; a destra le pill `All <n>` e `Agent's choice <n>`, che mostra solo le voci scelte dall'agente. Ogni campo non vuoto, nell'ordine del formato delle Specs, con titolo, numero di voci, una riga e `Open in editor`, che apre il file delle Specs alla riga del campo. Voci in griglia (colonne di almeno 250 px, al massimo 1180 px): riquadro neutro con l'iniziale, nome, versione come etichetta monospace, nota e, per le scelte dell'agente, la pill Lavender `Agent's choice` col motivo nel tooltip. Le note in un riquadro con anello. Nessun attributo «Local only».
      - I test delle due viste sono aggiornati; i test end-to-end trovano ancora i testi dei Requirements della fixture.

      Riferimento grafico negli Assets: `redesign-requirements.png`/`.html` e `redesign-specs.png`/`.html` (i colori per Area e per voce e il filtro «Local only» delle tavole non vanno implementati).

      ## Code notes

      Commit: 450785e1b59d7321510ae229ca9cc4280a0dc799

      - Requirements: `ui/src/components/views/requirements-view.tsx` (`RequirementsView` → `ViewFrame` con `toolbar={<AskRequirementsButton/>}` → `RequirementsPage` su `snapshot.data.requirements`). Tipi in `ui/src/bindings/AreaView.ts`, `FragmentView.ts`, `SectionView.ts` (sezioni per frammento = `fragment.sections.length`; «changed» = `fragment.change` o una `section.change` con `snapshot.overlay` non nullo).
      - Focus e sovrapposizione: `views/views-state.tsx` (`useViews()`: `snapshot`, `overlay`, `focus[view]`, `goStep`, `clearFocus`, `setOnly`, `focusOn`), `views/focus-model.ts` (`planFocus`, `sectionKey`), passi trovati con `data-step-key`; `views/focus-bar.tsx`; colori in `views/change-badge.tsx`.
      - `Ask for a change`: il testo del campo della Chat è un `useState("")` locale in `general/general-view.tsx:37`; `ProjectContext` (`project-context.tsx`) ha solo `showView`, `openJob`, `openedJob`: serve un canale nuovo (bozza + nonce), sul modello di `openedJob`.
      - `OpenInEditor({file, line})` in `components/open-in-editor.tsx` (`open_in_editor`, percorso relativo alla radice del progetto).
      - Specs: `views/specs-view.tsx` (`SpecsPage` su `snapshot.data.specs?.fields`; `Field` e `Entry` con `chosenReason`); tipi `bindings/SpecsFieldView.ts` (`file`, `line` per campo) e `SpecEntryView.ts`; il backend omette già i campi vuoti (`crates/core/src/project/view/docs.rs:300-397`).
      - Test: `requirements-view.test.tsx`, `specs-view.test.tsx`; e2e cerca `Product`, `Greeting`, `The greeting is always shown in English.`.
  ca432wxp:
    title: Board ridisegnata
    refs:
    - work
    - task
    - proposal
    - drift
    - requirements/ui#work
    - requirements/ui#task
    - requirements/ui#proposal
    technical: false
    proposal: e8oe9509
    label: Redesign
    finished: 2026-11-07T10:52:18.340Z
    body: |-
      La Board prende il disegno del redesign mantenendo le sue due colonne, Backlog e Active, e tutte le decisioni di oggi.

      Quando la Task è finita:
      - Le due colonne sono corsie (fondo Lane, anello interno, raggio 14 px) con un'intestazione fatta di un punto da 8 px (Lavender per Backlog, Sky per Active), titolo e numero delle card reali in Muted. Non c'è nessuna colonna Done.
      - L'header della Board ha il sottotitolo `<n> active · <m> done since last scan` (ogni parte omessa quando è 0) e a destra, in ordine, `Merge` con la Solo mode attiva, `Scan System` (secondario, con la conferma di oggi) e `Run all` (primario, `Play`) con dentro `<n> ready`, le Task in To do non bloccate. I motivi di disattivazione di `Run all` e di `Scan System` restano quelli di oggi. I piè di colonna con `+ Work Item` e `Run all` non esistono più.
      - L'intestazione del Backlog termina con il pulsante a icona `New Work Item` (`Plus`), che apre una nuova design request. Il Backlog vuoto mostra il disegno a fili, `Nothing to decide`, `New work items and Proposals from Chat land here for your go-ahead.`, `Work item` (apre una nuova design request) e `Ask in Chat` (mostra la Chat). Active vuoto mostra `Nothing active`.
      - La card della Task: id, etichetta in Muted e, a destra, la pill di stato (`To do` neutra; `In progress` Sky; `Agent working · <tempo del job>` mentre un job la tiene, che scorre ogni secondo); titolo e primo paragrafo del corpo in testo semplice, al massimo 3 righe; chip `technical`, `blocked`, `waiting <id>` e un chip per ogni frammento di Requirements e campo di Specs nominati dai riferimenti (es. `stack.md`, `specs.yaml`), ciascuno apre il focus del primo riferimento che rappresenta; dopo una riga, il piè: mentre un job la tiene il nome di quel job, `View requirements` e `Open job`, che mostra Jobs con quel job selezionato; altrimenti `View requirements` e `Run`. Nessuna fase del lavoro è mostrata. Il bordo rotante resta per la Task tenuta da un job.
      - Le card di Proposal e Drift, le card fantasma, il banner di Merge, gli avvisi e i dialog di dettaglio hanno lo stesso aspetto (Surface, anello, ombra leggera, raggio 12 px, id come etichetta, titolo a 15 px, controlli in un piè dopo una riga) con i contenuti, i controlli e i motivi di oggi.
      - Il backend fa sapere alla Board quale job tiene una Task, così che la card mostri il nome e il tempo del job e `Open job` lo selezioni.
      - I test della Board e delle card sono aggiornati; i test end-to-end trovano ancora `Backlog`, `Active` e la card della Task.

      Riferimento grafico negli Assets: `redesign-board.png` e `redesign-board.html` (la colonna Done e le fasi della tavola non vanno implementate).

      ## Code notes

      Commit: 450785e1b59d7321510ae229ca9cc4280a0dc799

      - Layout: `ui/src/components/board/board-view.tsx` (`BoardView`, due `Column` in `grid-cols-2`; header con `<MergeButton/>`, Scan in `GuardedButton` + `ConfirmDialog` → `actions.scanSystem`, testo `done since last scan`; sotto `MergeBanner`, `NoticeStack`, avvisi di History); `board/column.tsx` (sezione con `rounded-lg border bg-muted/30`, `ScrollArea`, piè fisso); `BoardFooterButtons` usa `snapshot.runAllReasons`. «n ready» si calcola nella UI come in `crates/core/src/project/board.rs:140` `runnable()` (`!doing && blockedBy` e `waitingFor` vuoti). Nessun istante dell'ultimo scan esiste.
      - Nuova design request: `requests/requests-layer.tsx` (`useRequests().newRequest`); mostrare la Chat: `useProject().showView("general")`; aprire un job: `useProject().openJob(jobId)` (`project-context.tsx`, nessun nonce: riaprire lo stesso job non riattiva l'effetto in `JobsView`).
      - Card: registro `board/card-registry.ts` (`registerCardRenderer`), `board/default-cards.tsx`, guscio comune `board/card-shell.tsx` `CardShell` (slot `cardId`, `shortNumber`, `title`, `mark`, `accent`, `badges`, `body`, `controls`, `lead`; click fuori dai controlli → `useBoardUi().openDetails`). Task: `board/task-card.tsx` (`StatusDot`, `taskStatus()`, `RefsPopover`, `TaskControls` con `FocusButton` e Run, `TaskDetails` con `RefChip`); Drift `drift-card.tsx`; Proposal `proposal-card.tsx` (`ProposalControls` 335, resolution 475); fantasmi `ghost-card.tsx`; `kind-mark.tsx`.
      - Job che tiene la Task: la Task ha solo `heldByAgent: boolean` (`ui/src/bindings/BoardCard.ts`), impostato in `src-tauri/src/board.rs:142,337` da `facts.holds(task)`; il backend conosce il job dalla chiave `holder` del file (`crates/core/src/project/board.rs:69-73` `holds()`). Aggiungere un campo con l'id del job (ts-rs rigenera il binding); nome e tempo si prendono da `useJobs().jobs` (`JobRow`).
      - Test: `board-view.test.tsx`, `task-card.test.tsx`; e2e `e2e/specs/smoke.e2e.mjs` cerca `Backlog`, `Active` e il titolo della card.
  aqw1ayao:
    title: Chat ridisegnata
    refs:
    - general-chat
    - requirements/ui#general-chat
    technical: false
    proposal: aih5exkj
    label: Redesign
    finished: 2026-11-07T10:53:23.657Z
    body: |-
      La Chat prende il disegno del redesign e mostra le Proposal consegnate dai job che ha avviato, da decidere sul posto come sulla Board.

      Quando la Task è finita:
      - Header: titolo `Chat`, la pill `Weaver · <stato>` con lo stato dell'agente per tutta l'app (`ready` Sage; `checking`, `installing`, `updating`, `logging in` Sky; `not installed`, `not connected` Peach), con un tooltip su cosa fare, mai il nome omp; a destra `New chat`, che è il Clear di oggi con la conferma `Start a new chat?`. La Chat resta una sola conversazione persistente.
      - Conversazione centrata, larga al massimo 760 px: messaggi dello sviluppatore in bolle su Paper; messaggi dell'agente con logo, `Weaver` e ora `HH:MM`. Ogni sequenza consecutiva di `Thinking`, righe dei tool e righe dei sotto-agenti di un turno è un riquadro comprimibile, chiuso all'inizio, `Worked for <durata> · <n> steps` (`Working · <n> steps` con il punto Sky mentre è l'ultima cosa di un turno in corso); aprendolo si vedono le righe come oggi. Permessi, avvisi e messaggi non finiscono mai dentro il riquadro. Il backend registra l'istante di arrivo di ogni voce, da cui vengono durata e ora.
      - Sotto la riga `Started <job name>` di un job avviato dalla Chat, o del job della richiesta che la Chat ha inviato, c'è una card per ogni Proposal che quel job ha consegnato, dal momento della consegna e anche dopo il ricaricamento della conversazione, finché la Proposal esiste; mai per una Proposal attaccata a una Drift. La card segue la Proposal dal vivo: numero, pill di stato, badge `stale` e `blocked`, titolo; i file aggiunti, cambiati e rimossi (al massimo 6, poi `and <n> more`); le Task che crea; i motivi di blocco in un riquadro Peach.
      - Il piè della card ha gli stessi controlli, motivi di disattivazione e dialog della card sulla Board: `Approve` o `Refine`, `Reject` con `Rework with feedback…`, e per una Draft lasciata a metà da una rifinitura `Refine` e `Discard`. Niente scelta del modello, niente `View requirements`, niente diff, niente Undo: l'approvazione è definitiva. Decisa la Proposal, il piè diventa una riga (`Approved. <n> Tasks are on the Board.` con `Open Board`, `Rejected.`, `Sent back for rework.` con `Open conversation`, `Discarded.`); una decisione rifiutata quando si applica mostra il motivo sotto i controlli e non cambia nulla.
      - Mentre la Chat aspetta le risposte del suo round, la conversazione finisce con la riga `Weaver is waiting for your answers.` e `Answer`, che riporta in primo piano il round ridotto a barra; il badge `1` della Chat nella sidebar resta.
      - Campo di testo: riquadro con anello, ombra e raggio 16 px, segnaposto `Message Weaver… use @ to add assets`, tag degli asset come oggi, nessun pulsante per allegare file e nessun suggerimento; a destra il controllo del modello, poi `Send` o, durante un turno, `Stop`. Il controllo del modello mostra il modello Analysis dei Settings (`Default model` se vuoto) e apre la stessa scelta del Settings: è quel cambio di impostazione, salvato subito per tutti i progetti. È disattivato durante un turno, un caricamento o un round. Cambiare il modello Analysis, dalla Chat o dai Settings, mentre omp della Chat è avviato e nessun turno è in corso chiude omp tenendo la conversazione: il messaggio successivo lo riavvia col nuovo modello e ricarica la conversazione.
      - Testi senza omp: `Think with Weaver about this project. Ask it for a Proposal or a job when you want one.`, `Starting Weaver…`, `The agent stopped: <reason>.`, `Your next message starts the agent again.`, `The agent is not installed.` con `Install the agent`.
      - I test della Chat sono aggiornati, compresi raggruppamento dei passi, card della Proposal con i suoi controlli e il cambio di modello.

      Riferimento grafico negli Assets: `redesign-chat.png` e `redesign-chat.html` (i chip di suggerimento, il pulsante per allegare, Undo, View diff e le sezioni «Cosa decide», «Da sapere», «Da chiarire» della tavola non vanno implementati).

      ## Code notes

      Commit: 450785e1b59d7321510ae229ca9cc4280a0dc799

      - Vista: `ui/src/components/general/general-view.tsx` (EMPTY 20, LOADING 21, `lastQueued()` 24, `clearButton` `GuardedButton` + `ConfirmDialog` → `general_clear`, composer `TaggedTextarea` con `aria-label` «Message omp», Stop durante `state.working`, `SubAgentConversationView` con `ownerName="Chat"`); stato in `general/use-general.tsx` (`useGeneral`, `general_snapshot`, batch `general-entries`, `general-changed`, `useChatWaiting`). La conversazione usa `EntryView` di `jobs/jobs-view.tsx`, condiviso con Jobs e Ask more: il riquadro comprimibile va costruito nella Chat sulle sequenze di voci `thinking`/`tool`, senza rompere gli altri.
      - Le voci non hanno istanti (solo `note` ha `at`): `ui/src/bindings/Entry.ts` e il backend in `src-tauri/src/general/service.rs` / `jobs/conversation.rs` vanno estesi.
      - Job avviati: voci `job-started` (`job`, `name`), create in `general/service.rs:1107` `started_jobs()` e, per una richiesta inviata, `requests.rs:394` `agent_send_locked` + `jobs/authority.rs:33`.
      - Legame Proposal → job: chiave `job:` nel front matter della Proposal (`crates/core/src/project/proposal/create.rs:238`, letta da `proposal/data.rs:168` `job_of()`); non arriva alla UI: `CardHead` in `src-tauri/src/board.rs` (~62) ha solo `agent_finishing`. Lo snapshot della Board elenca solo Proposal Open, Refining e interrotte: per mostrare anche quelle decise serve un dato proprio della Chat.
      - Decisioni: `board/proposal-card.tsx` (`ProposalControls` 335, `RejectDialog` 220, `ReworkDialog` 257, `BlockedPopover` 51, `FilesLine` 103, `TasksLine` 117); azioni in `board/board-state.tsx` (`approveProposal`, `rejectProposal`, `discardProposal`, `decide()` manda i rifiuti tra gli avvisi della Board: in Chat il motivo va mostrato nella card). `BoardProvider` avvolge tutta la finestra (`project-content.tsx:135`). `plan_discard` accetta solo Draft (`crates/core/src/project/proposal/transitions.rs:560`).
      - Round: `questions/question-overlay.tsx`, `use-question-round.tsx` (`useOptionalQuestionRound()`, `act({kind:"minimize", minimized:false})` come `ask-conversation.tsx`); `GeneralState.waiting`.
      - Modello: la Chat usa `models.analysis` (`crates/core/src/jobs/general.rs:561` `args()`), letto all'avvio in `src-tauri/src/general/actor.rs` `launch()`; scelta dei modelli in `settings-view.tsx` (`ModelSetting settingKey="models.analysis"`, 269) e `omp-status.tsx:330` `modelCatalogOf`; salvataggio `useSettings().save({key, value})` → `settings_set`.
      - Stato dell'agente: `hooks/use-omp.tsx` (`OmpPhase`: `checking | missing | installing | updating | notConnected | loggingIn | ready`).
      - Test: `general/general-view.test.tsx` (testi con omp e `h-14`).
  26macxqz:
    title: Loghi e icone delle Specs
    refs:
    - app
    - requirements/ui#app
    - specs#frameworks
    technical: false
    proposal: 5j2rp57g
    label: Specs
    finished: 2026-11-07T22:50:10.209Z
    body: |-
      La vista Specs viene rivista graficamente come dice la sezione "Specs view" di `requirements/ui#app`; header, filtri `All`/`Agent's choice`, ordine dei campi, box delle note e overlay restano come sono.

      Quando il Task è finito:
      - Ogni intestazione di campo ha un'icona lucide-react di 16 px, nel colore del titolo e decorativa, prima del titolo: languages `Code`, frameworks `Layers`, data `Database`, testing `FlaskConical`, CI `Workflow`, repository `GitBranch`, deployment `Rocket`, cloud `Cloud`, infrastructure `Server`, notes `NotebookPen`.
      - L'intestazione e il contenuto di ogni campo partono dallo stesso bordo sinistro del titolo dell'intestazione della vista; il bordo sinistro e la tinta del focus non li spostano di lato.
      - Il riquadro di 26 px di ogni voce (Ink 2 su Paper, raggio 6 px) non mostra più l'iniziale: mostra il logo monocromo della tecnologia, 16 px in Ink 2 e decorativo, oppure l'icona del campo quando la voce non ha logo. Nessun logo prende il colore del marchio.
      - I loghi vengono da `simple-icons`, l'intera raccolta inclusa nell'app e mai scaricata dalla rete; la raccolta si carica la prima volta che la vista Specs si apre in una finestra. La vista non la aspetta: finché non è caricata ogni riquadro mostra l'icona del campo e ogni logo la sostituisce appena la raccolta c'è. Se la raccolta non si carica restano le icone dei campi, senza alcun avviso.
      - Una voce ha un logo solo quando il suo nome intero, senza distinguere maiuscole, spazi e punteggiatura, coincide con il titolo o lo slug di un logo di `simple-icons` confrontato allo stesso modo (`Tailwind CSS`, `Node.js`); nessuna corrispondenza parziale (`Tauri plugins` e `react-markdown` restano senza logo).
      - Ogni card ha l'anello Line, raggio 12 px, l'ombra delle card della Board (`0 2px 6px rgba(24,24,27,0.05)`) e la stessa altezza minima, circa 92 px.
      - Prima riga della card: riquadro, nome (14 px semibold) centrato verticalmente con il riquadro, e all'estremità destra la versione come tag monospace su Paper con raggio 6 px; un nome lungo va a capo senza spostare riquadro e versione.
      - Sotto: la nota in Ink 2 a 13 px; su una voce cambiata il toggle `Show before` e il vecchio testo `Before: <text>`, entrambi a 13 px come la nota.
      - In fondo alla card, su una sola riga: l'etichetta dell'overlay (`added`, `changed`, `removed`) e, per una voce scelta dall'agente, la pill Lavender `Agent's choice` con `Sparkles` e il motivo nel tooltip.
      - `simple-icons` è una dipendenza dell'interfaccia, come dice il campo frameworks delle Specs.

      ## Code notes
      Commit: 0b94932067beeaa5a3e8371caa857af77dd0249f

      - Vista: `ui/src/components/views/specs-view.tsx` (`SpecsPage`, `Field`, `Entry`, `FilterPill`, `fieldTitle`); test in `specs-view.test.tsx`, fixture in `view-fixtures.ts:53-68`.
      - Il riquadro oggi è uno `<span aria-hidden>` con `name.charAt(0)`; la sezione del campo ha sempre `border-l-4 border-transparent pl-4` (da qui il rientro): usare margine negativo o un bordo che non sposti il contenuto, mantenendo `focusClass` di `change-badge.tsx`.
      - `ChangeBadge` oggi sta su una riga sotto il nome; `Agent's choice` è un `Badge variant="lavender"` con `mt-auto self-start` e `Tooltip`.
      - Mappa campo→icona: seguire il pattern `TONE_DOT` di `ui/src/components/board/column.tsx` (mappa con classi/componenti scritti per intero). Le 10 chiavi sono `SPECS_FIELDS`/`NOTES_FIELD` di `crates/core/src/project/specs.rs`; nessun cambio lato Rust: l'entry ha solo `name`, `version`, `note`, `chosenReason`, `change`, `before`.
      - Icone lucide importate per nome da `lucide-react`, sempre `aria-hidden`; dimensione dal contenitore (`Button` 16 px, `Badge` 12 px).
      - `simple-icons`: dati SVG (`path`, `title`, `slug`); caricare con `import()` dinamico per avere un chunk separato, come mermaid. Rendere il path con `fill="currentColor"`, mai `hex`.
      - Solo token (`bg-paper`, `text-ink-2`, `ring-line`, `shadow-card`), mai hex; il tema scuro funziona scambiando variabili in `ui/src/index.css`.
      - I test fissano l'iniziale (`getByText("T")`), il `font-mono` della versione, la griglia `minmax(250px,1fr)`, `max-w-[1180px]` come genitore diretto della sezione, il conteggio `text-muted-foreground`, i nomi delle `li` (`"TypeScript, changed"`) e il testo delle pill (`["All2","Agent's choice1"]`): aggiornare solo ciò che il nuovo aspetto cambia.
  xmrfhnck:
    title: Vista dei job ariosa
    refs:
    - job
    - ask-conversation
    - requirements/ui#job
    - requirements/ui#ask-conversation
    - requirements/data#window
    technical: false
    proposal: jbny689v
    label: Vista dei job
    finished: 2026-11-07T23:02:00.086Z
    body: |-
      La vista dei job deve smettere di sembrare claustrofobica. Quando il Task è finito, nella vista dei job vale quanto segue, come dicono i Requirements `requirements/ui#job`, `requirements/ui#ask-conversation` e `requirements/data#window`.

      **Una sola fascia di intestazione.** Sopra lista e conversazione c'è una sola riga, con l'altezza, il bordo inferiore e il titolo dell'intestazione comune delle viste, divisa dove finisce la lista: il bordo della lista prosegue attraverso la fascia. A sinistra il titolo `Jobs` e, alla sua estremità destra, il pulsante a icona `Hide jobs list` (`PanelLeftClose`). A destra, con 28 px di margine ai lati, l'intestazione di ciò che è selezionato:
      - per un job: nome (17 px semibold), pill di stato con il tempo di lavoro per un job che lavora, sotto `Started today at HH:MM` (o la data), a destra `View request` quando previsto e `Stop job` finché il job è in coda, in corso, in attesa o sospeso;
      - per un sotto-agente: il percorso `<job> › … › <sotto-agente>` con il suo stato;
      - per una conversazione di Ask more: il suo nome con il suo stato (`running`, `waiting`, `idle`), senza riga di avvio né pulsanti;
      - vuota quando non è selezionato nulla.

      La fascia separata dell'intestazione del job dentro il riquadro destro non esiste più. `Back to questions` di Ask more resta in cima alla sua conversazione.

      **Lista ridimensionabile.** La lista parte larga 280 px. Trascinando il bordo tra lista e conversazione (area di presa di 8 px, puntatore `col-resize`) la larghezza va da 240 a 420 px, e la divisione della fascia la segue. Il bordo è un separatore verticale che riceve il focus, con nome accessibile `Resize jobs list`, minimo, massimo e valore in px: le frecce lo spostano di 16 px, `Home` e `End` lo portano a 240 e 420 px, `Enter` e il doppio clic tornano a 280 px. La conversazione mantiene almeno 480 px: con la finestra troppo stretta la lista si restringe da sola, mai sotto 240 px, e torna alla larghezza scelta quando la finestra si allarga; questo restringimento non cambia la larghezza scelta.

      **Lista nascondibile.** `Hide jobs list` nasconde la lista e il suo bordo e il riquadro destro prende tutta la larghezza; la parte sinistra della fascia resta con il solo titolo `Jobs` seguito da `Show jobs list` (`PanelLeftOpen`), che riporta la lista com'era (larghezza scelta, filtro, selezione). Aprire un job o una conversazione da fuori (Chat, Ask more, una notifica, la palette) lascia nascosta una lista nascosta.

      **Memoria per progetto.** La larghezza scelta e lo stato nascosto della lista sono ricordati per progetto, anche dopo il riavvio, nello stato locale di Weaver fuori dal progetto (`jobsList`, per hash del progetto, `{width, hidden}`): un progetto senza valore, o con un valore fuori intervallo o di tipo sbagliato, vale 280 px e lista visibile. Il backend scrive alla fine di un trascinamento, a ogni passo da tastiera e quando la lista viene nascosta o mostrata, mai durante il trascinamento; il restringimento automatico non viene mai scritto. L'interfaccia riceve i valori dal backend e gli manda le modifiche, senza tenere uno stato proprio.

      Nella stessa sezione dello stato locale, due frasi descrivono ora quello che il codice fa già e non richiedono lavoro: il file della dimensione e della posizione delle finestre sta nella cartella di configurazione dell'app, dove lo salva il plugin, e su Windows il percorso del progetto è messo in minuscolo prima di calcolarne l'hash.

      **Conversazione.** La colonna della conversazione è centrata nel riquadro destro, larga al massimo 860 px, e la casella del messaggio sta nella stessa colonna. Le voci sono come in Chat: messaggi dello sviluppatore, messaggi dell'agente con la riga del mittente (logo, `Weaver`, ora) e il testo di Chat, e il riepilogo dei passi che raccoglie ogni serie di `Thinking`, righe dei tool e righe dei sotto-agenti, chiuso all'inizio; note, permessi, messaggi in coda, righe di stato e di fine restano fuori dal riepilogo. Il riquadro dei passi in corso, quando è l'ultima cosa di un turno che lavora, dice `Working · <n> steps · <ultimo passo>`, dove l'ultimo passo è il titolo della sua ultima riga di tool, o `Thinking`, su una riga tagliata con `…`. L'istruzione di Weaver resta la prima voce, chiusa.

      **Casella del messaggio.** Come quella di Chat (aspetto, due righe all'inizio, posata su una sfumatura, posizione del testo), senza il controllo del modello, centrata nella colonna, con segnaposto, invio e regole del job di oggi. A job finito, al suo posto c'è una riga centrata nella colonna, sopra la sfumatura e senza bordo: `This job has ended. Continue in Chat.` con `Open Chat`.

      **Ask more e sotto-agenti.** Le conversazioni di Ask more e quelle in sola lettura dei sotto-agenti hanno la stessa colonna, le stesse voci e la stessa casella (disabilitata per i sotto-agenti). Nella conversazione di un sotto-agente la riga del mittente mostra un riquadro di 24 px con l'icona `Bot` e il nome del sotto-agente al posto del logo e di `Weaver`; `Conversation not available: <reason>.` compare in cima alla conversazione. Le conversazioni dei sotto-agenti aperte da Chat seguono lo stesso disegno; il resto della vista Chat non cambia.

      Le righe della lista, il filtro, l'ordine, i badge e i tempi restano come sono.

      ## Code notes

      Commit: bcb006abc30b7b93d0223e6a5d5faf43d3e2eb18

      - Quasi tutto è in `ui/src/components/jobs/jobs-view.tsx`: `JobsView` (lista `w-[300px]` fissa, nessun ridimensionamento), `ViewHeader view="jobs"` sopra i due pannelli, `JobHeader` (`px-7 py-[18px]` con bordo), `Conversation`, `FollowScroller`, `CONVERSATION_PADDING`/`CONVERSATION_CONTENT` (`max-w-[860px]` senza `mx-auto`), `MessageFrame`/`MessageBox`/`DisabledMessageBox`, `EntryView`, `ToolRow`, `SubAgentConversationView`, `AskListRow`.
      - Da riusare: `general/chat-conversation.tsx` (`SenderRow`, `ChatEntry`, `DeveloperMessage`, `WorkSummary`), `general/chat-model.ts` (`chatItems`, `workText`, `clockText`), la colonna centrata e la sfumatura di `general/general-view.tsx` (~131, ~192). `view-header.tsx` (`VIEW_HEADER_CLASS`, 76 px) per l'altezza della fascia.
      - Insidia: `EntryView` e `SubAgentConversationView` sono importati da Chat e `FollowScroller`/`EntryView` da `ask/ask-conversation.tsx`; cambiare spaziature lì cambia anche Chat. Ask oggi ha la sua barra `Back to questions` e casella con `border-t p-3`.
      - Nessun componente di pannello ridimensionabile in `ui/src/components/ui/`; nessuna dipendenza tipo `react-resizable-panels`.
      - Stato locale: `state.json` in `app_local_data_dir` (accanto a `recent`, `lastSession`); `ui/src/lib/backend.ts` per i comandi Tauri.
      - Tailwind v4, token in `ui/src/index.css`: `rounded-xl` = 14 px, `rounded-lg` = 12 px. Pill da `TONE_PILL`/`TONE_DOT` in `ui/src/lib/tones.ts`.
      - Test che fissano classi e testi: `jobs-view.test.tsx` (~91-117, 293-305, 341-348), `general-view.test.tsx`, `ask.test.tsx` (~134).
  yby4sopj:
    title: Barra domande in fondo
    refs:
    - question-round
    - requirements/ui#question-round
    - requirements/ui#window
    - requirements/ui#General
    technical: false
    proposal: e63hs4zm
    label: question-bar
    finished: 2026-11-07T23:05:16.605Z
    body: |-
      Quando un round di domande è minimizzato, la barra `The agent is waiting for your answers (question <n> of <N>)` + `Open questions` sta in fondo al piano di lavoro, dentro il pannello con i suoi margini e angoli arrotondati, a tutta larghezza del piano, sotto la vista selezionata.

      - Compare in ogni vista, Chat compresa (anche per un round di Chat, accanto alla riga di attesa e a `Answer`) e nelle conversazioni Ask more (accanto a `Back to questions`).
      - Occupa il proprio posto nel layout e non è mai disegnata sopra la vista: la vista si accorcia dell'altezza della barra, così nulla resta coperto — né la casella dei messaggi di Chat, di un job, di un sub-agente o di Ask more, né la fine di una lista, le colonne della Board, un editor o un foglio. La barra laterale resta invariata.
      - Aspetto: fondo tinta Peach, testo e icone in inchiostro Peach, bordo superiore di 1 px Peach.
      - Una sola riga ad altezza fissa a qualunque larghezza della finestra: il testo si tronca con `…` e il testo completo è nel tooltip; `Open questions` resta sempre intero all'estremità destra.
      - Finché la barra è visibile, i toast stanno sopra di essa, mai sopra la barra.
      - Riapertura e minimizzazione del round funzionano come prima; la finestra di dialogo delle domande non cambia.

      ## Code notes

      Commit: 0b94932067beeaa5a3e8371caa857af77dd0249f

      - Oggi `MinimizedBar` in `ui/src/components/questions/question-overlay.tsx` (~33–50) è `fixed inset-x-0 bottom-0 z-40 … bg-background`, `flex-wrap`, montata da `QuestionRoundLayer` (~148–156) dopo `{children}`, fuori dal layout di `ProjectWindow`: nessuno spazio riservato.
      - `ProjectWindow` (`ui/src/components/project-window.tsx` ~186–216): `[data-work-surface]` è una colonna flex con banner in alto e `main.min-h-0.flex-1`. Una barra `shrink-0` in flusso dopo `<main>` accorcia ogni vista senza modifiche per vista; niente padding inferiore per vista.
      - Aggiungere a `ProjectWindow` uno slot in fondo (es. `footer`) come la prop `banners` (passata in `project-content.tsx` ~106–110), invece di usare `useQuestionRound` dentro `ProjectWindow` (nei test è senza provider [inferenza]). Stato da `useQuestionRound`.
      - Lasciare invariato l'overlay Dialog (portal, `fixed inset-0 z-50`).
      - Token Tailwind v4 esistenti (`text-peach-ink`, `border-peach`, tinta Peach), primitive shadcn, `cn`.
      - Toast: `Toaster` in `ui/src/components/ui/sonner.tsx` (`bottom-right`), montato in `app-providers.tsx`; `offset` di Sonner può alzarli mentre la barra è visibile.
      - Il testo nel codice differisce (`Weaver is waiting…`, `(summary)`): seguire il Requirement.
      - `Jump to latest` (`sticky bottom-2`) continua a funzionare.
      - Test da tenere verdi: `question-overlay.test.tsx` (barra trovata per regione "Questions waiting"/testo), `ask.test.tsx`, `general-view.test.tsx` (~729–739).
---
# Ridisegno dell'interfaccia

Weaver adotta il ridisegno dell'interfaccia. I colori dicono solo uno stato (Sky, Sage, Peach, Lavender, Rose), le etichette perdono il colore e i testi fuori dalle Settings non nominano più omp. Cornice, logo, token, font e forme sono nuovi. La sidebar mostra Usage e lo switch di progetto, e c'è la palette dei comandi. Board, Jobs, History, Requirements, Specs e Chat sono ridisegnate; Chat mostra le Proposal dei job avviati.
