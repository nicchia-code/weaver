import { describe, expect, it } from "vitest";

import { containsWords, searchWords, splitMatches } from "@/lib/highlight";

describe("searchWords", () => {
  it("splits at white space and drops empty and repeated words", () => {
    expect(searchWords("  daily   counters daily ")).toEqual(["daily", "counters"]);
    expect(searchWords("   ")).toEqual([]);
  });
});

describe("containsWords", () => {
  it("needs every word, whatever the case", () => {
    expect(containsWords("Daily counters per account", ["COUNTERS", "daily"])).toBe(true);
    expect(containsWords("Daily counters", ["counters", "weekly"])).toBe(false);
  });

  it("takes the characters of a regular expression literally", () => {
    expect(containsWords("a (b) c", ["(b)"])).toBe(true);
    expect(containsWords("abc", ["a.c"])).toBe(false);
  });

  it("matches any text when there are no words", () => {
    expect(containsWords("anything", [])).toBe(true);
  });
});

describe("splitMatches", () => {
  it("cuts the text at the matches and keeps their case", () => {
    expect(splitMatches("Daily counters, daily", ["daily"])).toEqual([
      { text: "Daily", match: true },
      { text: " counters, ", match: false },
      { text: "daily", match: true },
    ]);
  });

  it("prefers the longest word where words overlap", () => {
    expect(splitMatches("counters", ["count", "counters"])).toEqual([{ text: "counters", match: true }]);
  });

  it("returns the whole text unmatched without words or matches", () => {
    expect(splitMatches("abc", [])).toEqual([{ text: "abc", match: false }]);
    expect(splitMatches("abc", ["x"])).toEqual([{ text: "abc", match: false }]);
  });
});
