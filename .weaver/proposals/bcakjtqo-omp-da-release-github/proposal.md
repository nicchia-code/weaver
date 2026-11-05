---
status: approved
kind: change
base:
  model/actors/developer.md: 89b49e009efa5f940e893a0edacccef9f870c03a1e332139b2852549693b4e90
  model/actors/omp.md: 2d3a8b5c3cedfb93e2e93c8c9c529356d44aefaad8d733b7e848e4a7d21aad8b
  model/project-setup/omp-setup.md: 269de496e35f5e1e07a0b27cbe8bdc19b3716b540a71fae1d00850fd52d20269
  model/project-setup/system-certificates.md: 213b379bd2a59e363e83b27550eef93844f325d3503c414de870fc06d070ac91
  requirements/docs/main.md: a6ace19d76e83a18c7383ef6cc1f6cb97e3eca776fd82a55a73b6f7009a9d077
  requirements/quality/main.md: b1b86087affe749bcdf78eaf5643648dddb7fdd386d70aa780677346f104b982
  requirements/quality/omp.md: new
  requirements/tech/omp.md: a2a81a438767984b03da93e8ae4d489040190a4ee1943fe924aa00d1f94c87dd
  requirements/tech/usage.md: new
  requirements/ui/system-certificates.md: bb19cab85115d334c1ba31abc5c1d41c8fa8c29cedf14e0168c284474e3d5b3e
  specs.yaml: c2bec212ad55a461d551ebc6c7b4466e3e31c37440562b7d09c9f0dcb87b15b6
created: 2026-11-05T11:32:58.949Z
request: 2026-11-05T12-24-50
label: omp da GitHub
job: ed4j6a0t
---
# omp da release GitHub

Dietro proxy aziendali come Zscaler `Install omp` fallisce: lo script ufficiale di omp scarica il binario da Cloudflare R2, che il proxy blocca come sito di file hosting. Le release di GitHub di omp pubblicano lo stesso file e di solito passano. Questa Proposal fa usare a Weaver le release di GitHub come ripiego sia per l'installazione sia per l'aggiornamento di omp.

## Decisioni

- Ripiego automatico: nello stesso terminale parte prima lo script ufficiale; se fallisce, Weaver scarica il binario da GitHub. Nessuna impostazione nuova.
- Il download dell'installazione avviene nel terminale di sistema (script `curl`/`sh` o PowerShell), con exit code nel marcatore e i ricontrolli di oggi; nessun client HTTP nuovo nel backend.
- Vale su Windows, macOS e Linux (anche musl), per x64 e arm64.
- Cartella come lo script ufficiale: `PI_INSTALL_DIR` se impostata, altrimenti `%LOCALAPPDATA%\omp` o `~/.local/bin`, come `omp.exe` o `omp`.
- PATH come lo script ufficiale: su Windows la cartella si aggiunge al `Path` dell'utente se manca; su macOS e Linux il terminale stampa solo come aggiungerla.
- Versione e verifica dall'API `releases/latest` di GitHub: tag, indirizzo, dimensione e SHA-256 (`digest`) dalla stessa risposta; il file vale solo se dimensione e hash coincidono e `omp --version` risponde.
- Aggiornamento: se `omp update` fallisce, Weaver scarica in background la release di GitHub e sostituisce il binario, con le stesse verifiche, per ogni aggiornamento; solo per un binario nelle cartelle dell'installer, non per omp di `bun`, Homebrew o `omp path`. Su Windows il file in uso si sposta da parte prima.
- Il controllo delle versioni nuove resta solo `omp update --check`.
- Un testo per causa quando anche GitHub fallisce: download bloccato, certificato non verificato (con il suggerimento su System Certificates), file incompleto o alterato, omp scaricato che non parte; ogni altra causa tiene il testo di oggi.
- README (`## Behind a corporate proxy`, `### Where Weaver keeps files`) e CHANGELOG descrivono il ripiego.

## Scelte dell'agente

- Exit code riservati per le cause (90 bloccato, 91 certificato, 92 file incompleto o alterato, 93 omp non parte), scritti nel marcatore di oggi.
- Il ripiego dell'aggiornamento parte solo quando il tag di GitHub è più nuovo della versione di omp; altrimenti resta l'errore di `omp update`. Limite di 15 minuti per il download, come lo script ufficiale.
- Il download in background riceve le variabili di System Certificates come gli altri processi di omp.
- Il frammento `quality/main` supererebbe 12 KB con la nuova sezione: la sezione `system-certificates` passa con la nuova `omp-setup` in un frammento `quality/omp`, senza cambiarne il testo.
- Il frammento `tech/omp` supera 12 KB con le nuove regole: la sezione `usage` passa, senza cambiarne il testo, nel nuovo frammento `tech/usage`.
- I testi di errore non nominano GitHub, per la regola esistente che nessun testo di errore nomina un prodotto: dicono "both of its download sites".
