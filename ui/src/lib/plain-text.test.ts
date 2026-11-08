import { describe, expect, it } from "vitest";

import { firstParagraph, plainText } from "@/lib/plain-text";

describe("plainText", () => {
  it("removes the inline marks and keeps the text of links", () => {
    expect(plainText("**Bold** and `code` with [a link](https://example.com)")).toBe("Bold and code with a link");
  });
});

describe("firstParagraph", () => {
  it("takes the first block, joins its lines and drops the inline marks", () => {
    expect(firstParagraph("The worker starts `omp acp`\nas a **child** process.\n\nSecond paragraph.")).toBe("The worker starts omp acp as a child process.");
  });

  it("drops heading, list and quote marks, and skips fenced code", () => {
    expect(firstParagraph("```\ncode\n```\n\n- first item\n- second item")).toBe("first item second item");
    expect(firstParagraph("## Title\n\n> quoted")).toBe("Title");
  });

  it("is empty without a paragraph", () => {
    expect(firstParagraph("")).toBe("");
    expect(firstParagraph("\n\n```\ncode\n```")).toBe("");
  });
});
