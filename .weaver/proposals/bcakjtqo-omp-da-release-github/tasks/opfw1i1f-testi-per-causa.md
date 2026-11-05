---
technical: false
refs:
  - system-certificates
  - requirements/ui#system-certificates
---
# Testi per causa

Quando omp non si installa o non si aggiorna perché il download è fallito, Weaver spiega la causa in parole semplici, con un testo per ogni codice di uscita. Lo script di installazione e il download in background da GitHub (Task della Machine `omp-setup`) escono con codici dedicati: `90` download bloccato, `91` certificato non verificato, `92` file incompleto o alterato (dimensione o sha256 diversi), `93` l'omp scaricato non parte, `94` release non più recente (solo aggiornamento: in quel caso resta il motivo di `omp update`, nessun testo nuovo). Questo Task costruisce i testi a partire dal codice che quel Task passa: il codice del file marker per l'installer, il codice di uscita o il superamento del limite di 15 minuti del download in background per gli aggiornamenti.

## Installer fallito (riga Missing)
Con il codice dell'installer la riga Missing del riquadro di stato di omp mostra, testi esatti in `requirements/ui#system-certificates` ("Failed installer"):
- `90`: download bloccato su entrambi i siti di download, probabilmente da un proxy aziendale che filtra i download;
- `91`: certificato non verificato, proxy che ispeziona HTTPS; con System Certificates spento si aggiunge ` Turning on System Certificates in Weaver's settings may fix it.`;
- `92`: omp scaricato incompleto o alterato, quindi non installato;
- `93`: l'omp scaricato non parte;
- ogni altro codice: il testo di oggi, con lo stesso suffisso quando l'impostazione è spenta.

## Download di un aggiornamento fallito
Il `(<reason>)` di un aggiornamento fallito in Missing e Ready, senza punto finale ("Failed download of an update"): `90` download bloccato; `91` la spiegazione del certificato ("Proxy explanations") per l'impostazione così com'è in quel momento; `92` incompleto o alterato; `93` non parte; altro codice `the download of omp failed (exit code <N>)`; oltre il limite `the download of omp timed out`.

## Regole
- Nessun testo di errore nomina un prodotto: i testi dicono "both of its download sites", mai GitHub; solo l'avviso di accensione automatica nomina Zscaler.
- Il download in background di omp da GitHub riceve le variabili dei certificati di `requirements/tech#system-certificates` come ogni processo omp avviato da Weaver, dalla stessa unica funzione `variables`; con l'impostazione spenta o senza bundle parte esattamente come senza.

## Dove nel codice
- `crates/core/src/omp/failure.rs`: `installer_failed_text` (:82) diventa per codice; una nuova funzione per il motivo del download di un aggiornamento, che riusa `explanation` (:60) per il `91`. Test esistenti a :616-622.
- `crates/core/src/omp/machine.rs:777`: dove la Machine costruisce il testo dell'installer; il motivo dell'aggiornamento si collega dove il Task omp-setup gestisce il fallimento del download.
- `crates/core/src/system_certificates/mod.rs:101` `variables` e `src-tauri/src/system_certificates.rs:524`; `crates/core/src/omp/run.rs:74` `with_certificates` per il nuovo processo.
- UI: `ui/src/components/omp-status.tsx:62` mostra `status.installerFailure` (binding `ui/src/bindings/OmpStatus.ts:60`).

## Test
Test Rust di ogni testo (90, 91, 92, 93, altro codice, timeout) con l'impostazione accesa e spenta; test che il download in background riceva le variabili. Vitest della riga Missing in `ui/src/components/omp-status.test.tsx` con i nuovi testi.
