import { GitMerge, Sparkles } from "lucide-react";
import { useState } from "react";

import { ConfirmDialog } from "@/components/dialogs";
import { GuardedButton } from "@/components/guarded-button";
import { AGENT_RESOLVING, useMerge } from "@/components/merge/use-merge";
import { Alert } from "@/components/ui/alert";

/**
 * The banner of a merge in progress (`requirements/ui#solo-merge`, **Merge banner**), above both
 * columns and above the notices: a `region` named `Merge in progress` whose text is announced when
 * it changes, with `Resolve with the agent` (only while some file has conflicts) and `Abort merge`.
 * Both wait for the job `Merge conflicts` to end. Weaver never commits the merge: the developer does.
 */
export function MergeBanner() {
  const merge = useMerge();
  const [aborting, setAborting] = useState(false);
  const progress = merge?.merge?.inProgress;
  if (!merge || !progress) return null;

  const reasons = [merge.agentResolving && AGENT_RESOLVING];
  const head = `Merging ${progress.branch} into ${progress.current}: `;
  const text =
    progress.conflicts > 0
      ? `${head}${progress.conflicts} files with conflicts. Resolve them here or by hand, then commit the merge in git.`
      : `${head}no conflicts left. Commit the merge in git to finish.`;

  return (
    <div className="px-4 pt-4">
      <Alert variant="default" role="region" aria-label="Merge in progress">
        <GitMerge aria-hidden />
        <p className="min-w-0 flex-1" aria-live="polite">
          {text}
        </p>
        <div className="-my-1.5 flex shrink-0 items-center gap-2">
          {progress.conflicts > 0 ? (
            <GuardedButton size="sm" reasons={reasons} onClick={() => void merge.resolveWithAgent()}>
              <Sparkles aria-hidden />
              Resolve with the agent
            </GuardedButton>
          ) : null}
          <GuardedButton size="sm" variant="outline" className="text-destructive" reasons={reasons} onClick={() => setAborting(true)}>
            Abort merge
          </GuardedButton>
        </div>
      </Alert>
      <ConfirmDialog
        open={aborting}
        onOpenChange={setAborting}
        title="Abort merge"
        description={`Abort the merge of ${progress.branch}? Every change of the merge, conflict resolutions included, is lost.`}
        confirmLabel="Abort merge"
        destructive
        onConfirm={() => void merge.abort()}
      />
    </div>
  );
}
