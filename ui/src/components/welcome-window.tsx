import { FolderOpen, TriangleAlert, X } from "lucide-react";
import { useCallback, useEffect, useState } from "react";

import type { MenuOpenFailed } from "@/bindings/MenuOpenFailed";
import type { RecentEntry } from "@/bindings/RecentEntry";
import type { WelcomeState } from "@/bindings/WelcomeState";
import { Logo } from "@/components/logo";
import { OmpSetup } from "@/components/omp-status";
import { StartupBanner } from "@/components/startup-banner";
import { SystemCertificatesBanners } from "@/components/system-certificates-banners";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { UpdateBanner } from "@/components/update-banner";
import { call, isAppError, on } from "@/lib/backend";

function failureText(action: string, error: unknown): string {
  return `${action}: ${isAppError(error) ? error.message : "unknown error"}`;
}

/** The text of an opening from the File menu that failed: the same ones the window's own actions use. */
function menuFailureText({ name, reason }: MenuOpenFailed): string {
  return name === null ? `The folder could not be opened: ${reason}` : `${name} could not open: ${reason}`;
}

function RecentProject({
  entry,
  onOpen,
  onRemove,
}: {
  entry: RecentEntry;
  onOpen: (entry: RecentEntry) => void;
  onRemove: (entry: RecentEntry) => void;
}) {
  const label = (
    <>
      <span className="block truncate font-medium">{entry.name}</span>
      <span className="block truncate text-xs text-muted-foreground">{entry.path}</span>
    </>
  );
  return (
    <li className="flex items-center gap-2 py-1">
      {entry.exists ? (
        <button
          type="button"
          onClick={() => onOpen(entry)}
          title={entry.path}
          className="min-w-0 flex-1 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent hover:text-accent-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        >
          {label}
        </button>
      ) : (
        <>
          <div className="min-w-0 flex-1 px-2 py-1.5 text-sm">{label}</div>
          <span className="shrink-0 text-xs text-muted-foreground">Not found</span>
          <Button variant="ghost" size="icon" className="size-8" aria-label={`Remove ${entry.name} from the recent projects`} onClick={() => onRemove(entry)}>
            <X aria-hidden />
          </Button>
        </>
      )}
    </li>
  );
}

/**
 * The welcome window, 640 × 480 and not resizable: the name and version of the app, the recent
 * projects (most recent first; a folder that is gone is marked `Not found` and can be removed),
 * `Open Folder` with the system's folder picker and the status of omp. It closes by itself once a
 * project window is open (the backend does it).
 */
export function WelcomeWindow() {
  const [state, setState] = useState<WelcomeState | null>(null);
  const [failure, setFailure] = useState<string | null>(null);

  const load = useCallback(() => {
    call("welcome_state")
      .then((loaded) => {
        setState(loaded);
        // An opening from the File menu that failed while no window had the focus.
        if (loaded.failure) setFailure(menuFailureText(loaded.failure));
      })
      .catch((error: unknown) => setFailure(failureText("The recent projects could not be read", error)));
  }, []);

  // Fresh when the window loads and whenever it comes forward again (a project may have closed).
  useEffect(() => {
    load();
    window.addEventListener("focus", load);
    return () => window.removeEventListener("focus", load);
  }, [load]);

  // The list follows every change at once, also those made from the File menu (`Clear Recent`, an
  // opening from `Open Recent`), and an opening from the File menu that failed says why.
  useEffect(() => {
    let current = true;
    const stops = [
      on("recent-changed", (recent) => {
        if (current) setState((previous) => (previous ? { ...previous, recent } : previous));
      }),
      on("menu-open-failed", (failed) => {
        if (current) setFailure(menuFailureText(failed));
      }),
    ];
    return () => {
      current = false;
      for (const stop of stops) stop.then((unlisten) => unlisten()).catch(() => {});
    };
  }, []);

  const open = (entry: RecentEntry) => {
    setFailure(null);
    call("open_project", { path: entry.path }).catch((error: unknown) => {
      setFailure(failureText(`${entry.name} could not open`, error));
      load();
    });
  };
  const openFolder = () => {
    setFailure(null);
    call("open_folder").catch((error: unknown) => setFailure(failureText("The folder could not be opened", error)));
  };
  const remove = (entry: RecentEntry) => {
    setFailure(null);
    call("remove_recent", { path: entry.path })
      .then(setState)
      .catch((error: unknown) => setFailure(failureText(`${entry.name} could not be removed`, error)));
  };

  return (
    // The frame of a project window: Paper ground and a Surface panel inset 8 px that holds the banners and the content.
    <div className="h-full bg-paper p-2">
      <div className="flex h-full flex-col overflow-hidden rounded-xl bg-surface shadow-ring">
        <StartupBanner allProjects />
        <UpdateBanner />
        <SystemCertificatesBanners />
        <main className="flex min-h-0 flex-1 flex-col gap-4 overflow-auto p-6">
          <header className="flex items-end gap-3">
            <div className="flex min-w-0 flex-1 flex-col gap-3">
              <Logo size={48} />
              <div className="min-w-0">
                <h1 className="text-xl font-semibold tracking-title">{state?.name ?? "Weaver"}</h1>
                {state ? <p className="text-sm text-muted-foreground">{`Version ${state.version}`}</p> : null}
              </div>
            </div>
            <Button onClick={openFolder}>
              <FolderOpen aria-hidden />
              Open Folder
            </Button>
          </header>

          {failure ? (
            <Alert variant="error">
              <TriangleAlert aria-hidden />
              <p className="min-w-0 flex-1">{failure}</p>
            </Alert>
          ) : null}

          <section aria-labelledby="recent-projects-title" className="flex min-h-0 flex-col gap-1">
            <h2 id="recent-projects-title" className="text-sm font-semibold">
              Recent projects
            </h2>
            {state && state.recent.length === 0 ? (
              <p className="text-sm text-muted-foreground">No recent projects. Open a folder to start.</p>
            ) : null}
            <ul className="flex flex-col">
              {state?.recent.map((entry) => (
                <RecentProject key={entry.path} entry={entry} onOpen={open} onRemove={remove} />
              ))}
            </ul>
          </section>

          <OmpSetup place="elsewhere" providers="when-needed" />
        </main>
      </div>
    </div>
  );
}
