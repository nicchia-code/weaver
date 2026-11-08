import { render } from "@testing-library/react";
import type { ReactNode } from "react";
import { vi } from "vitest";

import type { ProjectChanged } from "@/bindings/ProjectChanged";
import type { ViewSnapshot } from "@/bindings/ViewSnapshot";
import { BoardActionsContext, type BoardActions } from "@/components/board/board-ui";
import { ChatDraftProvider } from "@/components/general/chat-draft";
import { ProjectContext } from "@/components/project-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import { ViewsProvider } from "@/components/views/views-state";
import { PROJECT } from "@/components/views/view-fixtures";
import { call, on } from "@/lib/backend";
import type { ViewId } from "@/views";

/** What a test can see of the fake backend and drive. */
export interface FakeBackend {
  /** The overlay of each `view_snapshot` asked, in order. */
  asked: (string | null)[];
  /** The calls of `Open in editor`. */
  opened: { file: string; line?: number | null }[];
  /** A rebuild of the picture: the slices that changed and the revision. */
  emit: (slices: string[], revision: number) => void;
}

/** Fakes the backend of the views: `respond` answers `view_snapshot` for the overlay asked; `emit` sends `project-changed`. */
export function fakeBackend(respond: (overlay: string | null) => ViewSnapshot): FakeBackend {
  const listeners: ((event: ProjectChanged) => void)[] = [];
  const asked: (string | null)[] = [];
  const opened: { file: string; line?: number | null }[] = [];
  vi.mocked(on).mockImplementation(((name: string, handler: (event: ProjectChanged) => void) => {
    if (name === "project-changed") listeners.push(handler);
    return Promise.resolve(() => {});
  }) as unknown as typeof on);
  vi.mocked(call).mockImplementation(((name: string, args: { overlay?: string | null; file: string; line?: number | null }) => {
    if (name === "view_snapshot") {
      asked.push(args.overlay ?? null);
      return Promise.resolve(respond(args.overlay ?? null));
    }
    if (name === "open_in_editor") {
      opened.push({ file: args.file, line: args.line });
      return Promise.resolve(null);
    }
    return Promise.reject(new Error(`unexpected command ${name}`));
  }) as unknown as typeof call);
  return {
    asked,
    opened,
    emit: (slices: string[], revision: number) => listeners.forEach((l) => l({ projectId: PROJECT, revision, slices })),
  };
}

/** Renders `ui` inside a project window context and the provider of the views; `actions` are what the window above the views provides (`Ask more`...). */
export function renderViews(ui: ReactNode, showView: (view: ViewId) => void = vi.fn(), actions: BoardActions = {}) {
  return render(
    <TooltipProvider>
      <ProjectContext.Provider value={{ projectId: PROJECT, showView, openJob: vi.fn(), openedJob: null, openedJobRequest: 0 }}>
        <BoardActionsContext.Provider value={actions}>
          <ChatDraftProvider>
            <ViewsProvider projectId={PROJECT}>{ui}</ViewsProvider>
          </ChatDraftProvider>
        </BoardActionsContext.Provider>
      </ProjectContext.Provider>
    </TooltipProvider>,
  );
}
