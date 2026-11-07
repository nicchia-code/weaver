import { FolderCode, Sparkles, TriangleAlert, type LucideIcon } from "lucide-react";
import { useState, type ReactNode } from "react";

import type { BoardCard } from "@/bindings/BoardCard";
import type { JobRow } from "@/bindings/JobRow";
import { useBoard } from "@/components/board/board-state";
import { useJobs } from "@/components/jobs/use-jobs";
import { OmpSetup } from "@/components/omp-status";
import { ImportPanel } from "@/components/project/import-panel";
import { useProjectStatus } from "@/components/project/use-project-status";
import { useRequests } from "@/components/requests/use-requests";
import { useOmp } from "@/hooks/use-omp";
import { TONE_TEXT } from "@/lib/tones";

/** A setup or an import job that has not ended: the cards give way to it. */
export function setupWorks(jobs: readonly JobRow[]): boolean {
  return jobs.some(
    (job) => (job.kind === "setup" || job.kind === "import") && ["queued", "running", "waiting", "suspended"].includes(job.state),
  );
}

/** A Proposal is open on the Board (a resolution of Drifts is one too). */
export function proposalOpen(backlog: readonly BoardCard[]): boolean {
  return backlog.some((card) => card.kind === "proposal" || card.kind === "resolution");
}

function PathCard({
  icon: Icon,
  title,
  text,
  onClick,
}: {
  icon: LucideIcon;
  title: string;
  text: string;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="flex flex-1 flex-col items-start gap-2 rounded-lg p-4 text-left shadow-ring transition-colors duration-150 hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background motion-reduce:transition-none"
    >
      <span className="flex items-center gap-2 font-medium">
        <Icon aria-hidden className="size-4" />
        {title}
      </span>
      <span className="text-sm text-muted-foreground">{text}</span>
    </button>
  );
}

/** The omp status box while omp is not ready: no job starts without it. */
function OmpNotReady() {
  const { status } = useOmp();
  return status?.state === "ready" ? null : <OmpSetup place="elsewhere" providers="when-needed" />;
}

function Paths({ onImport }: { onImport: () => void }) {
  const { status } = useProjectStatus();
  const { startFromScratch } = useRequests();
  return (
    <>
      <div className="flex flex-col gap-3 min-[640px]:flex-row">
        <PathCard
          icon={Sparkles}
          title="Start from Scratch"
          text="For an application still to be built: you describe it, an agent designs it."
          onClick={startFromScratch}
        />
        <PathCard
          icon={FolderCode}
          title="Import from Code"
          text="For a project that already has code: an agent describes it as Proposals."
          onClick={onImport}
        />
      </div>
      {status?.folderHasFiles ? (
        <p className="flex items-center gap-2 text-sm">
          <TriangleAlert aria-hidden className={`size-4 shrink-0 ${TONE_TEXT.peach}`} />
          Start from Scratch ignores the files already in this folder unless your request mentions them.
        </p>
      ) : null}
    </>
  );
}

/**
 * `Empty Model`: above the Board, the two ways to begin (`Start from Scratch`, `Import from Code`),
 * the omp status box while omp is not ready, and the import panel. The cards hide while a setup or
 * an import job works or a Proposal is open.
 */
export function SetupLayer({ children }: { children: ReactNode }) {
  const { status } = useProjectStatus();
  const { jobs } = useJobs();
  const { snapshot } = useBoard();
  const [importing, setImporting] = useState(false);
  const empty = status?.state === "empty-model";
  const busy = setupWorks(jobs) || proposalOpen(snapshot?.backlog ?? []);
  return (
    <div className="flex h-full min-h-0 flex-col">
      {empty ? (
        <div className="flex shrink-0 flex-col gap-4 p-4 pb-0 empty:hidden">
          <OmpNotReady />
          {busy ? null : importing ? (
            <ImportPanel onClose={() => setImporting(false)} onStarted={() => setImporting(false)} />
          ) : (
            <Paths onImport={() => setImporting(true)} />
          )}
        </div>
      ) : null}
      <div className="min-h-0 flex-1">{children}</div>
    </div>
  );
}
