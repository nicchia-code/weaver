import { createContext, useContext, useEffect, useRef, useState, type ReactNode } from "react";

import type { ProjectGit } from "@/bindings/ProjectGit";
import { call, on } from "@/lib/backend";

const ProjectGitContext = createContext<ProjectGit | null | undefined>(undefined);

/**
 * The branch git is on and git's user name for the project of this window, and the project folder's
 * name. The UI keeps no copy of its own: the whole value arrives from `project_git` and from
 * `project-git-changed`, which the backend sends when the window opens, when it gets the focus and
 * when git changes branch, also outside Weaver; the value with the highest revision wins.
 */
export function ProjectGitProvider({ projectId, children }: { projectId: string; children: ReactNode }) {
  const [value, setValue] = useState<ProjectGit | null>(null);
  const revision = useRef(-1);

  useEffect(() => {
    let alive = true;
    revision.current = -1;
    const accept = (incoming: ProjectGit) => {
      if (!alive || incoming.projectId !== projectId || incoming.revision < revision.current) return;
      revision.current = incoming.revision;
      setValue(incoming);
    };
    // Listen first, then ask: a change between the two is not lost.
    const unlisten = on("project-git-changed", accept);
    call("project_git")
      .then(accept)
      .catch(() => {});
    return () => {
      alive = false;
      unlisten.then((stop) => stop()).catch(() => {});
    };
  }, [projectId]);

  return <ProjectGitContext.Provider value={value}>{children}</ProjectGitContext.Provider>;
}

/** What the backend last said of the project's git; `null` until it did. */
export function useProjectGit(): ProjectGit | null {
  const value = useContext(ProjectGitContext);
  if (value === undefined) throw new Error("useProjectGit needs a ProjectGitProvider");
  return value;
}
