import { TriangleAlert } from "lucide-react";
import { useEffect, useId, useRef, useState, type ReactNode } from "react";

import type { SoloMode } from "@/bindings/SoloMode";
import { useProjectStatus } from "@/components/project/use-project-status";
import { Alert } from "@/components/ui/alert";
import { Label } from "@/components/ui/label";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { useViewShown } from "@/components/view-shown";
import { call, isAppError } from "@/lib/backend";

/** `requirements/ui#solo-mode`: the three choices of the control, in order. */
export const SOLO_CHOICES: readonly { value: SoloMode; label: string }[] = [
  { value: "off", label: "Off" },
  { value: "folders", label: "Whole folders" },
  { value: "files", label: "Weaver's files only" },
];

export const SOLO_NOTE = "Applies only to this working copy: other clones start with it off.";
export const SOLO_VERSIONED_NOWHERE = "Weaver's files are not versioned anywhere: losing this working copy loses them.";
export const SOLO_CHOOSE_AT_INITIALISE = "Choose Solo mode when you initialise the project.";
export const SOLO_FINISH_MERGE = "Finish or abort the merge in progress first.";

/** The reasons that say git, or the repository around the folder, is not there (`requirements/rules#solo-mode`). */
const GIT_MISSING = ["git was not found", "this folder is not in a git repository"];

/** A reason without its own final full stop, so that the text around it can end the sentence. */
function bare(reason: string): string {
  return reason.trim().replace(/\.+$/, "");
}

/** `Solo mode needs git: <reason>.` */
export function soloNeedsGit(reason: string): string {
  return `Solo mode needs git: ${bare(reason)}.`;
}

/** `Could not change Solo mode: <reason>.` */
export function soloChangeFailure(reason: string): string {
  return `Could not change Solo mode: ${bare(reason)}.`;
}

/**
 * Solo mode is on but the block was not put in place at the last opening: `cannot exclude` when git
 * is missing or the folder is no longer in a repository, otherwise `may not be excluded`.
 */
export function soloNotExcluded(reason: string): string {
  const why = bare(reason);
  return GIT_MISSING.includes(why)
    ? `Solo mode is on but cannot exclude Weaver's files: ${why}.`
    : `Weaver's files may not be excluded from git: ${why}.`;
}

/** What Git still tracks: `Git still tracks <N> of Weaver's files: their changes still go into commits.` */
export function soloTrackedText(count: number): string {
  return `Git still tracks ${count} of Weaver's files: their changes still go into commits.`;
}

/** A line under the control: part of its description for assistive technology. */
export interface SoloMessage {
  /** Stable key of the line. */
  key: string;
  tone: "info" | "warning" | "error";
  text: string;
}

/**
 * The control of `requirements/ui#solo-mode`: the title `Solo mode`, a segmented control (a radio
 * group for assistive technology, the same component as `Theme`), the note under it, and the lines
 * that follow (the reason it is disabled, the warnings, a refusal). The note and the lines are its
 * description, and the lines are a polite live region, so a change of them is announced.
 */
export function SoloModeField({
  value,
  onChange,
  disabled,
  messages,
  children,
}: {
  value: SoloMode;
  onChange: (mode: SoloMode) => void;
  disabled?: boolean;
  messages: readonly SoloMessage[];
  /** Below the lines: the list of tracked files in Settings. */
  children?: ReactNode;
}) {
  const id = useId();
  const labelId = `${id}-label`;
  const noteId = `${id}-note`;
  const messageId = (key: string) => `${id}-${key}`;
  const describedBy = [noteId, ...messages.map((message) => messageId(message.key))].join(" ");
  return (
    <div className="flex flex-col gap-1.5">
      <Label id={labelId}>Solo mode</Label>
      <ToggleGroup
        type="single"
        value={value}
        disabled={disabled}
        aria-labelledby={labelId}
        aria-describedby={describedBy}
        // Choosing the pressed item would unset it; a value is always in force.
        onValueChange={(next) => next && onChange(next as SoloMode)}
        className="w-fit"
      >
        {SOLO_CHOICES.map((choice) => (
          <ToggleGroupItem key={choice.value} value={choice.value}>
            {choice.label}
          </ToggleGroupItem>
        ))}
      </ToggleGroup>
      <p id={noteId} className="text-sm text-muted-foreground">
        {SOLO_NOTE}
      </p>
      <div aria-live="polite" className="flex flex-col gap-1.5">
        {messages.map((message) => (
          <p
            key={message.key}
            id={messageId(message.key)}
            role={message.tone === "error" ? "alert" : undefined}
            className={
              message.tone === "info"
                ? "text-sm text-muted-foreground"
                : message.tone === "warning"
                  ? "flex items-start gap-1.5 text-sm text-amber-600 dark:text-amber-400"
                  : "flex items-start gap-1.5 text-sm text-red-600 dark:text-red-400"
            }
          >
            {message.tone === "info" ? null : <TriangleAlert className="mt-0.5 size-4 shrink-0" aria-hidden />}
            {message.text}
          </p>
        ))}
      </div>
      {children}
    </div>
  );
}

