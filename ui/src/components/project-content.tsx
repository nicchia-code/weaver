import { AskViewSwitch } from "@/components/ask/ask-view-switch";
import { AsksProvider } from "@/components/ask/use-asks";
import { AssetsView } from "@/components/assets/assets-view";
import { AssetsProvider } from "@/components/assets/use-assets";
import { BoardProvider, useBoard } from "@/components/board/board-state";
import { BoardView } from "@/components/board/board-view";
import { FilesView } from "@/components/files/files-view";
import { CommandPalette } from "@/components/command-palette";
import { ChatDraftProvider } from "@/components/general/chat-draft";
import { GeneralView } from "@/components/general/general-view";
import { useChatWaiting } from "@/components/general/use-general";
import { HistoryView } from "@/components/history/history-view";
import { JobsBoardActions } from "@/components/jobs/board-actions";
import { JobsView } from "@/components/jobs/jobs-view";
import { JobsProvider, useJobs } from "@/components/jobs/use-jobs";
import { MergeProvider } from "@/components/merge/use-merge";
import { NotesView } from "@/components/notes/notes-view";
import { NotesProvider } from "@/components/notes/use-notes";
import { ConnectedSettingsView } from "@/components/omp-status";
import { ProjectWindow } from "@/components/project-window";
import { QuestionBar, QuestionRoundLayer } from "@/components/questions/question-overlay";
import { RequestsBoardActions, RequestsLayer } from "@/components/requests/requests-layer";
import { AtomicCommitsSetting } from "@/components/project/atomic-commits";
import { SetupLayer } from "@/components/project/empty-model";
import { ProjectGate } from "@/components/project/project-gate";
import { SoloModeSetting } from "@/components/project/solo-mode";
import { OpeningCheckToast, SkillsBanner, UpdateSkills } from "@/components/project/update-skills";
import { ProjectStatusProvider } from "@/components/project/use-project-status";
import { RequestsProvider } from "@/components/requests/use-requests";
import { RequirementsView } from "@/components/views/requirements-view";
import { SpecsView } from "@/components/views/specs-view";
import { ViewsProvider } from "@/components/views/views-state";

function ProjectShell({ projectId }: { projectId: string }) {
  const { snapshot } = useBoard();
  const { waiting, running } = useJobs();
  const chatWaiting = useChatWaiting(projectId);
  const backlog = snapshot?.backlog.length ?? 0;
  return (
    <ProjectWindow
      projectId={projectId}
      views={{
        board: (
          <ProjectGate view="board">
            <RequestsLayer projectId={projectId}>
              <SetupLayer>
                <BoardView />
              </SetupLayer>
            </RequestsLayer>
          </ProjectGate>
        ),
        requirements: (
          <ProjectGate view="requirements">
            <RequirementsView />
          </ProjectGate>
        ),
        specs: (
          <ProjectGate view="specs">
            <SpecsView />
          </ProjectGate>
        ),
        history: (
          <ProjectGate view="history">
            <HistoryView />
          </ProjectGate>
        ),
        jobs: (
          <ProjectGate view="jobs">
            <JobsView />
          </ProjectGate>
        ),
        files: (
          <ProjectGate view="files">
            <FilesView />
          </ProjectGate>
        ),
        assets: (
          <ProjectGate view="assets">
            <AssetsView />
          </ProjectGate>
        ),
        notes: (
          <ProjectGate view="notes">
            <NotesView />
          </ProjectGate>
        ),
        general: (
          <ProjectGate view="general">
            <GeneralView />
          </ProjectGate>
        ),
        // Settings is the one view that works without a project.
        settings: (
          <ConnectedSettingsView
            projectSlot={
              <>
                <SoloModeSetting />
                <AtomicCommitsSetting />
                <UpdateSkills />
              </>
            }
          />
        ),
      }}
      // What the check of the agent files says when the window opens: a toast, or a banner with Retry.
      banners={
        <>
          <OpeningCheckToast />
          <SkillsBanner />
        </>
      }
      // The bar of a minimized question round: at the bottom of the work surface, below the view.
      footer={<QuestionBar />}
      // The sidebar shows the real cards of Backlog, drafts of requests included (ghosts never count,
      // they are not in `backlog`), the jobs waiting for the developer (never Chat) and whether Chat
      // waits for the answers of its round; the backend counts them.
      counts={{ backlog, jobsWaiting: waiting, jobsRunning: running, chatWaiting }}
      // What Requirements and Specs share, and the cards' `View requirements`; `Run`, `Run all` and
      // `Scan System` start jobs.
      around={(children) => (
        <ViewsProvider projectId={projectId}>
          <ChatDraftProvider>
            <JobsBoardActions>
              <AskViewSwitch />
              <CommandPalette />
              <RequestsBoardActions>{children}</RequestsBoardActions>
            </JobsBoardActions>
          </ChatDraftProvider>
        </ViewsProvider>
      )}
    />
  );
}

/** What a project window shows: the views of the project, fed by what the backend holds for it. */
export function ProjectContent({ projectId }: { projectId: string }) {
  return (
    <ProjectStatusProvider projectId={projectId}>
      {/* The assets of the project, for every view and every message box that recognises `@` tags. */}
      <AssetsProvider projectId={projectId}>
        <NotesProvider projectId={projectId}>
          <BoardProvider projectId={projectId}>
            <JobsProvider projectId={projectId}>
              <MergeProvider projectId={projectId}>
                <RequestsProvider projectId={projectId}>
                  {/* The round the agent (or Weaver, for a Drift) asks: a modal overlay over every view. */}
                  <AsksProvider projectId={projectId}>
                    <QuestionRoundLayer projectId={projectId}>
                      <ProjectShell projectId={projectId} />
                    </QuestionRoundLayer>
                  </AsksProvider>
                </RequestsProvider>
              </MergeProvider>
            </JobsProvider>
          </BoardProvider>
        </NotesProvider>
      </AssetsProvider>
    </ProjectStatusProvider>
  );
}
