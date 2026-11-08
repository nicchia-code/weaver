import { TriangleAlert } from "lucide-react";
import type { KeyboardEvent, ReactNode } from "react";

import { NoticeStack } from "@/components/notices";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { ErrorBanner } from "@/components/views/error-banner";
import type { ViewName } from "@/components/views/focus-model";
import { FocusBar } from "@/components/views/focus-bar";
import { ViewHeader } from "@/components/view-header";
import { useViews } from "@/components/views/views-state";

/**
 * What Requirements and Specs have in common: the view header (with the `toolbar` the view adds:
 * the Requirements' `Ask more`), the notice of an overlay that stopped being open, the error
 * banner, the focus bar, and a message while the snapshot is loading or could not be read. Esc
 * clears the view's focus, and with it the overlay that focus drew.
 */
export function ViewFrame({ view, subtitle, toolbar, children }: { view: ViewName; subtitle?: ReactNode; toolbar?: ReactNode; children: ReactNode }) {
  const { snapshot, loadError, reload, notices, closeNotice, focus, clearFocus } = useViews();

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key !== "Escape" || event.defaultPrevented || !focus[view]) return;
    clearFocus(view);
    event.preventDefault();
  };

  return (
    <div className="flex h-full min-h-0 flex-col" onKeyDown={onKeyDown}>
      <ViewHeader view={view} subtitle={subtitle}>
        {toolbar}
      </ViewHeader>
      <NoticeStack notices={notices} onClose={closeNotice} />
      {snapshot ? <ErrorBanner errors={snapshot.data.errors} /> : null}
      <FocusBar view={view} />
      <div className="min-h-0 flex-1">
        {snapshot ? (
          children
        ) : loadError ? (
          <Alert variant="error" className="m-4 w-auto">
            <TriangleAlert aria-hidden />
            <p className="min-w-0 flex-1">{loadError}</p>
            <Button variant="outline" size="sm" onClick={reload}>
              Try again
            </Button>
          </Alert>
        ) : (
          <p className="p-4 text-sm text-muted-foreground" role="status">
            Loading…
          </p>
        )}
      </div>
    </div>
  );
}
