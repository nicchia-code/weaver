---
technical: false
refs:
  - general-chat
  - requirements/ui#general-chat
  - requirements/ui#assets
  - requirements/tech#assets
  - requirements/quality#assets
---
# Cursore allineato al testo

Nella casella dei messaggi della Chat il testo sembra scritto a metà della parola: il testo visibile è disegnato da uno strato specchio dietro l'area di testo trasparente, e la casella della Chat toglie i margini laterali solo all'area di testo, non allo specchio. Il testo disegnato risulta così 12 px a destra di cursore, selezione e caratteri veri, e le righe lunghe vanno a capo in punti diversi.

A lavoro finito vale quanto segue, in tutte e quattro le caselle con tag (messaggio della Chat, risposta alla conversazione di un job nella vista Jobs, messaggio di una conversazione Ask more, testo dell'editor delle richieste):

- Il cursore e la selezione stanno esattamente sui caratteri disegnati, su ogni riga, anche dopo un a capo automatico e con la lista dei suggerimenti `@` aperta; un clic mette il cursore sul carattere sotto il puntatore e ogni carattere digitato compare al cursore.
- Il componente condiviso applica le metriche della casella (margini interni, spessore del bordo, famiglia, dimensione, interlinea e spaziatura del carattere, a capo) a entrambi gli strati da un unico punto. Chi usa il componente può cambiarle solo da lì, per entrambi gli strati insieme; ciò che può passare alla sola area di testo (sfondo, colore del bordo, anello del focus, ridimensionamento) non sposta mai un carattere. Una metrica passata a un solo strato non è ammessa dal componente.
- Il segnaposto che lo specchio usa per misurare il cursore per la lista dei suggerimenti non aggiunge mai larghezza né un punto di a capo: con la lista aperta o chiusa lo specchio va a capo agli stessi caratteri dell'area di testo.
- Invio o Tab premuti per confermare una composizione del metodo di input (accenti, tasti morti, IME) confermano solo la composizione: non inviano il messaggio, non scelgono un suggerimento e non spostano il focus.

Nella Chat il testo resta dove lo sviluppatore lo vede oggi: inizia 12 px dentro il margine interno della cornice (il margine laterale di un normale `Textarea`), e cursore, selezione e testo disegnato iniziano tutti lì, come nelle altre caselle; la casella non toglie alcun margine laterale alla sua area di testo.

Test Vitest, per ciascuna delle quattro caselle come la sua vista la mostra: l'area di testo e lo specchio ricevono gli stessi margini, spessore del bordo, famiglia, dimensione, interlinea e spaziatura del carattere; Invio e Tab durante una composizione non inviano, non scelgono un suggerimento e non spostano il focus. jsdom non calcola il layout, quindi i test verificano gli stili ricevuti dai due strati, non posizioni misurate.

## Code notes

Commit: 450785e1b59d7321510ae229ca9cc4280a0dc799

- `ui/src/components/assets/tagged-textarea.tsx`: `TaggedTextarea`. Mirror `div` (`data-testid="tag-mirror"`, ~:153-171) hard-codes `border border-transparent px-3 py-2 text-sm whitespace-pre-wrap break-words`; textarea (~:193-200) has `border px-3 py-2 text-sm text-transparent caret-foreground` merged with `areaClassName` via `cn` (twMerge, `ui/src/lib/utils.ts`), so a caller's `px-*` silently wins on the textarea only. Props doc at ~:48-54 (`className` outer box, also font; `areaClassName` textarea only).
- Cause: `ui/src/components/general/general-view.tsx:200` passes `areaClassName="resize-none border-transparent px-0 …"`. Removing `px-0` keeps the text where it shows today (decided). Frame is `pl-4 pr-3 py-3` (:193).
- Caret marker: `piecesOf(value, tags, open ? caret : null)` (~:150) inserts an `inline-block w-0` span (~:161-162) that can add a break point inside a word; measured via `marker.offsetLeft/Top` (~:113-118).
- Composition: the list's key handler already checks `isComposing` (~:128); Chat's Enter handler (`general-view.tsx:203-208`) does not. Check the Jobs `MessageBox` (`ui/src/components/jobs/jobs-view.tsx:413-470`) and Ask more (`ui/src/components/ask/ask-conversation.tsx:94-107`) handlers too. Request editor: `ui/src/components/requests/request-pane.tsx:420-434` (`font-mono` on wrapper, convention for box-level font).
- Tests: `tagged-textarea.test.tsx:257-266` (mirror test), `general-view.test.tsx`; render inside `AssetsProvider` or `view-test-utils.tsx`.
