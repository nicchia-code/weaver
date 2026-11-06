---
technical: true
refs:
  - requirements/rules#project
---
# Sezione Chat nelle skill

L'istruzione della Chat, su ordine esplicito del developer, manda l'agente a caricare `weaver-design` (Proposal di modifica) o `weaver-resolve-drifts` (risoluzione di Drift aperti). Le due skill devono quindi dire come si scrive dalla Chat.

## Cosa cambia (`requirements/rules#project`, punto Chat texts)

Nei sorgenti `crates/core/assets/skills/weaver-design/SKILL.md` e `crates/core/assets/skills/weaver-resolve-drifts/SKILL.md` (installati da `crates/core/src/agent_files.rs`; le copie in `.agents/skills/` si aggiornano con l'installazione, non a mano):

- Nuova sezione `## Writing from Chat` in entrambe, che vale quando l'istruzione della Chat manda lì l'agente su ordine esplicito del developer. Dice:
  - la conversazione vale come intervista: ciò che developer e agente hanno concordato è deciso e non si richiede; una decisione ancora aperta si chiede nella risposta, o con un solo round `weaver question new` solo se il developer ha chiesto di essere interrogato così; allo stesso modo si decide se legare le immagini di una richiesta;
  - nessun annuncio e nessuna nota sul job; la Proposal si apre subito, una modifica con `weaver-design` o una risoluzione con `weaver-resolve-drifts`, legata con `request` a una richiesta non inviata, o con `reworks` a una Proposal rifiutata, solo quando il developer l'ha nominata;
  - poi file, Task di una modifica, `## Already present`, consegna e, per una modifica, la scansione leggera del codice, come dicono i passi della skill, sotto-agenti inclusi; una contraddizione trovata dalla scansione si chiede come sopra;
  - dopo la consegna l'agente riassume la Proposal in poche righe e si ferma; non decide nulla a meno che il developer chieda esplicitamente quella decisione.
- `## Hard rules` (e la `description` del front matter di `weaver-design`, che oggi dice "never in Chat"): non dicono più che la Chat non le segue mai o non scrive mai Proposal; continuano a tenere fuori ogni altra conversazione (Ask more). In `weaver-design` togliere il rimando obbligato a `+ Work Item` per la Chat.
- Il testo resta in inglese, senza prima persona, rispettando le altre regole di `requirements/rules#project` (lunghezza e formato delle skill).

## Test

- `crates/core/tests/format_md.rs` e i test di `agent_files.rs` che controllano le skill incorporate: aggiornare le asserzioni che fissano la frase "never in Chat" o l'assenza della sezione; aggiungere un controllo che entrambe le skill abbiano `## Writing from Chat`.
