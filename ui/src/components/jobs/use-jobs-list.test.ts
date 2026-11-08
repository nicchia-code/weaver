import { describe, expect, it } from "vitest";

import { clampWidth, shownWidth } from "@/components/jobs/use-jobs-list";

describe("shownWidth", () => {
  it("shows the chosen width while the conversation keeps 480 px", () => {
    expect(shownWidth(280, 760)).toBe(280);
    expect(shownWidth(420, 900)).toBe(420);
  });

  it("narrows the list to leave the conversation 480 px, never under 240 px", () => {
    expect(shownWidth(400, 800)).toBe(320);
    expect(shownWidth(280, 700)).toBe(240);
    expect(shownWidth(400, 300)).toBe(240);
  });

  it("shows the chosen width in a window that is not measured yet", () => {
    expect(shownWidth(350, 0)).toBe(350);
  });
});

describe("clampWidth", () => {
  it("keeps a width within 240–420 px", () => {
    expect(clampWidth(100)).toBe(240);
    expect(clampWidth(300)).toBe(300);
    expect(clampWidth(999)).toBe(420);
  });
});
