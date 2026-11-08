import type { DragDropEvent } from "@tauri-apps/api/webview";
import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { NotesChanged } from "@/bindings/NotesChanged";
import { AUTOSAVE_MS, EMPTY_TEXT } from "@/components/notes/notes-model";
import type { DropPoint, EditorEvents, InsertAnchor, NotesEditorModule, NotesPort } from "@/components/notes/notes-session";
import { NotesView } from "@/components/notes/notes-view";
import { NotesProvider } from "@/components/notes/use-notes";
import { ProjectWindow } from "@/components/project-window";
import { TooltipProvider } from "@/components/ui/tooltip";

const { backend, toast } = vi.hoisted(() => ({
  backend: {
    call: vi.fn(),
    on: vi.fn(),
    onDragDrop: vi.fn(),
    handlers: new Map<string, Set<(payload: never) => void>>(),
    drags: new Set<(event: DragDropEvent) => void>(),
  },
  toast: { error: vi.fn(), warning: vi.fn(), loading: vi.fn(), dismiss: vi.fn() },
}));

vi.mock("@/hooks/use-omp", () => ({ useOmp: () => ({ status: null }) }));
vi.mock("sonner", () => ({ toast }));
vi.mock("@/lib/backend", () => ({
  call: backend.call,
  on: backend.on,
  onDragDrop: backend.onDragDrop,
  isAppError: (value: unknown) => typeof value === "object" && value !== null && "code" in value && "message" in value,
}));

const PROJECT = "0123456789abcdef";

/** What the fake editor is: a text the test changes, and what the view asked of it. */
interface Fake {
  text: string;
  events: EditorEvents;
  port: NotesPort;
  anchor: InsertAnchor;
  created: ReturnType<typeof vi.fn>;
  drop: DropPoint | null;
}

let fake: Fake;
let disk: { text: string | null; hash: string | null };
let calls: [string, Record<string, unknown>][];
let writeAnswer: (text: string) => Promise<{ hash: string }>;
let counter: number;

function makeFake(): { module: NotesEditorModule; fake: Fake } {
  const state = { text: "", drop: null as DropPoint | null } as Fake;
  state.anchor = { insert: vi.fn(), release: vi.fn() };
  state.port = {
    getMarkdown: vi.fn(() => state.text),
    replaceFromDisk: vi.fn((markdown: string) => {
      state.text = markdown;
    }),
    focus: vi.fn(),
    track: vi.fn(() => state.anchor),
    dropPoint: vi.fn(() => state.drop),
    destroy: vi.fn(),
  };
  state.created = vi.fn(({ markdown, events }: { markdown: string; events: EditorEvents }) => {
    state.text = markdown;
    state.events = events;
    return state.port;
  });
  const module: NotesEditorModule = {
    createNotesEditor: state.created as unknown as NotesEditorModule["createNotesEditor"],
    NotesSheet: () => <div data-testid="sheet" />,
  };
  return { module, fake: state };
}

function backendCall(name: string, args: Record<string, unknown>): Promise<unknown> {
  calls.push([name, args]);
  switch (name) {
    case "notes_read":
      return Promise.resolve({ ...disk });
    case "notes_write":
      return writeAnswer(args.text as string);
    case "notes_trash":
      return Promise.resolve({ refused: [] });
    case "notes_flushed":
      return Promise.resolve(null);
    case "notes_add_dropped":
    case "notes_add_pasted":
      return Promise.resolve({ files: [{ status: "added", name: "pic.png" }] });
    case "project_status":
      return Promise.resolve({ state: "has-model" });
    default:
      return Promise.reject(new Error(`unexpected command ${name}`));
  }
}

const called = (name: string) => calls.filter(([n]) => n === name).map(([, args]) => args);
const emit = <T,>(name: string, payload: T) => {
  for (const handler of backend.handlers.get(name) ?? []) (handler as (payload: T) => void)(payload);
};
const changed = (patch: Partial<NotesChanged>): NotesChanged => ({ projectId: PROJECT, generation: ++counter, sheetChanged: false, hash: null, images: [], ...patch });

