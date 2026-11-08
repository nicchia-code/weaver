import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { Markdown } from "@/components/markdown";
import { ThemeProvider } from "@/components/theme-provider";
import { call } from "@/lib/backend";

vi.mock("@/lib/backend", () => ({ call: vi.fn(() => Promise.resolve(null)), on: vi.fn() }));
vi.mock("sonner", () => ({ toast: { error: vi.fn() }, Toaster: () => null }));

const mermaid = vi.hoisted(() => ({
  initialize: vi.fn(),
  render: vi.fn(),
}));
vi.mock("mermaid", () => ({ default: mermaid }));

beforeEach(() => {
  mermaid.initialize.mockReset();
  mermaid.render.mockReset();
  vi.mocked(call).mockClear();
});

describe("Markdown", () => {
  it("shows raw HTML as text and never interprets it", () => {
    const { container } = render(<Markdown>{'Hello <b>bold</b>\n\n<script>alert(1)</script>\n\n<img src="x" onerror="boom()">'}</Markdown>);
    expect(container.querySelector("b")).toBeNull();
    expect(container.querySelector("script")).toBeNull();
    expect(container.querySelector("img")).toBeNull();
    expect(container).toHaveTextContent("<b>bold</b>");
    expect(container).toHaveTextContent("<script>alert(1)</script>");
  });

  it("renders GFM: table in its own scrolling box, strikethrough, read-only task list, autolink, code", () => {
    const { container } = render(
      <Markdown>
        {[
          "| a | b |",
          "|---|---|",
          "| 1 | 2 |",
          "",
          "~~gone~~",
          "",
          "- [x] done",
          "- [ ] todo",
          "",
          "see https://example.com/page",
          "",
          "```ts",
          "const x = 1;",
          "```",
          "",
          "and `inline`",
        ].join("\n")}
      </Markdown>,
    );
    const table = screen.getByRole("table");
    expect(table.parentElement).toHaveClass("overflow-x-auto");
    expect(container.querySelector("del")).toHaveTextContent("gone");
    const boxes = screen.getAllByRole("checkbox");
    expect(boxes.map((b) => (b as HTMLInputElement).checked)).toEqual([true, false]);
    for (const box of boxes) {
      expect(box).toBeDisabled();
      expect(box).toHaveAccessibleName(/Done|Not done/);
    }
    expect(screen.getByRole("link", { name: "https://example.com/page" })).toHaveAttribute("href", "https://example.com/page");
    const block = container.querySelector("pre");
    expect(block).toHaveClass("overflow-x-auto");
    expect(block).toHaveTextContent("const x = 1;");
    expect(screen.getByText("inline").tagName).toBe("CODE");
  });

  it("opens http and https links in the system browser through the backend, never in the webview", async () => {
    const user = userEvent.setup();
    render(<Markdown>{"[docs](https://example.com/docs) and [plain](http://example.org)"}</Markdown>);
    await user.click(screen.getByRole("link", { name: "docs" }));
    expect(call).toHaveBeenCalledWith("open_url", { url: "https://example.com/docs" });
    await user.click(screen.getByRole("link", { name: "plain" }));
    expect(call).toHaveBeenLastCalledWith("open_url", { url: "http://example.org" });
  });

  it("renders links that are not http or https as plain text and opens nothing", async () => {
    const user = userEvent.setup();
    render(
      <Markdown>
        {"[js](javascript:alert(1)) [file](file:///etc/passwd) [mail](mailto:a@b.c) [rel](../x.md) [anchor](#top) [data](data:text/html,x)"}
      </Markdown>,
    );
    expect(screen.queryByRole("link")).not.toBeInTheDocument();
    for (const text of ["js", "file", "mail", "rel", "anchor", "data"]) {
      await user.click(screen.getByText(text));
    }
    expect(call).not.toHaveBeenCalled();
  });

  it("shows a remote image as a link and never fetches it", () => {
    const { container } = render(<Markdown>{"![a cat](https://example.com/cat.png) ![tracker](//evil.test/x.gif) ![inline](data:image/png;base64,AAAA)"}</Markdown>);
    expect(container.querySelector("img")).toBeNull();
    expect(screen.getByRole("link", { name: "a cat" })).toHaveAttribute("href", "https://example.com/cat.png");
    expect(screen.getByText("tracker")).toBeInTheDocument();
    expect(screen.getByText("inline")).toBeInTheDocument();
    expect(screen.getAllByRole("link")).toHaveLength(1);
  });

  it("shows an image inside the project only through the resolver, and never one outside it", () => {
    const resolveImage = vi.fn((path: string) => `asset://project/${path}`);
    const { container } = render(<Markdown resolveImage={resolveImage}>{"![shot](images/image-1.png) ![up](../secret.png) ![abs](/etc/passwd)"}</Markdown>);
    expect(resolveImage).toHaveBeenCalledTimes(1);
    expect(resolveImage).toHaveBeenCalledWith("images/image-1.png");
    const images = container.querySelectorAll("img");
    expect(images).toHaveLength(1);
    expect(images[0]).toHaveAttribute("src", "asset://project/images/image-1.png");
    expect(images[0]).toHaveAttribute("alt", "shot");
    expect(screen.getByText("up")).toBeInTheDocument();
    expect(screen.getByText("abs")).toBeInTheDocument();
  });

  it("shows an image of the project as its alt text when nothing can serve it", () => {
    const { container } = render(<Markdown>{"![shot](images/image-1.png)"}</Markdown>);
    expect(container.querySelector("img")).toBeNull();
    expect(screen.getByText("shot")).toBeInTheDocument();
  });

  it("never lets a heading be higher than one level below its holder, nor larger than body text", () => {
    render(<Markdown holderHeadingLevel={2}>{"# One\n\n## Two\n\n#### Four"}</Markdown>);
    const levels = screen.getAllByRole("heading").map((h) => [h.textContent, h.tagName]);
    expect(levels).toEqual([
      ["One", "H3"],
      ["Two", "H3"],
      ["Four", "H4"],
    ]);
    for (const heading of screen.getAllByRole("heading")) {
      expect(heading).toHaveClass("text-sm");
      expect(heading.className).not.toMatch(/text-(base|lg|xl|2xl|3xl)/);
    }
  });

  it("never scrolls sideways outside tables and code: long text wraps", () => {
    const { container } = render(<Markdown>{"x".repeat(300)}</Markdown>);
    expect(container.firstElementChild).toHaveClass("break-words", "max-w-full", "min-w-0");
    expect(container.firstElementChild?.className).not.toMatch(/overflow-x-(auto|scroll)/);
  });
});

