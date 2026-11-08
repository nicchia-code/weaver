import type { ReactNode } from "react";

import { VIEWS, type ViewId } from "@/views";

/**
 * The row every view starts with (`requirements/ui#General`, "View header"): full width, a fixed
 * 76 px plus its 1 px bottom border (`#EFEDE7`) whether or not it holds a subtitle or controls,
 * side padding 28 px, content centred in the row (controls are 36 px high). A view that replaces
 * the header (Chat in front of a sub-agent) takes the same class so the border does not move.
 */
export const VIEW_HEADER_CLASS = "box-content flex h-[76px] shrink-0 items-center gap-3 border-b border-line-soft px-7";

/** The id of the heading the region of a view is labelled by. */
export const viewTitleId = (view: ViewId) => `view-${view}`;

/** The name of a view as the main heading of its header: the sidebar entry's name. */
export function ViewTitle({ view }: { view: ViewId }) {
  const label = VIEWS.find((entry) => entry.id === view)?.label ?? view;
  return (
    <h1 id={viewTitleId(view)} className="min-w-0 truncate text-[19px] font-semibold leading-tight tracking-title">
      {label}
    </h1>
  );
}

/**
 * The header of a view: its name (the sidebar entry's) as the title on the left, under it an
 * optional one-line `subtitle` in Muted, what the view puts right after the title (`after`, the
 * agent pill of Chat), the view's own controls (`children`) on the right. The title is the
 * view's main heading for assistive technologies.
 */
export function ViewHeader({
  view,
  subtitle,
  after,
  children,
}: {
  view: ViewId;
  subtitle?: ReactNode;
  after?: ReactNode;
  children?: ReactNode;
}) {
  const title = <ViewTitle view={view} />;
  return (
    <header className={VIEW_HEADER_CLASS}>
      {subtitle ? (
        <div className="flex min-w-0 flex-col">
          {title}
          <p className="min-w-0 truncate text-[13px] text-muted-foreground">{subtitle}</p>
        </div>
      ) : (
        title
      )}
      {after}
      {children ? <div className="ml-auto flex shrink-0 items-center gap-3 [&_button]:h-9">{children}</div> : null}
    </header>
  );
}
