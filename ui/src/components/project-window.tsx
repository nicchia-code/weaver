import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { toast } from "sonner";

import { LeaveGuardContext, type LeaveGuard, type LeaveGuardSlot } from "@/components/files/leave-guard";
import { ProjectContext, type ProjectContextValue } from "@/components/project-context";
import { Sidebar, type SidebarAction } from "@/components/sidebar";
import { StartupBanner } from "@/components/startup-banner";
import { SystemCertificatesBanners } from "@/components/system-certificates-banners";
import { UpdateBanner } from "@/components/update-banner";
import { UsageDialog } from "@/components/usage-dialog";
import { ViewShownContext } from "@/components/view-shown";
import { ViewHeader, viewTitleId } from "@/components/view-header";
import { on } from "@/lib/backend";
import { NO_COUNTS, USAGE_ENTRY, VIEWS, type ShellCounts, type ViewId } from "@/views";

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
}

/**
 * The shell of a project window: the sidebar and the selected view. Every view that was opened
 * stays mounted while another is shown, so switching entries keeps the state of each view (level,
 * focus, overlay, scroll). When the window comes forward because of a notification
 * (`show-requested`), it shows what the notification was about.
 */
export function ProjectWindow({ projectId, views = {}, counts = NO_COUNTS, initialView = "board", around = (children) => children, banners }: ProjectWindowProps) {
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
  const openJob = useCallback(
    (jobId: string) => {
      setOpenedJob(jobId);
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

  const context = useMemo<ProjectContextValue>(
    () => ({ projectId, showView: select, openJob, openedJob }),
    [projectId, select, openJob, openedJob],
  );

  // The only sidebar entry of the bottom group that is no view, besides Settings: `Usage` opens the
  // dialog over the view shown. It is never selected.
  const [usageOpen, setUsageOpen] = useState(false);
  const actions = useMemo<readonly SidebarAction[]>(
    () => [{ ...USAGE_ENTRY, onActivate: () => setUsageOpen(true) }],
    [],
  );
  const openSettingsFromUsage = useCallback(() => {
    setUsageOpen(false);
    select("settings");
  }, [select]);
  // The focus goes back to the entry that opened the dialog, also where a click does not focus a button.
  const focusUsageEntry = useCallback((event: Event) => {
    event.preventDefault();
    document.querySelector<HTMLElement>(`[data-sidebar-action="${USAGE_ENTRY.id}"]`)?.focus();
  }, []);

  return (
    <ProjectContext.Provider value={context}>
      <LeaveGuardContext.Provider value={guardSlot}>
        {around(
          <div className="flex h-full min-w-0">
            <Sidebar selected={selected} onSelect={select} counts={counts} actions={actions} />
            <div className="flex min-w-0 flex-1 flex-col">
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
            </div>
          </div>,
        )}
        <UsageDialog open={usageOpen} onClose={() => setUsageOpen(false)} onOpenSettings={openSettingsFromUsage} onCloseAutoFocus={focusUsageEntry} />
      </LeaveGuardContext.Provider>
    </ProjectContext.Provider>
  );
}
