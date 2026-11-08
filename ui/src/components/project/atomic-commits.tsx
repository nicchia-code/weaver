import { TriangleAlert } from "lucide-react";
import { useEffect, useId, useRef, useState } from "react";

import { useProjectStatus } from "@/components/project/use-project-status";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { useViewShown } from "@/components/view-shown";
import { call, isAppError } from "@/lib/backend";

export const ATOMIC_COMMITS_NOTE = "Applies only to this working copy: after each History entry, Weaver commits the code the agent wrote for it.";
export const ATOMIC_COMMITS_INITIALISE = "Initialise the project to use Atomic commits.";

/** A reason without its own final full stop, so that the text around it can end the sentence. */
function bare(reason: string): string {
  return reason.trim().replace(/\.+$/, "");
}

/** `Atomic commits needs git: <reason>.` */
export function atomicCommitsNeedsGit(reason: string): string {
  return `Atomic commits needs git: ${bare(reason)}.`;
}

/** `The Atomic commits file counts as off: <path>: <reason>.` */
export function atomicCommitsDiagnostic(diagnostic: string): string {
  return `The Atomic commits file counts as off: ${bare(diagnostic)}.`;
}

/** `Could not change Atomic commits: <reason>.` */
export function atomicCommitsChangeFailure(reason: string): string {
  return `Could not change Atomic commits: ${bare(reason)}.`;
}

/**
 * `Atomic commits` in Settings (group `Project`, between `Solo mode` and `Update skills`,
 * `requirements/ui#atomic-commits`): a Switch like `System Certificates` that shows the project's
 * state, not a setting. A change is sent to the backend at once with no confirmation, the switch is
 * disabled while it runs, and a change that fails says why and puts back the project's value. The
 * note and the reason it is disabled are its description. Whether git can be used is asked of the
 * backend each time the view is shown.
 */
export function AtomicCommitsSetting() {
  const { projectId, status, refresh } = useProjectStatus();
  const shown = useViewShown();
  const controlId = useId();
  const [pending, setPending] = useState<boolean | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  const alive = useRef(true);
  const busy = useRef(false);

  const ready = status !== null;
  const notInitialised = status?.state === "not-initialised";

  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
    };
  }, []);

  // The reason it cannot be turned on is worked out again each time the view is shown; the answer
  // also arrives with `project-state`.
  useEffect(() => {
    if (!shown || !ready || notInitialised) return;
    call("check_atomic_commits", { projectId }).catch(() => {});
  }, [shown, ready, notInitialised, projectId]);

  if (!status) return null;

  const atomic = status.atomicCommits;
  const enabled = !notInitialised && atomic.enabled;
  const noGit = !enabled && atomic.unavailable !== undefined;
  const disabled = notInitialised || noGit || pending !== null;

  const change = (next: boolean) => {
    if (busy.current || next === enabled) return;
    busy.current = true;
    setFailure(null);
    setPending(next);
    call("set_atomic_commits", { projectId, enabled: next })
      .catch((error: unknown) => {
        if (alive.current) setFailure(atomicCommitsChangeFailure(isAppError(error) ? error.message : "Weaver could not change it"));
      })
      .finally(() => {
        busy.current = false;
        if (!alive.current) return;
        setPending(null);
        refresh();
      });
  };

  const noteId = `${controlId}-note`;
  const reasonId = `${controlId}-reason`;
  const diagnosticId = `${controlId}-diagnostic`;
  const failureId = `${controlId}-failure`;
  const reason = notInitialised ? ATOMIC_COMMITS_INITIALISE : noGit && atomic.unavailable ? atomicCommitsNeedsGit(atomic.unavailable) : null;
  // A state file that cannot be used counts as off; the next change rewrites it.
  const diagnostic = !enabled && atomic.diagnostic ? atomicCommitsDiagnostic(atomic.diagnostic) : null;
  const describedBy = [noteId, reason ? reasonId : null, diagnostic ? diagnosticId : null, failure ? failureId : null].filter(Boolean).join(" ");

  return (
    <div className="flex flex-col gap-1.5">
      <div className="flex items-center justify-between gap-3">
        <Label htmlFor={controlId}>Atomic commits</Label>
        <Switch
          id={controlId}
          checked={pending ?? enabled}
          disabled={disabled}
          aria-describedby={describedBy}
          aria-invalid={failure ? true : undefined}
          onCheckedChange={change}
        />
      </div>
      <p id={noteId} className="text-sm text-muted-foreground">
        {ATOMIC_COMMITS_NOTE}
      </p>
      <div aria-live="polite" className="flex flex-col gap-1.5 empty:hidden">
        {reason ? (
          <p id={reasonId} className="text-sm text-muted-foreground">
            {reason}
          </p>
        ) : null}
        {diagnostic ? (
          <p id={diagnosticId} className="flex items-start gap-1.5 text-sm text-peach-ink">
            <TriangleAlert className="mt-0.5 size-4 shrink-0" aria-hidden />
            {diagnostic}
          </p>
        ) : null}
      </div>
      {failure ? (
        <p id={failureId} role="alert" className="flex items-start gap-1.5 text-sm text-rose-ink">
          <TriangleAlert className="mt-0.5 size-4 shrink-0" aria-hidden />
          {failure}
        </p>
      ) : null}
    </div>
  );
}