beforeEach(() => {
  counter = 0;
  calls = [];
  disk = { text: "# Notes\n", hash: "h0" };
  writeAnswer = () => Promise.resolve({ hash: `h${++counter}` });
  backend.handlers.clear();
  backend.drags.clear();
  toast.error.mockReset();
  toast.warning.mockReset();
  backend.call.mockReset().mockImplementation(((name: string, args: Record<string, unknown>) => backendCall(name, args)) as never);
  backend.on.mockReset().mockImplementation(((name: string, handler: (payload: never) => void) => {
    const set = backend.handlers.get(name) ?? new Set();
    set.add(handler);
    backend.handlers.set(name, set);
    return Promise.resolve(() => set.delete(handler));
  }) as never);
  backend.onDragDrop.mockReset().mockImplementation(((handler: (event: DragDropEvent) => void) => {
    backend.drags.add(handler);
    return Promise.resolve(() => backend.drags.delete(handler));
  }) as never);
  document.elementFromPoint = vi.fn(() => document.querySelector("[data-testid=sheet]"));
  vi.useFakeTimers({ shouldAdvanceTime: false });
});

afterEach(() => {
  vi.useRealTimers();
});

const settle = () =>
  act(async () => {
    for (let turn = 0; turn < 10; turn++) await vi.advanceTimersByTimeAsync(0);
  });
const advance = (ms: number) => act(async () => void (await vi.advanceTimersByTimeAsync(ms)));

async function renderView(initialView: "notes" | "board" = "notes") {
  const made = makeFake();
  fake = made.fake;
  render(
    <TooltipProvider>
      <ProjectWindow
        projectId={PROJECT}
        initialView={initialView}
        views={{ notes: <NotesView />, board: <p>Board content</p> }}
        around={(children) => <NotesProvider projectId={PROJECT} loadModule={() => Promise.resolve(made.module)}>{children}</NotesProvider>}
      />
    </TooltipProvider>,
  );
  await settle();
}

const goTo = async (name: string) => {
  fireEvent.click(screen.getByRole("button", { name }));
  await settle();
};

const status = () => screen.getAllByRole("status").find((element) => element.closest("header")) as HTMLElement;
const typeText = (text: string) => {
  fake.text = text;
  act(() => fake.events.edited());
};

