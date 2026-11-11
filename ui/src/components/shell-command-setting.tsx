import { Loader, Terminal, TriangleAlert } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";

import type { ShellCommandResult } from "@/bindings/ShellCommandResult";
import { Button } from "@/components/ui/button";
import { useViewShown } from "@/components/view-shown";
import { call, isAppError } from "@/lib/backend";
import { cn } from "@/lib/utils";

type Action = "install" | "remove";

/** What stays under the button until the next action: a line in a tone. Nothing for a cancelled password request. */
export interface ShellCommandNote {
  tone: "info" | "warning" | "error";
  text: string;
}

/** A reason without its own final full stop, so that the text around it can end the sentence. */
function bare(reason: string): string {
  return reason.trim().replace(/\.+$/, "");
}

/** `Could not install the weaver command: <reason>.` / `Could not remove the weaver command: <reason>.` */
export function shellCommandFailure(action: Action, reason: string): string {
  return `Could not ${action} the weaver command: ${bare(reason)}.`;
}

export const PATH_WARNING = "~/.local/bin may not be in your PATH; see the README.";

/** What an action's result says under the button (`requirements/ui#shell-command`, **Result**). */
export function noteOf(action: Action, result: ShellCommandResult): ShellCommandNote | null {
  const outcome = result.outcome;
  switch (outcome.kind) {
    case "refused":
      return {
        tone: "info",
        text:
          action === "install"
            ? `${outcome.found} already exists and is not Weaver's command; nothing was changed.`
            : `${outcome.found} is not Weaver's command; nothing was removed.`,
      };
    case "failed":
      return { tone: "error", text: shellCommandFailure(action, outcome.reason) };
    case "cancelled":
      return null;
    case "done":
      return action === "install" && result.pathWarning ? { tone: "warning", text: PATH_WARNING } : null;
  }
}

const TONE: Record<ShellCommandNote["tone"], string> = {
  info: "text-foreground",
  warning: "text-peach-ink",
  error: "text-rose-ink",
};

/**
 * The `weaver` command in the developer's own terminal (`requirements/ui#shell-command`): what
 * `weaver open` does, whether the command is installed and where, and one button that installs or
 * removes it. The state is asked of the backend each time Settings shows and is replaced by what
 * every action answers; the result of an action stays under the button until the next one. Weaver
 * never installs the command by itself: only this button does.
 */
export function ShellCommandSetting() {
  const shown = useViewShown();
  const [status, setStatus] = useState<ShellCommandResult | null>(null);
  const [readFailure, setReadFailure] = useState<string | null>(null);
  const [running, setRunning] = useState<Action | null>(null);
  const [note, setNote] = useState<ShellCommandNote | null>(null);
  const alive = useRef(true);
  const busy = useRef(false);
  // A reading that started before an action must not replace what the action answered.
  const epoch = useRef(0);

  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
    };
  }, []);

  const read = useCallback(() => {
    const ticket = epoch.current;
    call("shell_command_status")
      .then((result) => {
        if (!alive.current || epoch.current !== ticket || !result) return;
        setStatus(result);
        setReadFailure(result.outcome.kind === "failed" ? result.outcome.reason : null);
      })
      .catch((error: unknown) => {
        if (!alive.current || epoch.current !== ticket) return;
        setReadFailure(isAppError(error) ? error.message : "Weaver could not read it");
      });
  }, []);

  useEffect(() => {
    if (shown) read();
  }, [shown, read]);

  const act = (action: Action) => {
    if (busy.current) return;
    busy.current = true;
    epoch.current += 1;
    setRunning(action);
    setNote(null);
    call(action === "install" ? "shell_command_install" : "shell_command_remove")
      .then((result) => {
        if (!alive.current) return;
        setStatus(result);
        setReadFailure(null);
        setNote(noteOf(action, result));
      })
      .catch((error: unknown) => {
        if (!alive.current) return;
        setNote({ tone: "error", text: shellCommandFailure(action, isAppError(error) ? error.message : "Weaver could not do it") });
        read();
      })
      .finally(() => {
        busy.current = false;
        if (alive.current) setRunning(null);
      });
  };

  const installed = status?.installed ?? false;
  const action: Action = installed ? "remove" : "install";

  return (
    <div className="flex flex-col gap-3">
      <p className="text-sm text-muted-foreground">
        Open a folder in Weaver from a terminal with <code className="font-mono text-foreground">weaver open &lt;folder&gt;</code>.
      </p>
      {status ? (
        <p className="text-sm">
          {installed ? (
            <>
              Installed at <code className="font-mono">{status.place}</code>
            </>
          ) : (
            "Not installed"
          )}
        </p>
      ) : null}
      {readFailure ? (
        <p role="alert" className="flex items-start gap-1.5 text-sm text-rose-ink">
          <TriangleAlert className="mt-0.5 size-4 shrink-0" aria-hidden />
          {`Could not check the weaver command: ${bare(readFailure)}.`}
        </p>
      ) : null}
      {status ? (
        <div>
          <Button type="button" variant="outline" disabled={running !== null} onClick={() => act(action)}>
            {action === "install" ? (
              <Terminal aria-hidden className={cn(running === "install" && "animate-spin motion-reduce:animate-none")} />
            ) : running === "remove" ? (
              <Loader aria-hidden className="animate-spin motion-reduce:animate-none" />
            ) : null}
            {action === "install" ? "Install weaver command" : "Remove"}
          </Button>
        </div>
      ) : null}
      <div role="status" aria-live="polite" className="text-sm empty:hidden">
        {note ? <p className={TONE[note.tone]}>{note.text}</p> : null}
      </div>
    </div>
  );
}
