import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { useEffect } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { RequestStatus } from "@/bindings/RequestStatus";
import { expectOneGeometry, pressWhileComposing } from "@/components/assets/tag-box-checks";
import { ProjectContext } from "@/components/project-context";
import { RequestEditor } from "@/components/requests/request-editor";
import { RequestsProvider, useRequests } from "@/components/requests/use-requests";
import { ViewShownContext } from "@/components/view-shown";
import { TooltipProvider } from "@/components/ui/tooltip";
import { call, on, onFilesDropped } from "@/lib/backend";

vi.mock("@/lib/backend", () => ({
  call: vi.fn(),
  on: vi.fn(),
  onFilesDropped: vi.fn(),
  isAppError: (value: unknown) => typeof value === "object" && value !== null && "code" in value && "message" in value,
}));

const toasts = vi.hoisted(() => ({ notice: vi.fn(), warning: vi.fn(), error: vi.fn() }));
vi.mock("sonner", () => ({ toast: Object.assign(toasts.notice, { warning: toasts.warning, error: toasts.error }) }));

const PROJECT = "0123456789abcdef";

const ASSETS = vi.hoisted(() => [
  { name: "logo.png", size: 1 },
  { name: "spec.pdf", size: 1 },
]);
vi.mock("@/components/assets/use-assets", () => ({ useAssets: () => ({ assets: ASSETS, loaded: true, apply: () => {} }) }));
const ID = "2026-11-02T09-30-00";

interface Fake {
  text: string;
  status: RequestStatus;
  jobId: string | null;
  jobName: string | null;
  returned: boolean;
  /** Reject the next save with this message. */
  failSave: string | null;
  modelEmpty: boolean;
  gone: boolean;
}

const fake: Fake = { text: "", status: "writing", jobId: null, jobName: null, returned: false, failSave: null, modelEmpty: false, gone: false };
let handlers = new Map<string, (payload: unknown) => void>();
const openJob = vi.fn();
const showView = vi.fn();

function readView() {
  return { id: ID, text: fake.text, status: fake.status, jobId: fake.jobId, jobName: fake.jobName, returned: fake.returned, modelEmpty: fake.modelEmpty };
}

/** Opens the request `ID` in the editor, as a card of the Board does, and shows whether the editor is open. */
function Open() {
  const { openEditor, editing } = useRequests();
  useEffect(() => {
    openEditor(ID);
  }, [openEditor]);
  return <output aria-label="Editing">{editing ? "open" : "closed"}</output>;
}

function install() {
  handlers = new Map();
  vi.mocked(onFilesDropped).mockResolvedValue(() => {});
  vi.mocked(on).mockImplementation((async (name: string, handler: (payload: unknown) => void) => {
    handlers.set(name, handler);
    return () => {};
  }) as unknown as typeof on);
  vi.mocked(call).mockImplementation((async (name: string, args: Record<string, unknown>) => {
    switch (name) {
      case "request_read":
        if (fake.gone) throw { code: "refused", message: "the request is gone" };
        return readView();
      case "request_save":
        if (fake.failSave) {
          const message = fake.failSave;
          fake.failSave = null;
          throw { code: "internal", message };
        }
        fake.text = args.text as string;
        return null;
      case "request_image_add":
        return `${ID}.images/image-1.png`;
      case "request_send":
        fake.status = "sent";
        fake.jobId = "jjjjjjjj";
        fake.jobName = "Design Add a cart";
        return null;
      case "request_discard":
      case "request_leave":
      case "requests_flushed":
        return null;
      default:
        throw new Error(`unexpected command ${name}`);
    }
  }) as unknown as typeof call);
}

async function settle() {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(0);
  });
}

async function mount(patch: Partial<Fake> = {}) {
  Object.assign(fake, { text: "", status: "writing", jobId: null, jobName: null, returned: false, failSave: null, modelEmpty: false, gone: false }, patch);
  install();
  render(
    <TooltipProvider>
      <ProjectContext.Provider value={{ projectId: PROJECT, showView, openJob, openedJob: null, openedJobRequest: 0 }}>
        <RequestsProvider projectId={PROJECT}>
          <Open />
          <RequestEditor projectId={PROJECT} />
        </RequestsProvider>
      </ProjectContext.Provider>
    </TooltipProvider>,
  );
  await settle();
}

