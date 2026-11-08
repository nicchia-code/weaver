import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { TreeEntry } from "@/bindings/TreeEntry";
import { ImportPanel } from "@/components/project/import-panel";
import { ProjectStatusProvider } from "@/components/project/use-project-status";
import { TooltipProvider } from "@/components/ui/tooltip";
import { call, on } from "@/lib/backend";

vi.mock("@/lib/backend", () => ({
  call: vi.fn(),
  on: vi.fn(),
  isAppError: (value: unknown) => typeof value === "object" && value !== null && "code" in value && "message" in value,
}));

const PROJECT = "0123456789abcdef";
const dir = (path: string): TreeEntry => ({ name: path.split("/").pop() ?? path, path, folder: true });
const file = (path: string): TreeEntry => ({ name: path.split("/").pop() ?? path, path, folder: false });

const FOLDERS: Record<string, TreeEntry[]> = {
  "": [dir("docs"), dir("src"), file("README.md")],
  src: [file("src/main.ts"), file("src/lib.ts")],
  docs: [file("docs/guide.md")],
};

beforeEach(() => {
  vi.mocked(call).mockReset();
  vi.mocked(on).mockImplementation((() => Promise.resolve(() => {})) as unknown as typeof on);
  vi.mocked(call).mockImplementation(((name: string, args: { path: string }) => {
    if (name === "project_status") return Promise.reject(new Error("not needed"));
    if (name === "project_folder") return Promise.resolve(FOLDERS[args.path] ?? []);
    if (name === "import_code") return Promise.resolve(null);
    return Promise.reject(new Error(`unexpected command ${name}`));
  }) as unknown as typeof call);
});

function renderPanel(handlers: { onClose?: () => void; onStarted?: () => void } = {}) {
  return render(
    <TooltipProvider>
      <ProjectStatusProvider projectId={PROJECT}>
        <ImportPanel onClose={handlers.onClose ?? (() => {})} onStarted={handlers.onStarted ?? (() => {})} />
      </ProjectStatusProvider>
    </TooltipProvider>,
  );
}

describe("Import from Code panel", () => {
  it("shows the first level with the folders collapsed, and everything included", async () => {
    renderPanel();
    const tree = await screen.findByRole("tree", { name: "Files and folders to import" });
    const items = within(tree).getAllByRole("treeitem");
    expect(items.map((item) => item.textContent)).toEqual(["docs", "src", "README.md"]);
    expect(items[0]).toHaveAttribute("aria-expanded", "false");
    expect(items[2]).not.toHaveAttribute("aria-expanded");
    expect(items.every((item) => item.getAttribute("aria-level") === "1")).toBe(true);
    expect(screen.getByText("Everything is included")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Start import" })).not.toHaveAttribute("aria-disabled");
  });

  it("moves with the arrows, opens and closes with Right and Left, and toggles with Space", async () => {
    const user = userEvent.setup();
    renderPanel();
    const tree = await screen.findByRole("tree");
    await user.tab();
    const [docs, src] = within(tree).getAllByRole("treeitem");
    expect(docs).toHaveFocus();
    await user.keyboard("{ArrowDown}");
    expect(src).toHaveFocus();
    await user.keyboard("{ArrowRight}");
    expect(src).toHaveAttribute("aria-expanded", "true");
    const main = await screen.findByRole("treeitem", { name: "main.ts" });
    expect(main).toHaveAttribute("aria-level", "2");
    await user.keyboard("{ArrowRight}");
    expect(main).toHaveFocus();
    await user.keyboard(" ");
    expect(main).toHaveAttribute("aria-checked", "false");
    expect(src).toHaveAttribute("aria-checked", "mixed");
    expect(screen.getByText("1 excluded")).toBeInTheDocument();
    await user.keyboard("{ArrowLeft}");
    expect(src).toHaveFocus();
    await user.keyboard("{ArrowLeft}");
    expect(src).toHaveAttribute("aria-expanded", "false");
    await user.keyboard("{End}");
    expect(screen.getByRole("treeitem", { name: "README.md" })).toHaveFocus();
    await user.keyboard("{Home}");
    expect(docs).toHaveFocus();
  });

  it("excludes everything inside a folder and disables Start import, with its reason, when nothing is included", async () => {
    const user = userEvent.setup();
    renderPanel();
    await screen.findByRole("tree");
    await user.tab();
    await user.keyboard(" ");
    await user.keyboard("{ArrowDown} ");
    await user.keyboard("{ArrowDown} ");
    expect(screen.getByText("3 excluded")).toBeInTheDocument();
    const start = screen.getByRole("button", { name: "Start import" });
    expect(start).toHaveAttribute("aria-disabled", "true");
    expect(start).toHaveAccessibleDescription("Include at least one file or folder.");
  });

  it("starts the import with the excluded paths, relative to the project folder", async () => {
    const user = userEvent.setup();
    const onStarted = vi.fn();
    renderPanel({ onStarted });
    await screen.findByRole("tree");
    await user.tab();
    await user.keyboard(" ");
    await user.click(screen.getByRole("button", { name: "Start import" }));
    expect(call).toHaveBeenCalledWith("import_code", { projectId: PROJECT, excluded: ["docs"] });
    expect(onStarted).toHaveBeenCalled();
  });

  it("shows why the import could not start and keeps the exclusions", async () => {
    const user = userEvent.setup();
    const onStarted = vi.fn();
    vi.mocked(call).mockImplementation(((name: string, args: { path: string }) => {
      if (name === "project_folder") return Promise.resolve(FOLDERS[args.path] ?? []);
      if (name === "import_code") return Promise.reject({ code: "refused", message: "Could not start the import: the agent is not ready." });
      return Promise.reject(new Error("unexpected"));
    }) as unknown as typeof call);
    renderPanel({ onStarted });
    await screen.findByRole("tree");
    await user.tab();
    await user.keyboard(" ");
    await user.click(screen.getByRole("button", { name: "Start import" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Could not start the import: the agent is not ready.");
    expect(onStarted).not.toHaveBeenCalled();
    expect(screen.getByText("1 excluded")).toBeInTheDocument();
  });

  it("leaves the panel with Cancel", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    renderPanel({ onClose });
    await user.click(await screen.findByRole("button", { name: "Cancel" }));
    expect(onClose).toHaveBeenCalled();
  });
});
