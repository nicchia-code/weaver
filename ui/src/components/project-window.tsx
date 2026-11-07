import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { toast } from "sonner";

import { LeaveGuardContext, type LeaveGuard, type LeaveGuardSlot } from "@/components/files/leave-guard";
import { isMacPlatform } from "@/components/assets/asset-kinds";
import { ProjectContext, type ProjectContextValue, type Reveal, type RevealTarget, type WindowControls, WindowControlsContext } from "@/components/project-context";
import { Sidebar } from "@/components/sidebar";
import { StartupBanner } from "@/components/startup-banner";
import { SystemCertificatesBanners } from "@/components/system-certificates-banners";
import { UpdateBanner } from "@/components/update-banner";
import { UsageDialog } from "@/components/usage-dialog";
import { ViewShownContext } from "@/components/view-shown";
import { ViewHeader, viewTitleId } from "@/components/view-header";
import { ProjectGitProvider } from "@/hooks/use-project-git";
import { UsageProvider } from "@/hooks/use-usage";
import { on } from "@/lib/backend";
import { NO_COUNTS, VIEWS, type ShellCounts, type ViewId } from "@/views";

export interface ProjectWindowProps {
  /** The project hash (the window's label is `p-<hash>`). */
  projectId: string;
  /** The content of each view; a view without one shows its header only. */
  views?: Partial<Record<ViewId, ReactNode>>;
  counts?: ShellCounts;
  initialView?: ViewId;
  /** Wraps everything the window shows, inside the project context (what the views of the window share). */
  around?: (children: ReactNode) => ReactNode;
  /** Shown under the startup and update banners, at the top of the window (what the check of the agent files says). */
  banners?: ReactNode;
  /** Shown at the bottom of the work surface, below the selected view, which gets shorter by its height (the bar of a minimized question round). */
  footer?: ReactNode;
}

/** What the sidebar of a window and its dialogs read: the usage limits of the app and the git state of this project. */
function WindowProviders({ projectId, children }: { projectId: string; children: ReactNode }) {
  return (
    <UsageProvider>
      <ProjectGitProvider projectId={projectId}>{children}</ProjectGitProvider>
    </UsageProvider>
  );
}

/**
 * The shell of a project window: the sidebar and the selected view. Every view that was opened
 * stays mounted while another is shown, so switching entries keeps the state of each view (level,
 * focus, overlay, scroll). When the window comes forward because of a notification
 * (`show-requested`), it shows what the notification was about.
 */
