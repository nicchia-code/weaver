import { CircleCheck, Loader, RefreshCw, TriangleAlert } from "lucide-react";
import { useId, useRef, useState, type ReactNode } from "react";

import type { SettingField } from "@/bindings/SettingField";
import type { SettingKey } from "@/bindings/SettingKey";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { GuardedButton } from "@/components/guarded-button";
import { ViewHeader } from "@/components/view-header";
import { Combobox, type ComboboxOption } from "@/components/ui/combobox";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { isAppError } from "@/lib/backend";
import { EXPORTING_TEXT, systemCertificatesLine, type SystemCertificatesLineKind } from "@/lib/system-certificates";
import { cn } from "@/lib/utils";
import { settingText, useSettings } from "@/hooks/use-settings";
import { useSystemCertificates } from "@/hooks/use-system-certificates";

/**
 * Where a text is shown. The name `omp` appears only in Settings; every other place calls the agent
 * `the agent` (`requirements/quality#Texts`).
 */
export type Place = "settings" | "elsewhere";

/** A text of the settings schema, which names omp, as it reads outside Settings. */
function outsideSettings(text: string): string {
  return text.replace(/^omp\b/, "The agent").replace(/\bomp\b/g, "the agent");
}

/** A model omp lists, in omp's order. */
export interface ModelEntry {
  /** What is stored and passed to omp. */
  selector: string;
  /** The readable name, shown next to the selector when it differs. */
  name?: string;
  /** The thinking levels omp lists for the model; none means no level Select. */
  thinkingLevels?: readonly string[];
}

/** What omp said about its models: the list, or why there is none (`omp not found at <path>`, …). */
export type ModelCatalog = { status: "listed"; models: readonly ModelEntry[] } | { status: "unavailable"; reason: string };

export interface SettingsViewProps {
  /** The `omp` status box and `Providers`, shown first in the `omp` group. */
  ompSlot?: ReactNode;
  /** `Update skills`; the `Project` group is shown only with it. */
  projectSlot?: ReactNode;
  /** The models omp lists; without it the model fields only take typing. */
  modelCatalog?: ModelCatalog;
}

/** Sentinel of the thinking-level Select for "no level" (Radix items cannot hold an empty value). */
const NO_LEVEL = "__default__";
const DEFAULT_MODEL_LABEL = "omp default";

// ---------------------------------------------------------------------------------------------
// One setting: its value, what the developer typed, and the outcome of the last save.
// ---------------------------------------------------------------------------------------------

/** How a setting's stored value maps to the text of its control (models carry a level suffix). */
interface Codec {
  display: (stored: string) => string;
  store: (text: string, stored: string) => string;
}

const IDENTITY: Codec = { display: (stored) => stored, store: (text) => text.trim() };

function failureText(title: string, error: unknown): string {
  const reason = isAppError(error) ? error.message : error instanceof Error ? error.message : "unknown error";
  return `Could not save ${title}: ${reason.replace(/\.$/, "")}.`;
}

/**
 * The state of one field. `text` is what the control shows: what the developer typed, else the
 * stored value; a field with the focus keeps showing what it had when focused (a change from
 * another window reaches it on blur, when the developer typed nothing). A refused value stays in
 * the field with its reason; a failed write puts the stored value back.
 */
