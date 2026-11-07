import type { ComponentType, ReactNode } from "react";

import type { BoardCard } from "@/bindings/BoardCard";

/** The kinds of card the Board shows; the backend tags every card with one. */
export type CardKind = BoardCard["kind"];

/** A card of kind `K`, as the backend sends it. */
export type BoardCardOf<K extends CardKind> = Extract<BoardCard, { kind: K }>;

/** The permanent identity of a card (the id of the request, for a draft). */
export function cardIdOf(card: BoardCard): string {
  return card.kind === "request" ? card.id : card.head.id;
}

/**
 * How one kind of card looks on the Board and in its details dialog. The Board knows only this
 * shape: each kind of card (Proposal, resolution, Drift, Task) registers its own renderer, so
 * the work on one never touches the others or the Board itself.
 *
 * Both components are rendered inside the Board, so they may call `useBoard()`, `useBoardUi()`
 * and `useProject()`. `Card` is built on `CardShell`; the dialog around `Details` (title with the
 * id, close button, focus handling) belongs to the Board.
 */
export interface CardRenderer<K extends CardKind> {
  /** The card in its column. */
  Card: ComponentType<{ card: BoardCardOf<K> }>;
  /**
   * The body of the details dialog; it updates in place with each new picture. A kind without one
   * has no details dialog (the draft of a request opens the request editor instead).
   */
  Details?: ComponentType<{ card: BoardCardOf<K> }>;
  /** The buttons at the bottom of the details dialog. */
  Footer?: ComponentType<{ card: BoardCardOf<K> }>;
  /** A line under the title of the details dialog (the status, for instance). */
  description?: (card: BoardCardOf<K>) => ReactNode;
}

type Registry = { [K in CardKind]?: CardRenderer<K> };

/** A renderer as the Board uses it: it only ever hands a renderer the cards of its own kind. */
export interface AnyCardRenderer {
  Card: ComponentType<{ card: BoardCard }>;
  Details?: ComponentType<{ card: BoardCard }>;
  Footer?: ComponentType<{ card: BoardCard }>;
  description?: (card: BoardCard) => ReactNode;
}

const renderers: Registry = {};

/** Sets the renderer of a kind of card, replacing the one there was. */
export function registerCardRenderer<K extends CardKind>(kind: K, renderer: CardRenderer<K>): void {
  (renderers as Record<CardKind, unknown>)[kind] = renderer;
}

/** The renderer of a card's kind. */
export function rendererFor(card: BoardCard): AnyCardRenderer {
  const renderer = (renderers as Record<CardKind, unknown>)[card.kind] as AnyCardRenderer | undefined;
  if (!renderer) throw new Error(`No renderer is registered for the Board card kind \`${card.kind}\``);
  return renderer;
}