const saves = () => vi.mocked(call).mock.calls.filter(([name]) => name === "request_save");
const area = () => screen.getByRole("combobox", { name: "Request text" });

async function type(text: string) {
  fireEvent.change(area(), { target: { value: text } });
  await settle();
}

async function wait(ms: number) {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(ms);
  });
}

describe("the request editor", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    toasts.notice.mockClear();
    toasts.warning.mockClear();
    toasts.error.mockClear();
    openJob.mockClear();
    showView.mockClear();
  });
  afterEach(() => vi.useRealTimers());

  it("opens with the focus in the text, headed Design request, with a back control and no list of requests", async () => {
    await mount();
    expect(screen.getByRole("heading", { name: "Design request" })).toBeInTheDocument();
    expect(area()).toHaveFocus();
    expect(screen.getByRole("button", { name: "Board" })).toBeInTheDocument();
    expect(screen.queryByRole("navigation", { name: "Open requests" })).toBeNull();
    expect(screen.queryByRole("button", { name: /New request/i })).toBeNull();
    expect(screen.queryByText("Draft")).toBeNull();
    expect(screen.queryByText("Sent")).toBeNull();
  });

  it("says Setup request when the Model is empty", async () => {
    await mount({ modelEmpty: true });
    expect(screen.getByRole("heading", { name: "Setup request" })).toBeInTheDocument();
  });

  it("saves 500 ms after the last keystroke, showing Saving… then Saved", async () => {
    await mount();
    await type("# Cart");
    await wait(499);
    expect(saves()).toHaveLength(0);
    await type("# Cart\nA cart.");
    await wait(499);
    expect(saves()).toHaveLength(0);
    let resolve: () => void = () => {};
    vi.mocked(call).mockImplementationOnce((() => new Promise<null>((done) => (resolve = () => done(null)))) as unknown as typeof call);
    await wait(1);
    expect(screen.getByText("Saving…")).toBeInTheDocument();
    await act(async () => resolve());
    expect(screen.getByText("Saved")).toBeInTheDocument();
    expect(saves().at(-1)?.[1]).toEqual({ projectId: PROJECT, requestId: ID, text: "# Cart\nA cart." });
  });

  it("saves on blur at once", async () => {
    await mount();
    await type("Something");
    fireEvent.blur(area());
    await settle();
    expect(saves()).toHaveLength(1);
    expect(screen.getByText("Saved")).toBeInTheDocument();
  });

  it("shows why a save failed, keeps the text and tries again at the next change", async () => {
    await mount();
    fake.failSave = "the disk is full";
    await type("First");
    await wait(500);
    expect(screen.getByText("Could not save: the disk is full.")).toBeInTheDocument();
    expect(area()).toHaveValue("First");
    await type("First and more");
    await wait(500);
    expect(screen.getByText("Saved")).toBeInTheDocument();
    expect(saves().at(-1)?.[1]).toMatchObject({ text: "First and more" });
  });

  it("starts no job while writing: only Send sends", async () => {
    await mount();
    await type("Add a cart");
    await wait(2000);
    expect(vi.mocked(call).mock.calls.some(([name]) => name === "request_send")).toBe(false);
  });

  it("disables Send while the text is empty, with its tooltip, and sends a written request after saving it", async () => {
    await mount();
    const send = screen.getByRole("button", { name: "Send" });
    expect(send).toHaveAttribute("aria-disabled", "true");
    fireEvent.click(send);
    await settle();
    expect(vi.mocked(call).mock.calls.some(([name]) => name === "request_send")).toBe(false);
    await type("   \n ");
    expect(screen.getByRole("button", { name: "Send" })).toHaveAttribute("aria-disabled", "true");

    await type("Add a cart");
    expect(screen.getByRole("button", { name: "Send" })).not.toHaveAttribute("aria-disabled");
    fireEvent.click(screen.getByRole("button", { name: "Send" }));
    await settle();
    const names = vi.mocked(call).mock.calls.map(([name]) => name);
    expect(names.indexOf("request_save")).toBeGreaterThan(-1);
    expect(names.indexOf("request_save")).toBeLessThan(names.indexOf("request_send"));
    // Sent: the editor closes and the Board shows.
    expect(screen.getByLabelText("Editing")).toHaveTextContent("closed");
  });

  it("sends with Ctrl+Enter and with Cmd+Enter", async () => {
    await mount({ text: "Add a cart" });
    fireEvent.keyDown(area(), { key: "Enter", ctrlKey: true });
    await settle();
    expect(vi.mocked(call).mock.calls.filter(([name]) => name === "request_send")).toHaveLength(1);
    await mount({ text: "Add a cart" });
    fireEvent.keyDown(screen.getAllByRole("combobox", { name: "Request text" }).at(-1) as HTMLElement, { key: "Enter", metaKey: true });
    await settle();
    expect(vi.mocked(call).mock.calls.filter(([name]) => name === "request_send")).toHaveLength(2);
  });

  it("asks before discarding and deletes only when confirmed", async () => {
    await mount({ text: "Draft" });
    fireEvent.click(screen.getByRole("button", { name: "Discard" }));
    await settle();
    const dialog = screen.getByRole("alertdialog");
    expect(within(dialog).getByText("Discard this request?")).toBeInTheDocument();
    expect(within(dialog).getByText("Its file and images are deleted.")).toBeInTheDocument();
    fireEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await settle();
    expect(vi.mocked(call).mock.calls.some(([name]) => name === "request_discard")).toBe(false);

    fireEvent.click(screen.getByRole("button", { name: "Discard" }));
    await settle();
    fireEvent.click(within(screen.getByRole("alertdialog")).getByRole("button", { name: "Discard" }));
    await settle();
    expect(vi.mocked(call)).toHaveBeenCalledWith("request_discard", { projectId: PROJECT, requestId: ID });
    // Discarded: the editor closes and the Board shows.
    expect(screen.getByLabelText("Editing")).toHaveTextContent("closed");
  });

  it("saves a pasted image in the request's folder and links it at the cursor", async () => {
    await mount({ text: "Look: " });
    const text = area() as HTMLTextAreaElement;
    text.setSelectionRange(6, 6);
    const file = { type: "image/png", size: 4, arrayBuffer: async () => new Uint8Array([1, 2, 3, 4]).buffer } as unknown as File;
    fireEvent.paste(text, { clipboardData: { files: [file], getData: () => "" } });
    await settle();
    expect(vi.mocked(call)).toHaveBeenCalledWith("request_image_add", { projectId: PROJECT, requestId: ID, data: "AQIDBA==" });
    expect(area()).toHaveValue(`Look: ![](${ID}.images/image-1.png)`);
  });

  it("refuses an image that is not PNG, JPEG, GIF or WebP, or is over 10 MB", async () => {
    await mount();
    const text = area();
    const bad = { type: "image/bmp", size: 10, arrayBuffer: async () => new ArrayBuffer(10) } as unknown as File;
    fireEvent.paste(text, { clipboardData: { files: [bad], getData: () => "" } });
    const big = { type: "image/png", size: 10 * 1024 * 1024 + 1, arrayBuffer: async () => new ArrayBuffer(1) } as unknown as File;
    fireEvent.paste(text, { clipboardData: { files: [big], getData: () => "" } });
    await settle();
    expect(toasts.error).toHaveBeenCalledTimes(2);
    expect(toasts.error).toHaveBeenCalledWith("Only PNG, JPEG, GIF or WebP images up to 10 MB.");
    expect(vi.mocked(call).mock.calls.some(([name]) => name === "request_image_add")).toBe(false);
    expect(area()).toHaveValue("");
  });

  it("keeps Weaver's version when the file changed on disk while a change was pending", async () => {
    await mount({ text: "Mine" });
    await type("Mine, edited");
    fake.text = "Theirs";
    await act(async () => handlers.get("requests-changed")?.({ projectId: PROJECT }));
    await settle();
    expect(toasts.warning).toHaveBeenCalledWith("The request changed on disk; Weaver kept your version.");
    expect(area()).toHaveValue("Mine, edited");
    expect(fake.text).toBe("Mine, edited");
  });

  it("reloads the text when the file changed on disk and nothing is pending", async () => {
    await mount({ text: "Mine" });
    fake.text = "Changed outside";
    await act(async () => handlers.get("requests-changed")?.({ projectId: PROJECT }));
    await settle();
    expect(area()).toHaveValue("Changed outside");
    expect(toasts.warning).not.toHaveBeenCalled();
  });

  it("does not take its own write for an outside change", async () => {
    await mount({ text: "Mine" });
    await type("Mine\n");
    await wait(500);
    fake.text = "Mine";
    await act(async () => handlers.get("requests-changed")?.({ projectId: PROJECT }));
    await settle();
    expect(area()).toHaveValue("Mine\n");
    expect(toasts.warning).not.toHaveBeenCalled();
  });

  it("tells when the request was deleted outside Weaver", async () => {
    await mount({ text: "Mine" });
    fake.gone = true;
    await act(async () => handlers.get("requests-changed")?.({ projectId: PROJECT }));
    await settle();
    expect(toasts.warning).toHaveBeenCalledWith("The request was deleted outside Weaver.");
    expect(screen.getByLabelText("Editing")).toHaveTextContent("closed");
    expect(openJob).not.toHaveBeenCalled();
  });

  it("closes a sent request that was consumed or discarded back to the conversation of its job, with no toast", async () => {
    await mount({ text: "Add a cart", status: "sent", jobId: "jjjjjjjj", jobName: "Design" });
    fake.gone = true;
    await act(async () => handlers.get("requests-changed")?.({ projectId: PROJECT }));
    await settle();
    expect(toasts.warning).not.toHaveBeenCalled();
    expect(screen.getByLabelText("Editing")).toHaveTextContent("closed");
    expect(openJob).toHaveBeenCalledWith("jjjjjjjj");
  });

  it("turns a sent request into the writable one, with its banner, when its job ends without a Proposal", async () => {
    await mount({ text: "Add a cart", status: "sent", jobId: "jjjjjjjj", jobName: "Design" });
    expect(screen.queryByRole("combobox", { name: "Request text" })).toBeNull();
    Object.assign(fake, { status: "writing", jobId: null, jobName: null, returned: true });
    await act(async () => handlers.get("requests-changed")?.({ projectId: PROJECT }));
    await settle();
    expect(screen.getByText("The job ended without a Proposal. Change the request and send it again.")).toBeInTheDocument();
    expect(area()).toHaveValue("Add a cart");
    expect(screen.getByLabelText("Editing")).toHaveTextContent("open");
  });

  it("discards an empty request when the developer goes back, without a confirmation or a toast", async () => {
    await mount();
    fireEvent.click(screen.getByRole("button", { name: "Board" }));
    await settle();
    expect(screen.getByLabelText("Editing")).toHaveTextContent("closed");
    expect(vi.mocked(call)).toHaveBeenCalledWith("request_leave", { projectId: PROJECT, requestId: ID, closing: false });
    expect(screen.queryByRole("alertdialog")).toBeNull();
    expect(toasts.warning).not.toHaveBeenCalled();
    expect(toasts.error).not.toHaveBeenCalled();
  });

  it("discards a request that holds only spaces when leaving it, and never one with text or images", async () => {
    await mount();
    await type("  \n ");
    fireEvent.click(screen.getByRole("button", { name: "Board" }));
    await settle();
    expect(vi.mocked(call).mock.calls.filter(([name]) => name === "request_leave")).toHaveLength(1);

    vi.mocked(call).mockClear();
    await mount({ text: "Add a cart" });
    fireEvent.click(screen.getAllByRole("button", { name: "Board" }).at(-1) as HTMLElement);
    await settle();
    expect(vi.mocked(call).mock.calls.some(([name]) => name === "request_leave")).toBe(false);

    vi.mocked(call).mockClear();
    await mount({ text: `![](${ID}.images/image-1.png)` });
    fireEvent.click(screen.getAllByRole("button", { name: "Board" }).at(-1) as HTMLElement);
    await settle();
    expect(vi.mocked(call).mock.calls.some(([name]) => name === "request_leave")).toBe(false);
  });

  it("discards an empty request when the window closes, and keeps one with text", async () => {
    await mount();
    await act(async () => handlers.get("requests-flush")?.({ projectId: PROJECT }));
    await settle();
    expect(vi.mocked(call)).toHaveBeenCalledWith("request_leave", { projectId: PROJECT, requestId: ID, closing: true });
    expect(toasts.warning).not.toHaveBeenCalled();
    expect(vi.mocked(call).mock.calls.map(([name]) => name).at(-1)).toBe("requests_flushed");

    vi.mocked(call).mockClear();
    await mount({ text: "Half written" });
    await act(async () => handlers.get("requests-flush")?.({ projectId: PROJECT }));
    await settle();
    expect(vi.mocked(call).mock.calls.some(([name]) => name === "request_leave")).toBe(false);
  });

  it("closes the editor, discarding an empty request, when another view takes the Board's place", async () => {
    Object.assign(fake, { text: "", status: "writing", jobId: null, jobName: null, returned: false, failSave: null, modelEmpty: false, gone: false });
    install();
    const tree = (shown: boolean) => (
      <TooltipProvider>
        <ViewShownContext.Provider value={shown}>
          <ProjectContext.Provider value={{ projectId: PROJECT, showView, openJob, openedJob: null, openedJobRequest: 0 }}>
            <RequestsProvider projectId={PROJECT}>
              <Open />
              <RequestEditor projectId={PROJECT} />
            </RequestsProvider>
          </ProjectContext.Provider>
        </ViewShownContext.Provider>
      </TooltipProvider>
    );
    const { rerender } = render(tree(true));
    await settle();
    rerender(tree(false));
    await settle();
    expect(screen.getByLabelText("Editing")).toHaveTextContent("closed");
    expect(vi.mocked(call)).toHaveBeenCalledWith("request_leave", { projectId: PROJECT, requestId: ID, closing: false });
  });

  it("shows a sent request read-only, with its job, and opens the conversation", async () => {
    await mount({ text: "# Cart\nA cart.", status: "sent", jobId: "jjjjjjjj", jobName: "Design Cart" });
    expect(screen.getByText("Sent to the agent · Design Cart")).toBeInTheDocument();
    expect(screen.queryByRole("combobox", { name: "Request text" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Send" })).toBeNull();
    expect(screen.queryByText("Sent")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Open conversation" }));
    expect(openJob).toHaveBeenCalledWith("jjjjjjjj");
  });

  it("says the job ended without a Proposal when the request is back in writing", async () => {
    await mount({ text: "Add a cart", returned: true });
    expect(screen.getByText("The job ended without a Proposal. Change the request and send it again.")).toBeInTheDocument();
    expect(area()).toBeInTheDocument();
  });

  it("shows the rendered text within 150 ms with Preview", async () => {
    await mount({ text: "# Cart" });
    fireEvent.click(screen.getByRole("button", { name: "Preview" }));
    await wait(150);
    const preview = screen.getByLabelText("Preview");
    expect(within(preview).getByText("Cart")).toBeInTheDocument();
    expect(screen.queryByRole("combobox", { name: "Request text" })).toBeNull();
  });

  it("saves what is pending and answers when the window is closing", async () => {
    await mount();
    await type("Half written");
    await act(async () => handlers.get("requests-flush")?.({ projectId: PROJECT }));
    await settle();
    expect(saves()).toHaveLength(1);
    const names = vi.mocked(call).mock.calls.map(([name]) => name);
    expect(names.indexOf("request_save")).toBeLessThan(names.indexOf("requests_flushed"));
    expect(vi.mocked(call).mock.calls.some(([name]) => name === "request_send")).toBe(false);
  });
});

describe("the @ tags in the request editor", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    openJob.mockClear();
  });
  afterEach(() => vi.useRealTimers());

  it("opens the list of assets under the text, inserts the chosen one with Enter, and colours the recognised tags", async () => {
    await mount();
    area().focus();
    await type("Use @lo");
    expect(screen.getAllByRole("option").map((option) => option.textContent)).toEqual(["logo.png"]);
    fireEvent.keyDown(area(), { key: "Enter" });
    await settle();
    expect(area()).toHaveValue("Use @logo.png ");
    expect(document.querySelector("[data-tag='logo.png']")?.textContent).toBe("@logo.png");
    await type("Use @logo.png and @nothing");
    expect(document.querySelectorAll("[data-tag]")).toHaveLength(1);
  });

  it("gives the textarea and the mirror the same metrics, the monospace font included", async () => {
    await mount();
    expectOneGeometry(area(), { has: ["font-mono"] });
  });

  it("leaves Enter and Tab of a composition to the input method: nothing is sent, picked or moved", async () => {
    await mount({ text: "Add a cart" });
    area().focus();
    await type("Add a cart @sp");
    expect(screen.getAllByRole("option").map((option) => option.textContent)).toEqual(["spec.pdf"]);
    pressWhileComposing(area(), "Enter");
    pressWhileComposing(area(), "Tab");
    expect(fireEvent.keyDown(area(), { key: "Enter", metaKey: true, isComposing: true })).toBe(true);
    await settle();
    expect(screen.getAllByRole("option").map((option) => option.textContent)).toEqual(["spec.pdf"]);
    expect(area()).toHaveValue("Add a cart @sp");
    expect(area()).toHaveFocus();
    expect(vi.mocked(call).mock.calls.some(([name]) => name === "request_send")).toBe(false);
  });
});

