import { readFileSync } from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";

// The tokens of `requirements/ui#General` live in `index.css`; this reads them from there, so the
// contrasts of `requirements/quality#General` (4.5:1 for text, 3:1 for icons, borders and focus) are
// checked on the values the app ships, in both themes.
const CSS = readFileSync(path.resolve(import.meta.dirname, "index.css"), "utf8");

function block(selector: string): Record<string, string> {
  const start = CSS.indexOf(`\n${selector} {`);
  const body = CSS.slice(start, CSS.indexOf("\n}", start));
  const values: Record<string, string> = {};
  for (const [, name, value] of body.matchAll(/^\s*--([\w-]+):\s*([^;]+);/gm)) values[name!] = value!.trim();
  return values;
}

const light = block(":root");
const dark = { ...light, ...block(".dark") };

/** Resolves `var(--x)` references, then returns `#rrggbb`. */
function colour(theme: Record<string, string>, name: string): string {
  let value = theme[name];
  while (value?.startsWith("var(")) value = theme[value.slice(6, -1)];
  if (!value?.startsWith("#")) throw new Error(`--${name} is not a hex colour: ${value}`);
  return value.length === 4 ? `#${[...value.slice(1)].map((c) => c + c).join("")}` : value;
}

function channel(value: number): number {
  const c = value / 255;
  return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
}

function luminance(hex: string): number {
  const n = Number.parseInt(hex.slice(1), 16);
  return 0.2126 * channel((n >> 16) & 255) + 0.7152 * channel((n >> 8) & 255) + 0.0722 * channel(n & 255);
}

function contrast(a: string, b: string): number {
  const [high = 0, low = 0] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (high + 0.05) / (low + 0.05);
}

const THREADS = ["sky", "sage", "peach", "lavender", "rose"] as const;

describe("light tokens are the ones of the Requirements", () => {
  it("has the neutrals", () => {
    const expected = {
      paper: "#f6f5f1",
      surface: "#ffffff",
      lane: "#faf9f6",
      line: "#e7e5df",
      "line-control": "#e0ddd5",
      "line-soft": "#efede7",
      ink: "#18181b",
      "ink-2": "#52525b",
      "muted-ink": "#6b6b73",
    };
    for (const [name, value] of Object.entries(expected)) expect(colour(light, name), name).toBe(value);
  });

  it("has the five threads, each with base, tint and ink", () => {
    const expected = {
      sky: ["#96bbe0", "#e6eff8", "#2e5f8f"],
      sage: ["#9dc09a", "#e9f2e7", "#3f6b3c"],
      peach: ["#edb08f", "#fbede4", "#9a4a22"],
      lavender: ["#a996d6", "#efeaf8", "#5b4a99"],
      rose: ["#e3a3b5", "#f9e8ed", "#9b3b57"],
    } as const;
    for (const thread of THREADS) {
      expect([colour(light, thread), colour(light, `${thread}-tint`), colour(light, `${thread}-ink`)], thread).toEqual(expected[thread]);
    }
  });

  it("makes Ink the only colour of actions: primary, ring and focus are Ink, with no blue left", () => {
    expect(light.primary).toBe("var(--ink)");
    expect(light.ring).toBe("var(--ink)");
    expect(colour(light, "primary-foreground")).toBe("#ffffff");
    expect(light.destructive).toBe("var(--rose-ink)");
    expect(colour(light, "rose-line")).toBe("#ebcbd4");
  });
});

for (const [name, theme] of [
  ["light", light],
  ["dark", dark],
] as const) {
  describe(`${name} contrasts`, () => {
    const c = (a: string, b: string) => contrast(colour(theme, a), colour(theme, b));

    it("keeps text at 4.5:1 on Paper, Surface and Lane", () => {
      for (const ground of ["paper", "surface", "lane"]) {
        for (const text of ["ink", "ink-2", "muted-ink"]) expect(c(text, ground), `${text} on ${ground}`).toBeGreaterThanOrEqual(4.5);
      }
    });

    it("keeps the ink of each thread at 4.5:1 on its tint and on Surface", () => {
      for (const thread of THREADS) {
        expect(c(`${thread}-ink`, `${thread}-tint`), `${thread} ink on tint`).toBeGreaterThanOrEqual(4.5);
        expect(c(`${thread}-ink`, "surface"), `${thread} ink on Surface`).toBeGreaterThanOrEqual(4.5);
      }
    });

    it("keeps primary buttons readable, and the focus ring, the unchecked border and the Ink on Surface at 3:1", () => {
      expect(c("primary-foreground", "ink")).toBeGreaterThanOrEqual(4.5);
      expect(c("ring", "surface")).toBeGreaterThanOrEqual(3);
      expect(c("ring", "paper")).toBeGreaterThanOrEqual(3);
      expect(c("control", "surface")).toBeGreaterThanOrEqual(3);
    });

    it("has Surface the lighter of the two grounds in dark, and the same roles in both", () => {
      expect(luminance(colour(theme, "surface"))).toBeGreaterThanOrEqual(luminance(colour(theme, "paper")));
      expect(Object.keys(theme).sort()).toEqual(Object.keys(light).sort());
    });
  });
}

describe("dark threads keep their hue with a darker tint and a lighter ink", () => {
  for (const thread of THREADS) {
    it(thread, () => {
      expect(luminance(colour(dark, `${thread}-tint`))).toBeLessThan(luminance(colour(light, `${thread}-tint`)));
      expect(luminance(colour(dark, `${thread}-ink`))).toBeGreaterThan(luminance(colour(light, `${thread}-ink`)));
    });
  }
});
