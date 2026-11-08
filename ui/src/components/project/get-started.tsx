import { TriangleAlert } from "lucide-react";
import { useRef, useState } from "react";

import { ConfirmDialog } from "@/components/dialogs";
import type { SoloMode } from "@/bindings/SoloMode";
import { SOLO_ON_MESSAGE, SoloModeField, soloNeedsGit, type SoloMessage } from "@/components/project/solo-mode";
import { useProjectStatus } from "@/components/project/use-project-status";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { call, isAppError } from "@/lib/backend";

/** `Could not initialise: <reason>.` (the reason without its own full stop). */
export function initialiseFailure(reason: string): string {
  return `Could not initialise: ${reason.trim().replace(/\.+$/, "")}.`;
}

/** The relative paths of agent files, in monospace; the list scrolls past 8 lines. */
function PathList({ paths, label }: { paths: readonly string[]; label: string }) {
  return (
    <ul aria-label={label} className="max-h-[12rem] overflow-auto rounded-lg bg-muted shadow-ring px-3 py-2 font-mono text-xs leading-6">
      {paths.map((path) => (
        <li key={path} className="break-all">
          {path}
        </li>
      ))}
    </ul>
  );
}

/**
 * `Not initialised`: what every view of the window except Settings shows. `Initialise` creates the
 * `.weaver` folder and the agent files; when agent files exist with other content nothing is created
 * until the developer confirms overwriting them. `headingId` is the id the view's region is labelled by.
 */
export function GetStarted({ headingId }: { headingId?: string }) {
  const { projectId, status, refresh } = useProjectStatus();
  const [running, setRunning] = useState(false);
  const [conflicts, setConflicts] = useState<string[] | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  // The choice of Solo mode lives only in this screen, and starts on `Off`.
  const [solo, setSolo] = useState<SoloMode>("off");
  const button = useRef<HTMLButtonElement>(null);

  // Without git, or outside a repository, the choice stays on `Off` and says why.
  const unavailable = status?.solo.unavailable;
  const chosen: SoloMode = unavailable !== undefined ? "off" : solo;
  const messages: SoloMessage[] = [];
  if (unavailable !== undefined) messages.push({ key: "git", tone: "info", text: soloNeedsGit(unavailable) });
  if (chosen !== "off") messages.push(SOLO_ON_MESSAGE);

  const initialise = (force: boolean) => {
    setRunning(true);
    setFailure(null);
    call("initialise_project", { projectId, force, solo: chosen })
      .then((result) => {
        if (result.conflicts.length > 0) setConflicts(result.conflicts);
        else refresh();
      })
      .catch((error: unknown) => setFailure(initialiseFailure(isAppError(error) ? error.message : "the project could not be created")))
      .finally(() => setRunning(false));
  };

  return (
    <div className="flex h-full items-start justify-center overflow-auto px-6 py-10">
      <div className="flex w-full max-w-[560px] flex-col gap-5">
        <h2 id={headingId} className="text-xl font-semibold">
          Set up Weaver in this folder
        </h2>
        <p className="text-sm">
          {chosen === "off"
            ? "Weaver will create a .weaver folder that describes this project. It holds no secrets and is meant to be committed, like the agent files it installs."
            : "Weaver will create a .weaver folder that describes this project. With Solo mode its files stay only in this working copy and never go into a commit."}
        </p>
        {status ? (
          <div className="flex flex-col gap-2">
            <p className="text-sm font-medium">Agent files Weaver installs</p>
            <PathList paths={status.files} label="Agent files" />
          </div>
        ) : null}
        {status ? (
          <SoloModeField value={chosen} onChange={setSolo} disabled={running || unavailable !== undefined} messages={messages} />
        ) : null}
        <div>
          <Button ref={button} type="button" disabled={running || !status} onClick={() => initialise(false)}>
            Initialise
          </Button>
        </div>
        {failure ? (
          <Alert variant="error">
            <TriangleAlert aria-hidden />
            <p className="min-w-0 flex-1">{failure}</p>
          </Alert>
        ) : null}
      </div>
      <ConfirmDialog
        open={conflicts !== null}
        onOpenChange={(open) => {
          if (!open) setConflicts(null);
        }}
        title="Some agent files already exist"
        description="They have other content than the files Weaver installs. Initialising overwrites them."
        confirmLabel="Overwrite and initialise"
        destructive
        returnFocus={button}
        onConfirm={() => initialise(true)}
      >
        {conflicts ? <PathList paths={conflicts} label="Existing agent files" /> : null}
      </ConfirmDialog>
    </div>
  );
}
