import { EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { Lock, TriangleAlert, X } from "lucide-react";
import { useEffect, useMemo, useRef, useState, type KeyboardEvent, type ReactNode } from "react";

import { FileTabDialogs } from "@/components/files/file-tab-dialogs";
import { useFileTabs } from "@/components/files/file-tabs-context";
import { ImageView } from "@/components/files/image-view";
import { fileName, formatWeight, isSvgTab, needsAttention, svgDataUrl, utf8Length, type TabImage, type TabModel } from "@/components/files/tab-model";
import { OpenInEditor } from "@/components/open-in-editor";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { cn } from "@/lib/utils";

const PANEL_ID = "file-tab-panel";

function tabDomId(id: string): string {
  return `file-tab-${id}`;
}

/** One tab: its name (the path is the tooltip), what is pending in it, and `Close`. */
function TabItem({ tab, active }: { tab: TabModel; active: boolean }) {
  const { showTabById, requestClose } = useFileTabs();
  return (
    <div className={cn("group relative flex shrink-0 items-center border-r", active ? "bg-background" : "bg-muted/40")}>
      <Tooltip>
        <TooltipTrigger asChild>
          <button
            type="button"
            role="tab"
            id={tabDomId(tab.id)}
            aria-selected={active}
            aria-controls={PANEL_ID}
            tabIndex={active ? 0 : -1}
            className={cn(
              "flex h-9 items-center gap-2 pl-3 pr-1 text-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring",
              active ? "font-medium text-foreground" : "text-muted-foreground hover:text-foreground",
            )}
            onClick={() => showTabById(tab.id)}
          >
            <span className="max-w-48 truncate">{fileName(tab.path)}</span>
            {tab.modified && <span role="img" aria-label="Unsaved changes" className="size-2 shrink-0 rounded-full bg-foreground" />}
            {tab.locked && <Lock role="img" aria-label="The agent is editing this file" className="size-3.5 shrink-0" />}
            {needsAttention(tab) && (
              <TriangleAlert role="img" aria-label="Needs your attention" className="size-3.5 shrink-0 text-peach-ink" />
            )}
          </button>
        </TooltipTrigger>
        <TooltipContent className="font-mono">{tab.path}</TooltipContent>
      </Tooltip>
      <Button variant="ghost" size="icon" aria-label="Close" className="size-6" tabIndex={active ? 0 : -1} onClick={() => requestClose(tab.id)}>
        <X aria-hidden />
      </Button>
      {active && <span aria-hidden className="absolute inset-x-0 bottom-0 h-0.5 bg-primary" />}
    </div>
  );
}

/** The tab bar: Left and Right move between the tabs (and Home, End), as an ARIA `tablist` does. */
function TabBar() {
  const { tabs, activeId, showTabById } = useFileTabs();
  const bar = useRef<HTMLDivElement>(null);
  const move = (event: KeyboardEvent) => {
    if (event.target instanceof HTMLElement && event.target.getAttribute("role") !== "tab") return;
    const at = tabs.findIndex((tab) => tab.id === activeId);
    const to =
      event.key === "ArrowRight" ? (at + 1) % tabs.length : event.key === "ArrowLeft" ? (at - 1 + tabs.length) % tabs.length : event.key === "Home" ? 0 : event.key === "End" ? tabs.length - 1 : -1;
    const target = tabs[to];
    if (!target) return;
    event.preventDefault();
    showTabById(target.id);
    // The tab shown takes the focus, where the keys are.
    bar.current?.querySelector<HTMLElement>(`#${tabDomId(target.id)}`)?.focus();
  };
  return (
    <div ref={bar} role="tablist" aria-label="Open files" aria-orientation="horizontal" className="flex shrink-0 overflow-x-auto border-b bg-muted/40" onKeyDown={move}>
      {tabs.map((tab) => (
        <TabItem key={tab.id} tab={tab} active={tab.id === activeId} />
      ))}
    </div>
  );
}

/** A line above the text that says why it cannot be changed. */
function InfoLine({ children }: { children: ReactNode }) {
  return <p className="flex shrink-0 items-center gap-2 border-b px-3 py-1.5 text-xs text-muted-foreground">{children}</p>;
}

/** A bar of a tab that waits for the developer, at the top of the editor. */
function Bar({ children, actions, tone = "warning" }: { children: ReactNode; actions?: ReactNode; tone?: "warning" | "error" }) {
  return (
    <Alert variant={tone} className="shrink-0 rounded-none border-x-0 border-t-0 py-2">
      <TriangleAlert aria-hidden />
      <div className="flex min-w-0 flex-1 flex-wrap items-center gap-x-3 gap-y-2">
        <span className="min-w-0">{children}</span>
        {actions && <div className="ml-auto flex flex-wrap gap-2">{actions}</div>}
      </div>
    </Alert>
  );
}

/** The bars and lines of a tab, as long as they are not resolved. */
function TabNotes({ tab }: { tab: TabModel }) {
  const { reload, overwrite, save, keepUnsaved, dismissSaveError } = useFileTabs();
  const editable = tab.kind === "text" && tab.readOnly === null;
  return (
    <>
      {tab.readOnly === "tooLarge" && <InfoLine>This file is larger than 2 MB: read-only, without colours.</InfoLine>}
      {tab.readOnly === "weaver" && <InfoLine>Files in .weaver are written by Weaver: read-only here.</InfoLine>}
      {tab.changedOnDisk && <Bar>This file changed on disk. Saving will ask what to keep.</Bar>}
      {tab.unlocked && (
        <Bar
          actions={
            <>
              <Button size="sm" variant="destructive" onClick={() => reload(tab.id)}>
                Take the agent&apos;s version
              </Button>
              <Button size="sm" variant="outline" onClick={() => overwrite(tab.id)}>
                Keep mine
              </Button>
              <Button size="sm" variant="outline" onClick={() => keepUnsaved(tab.id)}>
                Cancel
              </Button>
            </>
          }
        >
          The agent changed this file while it had unsaved changes.
        </Bar>
      )}
      {tab.deleted && !tab.locked && (
        <Bar
          actions={
            editable ? (
              <Button size="sm" variant="outline" onClick={() => save(tab.id)}>
                Save again
              </Button>
            ) : undefined
          }
        >
          {tab.deleted === "agent" ? "The agent deleted this file." : "This file was deleted on disk."}
        </Bar>
      )}
      {tab.saveError !== null && (
        <Bar
          tone="error"
          actions={
            <Button size="sm" variant="outline" onClick={() => dismissSaveError(tab.id)}>
              Dismiss
            </Button>
          }
        >
          Could not save {fileName(tab.path)}: {tab.saveError.replace(/\.$/, "")}.
        </Bar>
      )}
    </>
  );
}

/** The editor: one CodeMirror view that shows the state of the active tab. */
function EditorHost({ hidden, language }: { hidden: boolean; language: string | null }) {
  const { attachView } = useFileTabs();
  const host = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!host.current) return;
    const view = new EditorView({ parent: host.current, state: EditorState.create({ doc: "" }) });
    attachView(view);
    return () => {
      attachView(null);
      view.destroy();
    };
  }, [attachView]);
  return <div ref={host} data-language={language ?? undefined} hidden={hidden} className="min-h-0 flex-1 overflow-hidden" />;
}

