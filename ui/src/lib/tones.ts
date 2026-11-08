// Colour tells only a state, with the five threads of the logo (`requirements/ui#General`): Sky for
// queued, running and refining and whatever an agent is doing now; Sage for done, validated,
// connected and success; Peach for waiting for the developer, stale, blocked, almost used up, a Drift
// and warnings; Lavender for a Proposal, a question round and the agent's choices; Rose for failed,
// stopped, used up, errors and destructive controls. Suspended and every other state stay neutral
// (`text-muted-foreground` on `bg-muted`). Each thread has a base (dots, bars, stripes), a tint
// (backgrounds of badges and boxes) and an ink (text and icons on the tint); the CSS tokens live in
// `index.css`. Colour never stands alone: whoever uses a tone also shows a text, an icon or a badge
// and an accessible name.

export const TONES = ["sky", "sage", "peach", "lavender", "rose"] as const;
export type Tone = (typeof TONES)[number];

// Full class names, so Tailwind sees them.

/** The ink of the thread: text and icons, on Surface or on the tint. */
export const TONE_TEXT: Record<Tone, string> = {
  sky: "text-sky-ink",
  sage: "text-sage-ink",
  peach: "text-peach-ink",
  lavender: "text-lavender-ink",
  rose: "text-rose-ink",
};

/** The base of the thread as a border: boxes, alerts, toasts. */
export const TONE_BORDER: Record<Tone, string> = {
  sky: "border-sky",
  sage: "border-sage",
  peach: "border-peach",
  lavender: "border-lavender",
  rose: "border-rose",
};

/** The base of the thread as a 3 px left stripe. */
export const TONE_STRIPE: Record<Tone, string> = {
  sky: "border-l-[3px] border-l-sky",
  sage: "border-l-[3px] border-l-sage",
  peach: "border-l-[3px] border-l-peach",
  lavender: "border-l-[3px] border-l-lavender",
  rose: "border-l-[3px] border-l-rose",
};

/** The tint of the thread: the background of a badge or a box. */
export const TONE_TINT: Record<Tone, string> = {
  sky: "bg-sky-tint",
  sage: "bg-sage-tint",
  peach: "bg-peach-tint",
  lavender: "bg-lavender-tint",
  rose: "bg-rose-tint",
};

/** The base of the thread as a fill: dots, bars. */
export const TONE_DOT: Record<Tone, string> = {
  sky: "bg-sky",
  sage: "bg-sage",
  peach: "bg-peach",
  lavender: "bg-lavender",
  rose: "bg-rose",
};

/** Ink on tint: the look of a pill. */
export const TONE_PILL: Record<Tone, string> = {
  sky: "bg-sky-tint text-sky-ink",
  sage: "bg-sage-tint text-sage-ink",
  peach: "bg-peach-tint text-peach-ink",
  lavender: "bg-lavender-tint text-lavender-ink",
  rose: "bg-rose-tint text-rose-ink",
};

/** Meanings of the interface, by tone: success, warning, error, information. */
export const SEMANTIC_TONE = { success: "sage", warning: "peach", error: "rose", info: "sky" } as const satisfies Record<
  string,
  Tone
>;
export type Semantic = keyof typeof SEMANTIC_TONE;
