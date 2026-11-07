import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { toast } from "sonner";

import type { ActionInput } from "@/bindings/ActionInput";
import type { CardActionInput } from "@/bindings/CardActionInput";
import type { ChatCardView } from "@/bindings/ChatCardView";
import type { ChatCards } from "@/bindings/ChatCards";
import { call, isAppError, on } from "@/lib/backend";

const NONE: readonly ChatCardView[] = [];

export interface ChatCardsData {
  /** The cards of the conversation, in publication order; the backend keeps them, drafts included. */
  cards: readonly ChatCardView[];
  /** The one active card (always the last), or `null`. */
  active: ChatCardView | null;
  /** What the developer does in a card; `send` is its own command. Only the active card takes actions. */
  act: (card: string, action: ActionInput) => void;
}

/** The action of a card, or `null` for those that a card does not have (`Minimize`, `Skip all`) or that are their own command. */
function cardAction(action: ActionInput): CardActionInput | null {
  switch (action.kind) {
    case "go":
    case "draft":
    case "confirm":
    case "skip":
      return action;
    default:
      return null;
  }
}

/** What is on screen once a draft was written, before the backend answers. */
function withDraft(cards: readonly ChatCardView[], card: string, action: Extract<CardActionInput, { kind: "draft" }>): readonly ChatCardView[] {
  return cards.map((view) =>
    view.id !== card
      ? view
      : {
          ...view,
          questions: view.questions.map((question, index) =>
            index === action.index ? { ...question, draft: { selected: action.selected, text: action.text } } : question,
          ),
        },
  );
}

/**
 * The cards of questions of the Chat of a project: the UI listens to `chat-cards-changed`, then asks,
 * and drops an answer older than the revision it shows. The UI keeps no copy beyond this, except the
 * draft being written, which shows at once and which any event replaces. Actions go to the backend
 * one after the other.
 */
export function useChatCards(projectId: string): ChatCardsData {
  const [cards, setCards] = useState<readonly ChatCardView[]>(NONE);
  const latest = useRef<readonly ChatCardView[]>(NONE);
  const revision = useRef(-1);
  const alive = useRef(true);
  const chain = useRef<Promise<void>>(Promise.resolve());
  const issued = useRef(0);

  const show = useCallback((incoming: ChatCards) => {
    if (!alive.current || incoming.revision < revision.current) return;
    revision.current = incoming.revision;
    latest.current = incoming.cards;
    setCards(incoming.cards);
  }, []);

  useEffect(() => {
    alive.current = true;
    revision.current = -1;
    latest.current = NONE;
    setCards(NONE);
    // Listen first, then ask: a change between the two is not lost, and an older answer is dropped.
    const changed = on("chat-cards-changed", (event) => {
      if (event.projectId === projectId) show(event);
    });
    call("general_cards", { projectId })
      .then((incoming) => {
        if (incoming.projectId === projectId) show(incoming);
      })
      .catch(() => {});
    return () => {
      alive.current = false;
      changed.then((unlisten) => unlisten()).catch(() => {});
    };
  }, [projectId, show]);

  const act = useCallback(
    (card: string, action: ActionInput) => {
      const target = latest.current.find((view) => view.id === card);
      if (!target || target.state !== "active") return;
      const mapped = cardAction(action);
      if (!mapped && action.kind !== "send") return;
      const ticket = ++issued.current;
      if (mapped?.kind === "draft") {
        latest.current = withDraft(latest.current, card, mapped);
        setCards(latest.current);
      }
      chain.current = chain.current.then(async () => {
        try {
          if (!mapped) {
            await call("general_send_answers", { projectId, card });
            return;
          }
          const next = await call("general_card_act", { projectId, card, action: mapped });
          // A newer action is on its way: its answer is the one that counts.
          if (ticket === issued.current && next.projectId === projectId) show(next);
        } catch (error) {
          if (alive.current) toast.error(isAppError(error) ? error.message : "The answer could not be recorded.");
        }
      });
    },
    [projectId, show],
  );

  const active = useMemo(() => cards.find((view) => view.state === "active") ?? null, [cards]);

  return useMemo(() => ({ cards, active, act }), [cards, active, act]);
}