/** The panel of a binary file, or of an image that cannot be shown: no text, and why there is no picture. */
function BinaryPanel({ tab }: { tab: TabModel }) {
  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-3 p-6 text-center">
      <p className="text-sm font-medium">Binary file</p>
      {tab.imageTooLarge && <p className="text-sm text-muted-foreground">This image is larger than 10 MB: no preview.</p>}
      {tab.kind === "image" && tab.undrawable && <p className="text-sm text-muted-foreground">This image cannot be shown.</p>}
      <OpenInEditor file={tab.path} variant="outline" size="default" />
    </div>
  );
}

/** The image of an image tab. */
function ImagePanel({ tab, image }: { tab: TabModel; image: TabImage }) {
  const { toggleActualSize, imageDrawn, imageFailed } = useFileTabs();
  return (
    <ImageView
      src={image.dataUrl}
      name={fileName(tab.path)}
      actualSize={tab.actualSize}
      onToggle={() => toggleActualSize(tab.id)}
      onLoad={(width, height) => imageDrawn(tab.id, { width, height })}
      onError={() => imageFailed(tab.id)}
    />
  );
}

/**
 * The drawing of an SVG tab's text: an `<img>` on a `data:` URL, so that no script runs and nothing
 * external loads. It is built when the drawing is shown, and again when the text was replaced by a
 * read from the disk; typing while the text is shown builds nothing.
 */
