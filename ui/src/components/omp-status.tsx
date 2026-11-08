import { CircleCheck, Loader, LogIn, LogOut, TriangleAlert, type LucideIcon } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";

import type { LogoutNote } from "@/bindings/LogoutNote";
import type { OmpStatus } from "@/bindings/OmpStatus";
import type { ProviderRow } from "@/bindings/ProviderRow";
import { ConfirmDialog } from "@/components/dialogs";
import { SettingsView, SystemCertificatesSetting, type ModelCatalog, type Place, type SettingsViewProps } from "@/components/settings-view";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { useOmp } from "@/hooks/use-omp";
import { call, isAppError } from "@/lib/backend";
import { SEMANTIC_TONE, TONE_TEXT } from "@/lib/tones";
import { cn } from "@/lib/utils";

// ---------------------------------------------------------------------------------------------
// The status box.
// ---------------------------------------------------------------------------------------------

type Appearance = "neutral" | "warning" | "success";

interface BoxFace {
  icon: LucideIcon;
  /** The `Loader` turns; under reduced motion it stays still. */
  spins: boolean;
  appearance: Appearance;
  text: string;
  action: { label: string; variant: "default" | "outline" } | null;
  /** The secondary line under the text. */
  hint: string | null;
}

const FINISH_IN_TERMINAL = "Finish it in the terminal window. Weaver checks again every few seconds.";
const EXPORTING_FIRST = "Exporting system certificates first…";

/** The name in the middle of a sentence. */
function nameIn(place: Place): string {
  return place === "settings" ? "omp" : "the agent";
}

/** The name that starts a sentence, with the version Settings knows. */
function subjectIn(place: Place, version: string | null): string {
  if (place === "elsewhere") return "The agent";
  return version ? `omp ${version}` : "omp";
}