describe("opening the view", () => {
  it("reads nothing while the view is not shown, and starts with the header row titled Notes", async () => {
    await renderView("board");
    expect(called("notes_read")).toHaveLength(0);
    expect(fake.created).not.toHaveBeenCalled();
    await goTo("Notes");
    expect(screen.getByRole("heading", { level: 1, name: "Notes" })).toBeInTheDocument();
    expect(called("notes_read")).toHaveLength(1);
    expect(fake.created).toHaveBeenCalledWith(expect.objectContaining({ markdown: "# Notes\n" }));
    expect(screen.getByTestId("sheet")).toBeInTheDocument();
  });

  it("shows the empty sheet for a sheet that does not exist yet, and writes nothing by itself", async () => {
    disk = { text: null, hash: null };
    await renderView();
    expect(fake.created).toHaveBeenCalledWith(expect.objectContaining({ markdown: "" }));
    await advance(5000);
    expect(called("notes_write")).toHaveLength(0);
    expect(EMPTY_TEXT).toBe("Write notes for this project. Start a line with [ ] for a checklist.");
  });

  it("shows nothing at the right of the header before the first change of the session", async () => {
    await renderView();
    expect(status()).toHaveTextContent("");
  });

  it("says why the sheet could not be read, and reads again on Retry", async () => {
    const answer = backend.call.getMockImplementation();
    backend.call.mockImplementation(((name: string, args: Record<string, unknown>) =>
      name === "notes_read" ? Promise.reject({ code: "io", message: "permission denied" }) : answer?.(name as never, args as never)) as never);
    await renderView();
    expect(screen.getByRole("alert")).toHaveTextContent("Could not read the notes: permission denied.");
    expect(fake.created).not.toHaveBeenCalled();
    backend.call.mockImplementation(answer as never);
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    await settle();
    expect(fake.created).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("follows the notes only while it is shown", async () => {
    await renderView();
    expect(backend.handlers.get("notes-changed")?.size).toBe(1);
    await goTo("Board");
    expect(backend.handlers.get("notes-changed")?.size).toBe(0);
    await goTo("Notes");
    expect(backend.handlers.get("notes-changed")?.size).toBe(1);
  });
});

describe("autosave", () => {
  it("writes 500 ms after the last change, never before, and a change resets the pause", async () => {
    await renderView();
    expect(AUTOSAVE_MS).toBe(500);
    typeText("a\n");
    await advance(499);
    expect(called("notes_write")).toHaveLength(0);
    typeText("ab\n");
    await advance(499);
    expect(called("notes_write")).toHaveLength(0);
    await advance(1);
    expect(called("notes_write")).toEqual([{ projectId: PROJECT, text: "ab\n" }]);
  });

  it("shows Saving… while a write runs and Saved after it", async () => {
    let finish: (value: { hash: string }) => void = () => {};
    writeAnswer = () => new Promise((resolve) => (finish = resolve));
    await renderView();
    typeText("a\n");
    expect(status()).toHaveTextContent("");
    await advance(500);
    expect(status()).toHaveTextContent("Saving…");
    await act(async () => finish({ hash: "h9" }));
    expect(status()).toHaveTextContent("Saved");
    expect(status()).toHaveClass("text-muted-foreground");
  });

  it("serialises the text when it writes, not on every change", async () => {
    await renderView();
    vi.mocked(fake.port.getMarkdown).mockClear();
    for (const text of ["a", "ab", "abc"]) typeText(text);
    expect(fake.port.getMarkdown).not.toHaveBeenCalled();
    await advance(500);
    expect(fake.port.getMarkdown).toHaveBeenCalledTimes(1);
  });

  it("runs one write at a time and merges what arrives meanwhile into one more write of the latest", async () => {
    const running: Array<(value: { hash: string }) => void> = [];
    writeAnswer = () => new Promise((resolve) => running.push(resolve));
    await renderView();
    typeText("one\n");
    await advance(500);
    expect(called("notes_write")).toHaveLength(1);
    typeText("two\n");
    await advance(500);
    typeText("three\n");
    await advance(500);
    expect(called("notes_write")).toHaveLength(1);
    await act(async () => running[0]?.({ hash: "h1" }));
    await settle();
    expect(called("notes_write").map((args) => args.text)).toEqual(["one\n", "three\n"]);
    await act(async () => running[1]?.({ hash: "h2" }));
    await advance(2000);
    expect(called("notes_write")).toHaveLength(2);
    expect(status()).toHaveTextContent("Saved");
  });

  it("tells why a write failed, in red with an icon, keeps the text and tries again with the next change", async () => {
    writeAnswer = () => Promise.reject({ code: "io", message: "No space left on device." });
    await renderView();
    typeText("a\n");
    await advance(500);
    expect(status()).toHaveTextContent("Could not save: No space left on device.");
    expect(status()).toHaveClass("text-rose-ink");
    expect(status().querySelector("svg")).toHaveClass("lucide-circle-x");
    expect(fake.text).toBe("a\n");
    writeAnswer = () => Promise.resolve({ hash: "h5" });
    typeText("ab\n");
    await advance(500);
    expect(called("notes_write").map((args) => args.text)).toEqual(["a\n", "ab\n"]);
    expect(status()).toHaveTextContent("Saved");
  });

  it("writes at once when the editor loses the focus", async () => {
    await renderView();
    typeText("a\n");
    act(() => fake.events.blurred());
    await settle();
    expect(called("notes_write")).toHaveLength(1);
    await advance(2000);
    expect(called("notes_write")).toHaveLength(1);
  });

  it("writes at once after a tick", async () => {
    await renderView();
    fake.text = "- [x] a\n";
    act(() => fake.events.ticked());
    await settle();
    expect(called("notes_write")).toEqual([{ projectId: PROJECT, text: "- [x] a\n" }]);
  });

  it("writes what waits at once when another view is chosen, and keeps the editor", async () => {
    await renderView();
    typeText("a\n");
    await goTo("Board");
    expect(called("notes_write")).toEqual([{ projectId: PROJECT, text: "a\n" }]);
    expect(fake.port.destroy).not.toHaveBeenCalled();
  });
});

describe("coming back", () => {
  it("keeps the editor as it was when the sheet on disk is still the one shown", async () => {
    await renderView();
    typeText("a\n");
    await advance(500);
    disk = { text: "a\n", hash: "h1" };
    await goTo("Board");
    await goTo("Notes");
    expect(fake.created).toHaveBeenCalledTimes(1);
    expect(fake.port.replaceFromDisk).not.toHaveBeenCalled();
  });

  it("replaces the content from disk when the sheet changed while the view was left", async () => {
    await renderView();
    await goTo("Board");
    disk = { text: "changed outside\n", hash: "other" };
    await goTo("Notes");
    expect(fake.port.replaceFromDisk).toHaveBeenCalledWith("changed outside\n");
    expect(fake.created).toHaveBeenCalledTimes(1);
    expect(toast.warning).not.toHaveBeenCalled();
  });

  it("shows the empty sheet when the file was deleted meanwhile", async () => {
    await renderView();
    await goTo("Board");
    disk = { text: null, hash: null };
    await goTo("Notes");
    expect(fake.port.replaceFromDisk).toHaveBeenCalledWith("");
  });

  it("keeps a text that could not be written, writes it again and warns when the file changed", async () => {
    writeAnswer = () => Promise.reject({ code: "io", message: "read-only" });
    await renderView();
    typeText("mine\n");
    await goTo("Board");
    disk = { text: "theirs\n", hash: "other" };
    writeAnswer = () => Promise.resolve({ hash: "h7" });
    await goTo("Notes");
    expect(fake.port.replaceFromDisk).not.toHaveBeenCalled();
    expect(toast.warning).toHaveBeenCalledWith("The notes changed on disk; Weaver kept your version.");
    expect(called("notes_write").map((args) => args.text)).toEqual(["mine\n", "mine\n"]);
  });
});

describe("an outside change", () => {
  it("shows the file as it is now, silently, when no change waits", async () => {
    await renderView();
    disk = { text: "outside\n", hash: "h-out" };
    act(() => emit("notes-changed", changed({ sheetChanged: true, hash: "h-out" })));
    await settle();
    expect(called("notes_read")).toHaveLength(2);
    expect(fake.port.replaceFromDisk).toHaveBeenCalledWith("outside\n");
    expect(toast.warning).not.toHaveBeenCalled();
    expect(called("notes_write")).toHaveLength(0);
  });

  it("shows an empty sheet when the file was deleted", async () => {
    await renderView();
    disk = { text: null, hash: null };
    act(() => emit("notes-changed", changed({ sheetChanged: true, hash: null })));
    await settle();
    expect(fake.port.replaceFromDisk).toHaveBeenCalledWith("");
  });

  it("writes the developer's version over the file at once, with a warning, when a change waits", async () => {
    await renderView();
    typeText("mine\n");
    act(() => emit("notes-changed", changed({ sheetChanged: true, hash: "h-out" })));
    await settle();
    expect(toast.warning).toHaveBeenCalledWith("The notes changed on disk; Weaver kept your version.");
    expect(called("notes_write")).toEqual([{ projectId: PROJECT, text: "mine\n" }]);
    expect(fake.port.replaceFromDisk).not.toHaveBeenCalled();
    expect(called("notes_read")).toHaveLength(1);
  });

  it("ignores the sheet Weaver itself last read or wrote, and events that do not concern the sheet", async () => {
    await renderView();
    act(() => emit("notes-changed", changed({ sheetChanged: true, hash: "h0" })));
    act(() => emit("notes-changed", changed({ images: ["pic.png"] })));
    await settle();
    expect(called("notes_read")).toHaveLength(1);
    expect(fake.port.replaceFromDisk).not.toHaveBeenCalled();
  });

  it("ignores another project's events", async () => {
    await renderView();
    act(() => emit("notes-changed", { ...changed({ sheetChanged: true, hash: "x" }), projectId: "other" }));
    await settle();
    expect(called("notes_read")).toHaveLength(1);
  });
});

describe("removed images", () => {
  it("sends the removed images that the written sheet no longer names to the trash on leaving, and starts again empty", async () => {
    await renderView();
    act(() => fake.events.imagesRemoved(["gone.png", "kept.png", "https://example.com/remote.png", "sub/other.png"]));
    typeText("text with ![](<kept.png>)\n");
    await goTo("Board");
    expect(called("notes_write")).toHaveLength(1);
    expect(called("notes_trash")).toEqual([{ projectId: PROJECT, names: ["gone.png"] }]);
    await goTo("Notes");
    await goTo("Board");
    expect(called("notes_trash")).toHaveLength(1);
  });

  it("trashes nothing when the removed images are still named", async () => {
    await renderView();
    act(() => fake.events.imagesRemoved(["a.png"]));
    fake.text = "![](<a.png>)\n";
    await goTo("Board");
    expect(called("notes_trash")).toHaveLength(0);
  });

  it("shows the reason for the image the trash refused, and for several", async () => {
    await renderView();
    backend.call.mockImplementation(((name: string, args: Record<string, unknown>) =>
      name === "notes_trash"
        ? Promise.resolve({ refused: (args.names as string[]).map((n) => ({ name: n, reason: "in use" })) })
        : backendCall(name, args)) as never);
    act(() => fake.events.imagesRemoved(["a.png"]));
    await goTo("Board");
    expect(toast.error).toHaveBeenCalledWith('Could not move "a.png" to the trash: in use');
    await goTo("Notes");
    act(() => fake.events.imagesRemoved(["b.png", "c.png"]));
    await goTo("Board");
    expect(toast.error).toHaveBeenLastCalledWith("Could not move 2 images to the trash: in use");
  });

  it("trashes nothing while the text could not be written", async () => {
    writeAnswer = () => Promise.reject({ code: "io", message: "read-only" });
    await renderView();
    act(() => fake.events.imagesRemoved(["a.png"]));
    typeText("changed\n");
    await goTo("Board");
    expect(called("notes_trash")).toHaveLength(0);
  });
});

describe("before the window closes", () => {
  const flush = async () => {
    act(() => emit("notes-flush", { projectId: PROJECT }));
    await settle();
  };

  it("answers with the waiting text and the images to trash", async () => {
    await renderView();
    act(() => fake.events.imagesRemoved(["gone.png", "kept.png"]));
    typeText("closing ![](<kept.png>)\n");
    await flush();
    expect(called("notes_flushed")).toEqual([{ projectId: PROJECT, text: "closing ![](<kept.png>)\n", trash: ["gone.png"] }]);
    expect(called("notes_write")).toHaveLength(0);
  });

  it("answers with no text when nothing waits", async () => {
    await renderView();
    await flush();
    expect(called("notes_flushed")).toEqual([{ projectId: PROJECT, text: null, trash: [] }]);
  });

  it("answers even when the view was never opened", async () => {
    await renderView("board");
    await flush();
    expect(called("notes_flushed")).toEqual([{ projectId: PROJECT, text: null, trash: [] }]);
    expect(called("notes_read")).toHaveLength(0);
  });

  it("answers with no text once the view was left, which wrote it", async () => {
    await renderView();
    typeText("a\n");
    await goTo("Board");
    await flush();
    expect(called("notes_flushed")).toEqual([{ projectId: PROJECT, text: null, trash: [] }]);
  });

  it("ignores another project's request", async () => {
    await renderView();
    act(() => emit("notes-flush", { projectId: "other" }));
    await settle();
    expect(called("notes_flushed")).toHaveLength(0);
  });
});

describe("adding images", () => {
  const drag = (event: DragDropEvent) => act(async () => void backend.drags.forEach((handler) => handler(event)));
  const at = { x: 10, y: 20 };

  it("listens to dropped files only while the view is shown", async () => {
    await renderView();
    expect(backend.drags.size).toBe(1);
    await goTo("Board");
    expect(backend.drags.size).toBe(0);
  });

  it("shows a line where dropped files will go, and adds them there in the order given", async () => {
    await renderView();
    fake.drop = { pos: 7, line: { left: 10, top: 100, width: 300 } };
    await drag({ type: "enter", paths: ["/a.png"], position: { x: at.x, y: at.y } } as DragDropEvent);
    const line = document.querySelector(".bg-primary.h-0\\.5") as HTMLElement;
    expect(line).toHaveStyle({ left: "10px", width: "300px" });
    await drag({ type: "drop", paths: ["/a.png", "/b.png"], position: { x: at.x, y: at.y } } as DragDropEvent);
    expect(document.querySelector(".bg-primary.h-0\\.5")).toBeNull();
    await settle();
    expect(fake.port.track).toHaveBeenCalledWith(7);
    expect(called("notes_add_dropped")).toEqual([{ projectId: PROJECT, paths: ["/a.png", "/b.png"], localTime: expect.stringMatching(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}$/) }]);
    expect(fake.anchor.insert).toHaveBeenCalledWith(["pic.png"]);
    expect(fake.anchor.release).toHaveBeenCalled();
  });

  it("ignores a drop outside the sheet", async () => {
    await renderView();
    fake.drop = { pos: 7, line: { left: 0, top: 0, width: 1 } };
    document.elementFromPoint = vi.fn(() => document.body);
    await drag({ type: "drop", paths: ["/a.png"], position: { x: 1, y: 1 } } as DragDropEvent);
    expect(called("notes_add_dropped")).toHaveLength(0);
  });

  it("shows one error toast per refused file, adds the others, and shows nothing for files left out", async () => {
    await renderView();
    fake.drop = { pos: 7, line: { left: 0, top: 0, width: 1 } };
    backend.call.mockImplementation(((name: string, args: Record<string, unknown>) =>
      name === "notes_add_dropped"
        ? Promise.resolve({
            files: [
              { status: "refused", source: "huge.png", reason: "tooLarge" },
              { status: "refused", source: "doc.gif", reason: "notAccepted" },
              { status: "failed", source: "locked.png", message: "permission denied." },
              { status: "leftOut" },
              { status: "added", name: "ok.png" },
            ],
          })
        : backendCall(name, args)) as never);
    await drag({ type: "drop", paths: ["/1", "/2", "/3", "/4", "/5"], position: { x: 1, y: 1 } } as DragDropEvent);
    await settle();
    expect(toast.error.mock.calls.map(([text]) => text)).toEqual([
      '"huge.png" is larger than 10 MB: notes images can be at most 10 MB.',
      '"doc.gif" is not a PNG, JPEG, GIF or WebP image.',
      'Could not add "locked.png": permission denied.',
    ]);
    expect(fake.anchor.insert).toHaveBeenCalledWith(["ok.png"]);
  });

  it("sends pasted image bytes with their file name, and inserts the added ones at the cursor", async () => {
    await renderView();
    const named = new File([new Uint8Array([104, 105])], "shot.png", { type: "image/png" });
    const bare = new File([new Uint8Array([1])], "", { type: "image/png" });
    act(() => fake.events.pasted([named, bare], fake.anchor));
    await settle();
    expect(called("notes_add_pasted")).toEqual([
      { projectId: PROJECT, files: [{ name: "shot.png", data: "aGk=" }, { name: null, data: "AQ==" }], localTime: expect.any(String) },
    ]);
    expect(fake.anchor.insert).toHaveBeenCalledWith(["pic.png"]);
  });

  it("shows the reason when a copy fails as a whole", async () => {
    await renderView();
    backend.call.mockImplementation(((name: string, args: Record<string, unknown>) =>
      name === "notes_add_pasted" ? Promise.reject({ code: "io", message: "The notes folder is not writable." }) : backendCall(name, args)) as never);
    act(() => fake.events.pasted([new File([new Uint8Array([1])], "a.png")], fake.anchor));
    await settle();
    expect(toast.error).toHaveBeenCalledWith("The notes folder is not writable.");
    expect(fake.anchor.insert).not.toHaveBeenCalled();
    expect(fake.anchor.release).toHaveBeenCalled();
  });
});

describe("logs", () => {
  it("never logs the sheet's text or an image", async () => {
    const spies = [vi.spyOn(console, "log"), vi.spyOn(console, "info"), vi.spyOn(console, "warn"), vi.spyOn(console, "error"), vi.spyOn(console, "debug")];
    await renderView();
    typeText("secret text\n");
    await advance(500);
    for (const spy of spies) expect(spy).not.toHaveBeenCalled();
  });
});
