import { Loader, TriangleAlert } from "lucide-react";
import { useEffect, useState } from "react";

import type { MergeBranches } from "@/bindings/MergeBranches";
import { FormDialog } from "@/components/dialogs";
import { useMerge } from "@/components/merge/use-merge";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Command, CommandEmpty, CommandGroup, CommandInput, CommandItem, CommandList } from "@/components/ui/command";
import { isAppError } from "@/lib/backend";

type Listing = { state: "loading" } | { state: "failed"; reason: string } | { state: "ready"; branches: MergeBranches };

/** The value of an item: the group and the branch, so a local and a remote branch never clash. */
const itemValue = (group: "local" | "remote", branch: string) => `${group}:${branch}`;

function matching(branches: readonly string[], query: string): string[] {
  const needle = query.trim().toLowerCase();
  return branches.filter((branch) => branch.toLowerCase().includes(needle));
}

/**
 * `Merge` (`requirements/ui#solo-merge`, **Dialog**): `Search branches` over the local branches and
 * then the remote ones git already knows, then the confirmation naming both branches. While git
 * merges both buttons wait; the dialog closes when git answers, whatever the answer.
 */
export function MergeDialog({ open, onOpenChange }: { open: boolean; onOpenChange: (open: boolean) => void }) {
  const merge = useMerge();
  const [listing, setListing] = useState<Listing>({ state: "loading" });
  const [query, setQuery] = useState("");
  const [chosen, setChosen] = useState<string | null>(null);
  const [running, setRunning] = useState(false);
  const branchesOf = merge?.branches;

  // The branches are asked each time the dialog opens: git may have new ones.
  useEffect(() => {
    if (!open || !branchesOf) return;
    let current = true;
    setListing({ state: "loading" });
    setQuery("");
    setChosen(null);
    setRunning(false);
    branchesOf()
      .then((branches) => current && setListing({ state: "ready", branches }))
      .catch((error: unknown) => current && setListing({ state: "failed", reason: isAppError(error) ? error.message : "The branches could not be read." }));
    return () => {
      current = false;
    };
  }, [open, branchesOf]);

  if (!merge) return null;
  const ready = listing.state === "ready" ? listing.branches : null;
  const current = ready?.current ?? merge.merge?.current ?? "the current branch";

  const choose = (value: string) => {
    const [, ...rest] = value.split(":");
    setChosen(rest.join(":"));
  };

  const confirm = async () => {
    if (!chosen || running) return;
    setRunning(true);
    try {
      await merge.start(chosen);
    } finally {
      setRunning(false);
      onOpenChange(false);
    }
  };

  const footer = (
    <>
      <Button variant="outline" disabled={running} onClick={() => onOpenChange(false)}>
        Cancel
      </Button>
      {chosen ? (
        <Button disabled={running} onClick={confirm}>
          {running ? <Loader className="animate-spin motion-reduce:animate-none" aria-hidden /> : null}
          Merge
        </Button>
      ) : null}
    </>
  );

  return (
    <FormDialog
      open={open}
      // The dialog stays while git works: Esc and a click outside change nothing.
      onOpenChange={(next) => (running ? undefined : onOpenChange(next))}
      title="Merge a branch"
      description={chosen ? `Merge ${chosen} into ${current}?` : undefined}
      footer={footer}
    >
      {listing.state === "failed" ? (
        <Alert variant="error">
          <TriangleAlert aria-hidden />
          <p className="min-w-0 flex-1">{listing.reason}</p>
        </Alert>
      ) : chosen ? (
        ready && ready.tracked.length > 0 ? (
          <Alert variant="warning">
            <TriangleAlert aria-hidden />
            <p className="min-w-0 flex-1">{`Git still tracks ${ready.tracked.length} of Weaver's files: their changes go into the merge commit.`}</p>
          </Alert>
        ) : null
      ) : ready ? (
        <BranchList branches={ready} query={query} onQuery={setQuery} onChoose={choose} />
      ) : (
        <p role="status" className="text-sm text-muted-foreground">
          Reading the branches…
        </p>
      )}
    </FormDialog>
  );
}

function BranchList({ branches, query, onQuery, onChoose }: { branches: MergeBranches; query: string; onQuery: (text: string) => void; onChoose: (value: string) => void }) {
  const local = matching(branches.local, query);
  const remote = matching(branches.remote, query);
  const none = branches.local.length === 0 && branches.remote.length === 0;
  return (
    <Command onChoose={onChoose}>
      <CommandInput value={query} onValueChange={onQuery} placeholder="Search branches" aria-label="Search branches" autoFocus />
      <CommandList aria-label="Branches">
        {none ? <CommandEmpty>No other branch</CommandEmpty> : null}
        {!none && local.length + remote.length === 0 ? <CommandEmpty>No branch matches</CommandEmpty> : null}
        {local.length > 0 ? (
          <CommandGroup heading="Local">
            {local.map((branch) => (
              <CommandItem key={branch} value={itemValue("local", branch)}>
                {branch}
              </CommandItem>
            ))}
          </CommandGroup>
        ) : null}
        {remote.length > 0 ? (
          <CommandGroup heading="Remote">
            {remote.map((branch) => (
              <CommandItem key={branch} value={itemValue("remote", branch)}>
                {branch}
              </CommandItem>
            ))}
          </CommandGroup>
        ) : null}
      </CommandList>
    </Command>
  );
}