/** The explanations of a certificate or HTTP error (`requirements/ui#system-certificates`) have no final full stop. */
const PROXY_EXPLANATION = /^the agent (could not verify a server's certificate|received an HTTP \d{3} error;)/;

/** The Missing text of a proxy explanation ends with a full stop; the other reasons are shown as they are. */
function withFinalStop(reason: string): string {
  return PROXY_EXPLANATION.test(reason) && !reason.endsWith(".") ? `${reason}.` : reason;
}

function providerName(status: OmpStatus, id: string | null): string {
  return status.providers.find((p) => p.id === id)?.name ?? id ?? "the provider";
}

/** The icon, text and action of each State (`requirements/ui#omp-setup`); `null` status is Checking. */
export function boxFace(status: OmpStatus | null, place: Place): BoxFace {
  const agent = nameIn(place);
  const checking: BoxFace = { icon: Loader, spins: true, appearance: "neutral", text: `Checking ${agent}…`, action: null, hint: null };
  if (!status) return checking;
  const subject = subjectIn(place, status.version);
  switch (status.state) {
    case "checking":
      // At the app's start, while System Certificates exports its bundle: the first check waits.
      return status.waitingForExport ? { ...checking, hint: EXPORTING_FIRST } : checking;
    case "missing": {
      const reason = status.reason ? withFinalStop(status.reason) : null;
      // After a failed installer its text, with the exit code; the reason of the last check, if
      // any, stays under it.
      const text = status.installerFailure ?? reason ?? `${subjectIn(place, null)} is not installed.`;
      return {
        icon: TriangleAlert,
        spins: false,
        appearance: "warning",
        text,
        action: { label: `Install ${agent}`, variant: "default" },
        hint: status.installerFailure ? reason : null,
      };
    }
    case "installing":
      return {
        icon: Loader,
        spins: true,
        appearance: "neutral",
        text: `Installing ${agent} in the terminal…`,
        action: { label: "Open installer again", variant: "outline" },
        hint: FINISH_IN_TERMINAL,
      };
    case "updating":
      return {
        icon: Loader,
        spins: true,
        appearance: "neutral",
        text: status.updateTarget ? `Updating ${agent} to ${status.updateTarget}…` : `Updating ${agent}…`,
        action: null,
        hint: null,
      };
    case "notConnected":
      return {
        icon: TriangleAlert,
        spins: false,
        appearance: "warning",
        text: `${subject} has no model provider. Log in to one below.`,
        action: null,
        hint: null,
      };
    case "loggingIn":
      return {
        icon: Loader,
        spins: true,
        appearance: "neutral",
        text: `Waiting for the login to ${providerName(status, status.loginProvider)} in the terminal…`,
        action: null,
        hint: FINISH_IN_TERMINAL,
      };
    case "ready": {
      const models = status.models.length === 1 ? "1 model" : `${status.models.length} models`;
      const ready = `${subject} is ready with ${models}.`;
      return status.updateFailure
        ? {
            icon: TriangleAlert,
            spins: false,
            appearance: "warning",
            text: `Could not update ${agent} (${status.updateFailure.replace(/\.$/, "")}). ${ready}`,
            action: null,
            hint: null,
          }
        : { icon: CircleCheck, spins: false, appearance: "success", text: ready, action: null, hint: null };
    }
  }
}

const APPEARANCE_VARIANT = { neutral: "default", warning: "warning", success: "success" } as const;

function TerminalProblemNote({ reason, command }: { reason: string; command: string }) {
  const [copiedCommand, setCopiedCommand] = useState<string | null>(null);
  const copy = () => {
    navigator.clipboard
      .writeText(command)
      .then(() => setCopiedCommand(command))
      .catch(() => {});
  };
  return (
    <div className="flex flex-col gap-2">
      <p className={TONE_TEXT[SEMANTIC_TONE.error]}>{`Could not open a terminal: ${reason.replace(/\.$/, "")}.`}</p>
      <div className="flex items-start gap-2">
        <code className="min-w-0 flex-1 break-all rounded-md bg-muted px-2 py-1 font-mono text-xs">{command}</code>
        <Button variant="outline" size="sm" onClick={copy}>
          {copiedCommand === command ? "Copied" : "Copy"}
        </Button>
      </div>
    </div>
  );
}

/**
 * The one status box of omp, the same in Settings, Get Started and the welcome window: a bordered
 * `aria-live="polite"` region with an icon, the text of the State and its action. The state is
 * always text, never colour only.
 */
export function OmpStatusBox({ status, place }: { status: OmpStatus | null; place: Place }) {
  const face = boxFace(status, place);
  const Icon = face.icon;
  const problem = status?.terminalProblem ?? null;
  const install = () => void call("omp_install").catch(() => {});
  return (
    <Alert role="status" aria-live="polite" aria-label={place === "settings" ? "omp status" : "agent status"} variant={APPEARANCE_VARIANT[face.appearance]}>
      <Icon aria-hidden className={cn(face.spins && "animate-spin motion-reduce:animate-none")} />
      <div className="flex min-w-0 flex-1 flex-col gap-2">
        <p>{face.text}</p>
        {face.hint ? <p className="text-muted-foreground">{face.hint}</p> : null}
        {problem ? <TerminalProblemNote reason={problem.reason} command={problem.command} /> : null}
      </div>
      {face.action ? (
        <Button size="sm" variant={face.action.variant} onClick={install}>
          {face.action.label}
        </Button>
      ) : null}
    </Alert>
  );
}

// ---------------------------------------------------------------------------------------------
// Providers.
// ---------------------------------------------------------------------------------------------

function ConnectionState({ connected }: { connected: boolean | null }) {
  if (connected === null) return null;
  return connected ? (
    <span className={cn("inline-flex items-center gap-1 text-sm", TONE_TEXT[SEMANTIC_TONE.success])}>
      <CircleCheck aria-hidden className="size-4" />
      Connected
    </span>
  ) : (
    <span className="text-sm text-muted-foreground">Not connected</span>
  );
}

function LogoutResult({ row, note, failure, place }: { row: ProviderRow; note: LogoutNote | null; failure: string | null; place: Place }) {
  const reason = failure ?? (note?.phase === "failed" ? (note.reason ?? `${nameIn(place)} did not say why`) : null);
  if (reason !== null) {
    return <p className={cn("text-sm", TONE_TEXT[SEMANTIC_TONE.error])}>{`Could not log out of ${row.name}: ${reason.replace(/\.$/, "")}.`}</p>;
  }
  if (note?.phase === "stillConnected") {
    return (
      <p className="text-sm text-muted-foreground">
        {`${row.name} is still connected through another credential (environment variable, .env file or models.yml). Remove it there to disconnect.`}
      </p>
    );
  }
  return null;
}

function ProviderRowView({ status, row, place }: { status: OmpStatus; row: ProviderRow; place: Place }) {
  const note = status.logout?.provider === row.id ? status.logout : null;
  const [requested, setRequested] = useState(false);
  const [failure, setFailure] = useState<string | null>(null);
  const [confirming, setConfirming] = useState(false);
  const logIn = useRef<HTMLButtonElement>(null);

  const busy = requested || note?.phase === "running" || note?.phase === "verifying";
  // When a log out ends, the focus goes to the row's `Log in` (the `Log out` button may be gone).
  const wasBusy = useRef(false);
  useEffect(() => {
    if (wasBusy.current && !busy) logIn.current?.focus();
    wasBusy.current = busy;
  }, [busy]);

  const connected = row.connected === true;
  const loginLabel = connected ? "Log in again" : "Log in";
  // Checking holds the log out of a row and nothing else is asked of omp meanwhile.
  const waiting = status.state === "checking";
  const nameId = `omp-provider-${row.id}`;

  const startLogin = () => {
    setFailure(null);
    void call("omp_log_in", { provider: row.id }).catch(() => {});
  };
  const logOut = () => {
    setFailure(null);
    setRequested(true);
    call("omp_log_out", { provider: row.id })
      .catch((error: unknown) => setFailure(isAppError(error) ? error.message : `${nameIn(place)} could not be asked`))
      .finally(() => setRequested(false));
  };

  return (
    <li aria-labelledby={nameId} aria-busy={busy} className="flex flex-col gap-2 py-3">
      <div className="flex flex-col gap-2 min-[480px]:flex-row min-[480px]:items-center min-[480px]:justify-between">
        <div className="flex min-w-0 flex-wrap items-center gap-x-3 gap-y-1">
          <div className="flex min-w-0 flex-col">
            <span id={nameId} className="font-semibold">
              {row.name}
            </span>
            <span className="text-xs text-muted-foreground">{row.plan}</span>
          </div>
          <ConnectionState connected={row.connected} />
        </div>
        <div className="flex items-center gap-2">
          <Tooltip>
            <TooltipTrigger asChild>
              <Button ref={logIn} variant="outline" size="sm" disabled={busy || waiting} onClick={startLogin}>
                <LogIn aria-hidden />
                {loginLabel}
              </Button>
            </TooltipTrigger>
            <TooltipContent>{`Open a terminal that logs ${nameIn(place)} in to ${row.name}`}</TooltipContent>
          </Tooltip>
          {connected ? (
            <Button variant="ghost" size="sm" disabled={busy || status.state !== "ready"} onClick={() => setConfirming(true)}>
              {busy ? <Loader aria-hidden className="animate-spin motion-reduce:animate-none" /> : <LogOut aria-hidden />}
              Log out
            </Button>
          ) : null}
        </div>
      </div>
      <LogoutResult row={row} note={note} failure={failure} place={place} />
      <ConfirmDialog
        open={confirming}
        onOpenChange={setConfirming}
        title={`Log out of ${row.name}?`}
        description={`${place === "settings" ? "omp" : "The agent"} forgets the accounts of ${row.name} it stored. Credentials in environment variables, .env files or models.yml stay.`}
        confirmLabel="Log out"
        destructive
        onConfirm={logOut}
      />
    </li>
  );
}

/** Whether `Providers` is shown: when omp answers, and while a log out checks the providers again. */
export function providersShown(status: OmpStatus | null): status is OmpStatus {
  if (!status) return false;
  if (status.state === "notConnected" || status.state === "loggingIn" || status.state === "ready") return true;
  return status.state === "checking" && (status.logout?.phase === "running" || status.logout?.phase === "verifying");
}

/** One row per provider of the fixed list, in its order: its state, `Log in` and, when connected, `Log out`. */
export function Providers({ status, place }: { status: OmpStatus; place: Place }) {
  return (
    <section aria-labelledby="omp-providers-title" className="flex flex-col gap-1">
      <h3 id="omp-providers-title" className="text-sm font-semibold">
        Providers
      </h3>
      <ul className="divide-y divide-border">
        {status.providers.map((row) => (
          <ProviderRowView key={row.id} status={status} row={row} place={place} />
        ))}
      </ul>
    </section>
  );
}

/**
 * The status box and, where the developer needs them, the `Providers` below it, then the
 * `System Certificates` switch (outside Settings, where it is not the last of the group). `providers`
 * is `always` in Settings (so a provider can be logged out) and `when-needed` where room is short:
 * only while omp has no provider or a login runs.
 */
export function OmpSetup({
  place,
  providers = "always",
  systemCertificates = true,
}: {
  place: Place;
  providers?: "always" | "when-needed";
  systemCertificates?: boolean;
}) {
  const { status } = useOmp();
  const needed = status?.state === "notConnected" || status?.state === "loggingIn";
  const show = providersShown(status) && (providers === "always" || needed);
  return (
    <div className="flex flex-col gap-4">
      <OmpStatusBox status={status} place={place} />
      {show ? <Providers status={status} place={place} /> : null}
      {systemCertificates ? <SystemCertificatesSetting place={place} /> : null}
    </div>
  );
}

// ---------------------------------------------------------------------------------------------
// Settings, fed by omp.
// ---------------------------------------------------------------------------------------------

/** The models omp lists once it is Ready; in any other State, why there are none. */
export function modelCatalogOf(status: OmpStatus | null): ModelCatalog {
  if (status?.state === "ready") {
    return {
      status: "listed",
      models: status.models.map((model) => ({
        selector: model.selector,
        name: model.name ?? undefined,
        thinkingLevels: model.thinkingLevels,
      })),
    };
  }
  if (!status || status.state === "checking") return { status: "unavailable", reason: "Checking omp…" };
  switch (status.state) {
    case "missing":
      return { status: "unavailable", reason: status.reason ?? "omp is not installed." };
    case "installing":
      return { status: "unavailable", reason: "omp is being installed." };
    case "updating":
      return { status: "unavailable", reason: "omp is being updated." };
    default:
      return { status: "unavailable", reason: "omp has no model provider yet." };
  }
}

/** The Settings view with the omp status box, the providers and the models omp lists. */
export function ConnectedSettingsView({ projectSlot }: Pick<SettingsViewProps, "projectSlot">) {
  const { status } = useOmp();
  const modelCatalog = useMemo(() => modelCatalogOf(status), [status]);
  return <SettingsView ompSlot={<OmpSetup place="settings" systemCertificates={false} />} projectSlot={projectSlot} modelCatalog={modelCatalog} />;
}
