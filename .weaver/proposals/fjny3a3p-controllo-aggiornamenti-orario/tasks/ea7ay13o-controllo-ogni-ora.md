---
technical: true
refs:
  - requirements/release#General
  - requirements/docs#General
---
# Controllo aggiornamenti ogni ora

Weaver controlla se esiste una sua nuova versione sulla release GitHub all'avvio e poi al massimo una volta ogni ora mentre resta aperto, non più ogni 24 ore (`requirements/release#General`, "Updates of Weaver"). Quando il Task è finito vale tutto questo:

- L'intervallo è fisso, un'ora: nessuna impostazione nuova nelle Settings.
- Il controllo all'avvio resta e l'istante di ogni controllo continua a essere salvato nelle settings, come oggi.
- Un controllo fallito è solo scritto nel log, mai mostrato al developer, e ritentato alla scadenza successiva, quindi entro un'ora.
- **Later** non cambia: nasconde l'aggiornamento trovato fino al controllo successivo, quindi l'aggiornamento riappare dopo al massimo un'ora. **Install and restart** e la verifica della firma non cambiano.
- Il controllo degli aggiornamenti di omp è un'altra cosa, già orario, e non cambia.
- I test del calendario dei controlli si aspettano un'ora, non più 24 ore.
- La documentazione per chi usa e sviluppa Weaver dice un'ora dove oggi dice 24 ore, e il changelog ha una riga sotto `## [Unreleased]`, gruppo `Changed`, che dice che Weaver cerca una sua nuova versione ogni ora invece che ogni giorno (`requirements/docs#General`).

## Code notes

Commit: 450785e1b59d7321510ae229ca9cc4280a0dc799

- `src-tauri/src/updates.rs`: `pub const CHECK_INTERVAL` (oggi `Duration::from_secs(24 * 60 * 60)`) è l'unico valore da cambiare; `run_schedule(updates, CHECK_INTERVAL, sleep)` controlla subito e poi dorme l'intervallo dopo ogni controllo. Anche il commento del modulo (riga 4, "(24 hours)") va aggiornato.
- `Updates::check` salva l'istante in `lastUpdateCheck` di `settings.json` tramite `AppSettings`; `Updates::later` imposta `hidden = true` e un controllo riuscito lo azzera: nessuna modifica lì.
- Test nello stesso file: `checks_at_start_and_then_once_every_24_hours` confronta con `Duration::from_secs(24 * 60 * 60)` e ha un commento "waits a whole day"; rinominarlo e confrontare con un'ora. Gli altri test usano già `CHECK_INTERVAL` (`h.clock.advance(CHECK_INTERVAL.as_millis() as i64)`), quindi seguono la costante. Il calendario usa uno `sleep` iniettato (`scheduled(...)`, `ticks.asked`/`ticks.grant`): mai attese reali.
- `README.md`: la tabella dei moduli (riga del modulo `updates`, "every 24 hours") e la sezione sugli aggiornamenti ("at most once every 24 hours").
- `CHANGELOG.md`: Keep a Changelog 1.1.0, riga in inglese sotto `## [Unreleased]` → `### Changed`.
- Non toccare i timer di 24 h di System Certificates né il controllo di omp in `src-tauri/src/omp.rs`.
