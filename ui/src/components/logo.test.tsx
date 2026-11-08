import { render } from "@testing-library/react";
import { readFileSync } from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";

import { Logo } from "@/components/logo";

const MASTER = path.resolve(import.meta.dirname, "../../../src-tauri/icons/source.svg");

/** The shapes of an SVG as `tag attr=value …` lines, attributes sorted, in drawing order. */
function shapes(root: Element): string[] {
  return Array.from(root.querySelectorAll("rect, path")).map((element) => {
    const attributes = Array.from(element.attributes)
      .map((attribute) => `${attribute.name}=${attribute.value}`)
      .sort();
    return `${element.tagName.toLowerCase()} ${attributes.join(" ")}`;
  });
}

describe("Logo", () => {
  it("draws the master of the app icon, with the five threads and the cream square", () => {
    const master = new DOMParser().parseFromString(readFileSync(MASTER, "utf8"), "image/svg+xml").documentElement;
    const { container } = render(<Logo />);
    const drawn = shapes(container.querySelector("svg")!);
    expect(drawn.length).toBe(6);
    expect(shapes(master)).toEqual(drawn);
    expect(drawn[0]).toContain("fill=#FBF6EE");
    expect(drawn[0]).toContain("stroke=#E7E5DF");
    expect(drawn.slice(1).map((line) => /stroke=(#[0-9A-F]{6})/.exec(line)![1])).toEqual([
      "#96BBE0",
      "#EDB08F",
      "#A996D6",
      "#E3A3B5",
      "#9DC09A",
    ]);
  });

  it("is decorative: hidden from assistive technologies, at the size asked", () => {
    const { container } = render(<Logo size={48} />);
    const svg = container.querySelector("svg")!;
    expect(svg).toHaveAttribute("aria-hidden", "true");
    expect(svg).toHaveAttribute("width", "48");
    expect(svg).toHaveAttribute("height", "48");
  });

  it("is the master centred on a square canvas: 896 of 1024 wide, nothing else drawn", () => {
    const text = readFileSync(MASTER, "utf8");
    expect(text).toContain('viewBox="0 0 1024 1024"');
    expect(text).toContain('translate(64 64) scale(29.866667)');
    // 30 units at that scale are 896 px: the rounded square, centred (64 px on every side).
    expect(30 * 29.866667).toBeCloseTo(896, 3);
  });
});