/** The warning of Solo mode on, as a line under the control. */
export const SOLO_ON_MESSAGE: SoloMessage = { key: "versioned", tone: "warning", text: SOLO_VERSIONED_NOWHERE };

/**
 * `Solo mode` in Settings (group `Project`, before `Update skills`): the project's mode, changed at
 * once with no confirmation and shown disabled while a change runs. A change that fails changes
 * nothing, says why and puts back the project's value. While Solo mode is on, the files git still
 * tracks are asked for each time the view is shown and replaced by the answer of every change.
 */
export function SoloModeSetting() {
  const { projectId, status, refresh } = useProjectStatus();
  const shown = useViewShown();
  const [pending, setPending] = useState<SoloMode | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  const [tracked, setTracked] = useState<readonly string[]>([]);
  const trackedId = useId();
  const alive = useRef(true);
  const busy = useRef(false);
  // The mode a change of ours already answered the tracked files of: the effect need not ask again.
  const answered = useRef<SoloMode | null>(null);

  const solo = status?.solo;
  const mode = solo?.mode ?? "off";
  const ready = status !== null;

  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
    };
  }, []);

  // `solo_tracked_files` each time the view is shown (and the status first arrives, or the mode changes
  // elsewhere); while it loads the earlier list stays.
  useEffect(() => {
    if (!shown || !ready || mode === "off") return;
    if (answered.current === mode) {
      answered.current = null;
      return;
    }
    let current = true;
    call("solo_tracked_files", { projectId })
      .then((files) => {
        if (current && alive.current) setTracked(files);
      })
      .catch(() => {});
    return () => {
      current = false;
    };
  }, [shown, ready, mode, projectId]);

  if (!status || !solo) return null;

  const notInitialised = status.state === "not-initialised";
  const noGit = mode === "off" && solo.unavailable !== undefined;
  // While a merge is in progress Solo mode cannot change in any way (`requirements/ui#solo-mode`, **During a merge**).
  const merging = status.merge?.inProgress !== undefined;
  const disabled = notInitialised || noGit || merging || pending !== null;

  const choose = (next: SoloMode) => {
    if (busy.current || next === mode) return;
    busy.current = true;
    setFailure(null);
    setPending(next);
    call("set_solo_mode", { projectId, mode: next })
      .then((change) => {
        answered.current = change.solo.mode;
        if (alive.current) setTracked(change.tracked);
      })
      .catch((error: unknown) => {
        if (alive.current) setFailure(soloChangeFailure(isAppError(error) ? error.message : "Weaver could not change it"));
      })
      .finally(() => {
        busy.current = false;
        if (!alive.current) return;
        setPending(null);
        refresh();
      });
  };

  const messages: SoloMessage[] = [];
  if (merging) messages.push({ key: "merge", tone: "info", text: SOLO_FINISH_MERGE });
  if (notInitialised) messages.push({ key: "uninitialised", tone: "info", text: SOLO_CHOOSE_AT_INITIALISE });
  else if (noGit && solo.unavailable) messages.push({ key: "git", tone: "info", text: soloNeedsGit(solo.unavailable) });
  const shownMode = pending ?? mode;
  if (shownMode !== "off") messages.push(SOLO_ON_MESSAGE);
  if (mode !== "off" && solo.problem) messages.push({ key: "problem", tone: "warning", text: soloNotExcluded(solo.problem) });
  if (failure) messages.push({ key: "failure", tone: "error", text: failure });

  return (
    <SoloModeField value={shownMode} onChange={choose} disabled={disabled} messages={messages}>
      {mode !== "off" && tracked.length > 0 ? (
        <Alert variant="warning" role="region" aria-labelledby={`${trackedId}-title`} className="mt-1">
          <TriangleAlert aria-hidden />
          <div className="flex min-w-0 flex-1 flex-col gap-2">
            <p id={`${trackedId}-title`}>{soloTrackedText(tracked.length)}</p>
            <ul aria-label="Files git still tracks" className="max-h-[12rem] overflow-auto font-mono text-xs leading-6">
              {tracked.map((path) => (
                <li key={path} className="break-all">
                  {path}
                </li>
              ))}
            </ul>
          </div>
        </Alert>
      ) : null}
    </SoloModeField>
  );
}