export function ProjectWindow({ projectId, views = {}, counts = NO_COUNTS, initialView = "board", around = (children) => children, banners, footer }: ProjectWindowProps) {
  const [selected, setSelected] = useState<ViewId>(initialView);
  const [opened, setOpened] = useState<ReadonlySet<ViewId>>(() => new Set([initialView]));
  const current = useRef<ViewId>(initialView);
  // The File view asks to be consulted before another view takes its place (unsaved changes).
  const leaveGuard = useRef<LeaveGuard | null>(null);
  const guardSlot = useMemo<LeaveGuardSlot>(() => ({ set: (guard) => (leaveGuard.current = guard) }), []);

  const show = useCallback((id: ViewId) => {
    current.current = id;
    setOpened((previous) => (previous.has(id) ? previous : new Set(previous).add(id)));
    setSelected(id);
  }, []);

  const select = useCallback(
    (id: ViewId) => {
      const guard = leaveGuard.current;
      if (current.current === "files" && id !== "files" && guard) guard(() => show(id));
      else show(id);
    },
    [show],
  );

  const [openedJob, setOpenedJob] = useState<string | null>(null);
  const [openedJobRequest, setOpenedJobRequest] = useState(0);
  const openJob = useCallback(
    (jobId: string) => {
      setOpenedJob(jobId);
      setOpenedJobRequest((count) => count + 1);
      select("jobs");
    },
    [select],
  );

  useEffect(() => {
    let current = true;
    const unlisten = on("show-requested", (request) => {
      if (!current || request.projectId !== projectId) return;
      if (request.jobId) openJob(request.jobId);
      else select(request.view);
    });
    return () => {
      current = false;
      unlisten.then((stop) => stop()).catch(() => {});
    };
  }, [projectId, select, openJob]);

  // This window could not go to the background after `Keep working in background` of the close
  // confirmation: it stays as it was and says why.
  useEffect(() => {
    let current = true;
    const unlisten = on("window-hide-failed", ({ reason }) => {
      if (current) toast.error("Could not hide the window.", { description: reason });
    });
    return () => {
      current = false;
      unlisten.then((stop) => stop()).catch(() => {});
    };
  }, []);

  // An opening chosen from the File menu of this window failed: it says why, in an error toast.
  useEffect(() => {
    let current = true;
    const unlisten = on("menu-open-failed", ({ name, reason }) => {
      if (current) toast.error(name === null ? "Could not open the folder." : `Could not open ${name}.`, { description: reason });
    });
    return () => {
      current = false;
      unlisten.then((stop) => stop()).catch(() => {});
    };
  }, []);

  // `Usage` is no view: its dialog opens over the view shown, from the summary in the sidebar or
  // from the palette. The focus goes back to the element that opened it, also where a click does
  // not focus a button.
  const [usageOpen, setUsageOpen] = useState(false);
  const usageOrigin = useRef<HTMLElement | null>(null);
  const openUsage = useCallback((origin: HTMLElement | null) => {
    usageOrigin.current = origin;
    setUsageOpen(true);
  }, []);
  const openSettingsFromUsage = useCallback(() => {
    setUsageOpen(false);
    select("settings");
  }, [select]);
  const focusUsageOrigin = useCallback((event: Event) => {
    event.preventDefault();
    const origin = usageOrigin.current;
    (origin?.isConnected ? origin : document.querySelector<HTMLElement>("[data-usage-summary]"))?.focus();
  }, []);

  // The palette: `Cmd+K` (`Ctrl+K` elsewhere) opens it from anywhere in the window, in the capture
  // phase so that no editor or field takes the key first, and closes it when it is open.
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [paletteOrigin, setPaletteOrigin] = useState<HTMLElement | null>(null);
  const paletteIsOpen = useRef(false);
  paletteIsOpen.current = paletteOpen;
  const showPalette = useCallback((origin: HTMLElement | null) => {
    setPaletteOrigin(origin);
    setPaletteOpen(true);
  }, []);
  const closePalette = useCallback(() => setPaletteOpen(false), []);
  useEffect(() => {
    const mac = isMacPlatform();
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key.toLowerCase() !== "k" || event.altKey || event.shiftKey) return;
      if (mac ? !event.metaKey || event.ctrlKey : !event.ctrlKey || event.metaKey) return;
      event.preventDefault();
      event.stopPropagation();
      if (paletteIsOpen.current) setPaletteOpen(false);
      else showPalette(document.activeElement instanceof HTMLElement ? document.activeElement : null);
    };
    document.addEventListener("keydown", onKeyDown, true);
    return () => document.removeEventListener("keydown", onKeyDown, true);
  }, [showPalette]);
  const palette = useMemo(() => ({ open: paletteOpen, origin: paletteOrigin, show: showPalette, close: closePalette }), [paletteOpen, paletteOrigin, showPalette, closePalette]);

  // What the palette asks of a view: the window shows the view, which answers once.
  const [revealed, setRevealed] = useState<Reveal | null>(null);
  const nonce = useRef(0);
  const reveal = useCallback(
    (target: RevealTarget) => {
      nonce.current += 1;
      setRevealed({ target, nonce: nonce.current });
      select(target.kind === "section" ? "requirements" : target.kind === "history" ? "history" : "board");
    },
    [select],
  );
  const revealHandled = useCallback((handled: number) => setRevealed((current) => (current?.nonce === handled ? null : current)), []);

  const context = useMemo<ProjectContextValue>(
    () => ({ projectId, showView: select, openJob, openedJob, openedJobRequest }),
    [projectId, select, openJob, openedJob, openedJobRequest],
  );
  const controls = useMemo<WindowControls>(() => ({ openUsage, palette, reveal, revealed, revealHandled }), [openUsage, palette, reveal, revealed, revealHandled]);

  return (
    <ProjectContext.Provider value={context}>
      <WindowControlsContext.Provider value={controls}>
      <LeaveGuardContext.Provider value={guardSlot}>
        <WindowProviders projectId={projectId}>
          {around(
            <div className="flex h-full min-w-0 bg-paper">
              <Sidebar selected={selected} onSelect={select} counts={counts} onOpenUsage={openUsage} onOpenPalette={showPalette} />
              {/* The work surface: a Surface panel inset 8 px from the window's edges and from the sidebar; the banners of the window sit at its top, above the view's header. */}
              <div data-work-surface className="m-2 flex min-w-0 flex-1 flex-col overflow-hidden rounded-xl bg-surface shadow-ring">
                <StartupBanner projectId={projectId} />
                <UpdateBanner />
                <SystemCertificatesBanners onOpenSettings={() => select("settings")} />
                {banners}
                <main className="relative min-h-0 flex-1">
                  {/* The File view is not kept: choosing another entry closes it. */}
                  {VIEWS.filter((view) => opened.has(view.id) && (view.id !== "files" || view.id === selected)).map((view) => (
                    <section
                      key={view.id}
                      aria-labelledby={viewTitleId(view.id)}
                      hidden={view.id !== selected}
                      className="h-full overflow-auto"
                    >
                      <ViewShownContext.Provider value={view.id === selected}>
                        {views[view.id] ?? <ViewHeader view={view.id} />}
                      </ViewShownContext.Provider>
                    </section>
                  ))}
                </main>
                {footer}
              </div>
            </div>,
          )}
          <UsageDialog open={usageOpen} onClose={() => setUsageOpen(false)} onOpenSettings={openSettingsFromUsage} onCloseAutoFocus={focusUsageOrigin} />
        </WindowProviders>
      </LeaveGuardContext.Provider>
      </WindowControlsContext.Provider>
    </ProjectContext.Provider>
  );
}
