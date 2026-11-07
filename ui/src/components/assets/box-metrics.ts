import type { CSSProperties } from "react";

/**
 * The metrics of a message box with tags: padding, border width, size, wrapping (and, with `metricsClassName`,
 * the font family, the line height and the letter spacing). They are applied once, to the textarea and to the
 * mirror alike, so that a character sits on the same place in both layers.
 */
export const BOX_METRICS = "border px-3 py-2 text-sm whitespace-pre-wrap break-words";

/** The variants of a class (`md:`, `[&>p]:`…) are cut off before it is tested. */
const VARIANTS = /^(?:(?:\[[^\]]*\]|[^\s:[]+):)*/;

/** The utilities that move a character. */
const METRIC_UTILITY = new RegExp(
  [
    "^!?p[xytrblse]?-",
    "^!?border(?:-[xytrblse])?(?:-(?:\\d+|\\[[^\\]]*\\]))?$",
    "^!?(?:font|leading|tracking|whitespace|break|wrap|hyphens|indent)-",
    "^!?text-(?:xs|sm|base|lg|[2-9]xl|wrap|nowrap|balance|pretty|\\[\\d[^\\]]*\\]|\\[length:[^\\]]*\\])$",
    "^!?\\[(?:padding|border|font|letter-spacing|line-height|white-space|word-break|overflow-wrap|word-wrap|text-wrap|text-indent|hyphens)[a-z-]*:",
  ].join("|"),
);

const METRIC_STYLES = [
  "padding",
  "paddingTop",
  "paddingRight",
  "paddingBottom",
  "paddingLeft",
  "border",
  "borderWidth",
  "borderTopWidth",
  "borderRightWidth",
  "borderBottomWidth",
  "borderLeftWidth",
  "font",
  "fontFamily",
  "fontSize",
  "fontWeight",
  "lineHeight",
  "letterSpacing",
  "whiteSpace",
  "wordBreak",
  "overflowWrap",
  "wordWrap",
] as const;

/** The classes of `className` that move a character, in the order given. */
export function metricClasses(className: string | undefined): string[] {
  return (className ?? "").split(/\s+/).filter((token) => token !== "" && METRIC_UTILITY.test(token.replace(VARIANTS, "")));
}

/** A metric given to one layer only is a mistake, not a style to merge: it would move the characters of one layer away from the other's. */
export function refuseMetrics(prop: string, className: string | undefined) {
  const [token] = metricClasses(className);
  if (token !== undefined) {
    throw new Error(`TaggedTextarea: \`${token}\` in \`${prop}\` is a metric of the box; pass it in \`metricsClassName\`, which reaches both layers.`);
  }
}

export function refuseMetricStyles(style: CSSProperties | undefined) {
  const key = METRIC_STYLES.find((name) => style?.[name] !== undefined);
  if (key !== undefined) {
    throw new Error(`TaggedTextarea: \`style.${key}\` is a metric of the box; pass it in \`metricsClassName\`, which reaches both layers.`);
  }
}
