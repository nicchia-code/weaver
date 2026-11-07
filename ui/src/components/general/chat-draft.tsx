import { createContext, useCallback, useContext, useMemo, useState, type ReactNode } from "react";

import { useProject } from "@/components/project-context";

/** A text a view asks Chat to put in its message box; `nonce` tells one request from the next, even for the same text. */
export interface ChatDraft {
  text: string;
  nonce: number;
}

interface ChatDraftValue {
  /** The last request, or `null` until a view made one. */
  draft: ChatDraft | null;
  /** Shows Chat and asks it to put `text` in its message box, focused, with the cursor at the end. Nothing is sent. */
  ask: (text: string) => void;
}

const ChatDraftContext = createContext<ChatDraftValue | null>(null);

/** What views ask of Chat: a text in its message box. Chat reads the request with `useChatDraftRequest`. */
export function ChatDraftProvider({ children }: { children: ReactNode }) {
  const { showView } = useProject();
  const [draft, setDraft] = useState<ChatDraft | null>(null);
  const ask = useCallback(
    (text: string) => {
      setDraft((previous) => ({ text, nonce: (previous?.nonce ?? 0) + 1 }));
      showView("general");
    },
    [showView],
  );
  const value = useMemo(() => ({ draft, ask }), [draft, ask]);
  return <ChatDraftContext.Provider value={value}>{children}</ChatDraftContext.Provider>;
}

/** For a view that asks Chat for a draft. */
export function useChatDraft(): (text: string) => void {
  const value = useContext(ChatDraftContext);
  if (!value) throw new Error("useChatDraft needs a ChatDraftProvider");
  return value.ask;
}

/** For Chat: the last request of a view, or `null` (also without a provider, where nothing asks). */
export function useChatDraftRequest(): ChatDraft | null {
  return useContext(ChatDraftContext)?.draft ?? null;
}

/** The text of a message box after a request: what was there stays and the requested text goes on a new line after it. */
export function withDraft(current: string, requested: string): string {
  if (current === "") return requested;
  return `${current}${current.endsWith("\n") ? "" : "\n"}${requested}`;
}