function SvgPanel({ tab }: { tab: TabModel }) {
  const { textOf, toggleActualSize, imageDrawn } = useFileTabs();
  // `textRev` is what says that the text was replaced by a read; typing does not change it.
  const url = useMemo(() => svgDataUrl(textOf(tab.id)), [textOf, tab.id, tab.textRev]);
  const [failed, setFailed] = useState<string | null>(null);
  useEffect(() => {
    // A new drawing has no size until it loads.
    imageDrawn(tab.id, null);
  }, [imageDrawn, tab.id, url]);
  if (failed === url) {
    return <p className="flex flex-1 items-center justify-center p-6 text-sm text-muted-foreground">This SVG cannot be drawn.</p>;
  }
  return (
    <ImageView
      src={url}
      name={fileName(tab.path)}
      actualSize={tab.actualSize}
      onToggle={() => toggleActualSize(tab.id)}
      onLoad={(width, height) => imageDrawn(tab.id, { width, height })}
      onError={() => {
        setFailed(url);
        imageDrawn(tab.id, null);
      }}
    />
  );
}

/** `Code` / `Preview` of an SVG tab. */
function SvgSwitch({ tab }: { tab: TabModel }) {
  const { setSvgView } = useFileTabs();
  return (
    <ToggleGroup
      type="single"
      value={tab.svgView}
      aria-label="SVG view"
      onValueChange={(value) => {
        if (value === "code" || value === "preview") setSvgView(tab.id, value);
      }}
    >
      <ToggleGroupItem value="code" className="h-6 px-2 text-xs">
        Code
      </ToggleGroupItem>
      <ToggleGroupItem value="preview" className="h-6 px-2 text-xs">
        Preview
      </ToggleGroupItem>
    </ToggleGroup>
  );
}

/**
 * The right side of the File view: the tab bar, the toolbar, the bars of the active tab and its
 * editor; without tabs, a line that says where to start.
 */
export function FileTabs() {
  const { tabs, activeId, cursorLine, textOf } = useFileTabs();
  const tab = tabs.find((candidate) => candidate.id === activeId) ?? null;
  if (!tab) {
    return (
      <>
        <p className="p-6 text-sm text-muted-foreground">Open a file from the tree.</p>
        <FileTabDialogs />
      </>
    );
  }
  // A binary file, or an image that cannot be shown, has the panel and no toolbar of its own.
  const image = tab.kind === "image" && !tab.undrawable ? tab.image : null;
  const panel = tab.kind === "binary" || (tab.kind === "image" && image === null);
  const svg = isSvgTab(tab);
  const drawing = svg && tab.svgView === "preview";
  // The weight of what is drawn: the image's file, or the text of the SVG as UTF-8.
  const weight = image ? image.size : drawing ? utf8Length(textOf(tab.id)) : null;
  return (
    <div className="flex h-full min-h-0 flex-col">
      <TabBar />
      <div className="flex shrink-0 items-center gap-2 border-b px-3 py-1">
        <span className="min-w-0 truncate font-mono text-xs">{tab.path}</span>
        {(image || drawing) && tab.pixels && (
          <span className="shrink-0 text-xs text-muted-foreground">
            {tab.pixels.width} × {tab.pixels.height} px
          </span>
        )}
        {weight !== null && <span className="shrink-0 text-xs text-muted-foreground">{formatWeight(weight)}</span>}
        <span className="flex-1" />
        {svg && <SvgSwitch tab={tab} />}
        {!panel && <OpenInEditor file={tab.path} line={image ? null : () => cursorLine(tab.id)} />}
      </div>
      <div role="tabpanel" id={PANEL_ID} aria-labelledby={tabDomId(tab.id)} className="flex min-h-0 flex-1 flex-col">
        {/* The region is there before the line, so that its coming and going are announced. */}
        <div aria-live="polite" className="shrink-0">
          {tab.locked && (
            <InfoLine>
              <Lock aria-hidden className="size-3.5" />
              The agent is editing this file.
            </InfoLine>
          )}
        </div>
        <TabNotes tab={tab} />
        {panel && <BinaryPanel tab={tab} />}
        {image && <ImagePanel tab={tab} image={image} />}
        {drawing && <SvgPanel tab={tab} />}
        <EditorHost hidden={tab.kind !== "text" || drawing} language={tab.language} />
      </div>
      <FileTabDialogs />
    </div>
  );
}
