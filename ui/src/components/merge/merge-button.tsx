import { GitMerge } from "lucide-react";
import { useState } from "react";

import { GuardedButton } from "@/components/guarded-button";
import { MergeDialog } from "@/components/merge/merge-dialog";
import { useMerge } from "@/components/merge/use-merge";

/**
 * `Merge` in the Board header (`requirements/ui#solo-merge`, **Button**): present only while Solo
 * mode is on, disabled with the reason the backend gives (the first that holds), which is its
 * tooltip and its accessible description.
 */
export function MergeButton() {
  const merge = useMerge();
  const [open, setOpen] = useState(false);
  const status = merge?.merge;
  if (!status) return null;
  return (
    <>
      <GuardedButton
        variant="outline"
        size="sm"
        tooltip={`Merge another branch into ${status.current ?? "the current branch"}`}
        reasons={[!status.available && (status.reason ?? "Merge is not available")]}
        onClick={() => setOpen(true)}
      >
        <GitMerge aria-hidden />
        Merge
      </GuardedButton>
      <MergeDialog open={open} onOpenChange={setOpen} />
    </>
  );
}
