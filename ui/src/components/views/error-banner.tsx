import { ChevronDown, ChevronRight, OctagonAlert } from "lucide-react";
import { useId, useState } from "react";

import type { ViewError } from "@/bindings/ViewError";
import { OpenInEditor } from "@/components/open-in-editor";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";

/**
 * An invalid Model or Requirements at the top of the Requirements and Specs: a red
 * `Alert` with the count and an expandable list, each error with its message, its file, its line
 * when known and `Open in editor`. Unreadable files are listed the same way.
 */
export function ErrorBanner({ errors }: { errors: readonly ViewError[] }) {
  const [open, setOpen] = useState(false);
  const listId = useId();
  if (errors.length === 0) return null;
  const Chevron = open ? ChevronDown : ChevronRight;
  return (
    <Alert variant="error" className="mx-4 mt-3 w-auto">
      <OctagonAlert aria-hidden />
      <div className="min-w-0 flex-1 space-y-2">
        <div className="flex items-center justify-between gap-3">
          <p className="font-medium">
            The Model has {errors.length} {errors.length === 1 ? "error" : "errors"}
          </p>
          <Button variant="ghost" size="sm" aria-expanded={open} aria-controls={listId} onClick={() => setOpen((v) => !v)}>
            <Chevron aria-hidden />
            {open ? "Hide errors" : "Show errors"}
          </Button>
        </div>
        {open ? (
          <ul id={listId} className="space-y-2">
            {errors.map((error, index) => (
              <li key={`${error.file}:${error.line ?? 0}:${index}`} className="flex items-start justify-between gap-3 rounded-lg p-2 shadow-ring">
                <div className="min-w-0">
                  <p>{error.message}</p>
                  <p className="font-mono text-xs text-muted-foreground">
                    {error.file}
                    {error.line !== null ? `, line ${error.line}` : ""}
                  </p>
                </div>
                <OpenInEditor file={error.file} line={error.line} variant="outline" />
              </li>
            ))}
          </ul>
        ) : null}
      </div>
    </Alert>
  );
}
