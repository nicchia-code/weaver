---
status: approved
kind: change
base:
  model/actors/agent.md: 1dbdd844fd5863177e13b1028a4852296ae435791a82d69dc0e2ed5a33ec1d1f
  model/agent-work/general-chat.md: 855205d1f139aca8d69bb1ff1715b0062bf3148016897c6d116e285f56aa1409
  model/agent-work/job.md: 7948d6a386b37984f067d2fc325c80b0913d3df7e0743aa4ee9362cbb56a3ed1
  model/model-changes/design-request.md: c0f900cf9f31abfde58c6bcb9159ab91e5123525932bf14bc5aef6a75b37a221
  model/model-changes/proposal.md: 99b1c7b572a0e4b06c29f396e6cdab8ef2e2f35a6261a48e0f5c5986fe135cfd
  requirements/cli/agent-commands.md: fa0b27b08a1be4ab3831b6c5cca45079661a48b355734d2bf955106854a303ae
  requirements/cli/commands.md: a3b7859a66afe28ee30c685aa7971adb20c5d61ad22a9334f1784cb68fb687a6
  requirements/data/card-files.md: ce854f95ed8e6468a2085b712255c5e10bd875894f57ac51404c0a8e5803641c
  requirements/rules/cards.md: 16a87493a214a768740c61c1ccbceb51ad921ba61affd9c7ea342bb4a7be329a
  requirements/rules/chat-instructions.md: 32b93a6228796d3853a7958870b049ca098142d2b3b6ee6b46f88015aaa4ff16
  requirements/rules/instructions.md: 1055d6193ddbe4b8a3cffd4b580858b5b9d3c1d64a0720efa2f45dec23b62037
  requirements/rules/skills.md: 2b77cef0f27250d405639dcc3876c038d167b5ebd5694fc9bb75d6722e60fa63
  requirements/tech/acp.md: 28033cce527f89703365668ae69cc713996737d31dae95681f71e9175bbf5b9f
  requirements/tech/assets.md: 1b5035f5e26b0a028ab0d68dbb1cc456b9c87e98875e5aaa2011ec89734daf76
  requirements/ui/board-cards.md: 1c07157b7d9b26ddd6b97987a2451072bdc68c36a3adcc14aa54bf246efe44f8
  requirements/ui/conversations.md: a8143802f9e38903c89003ec2c5a2cd903d0c68aa1790409f0b7552ee3216768
  requirements/ui/look.md: 4e4c8c3b8c4d24ec7d5736738a720c31ae5cf426cf0b7ae653898cac44cd50de
  requirements/ui/setup.md: e8eded18ea7548d1f79a370946492a1e01ff11a8f8cc700593c7a2202879d3bb
created: 2026-11-06T11:38:42.340Z
label: Chat request
chat: true
---
# Chat invia design request

In Chat, la richiesta di aprire una Proposal passa dal flusso normale, come se lo sviluppatore avesse scritto la request da "+ Add Work item": l'agente di Chat scrive una design request con quanto concordato e la invia, e il job di design esplora, fa i round di domande e consegna la Proposal. Il nuovo flusso rimpiazza quello attuale, in cui l'agente di Chat scriveva la Proposal da sé con la conversazione come intervista.

## Decisioni

- Un nuovo comando `weaver request new` scrive la request e la invia in un solo passo, come `Send` dell'editor: parte un job di design, o di setup con il Model vuoto.
- Una resolution di Drift chiesta in Chat parte come job `resolve` con `weaver job new`; Chat non carica più `weaver-resolve-drifts`.
- Chat non lega più un rework con `reworks`: il rework parte solo dal rifiuto con Rework sulla Board.
- Chat non modifica più Proposal esistenti, nemmeno su ordine esplicito.
- Di conseguenza Chat non scrive più Proposal: spariscono i campi `chat` e `chatWriting`, lo stato Held by Chat della request, il blocco delle decisioni mentre Chat lavora e lo scarto delle Draft di Chat con Clear.
- Restano `weaver task new` e `weaver drift new` su ordine esplicito, i round di Chat quando lo sviluppatore li chiede e l'avvio di job.

## Ipotesi da verificare

- La request di Chat è solo testo (niente immagini né tag di asset) e inizia con un H1, che ne è il titolo.
- È scritta nella lingua delle impostazioni.
- Il comando è accettato solo dall'identità di Chat durante un suo turno, con gli stessi rifiuti dell'invio dello sviluppatore; un testo vuoto è rifiutato.
- Chat dice quale job è partito e non ne aspetta la fine, come per `weaver job new`.