describe("the back control of the request editor", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    openJob.mockClear();
  });
  afterEach(() => vi.useRealTimers());

  function OpenedFrom({ from }: { from?: { kind: "jobs"; jobId: string } }) {
    const { openEditor, editing } = useRequests();
    useEffect(() => {
      openEditor(ID, from);
    }, [openEditor, from]);
    return <output aria-label="Editing">{editing ? "open" : "closed"}</output>;
  }

  async function mountFrom(from?: { kind: "jobs"; jobId: string }) {
    Object.assign(fake, { text: "A cart.", status: "sent", jobId: "jjjjjjjj", jobName: "Design", returned: false, failSave: null, modelEmpty: false, gone: false });
    install();
    render(
      <TooltipProvider>
        <ProjectContext.Provider value={{ projectId: PROJECT, showView, openJob, openedJob: null, openedJobRequest: 0 }}>
          <RequestsProvider projectId={PROJECT}>
            <OpenedFrom from={from} />
            <RequestEditor projectId={PROJECT} />
          </RequestsProvider>
        </ProjectContext.Provider>
      </TooltipProvider>,
    );
    await settle();
  }

  it("says `Board` and only closes the editor when it was opened from the Board", async () => {
    await mountFrom();
    expect(screen.queryByRole("button", { name: "Jobs" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Board" }));
    expect(screen.getByLabelText("Editing")).toHaveTextContent("closed");
    expect(openJob).not.toHaveBeenCalled();
  });

  it("says `Jobs` and goes back to the conversation of the job when it was opened from it", async () => {
    await mountFrom({ kind: "jobs", jobId: "jjjjjjjj" });
    expect(screen.queryByRole("button", { name: "Board" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Jobs" }));
    expect(screen.getByLabelText("Editing")).toHaveTextContent("closed");
    expect(openJob).toHaveBeenCalledWith("jjjjjjjj");
  });
});

describe("files dropped on the window", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("reach the request editor only while its view is the one shown", async () => {
    Object.assign(fake, { text: "", status: "writing", jobId: null, jobName: null, returned: false, failSave: null, modelEmpty: false, gone: false });
    install();
    vi.mocked(onFilesDropped).mockClear();
    render(
      <TooltipProvider>
        <ViewShownContext.Provider value={false}>
          <ProjectContext.Provider value={{ projectId: PROJECT, showView: vi.fn(), openJob, openedJob: null, openedJobRequest: 0 }}>
            <RequestsProvider projectId={PROJECT}>
              <Open />
              <RequestEditor projectId={PROJECT} />
            </RequestsProvider>
          </ProjectContext.Provider>
        </ViewShownContext.Provider>
      </TooltipProvider>,
    );
    await settle();
    expect(onFilesDropped).not.toHaveBeenCalled();
  });
});
