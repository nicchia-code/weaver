import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { NotesView } from "@/components/notes/notes-view";
import { NotesProvider } from "@/components/notes/use-notes";
import { ProjectWindow } from "@/components/project-window";
import { TooltipProvider } from "@/components/ui/tooltip";

const { backend } = vi.hoisted(() => ({ backend: { call: vi.fn(), on: vi.fn(), onDragDrop: vi.fn() } }));
vi.mock("@/hooks/use-omp", () => ({ useOmp: () => ({ status: null }) }));
vi.mock("sonner", () => ({ toast: { error: vi.fn(), warning: vi.fn() } }));
vi.mock("@/lib/backend", () => ({
  call: backend.call,
  on: backend.on,
  onDragDrop: backend.onDragDrop,
  isAppError: (value: unknown) => typeof value === "object" && value !== null && "code" in value && "message" in value,
}));

const PROJECT = "0123456789abcdef";

beforeEach(() => {
  backend.on.mockReset().mockImplementation(() => Promise.resolve(() => {}));
  backend.onDragDrop.mockReset().mockImplementation(() => Promise.resolve(() => {}));
  backend.call.mockReset().mockImplementation(((name: string) => {
    if (name === "notes_read") return Promise.resolve({ text: "# Groceries\n\n- [ ] milk\n- [x] bread\n", hash: "h0" });
    if (name === "notes_write") return Promise.resolve({ hash: "h1" });
    return Promise.reject(new Error(`unexpected command ${name}`));
  }) as never);
});

describe("the Notes view with the real editor", () => {
  it("loads the editor on first use, shows the sheet formatted, and writes a tick at once", async () => {
    render(
      <TooltipProvider>
        <ProjectWindow
          projectId={PROJECT}
          initialView="notes"
          views={{ notes: <NotesView />, board: <p>Board content</p> }}
          around={(children) => <NotesProvider projectId={PROJECT}>{children}</NotesProvider>}
        />
      </TooltipProvider>,
    );
    expect(await screen.findByRole("textbox", { name: "Notes" })).toHaveAttribute("aria-multiline", "true");
    expect(screen.getByRole("heading", { level: 1, name: "Groceries" })).toBeInTheDocument();
    expect(document.body.textContent).not.toContain("# Groceries");
    expect(document.body.textContent).not.toContain("[ ]");
    const milk = await screen.findByRole("checkbox", { name: "milk" });
    expect(await screen.findByRole("checkbox", { name: "bread" })).toHaveAttribute("aria-checked", "true");

    fireEvent.click(milk);
    await waitFor(() => expect(backend.call).toHaveBeenCalledWith("notes_write", { projectId: PROJECT, text: "# Groceries\n\n- [x] milk\n- [x] bread\n" }));
    await waitFor(() => expect(screen.getByText("Saved")).toBeInTheDocument());
  });
});
