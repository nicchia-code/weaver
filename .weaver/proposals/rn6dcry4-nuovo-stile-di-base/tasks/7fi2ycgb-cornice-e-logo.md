---
refs:
  - window
  - requirements/ui#window
  - requirements/release#General
---
# Cornice e logo

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