function useField(key: SettingKey, codec: Codec = IDENTITY) {
  const { snapshot, save } = useSettings();
  const field = snapshot?.schema.find((candidate) => candidate.key === key);
  const stored = snapshot ? settingText(snapshot.settings, key) : "";
  const issue = snapshot?.issues.find((candidate) => candidate.key === key);
  const [draft, setDraft] = useState<string | null>(null);
  const [frozen, setFrozen] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const edited = useRef(false);
  const sequence = useRef(0);
  const latest = useRef(stored);
  latest.current = stored;
  const title = field?.title ?? "the setting";

  const submit = (next: string, onDone?: () => void) => {
    const ticket = sequence.current;
    setError(null);
    save({ key, value: next }).then(
      () => {
        if (sequence.current === ticket) setDraft(null);
        onDone?.();
      },
      (failure: unknown) => {
        if (isAppError(failure) && failure.code === "invalid-input") {
          setError(failure.message);
          return;
        }
        setError(failureText(title, failure));
        if (sequence.current === ticket) setDraft(null);
        setFrozen((current) => (current === null ? null : codec.display(latest.current)));
      },
    );
  };

  return {
    field,
    stored,
    issue,
    error,
    text: draft ?? frozen ?? codec.display(stored),
    focus: () => setFrozen(codec.display(latest.current)),
    blur: () => setFrozen(null),
    edit: (text: string) => {
      edited.current = true;
      sequence.current += 1;
      setDraft(text);
      setError(null);
    },
    /** Saves the typed text: on blur, on Enter, on choosing a suggestion. */
    commit: (text: string) => {
      if (!edited.current) return;
      edited.current = false;
      const next = codec.store(text, latest.current);
      if (next === latest.current) {
        setDraft(null);
        setError(null);
        return;
      }
      submit(next, () => setFrozen((current) => (current === null ? null : codec.display(next))));
    },
    /** Saves at once (a choice in a Select, a segmented control, a radio group). */
    choose: (next: string) => {
      if (next !== latest.current) submit(next);
    },
    /** Saves `next` as the stored value (the thinking level of a model). */
    submit,
  };
}

// ---------------------------------------------------------------------------------------------
// Layout.
// ---------------------------------------------------------------------------------------------

function Group({ title, children }: { title: string; children: ReactNode }) {
  const id = useId();
  return (
    <section aria-labelledby={id} className="flex flex-col gap-5">
      <h2 id={id} className="border-b pb-2 text-sm font-semibold">
        {title}
      </h2>
      {children}
    </section>
  );
}

/** The Settings view: its header across the whole view, then the centred column that scrolls under it. */
function SettingsPage({ className, children }: { className: string; children: ReactNode }) {
  return (
    <div className="flex h-full min-h-0 flex-col">
      <ViewHeader view="settings" />
      <div className="min-h-0 flex-1 overflow-y-auto">
        <div className={cn("mx-auto flex w-full max-w-[720px] flex-col px-6 py-8", className)}>{children}</div>
      </div>
    </div>
  );
}

/** Title, bold advice and description above a control, the refusal and the ignored-key note below. */
function FieldShell({
  field,
  controlId,
  labelId,
  error,
  issue,
  children,
}: {
  field: SettingField;
  controlId?: string;
  labelId?: string;
  error: string | null;
  issue: { reason: string } | undefined;
  children: ReactNode;
}) {
  return (
    <div className="flex flex-col gap-1.5">
      <Label id={labelId} htmlFor={controlId}>
        {field.title}
      </Label>
      <p id={`${controlId ?? labelId}-description`} className="text-sm text-muted-foreground">
        {field.advice ? <strong className="font-semibold text-foreground">{field.advice} </strong> : null}
        {field.description}
      </p>
      {children}
      {error ? (
        <p id={`${controlId ?? labelId}-error`} role="alert" className="flex items-start gap-1.5 text-sm text-rose-ink">
          <TriangleAlert className="mt-0.5 size-4 shrink-0" aria-hidden />
          {error}
        </p>
      ) : null}
      {issue ? (
        <p className="flex items-start gap-1.5 text-sm text-peach-ink">
          <TriangleAlert className="mt-0.5 size-4 shrink-0" aria-hidden />
          {`The value in the settings file was ignored and the default is in force: ${issue.reason}`}
        </p>
      ) : null}
    </div>
  );
}

function describedBy(controlId: string, error: string | null): string {
  return error ? `${controlId}-description ${controlId}-error` : `${controlId}-description`;
}

// ---------------------------------------------------------------------------------------------
// Controls.
// ---------------------------------------------------------------------------------------------

