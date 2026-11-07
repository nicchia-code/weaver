import { fireEvent, within } from "@testing-library/react";
import { expect } from "vitest";

import { metricClasses } from "@/components/assets/box-metrics";

/**
 * jsdom does no layout and loads no style sheet, so the geometry of a box with tags is checked on the classes its two
 * layers receive: the textarea and the mirror carry the same padding, border width, font, size, line height, letter
 * spacing and wrapping, and nothing around them carries a metric of its own.
 */
export function expectOneGeometry(box: HTMLElement, expected: { base?: string[]; has?: string[]; lacks?: string[] } = {}) {
  const frame = box.parentElement as HTMLElement;
  const mirror = within(frame).getByTestId("tag-mirror");
  const own = metricClasses(box.className).sort();
  expect(own).toEqual(metricClasses(mirror.className).sort());
  const base = expected.base ?? ["border", "px-3", "py-2", "text-sm", "whitespace-pre-wrap", "break-words"];
  expect(own).toEqual(expect.arrayContaining([...base, ...(expected.has ?? [])]));
  for (const lacking of expected.lacks ?? []) expect(own).not.toContain(lacking);
  expect(metricClasses(frame.className)).toEqual([]);
  // The marker that measures the caret adds neither width nor a place to break a line.
  expect(frame.querySelector("[data-testid='tag-caret']")?.className ?? "").toBe("");
}

/**
 * Enter or Tab that confirm a composition of the input method: the box sends nothing and picks nothing. Enter is left
 * to the input method untouched; the default action of Tab, which would move the focus, is cancelled.
 */
export function pressWhileComposing(box: HTMLElement, key: "Enter" | "Tab") {
  const defaultRan = fireEvent.keyDown(box, { key, isComposing: true });
  expect(defaultRan).toBe(key === "Enter");
}