describe("mermaid blocks", () => {
  const diagram = "```mermaid\ngraph TD; A-->B;\n```";

  it("loads mermaid on first use with strict security and the current theme, and draws the diagram", async () => {
    mermaid.render.mockResolvedValue({ svg: '<svg xmlns="http://www.w3.org/2000/svg" data-testid="drawn"><g/></svg>' });
    const { unmount } = render(
      <ThemeProvider preference="dark">
        <Markdown>{diagram}</Markdown>
      </ThemeProvider>,
    );
    expect(await screen.findByTestId("drawn")).toBeInTheDocument();
    expect(mermaid.initialize).toHaveBeenCalledWith({ startOnLoad: false, securityLevel: "strict", theme: "dark" });
    expect(mermaid.render).toHaveBeenCalledWith(expect.stringMatching(/^mermaid-/), "graph TD; A-->B;");
    unmount();

    mermaid.render.mockResolvedValue({ svg: '<svg xmlns="http://www.w3.org/2000/svg" data-testid="light"><g/></svg>' });
    render(
      <ThemeProvider preference="light">
        <Markdown>{diagram}</Markdown>
      </ThemeProvider>,
    );
    expect(await screen.findByTestId("light")).toBeInTheDocument();
    expect(mermaid.initialize).toHaveBeenLastCalledWith({ startOnLoad: false, securityLevel: "strict", theme: "default" });
  });

  it("shows a diagram that fails as a code block with the error under it in muted text", async () => {
    mermaid.render.mockRejectedValue(new Error("Parse error on line 1"));
    const { container } = render(<Markdown>{diagram}</Markdown>);
    const error = await screen.findByText("Parse error on line 1");
    expect(error).toHaveClass("text-muted-foreground");
    const code = container.querySelector("pre");
    expect(code).toHaveTextContent("graph TD; A-->B;");
    expect(code?.compareDocumentPosition(error)).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
    await waitFor(() => expect(mermaid.render).toHaveBeenCalledTimes(1));
  });

  it("draws nothing it was not given: other code blocks do not load mermaid", () => {
    render(<Markdown>{"```js\nlet a;\n```"}</Markdown>);
    expect(mermaid.render).not.toHaveBeenCalled();
    expect(mermaid.initialize).not.toHaveBeenCalled();
  });
});

describe("Markdown highlight", () => {
  it("wraps the matches of the words in marks, inside formatting and lists, whatever the case", () => {
    const { container } = render(<Markdown highlight={["daily", "age"]}>{"Daily **counters** per page\n\n- Another DAILY item"}</Markdown>);
    expect([...container.querySelectorAll("mark")].map((mark) => mark.textContent)).toEqual(["Daily", "age", "DAILY"]);
    expect(container).toHaveTextContent("Daily counters per page");
  });

  it("leaves code blocks as they are and marks nothing without words", () => {
    const { container } = render(<Markdown highlight={["let"]}>{"```js\nlet a;\n```\n\nlet it be"}</Markdown>);
    expect(container.querySelector("pre mark")).toBeNull();
    expect(container.querySelectorAll("mark")).toHaveLength(1);
    const plain = render(<Markdown>{"let it be"}</Markdown>);
    expect(plain.container.querySelector("mark")).toBeNull();
  });
});