function TextSetting({
  settingKey,
  placeholder,
  mono,
  inputMode,
}: {
  settingKey: SettingKey;
  placeholder?: string;
  mono?: boolean;
  inputMode?: "numeric";
}) {
  const controlId = useId();
  const state = useField(settingKey);
  if (!state.field) return null;
  return (
    <FieldShell field={state.field} controlId={controlId} error={state.error} issue={state.issue}>
      <Input
        id={controlId}
        value={state.text}
        placeholder={placeholder}
        inputMode={inputMode}
        spellCheck={false}
        aria-invalid={state.error ? true : undefined}
        aria-describedby={describedBy(controlId, state.error)}
        className={cn(mono && "font-mono")}
        onFocus={state.focus}
        onChange={(event) => state.edit(event.target.value)}
        onBlur={(event) => {
          state.blur();
          state.commit(event.target.value);
        }}
        onKeyDown={(event) => {
          if (event.key === "Enter") state.commit(event.currentTarget.value);
        }}
      />
    </FieldShell>
  );
}

function splitModel(stored: string, catalog: ModelCatalog | undefined): { model: string; level: string } {
  const at = stored.lastIndexOf(":");
  if (at > 0 && catalog?.status === "listed") {
    const model = stored.slice(0, at);
    const level = stored.slice(at + 1);
    const entry = catalog.models.find((candidate) => candidate.selector === model);
    if (entry?.thinkingLevels?.includes(level)) return { model, level };
  }
  return { model: stored, level: "" };
}

