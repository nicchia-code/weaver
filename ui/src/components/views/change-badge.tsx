import { Minus, Pencil, Plus } from "lucide-react";

import type { Change } from "@/bindings/Change";
import { Badge } from "@/components/ui/badge";

const CHANGE: Record<Change, { variant: "success" | "warning" | "error"; icon: typeof Plus }> = {
  added: { variant: "success", icon: Plus },
  changed: { variant: "warning", icon: Pencil },
  removed: { variant: "error", icon: Minus },
};

/** The overlay change as text with an icon, as ink on its tint (Sage added, Peach changed, Rose removed): never colour alone. */
export function ChangeBadge({ change, label }: { change: Change; label?: string }) {
  const { variant, icon: Icon } = CHANGE[change];
  return (
    <Badge variant={variant}>
      <Icon aria-hidden />
      {label ?? change}
    </Badge>
  );
}

// Full class names, so Tailwind sees them.
const FOCUS_SKY = "border-l-4 border-sky bg-sky-tint";
const FOCUS_CHANGE: Record<Change, string> = {
  added: "border-l-4 border-sage bg-sage-tint",
  changed: "border-l-4 border-peach bg-peach-tint",
  removed: "border-l-4 border-rose bg-rose-tint",
};

/** A focused section or field: a left border and tint in Sky, or in its change colour under an overlay. */
export function focusClass(focused: boolean, change: Change | null): string {
  if (!focused) return "";
  return change ? FOCUS_CHANGE[change] : FOCUS_SKY;
}
