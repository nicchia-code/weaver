---
status: approved
kind: change
requires:
- lhn2o96u
base:
  model/work/task.md: ae45d5a06bda8cba81623922c65ac83e73dc2e4da0a5df7705bb81a466eab946
  requirements/data/card-files.md: new
  requirements/data/project-files.md: 277b0fc25768f755b386070b42662c7c276580cc73bdeda742a927940246c1f6
  requirements/rules/skills.md: 9629a510ebd939a784417b20ee30e1347b00d3b281ea7989e011f0c62e5ca897
created: 2026-11-05T18:48:55.042Z
label: Note di codice
job: bf1h0l1t
---
# Note di codice nelle Task

Oggi il codice si esplora due volte: la design lo esplora a fondo per l'intervista e per scrivere le Task, ma nella Task non passa quasi nulla di ciò che ha visto, e l'implementer rifà da zero lo stesso giro di scoperta, con ogni sub-agente che ripete le zone in comune con le altre Task. Con le note di codice la design, lo scan e il compare lasciano nella Task dove guardare, le convenzioni, le insidie e cosa riusare, con il commit su cui hanno letto il codice; l'implementer parte da lì e riesplora solo i file cambiati da quel commit. L'obiettivo è ridurre il tempo totale, design più run, senza perdere qualità. Richiede la Proposal della misura dei job, così il prima e il dopo si misurano con lo stesso metodo.

## Decisioni

- Le note stanno in una sezione `## Code notes` alla fine del corpo della Task, con il commit nella prima riga; lo sviluppatore le vede nei dettagli della Task e nella copia dentro la History.
- Le scrivono la design e anche scan e compare quando creano una Task; le fix Task nate da una Drift restano come oggi (rilettura in Where in the code).
- Le note nascono solo da ciò che l'esplorazione ha già letto: nessuna lettura in più per scriverle.
- Lunghezza: una guida delle skill di circa 1.500 caratteri al massimo per Task; Weaver non la controlla.
- Una Task senza note o senza commit (comprese quelle già scritte): comportamento di oggi, nessun rifiuto alla consegna.
- L'implementer confronta i file citati con il commit (`git diff <commit> -- <file>`, contro i file sul disco): file invariati, legge solo quelli che modifica e salta l'esplorazione; file cambiati, riesplora solo quelli. Le note non sostituiscono mai la lettura dei file da modificare.
- Confronto impossibile (nessun repository git, commit sconosciuto): tutti i file citati contano come cambiati e si riesplorano, partendo comunque dalle note.
- Una nota che contraddice il codice: vince il codice; l'implementer ignora la nota, esplora quella parte come oggi e lo dice nel report, senza Drift.
- Rifinitura di una Proposal stale: note e commit restano come sono.

## Scelte dell'agente da verificare

- Il commit è `git rev-parse HEAD` letto all'inizio dell'esplorazione; le modifiche fatte dopo risultano cambiate al confronto.
- La rielaborazione di una Proposal rifiutata scrive note nuove dalla propria esplorazione; lo scan leggero dopo la consegna non aggiunge note.
- Anche il sub-agente `weaver-analysis` scrive le note per le Task che crea.
- Weaver non legge né controlla la sezione: nessun cambio al codice dell'app e della riga di comando, solo ai testi installati per l'agente.
- Il fragment `requirements/data/project-files.md` supera 12 KB e si divide: le sezioni dei file delle card passano, senza cambiare testo, al nuovo fragment `card-files`.