/** The choice of the model of a role: a combobox of the models omp lists, and the thinking level of the one chosen. Chat opens it for `models.analysis`. */
export function ModelSetting({ settingKey, catalog }: { settingKey: SettingKey; catalog: ModelCatalog | undefined }) {
  const controlId = useId();
  const listed = catalog?.status === "listed" ? catalog.models : [];
  const codec: Codec = {
    display: (stored) => splitModel(stored, catalog).model,
    store: (text, stored) => {
      const model = text.trim();
      const { level } = splitModel(stored, catalog);
      const entry = listed.find((candidate) => candidate.selector === model);
      return model !== "" && level !== "" && entry?.thinkingLevels?.includes(level) ? `${model}:${level}` : model;
    },
  };
  const state = useField(settingKey, codec);
  if (!state.field) return null;

  const options: ComboboxOption[] = [
    { value: "", label: DEFAULT_MODEL_LABEL },
    ...listed.map((entry) => ({
      value: entry.selector,
      label: entry.selector,
      detail: entry.name && entry.name !== entry.selector ? entry.name : undefined,
    })),
  ];
  const { model, level } = splitModel(state.stored, catalog);
  const levels = model === "" ? [] : (listed.find((entry) => entry.selector === model)?.thinkingLevels ?? []);

  return (
    <FieldShell field={state.field} controlId={controlId} error={state.error} issue={state.issue}>
      <div className="flex gap-2">
        <Combobox
          id={controlId}
          value={state.text}
          options={options}
          placeholder={DEFAULT_MODEL_LABEL}
          spellCheck={false}
          aria-invalid={state.error ? true : undefined}
          aria-describedby={describedBy(controlId, state.error)}
          className="font-mono"
          onFocus={state.focus}
          onValueChange={state.edit}
          onBlur={state.blur}
          onCommit={state.commit}
        />
        {levels.length > 0 ? (
          <Select
            value={level === "" ? NO_LEVEL : level}
            onValueChange={(next) => state.submit(next === NO_LEVEL ? model : `${model}:${next}`)}
          >
            <SelectTrigger aria-label={`${state.field.title} thinking level`} className="w-40 shrink-0">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={NO_LEVEL}>Default</SelectItem>
              {levels.map((entry) => (
                <SelectItem key={entry} value={entry}>
                  {entry}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        ) : null}
      </div>
    </FieldShell>
  );
}

function SelectSetting({ settingKey }: { settingKey: SettingKey }) {
  const controlId = useId();
  const state = useField(settingKey);
  if (!state.field) return null;
  return (
    <FieldShell field={state.field} controlId={controlId} error={state.error} issue={state.issue}>
      <Select value={state.stored} onValueChange={state.choose}>
        <SelectTrigger id={controlId} aria-describedby={describedBy(controlId, state.error)}>
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {state.field.choices.map((choice) => (
            <SelectItem key={choice.value} value={choice.value}>
              {choice.label}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </FieldShell>
  );
}

function SegmentedSetting({ settingKey }: { settingKey: SettingKey }) {
  const labelId = useId();
  const state = useField(settingKey);
  if (!state.field) return null;
  return (
    <FieldShell field={state.field} labelId={labelId} error={state.error} issue={state.issue}>
      <ToggleGroup
        type="single"
        value={state.stored}
        aria-labelledby={labelId}
        aria-describedby={describedBy(labelId, state.error)}
        // Choosing the pressed item would unset it; a value is always in force.
        onValueChange={(next) => next && state.choose(next)}
        className="w-fit"
      >
        {state.field.choices.map((choice) => (
          <ToggleGroupItem key={choice.value} value={choice.value}>
            {choice.label}
          </ToggleGroupItem>
        ))}
      </ToggleGroup>
    </FieldShell>
  );
}

function RadioSetting({ settingKey }: { settingKey: SettingKey }) {
  const labelId = useId();
  const state = useField(settingKey);
  if (!state.field) return null;
  return (
    <FieldShell field={state.field} labelId={labelId} error={state.error} issue={state.issue}>
      <RadioGroup
        value={state.stored}
        aria-labelledby={labelId}
        aria-describedby={describedBy(labelId, state.error)}
        onValueChange={state.choose}
      >
        {state.field.choices.map((choice) => (
          <div key={choice.value} className="flex items-center gap-2">
            <RadioGroupItem id={`${labelId}-${choice.value}`} value={choice.value} />
            <Label htmlFor={`${labelId}-${choice.value}`} className="font-normal">
              {choice.label}
            </Label>
          </div>
        ))}
      </RadioGroup>
    </FieldShell>
  );
}

const LINE_ICON = {
  exporting: <Loader aria-hidden className="size-4 animate-spin motion-reduce:animate-none" />,
  ready: <CircleCheck aria-hidden className="size-4 text-sage-ink" />,
  failed: <TriangleAlert aria-hidden className="size-4 text-peach-ink" />,
} satisfies Record<SystemCertificatesLineKind, ReactNode>;

/**
 * `System Certificates` (`requirements/ui#system-certificates`): a Switch saved at once and, only
 * while it is on, the status line of the export with `Refresh`. The line is the description of the
 * switch for assistive technology and a polite live region. One component in every place; the value
 * comes from the settings and the status from the backend, so all places always agree.
 */
export function SystemCertificatesSetting({ place }: { place: Place }) {
  const controlId = useId();
  const state = useField("systemCertificates.enabled");
  const { status, refresh } = useSystemCertificates();
  if (!state.field) return null;

  const enabled = state.stored === "true";
  const line = enabled && status ? systemCertificatesLine(status, Date.now()) : null;
  const statusId = `${controlId}-status`;
  const describedIds = [`${controlId}-description`, line ? statusId : null, state.error ? `${controlId}-error` : null].filter(Boolean).join(" ");

  return (
    <div className="flex flex-col gap-1.5">
      <div className="flex items-center justify-between gap-3">
        <Label htmlFor={controlId}>{state.field.title}</Label>
        <Switch
          id={controlId}
          checked={enabled}
          aria-describedby={describedIds}
          aria-invalid={state.error ? true : undefined}
          onCheckedChange={(next) => state.choose(String(next))}
        />
      </div>
      <p id={`${controlId}-description`} className="text-sm text-muted-foreground">
        {place === "settings" ? state.field.description : outsideSettings(state.field.description)}
      </p>
      <div id={statusId} aria-live="polite" className="empty:hidden">
        {line ? (
          <div className="flex flex-wrap items-start gap-x-2 gap-y-2 text-sm text-muted-foreground">
            <p className="flex min-w-0 flex-1 basis-[12.5rem] items-start gap-1.5">
              <span className="mt-0.5 shrink-0">{LINE_ICON[line.kind]}</span>
              <span className="min-w-0">{line.text}</span>
            </p>
            <GuardedButton
              variant="outline"
              size="sm"
              aria-label="Refresh system certificates"
              reasons={[line.kind === "exporting" && EXPORTING_TEXT]}
              onClick={refresh}
            >
              <RefreshCw aria-hidden />
              Refresh
            </GuardedButton>
          </div>
        ) : null}
      </div>
      {state.error ? (
        <p id={`${controlId}-error`} role="alert" className="flex items-start gap-1.5 text-sm text-rose-ink">
          <TriangleAlert className="mt-0.5 size-4 shrink-0" aria-hidden />
          {state.error}
        </p>
      ) : null}
      {state.issue ? (
        <p className="flex items-start gap-1.5 text-sm text-peach-ink">
          <TriangleAlert className="mt-0.5 size-4 shrink-0" aria-hidden />
          {`The value in the settings file was ignored and the default is in force: ${state.issue.reason}`}
        </p>
      ) : null}
    </div>
  );
}

// ---------------------------------------------------------------------------------------------
// The view.
// ---------------------------------------------------------------------------------------------

/**
 * The Settings view: one centred column of about 720 px, six groups in order. Titles, descriptions,
 * defaults and choices come from the schema the backend sends; setting keys are never shown. A
 * value is saved at once, without confirmation: selects on choice, text and numbers on blur or
 * Enter. The status box of omp and `Providers` go in `ompSlot`, `Update skills` in `projectSlot`.
 */
export function SettingsView({ ompSlot, projectSlot, modelCatalog }: SettingsViewProps) {
  const { snapshot, loadError, reload } = useSettings();

  if (!snapshot) {
    return (
      <SettingsPage className="gap-4">
        {loadError ? (
          <Alert variant="error">
            <TriangleAlert aria-hidden />
            <p className="flex-1">{`The settings could not be loaded: ${loadError}`}</p>
            <Button size="sm" variant="outline" onClick={reload}>
              Retry
            </Button>
          </Alert>
        ) : (
          <p role="status" className="text-sm text-muted-foreground">
            Loading the settings…
          </p>
        )}
      </SettingsPage>
    );
  }

  const preset = snapshot.settings.editor.preset;

  return (
    <SettingsPage className="gap-8">
      {snapshot.fileError ? (
        <Alert variant="warning">
          <TriangleAlert aria-hidden />
          <p>
            {`The settings file could not be used: ${snapshot.fileError.replace(/\.$/, "")}. Every default is in force. The file is kept under another name when a setting changes.`}
          </p>
        </Alert>
      ) : null}

      <Group title="omp">
        {ompSlot}
        <TextSetting settingKey="ompPath" mono />
        <RadioSetting settingKey="permissions" />
        <SystemCertificatesSetting place="settings" />
      </Group>

      <Group title="Models">
        {modelCatalog?.status === "unavailable" ? (
          <p className="text-sm text-muted-foreground">{modelCatalog.reason}</p>
        ) : null}
        <ModelSetting settingKey="models.analysis" catalog={modelCatalog} />
        <ModelSetting settingKey="models.scan" catalog={modelCatalog} />
        <ModelSetting settingKey="models.execution" catalog={modelCatalog} />
      </Group>

      <Group title="Agents">
        <TextSetting settingKey="maxParallelAgents" inputMode="numeric" />
        <TextSetting settingKey="language" placeholder="English" />
      </Group>

      <Group title="Editor">
        <SelectSetting settingKey="editor.preset" />
        {preset === "custom" ? <TextSetting settingKey="editor.command" mono placeholder="code --goto {file}:{line}" /> : null}
      </Group>

      <Group title="Appearance">
        <SegmentedSetting settingKey="theme" />
      </Group>

      {projectSlot ? <Group title="Project">{projectSlot}</Group> : null}
    </SettingsPage>
  );
}
