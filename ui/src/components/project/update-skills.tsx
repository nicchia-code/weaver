import { RefreshCw, TriangleAlert } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { toast } from "sonner";

import type { SkillsReport } from "@/bindings/SkillsReport";
import { reportView, skillsFailure } from "@/components/project/skills-report";
import { useProjectStatus } from "@/components/project/use-project-status";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { call, isAppError } from "@/lib/backend";
import { cn } from "@/lib/utils";

/** What the last run of `Update skills` ended with: its report, or why it failed. */
export type SkillsRun = { kind: "report"; report: SkillsReport } | { kind: "failed"; reason: string };

/** `developer.UPDATE_SKILLS`: runs once at a time, and keeps what the last run said until the next one. */
export function useUpdateSkills(): { run: () => void; running: boolean; last: SkillsRun | null } {
  const { projectId, refresh } = useProjectStatus();
  const [running, setRunning] = useState(false);
  const [last, setLast] = useState<SkillsRun | null>(null);
  const busy = useRef(false);
  const alive = useRef(true);

  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
    };
  }, []);

  const run = useCallback(() => {
    if (busy.current) return;
    busy.current = true;
    setRunning(true);
    call("update_skills", { projectId })
      .then((report) => alive.current && setLast({ kind: "report", report }))
      .catch((error: unknown) => {
        if (alive.current) setLast({ kind: "failed", reason: isAppError(error) ? error.message : "the files could not be written" });
      })
      .finally(() => {
        busy.current = false;
        if (!alive.current) return;
        setRunning(false);
        refresh();
      });
  }, [projectId, refresh]);

  return { run, running, last };
}

/**
 * `Update skills` in Settings (group `Project`): the button spins while it runs and is disabled
 * meanwhile; its report stays under it until the next run.
 */
export function UpdateSkills() {
  const { run, running, last } = useUpdateSkills();
  const view = last?.kind === "report" ? reportView(last.report) : null;
  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-col gap-1">
        <p className="text-sm font-medium">Agent files</p>
        <p className="text-sm text-muted-foreground">
          Writes again every file Weaver installs for the agent that differs from the one this version of Weaver carries.
        </p>
      </div>
      <div>
        <Button type="button" variant="outline" disabled={running} onClick={run}>
          <RefreshCw aria-hidden className={cn(running && "animate-spin motion-reduce:animate-none")} />
          Update skills
        </Button>
      </div>
      <div role="status" aria-live="polite" className="text-sm">
        {view ? (
          <div className="flex flex-col gap-1">
            <p>{view.text}</p>
            {view.kind === "changed" ? (
              <ul className="flex flex-col gap-0.5">
                {view.files.map((file) => (
                  <li key={file.path} className="flex gap-2">
                    <code className="font-mono text-xs">{file.path}</code>
                    <span className="text-muted-foreground">{file.action}</span>
                  </li>
                ))}
              </ul>
            ) : null}
          </div>
        ) : last?.kind === "failed" ? (
          <p className="text-rose-ink">{skillsFailure(last.reason)}</p>
        ) : null}
      </div>
    </div>
  );
}

/**
 * What the check of the installed files, run in the background when the window opened, says: a
 * toast when it rewrote files (once per opening), and nothing else. A failure is the banner's.
 */
export function OpeningCheckToast() {
  const { status } = useProjectStatus();
  const told = useRef(0);
  const opening = status?.check.opening ?? 0;
  const outcome = status?.check.outcome;
  useEffect(() => {
    if (opening <= told.current) return;
    told.current = opening;
    if (outcome?.kind === "updated") toast("Updated the agent files to this version of Weaver.");
  }, [opening, outcome]);
  return null;
}

/**
 * `Could not update the agent files: <reason>.` at the top of the window, with `Retry` (Update
 * skills), at every opening until the files can be written.
 */
export function SkillsBanner() {
  const { status } = useProjectStatus();
  const { run, running, last } = useUpdateSkills();
  const outcome = status?.check.outcome;
  if (outcome?.kind !== "failed") return null;
  const reason = last?.kind === "failed" ? last.reason : outcome.reason;
  return (
    <div className="p-3 pb-0">
      <Alert variant="error">
        <TriangleAlert aria-hidden />
        <p className="min-w-0 flex-1">{skillsFailure(reason)}</p>
        <Button type="button" variant="outline" size="sm" disabled={running} onClick={run}>
          Retry
        </Button>
      </Alert>
    </div>
  );
}
