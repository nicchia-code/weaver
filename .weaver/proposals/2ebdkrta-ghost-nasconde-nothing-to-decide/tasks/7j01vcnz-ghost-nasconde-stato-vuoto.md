---
technical: false
refs:
  - work
  - requirements/ui#work
---
# Ghost nasconde stato vuoto

Sulla Board un ghost nasconde lo stato vuoto della sua colonna esattamente come una card vera.

Quando il Task è finito:

- **Backlog:** lo stato vuoto (il disegno a fili, `Nothing to decide`, `New requests and Proposals from Chat land here for your go-ahead.` in Muted, i pulsanti `Request` e `Ask in Chat`) compare solo quando nella Backlog non c'è nessuna card vera (bozze di richiesta comprese) e nessun ghost. Con soli ghost sparisce del tutto e la colonna mostra solo i ghost.
- **Active:** `Nothing active` compare solo quando in Active non c'è nessuna card vera e nessun ghost; un ghost di Task lo nasconde.
- Vale per ogni ghost, di qualsiasi tipo, anche il ghost che Weaver crea da solo per un job di setup, design o import.
- I numeri non cambiano: il numero accanto al titolo di ogni colonna, il sottotitolo `<n> active` della Board e il numero della voce `Board` nella barra laterale contano solo le card vere, mai i ghost. Una colonna con soli ghost mostra quindi 0 e nessuno stato vuoto.
- Nella stessa immagine in cui l'ultima card o l'ultimo ghost lascia una colonna, il suo stato vuoto ricompare (per esempio quando un job finisce senza consegnare la sua Proposal e nella Backlog non resta nulla).
- I test esistenti che si aspettano lo stato vuoto sopra un ghost riflettono il comportamento vecchio e vanno aggiornati al nuovo.

## Code notes

Commit: b9dd6f9e8316b018779f14a8d34c2e1e73ae5e43

- Solo interfaccia, nessuna modifica Rust. `ui/src/components/board/column.tsx:Column` mostra oggi `{count === 0 ? empty : null}{cards}{ghosts}`; `count` è il numero di card vere e `ghosts` è un `ReactNode` (può essere `null`), quindi `Column` non sa quanti ghost ha. I doc comment di `count` ed `empty` (righe ~11–13, "also above ghosts") diventano falsi.
- `ui/src/components/board/board-view.tsx:BoardView` (~305–325) crea le due `Column` con `count={snapshot.backlog.length}` / `snapshot.active.length` ed `empty={<EmptyBacklog />}` / `<p>Nothing active</p>`; i ghost arrivano da `ghostsOf(column)` (~231, filtra `snapshot.ghosts` con `ghostColumn`) avvolti in `GhostList`.
- Lasciare `count` sulle sole card vere (il titolo deve restare `Backlog0` con un ghost) e passare un segnale separato, per esempio decidere lo stato vuoto in `BoardView` con `cards.length === 0 && ghostsOf(col).length === 0`, calcolando `ghostsOf` una volta per colonna. La vista non raggruppa né ordina: resta un semplice controllo di lunghezza.
- Il ghost proprio di Weaver arriva già in `snapshot.ghosts` (kind `proposal`, da `src-tauri/src/jobs/registry.rs:ghosts_of`), senza flag: nessun caso speciale.
- Test in `ui/src/components/board/board-view.test.tsx`: "keeps the empty text above a ghost and counts 0" (~156–163) va invertito; "says Nothing to decide and Nothing active, also above ghosts" (~150) va rinominato; "shows in an empty Backlog the weave, …" (~310–330) usa un ghost di Drift da togliere. Manca un caso `Nothing active` con ghost di Task. Riutilizzabili la fixture `ghost(id, kind, title, patch)` (~55) e lo schema di "shows no empty Backlog text once a card is there" (~332–337).
