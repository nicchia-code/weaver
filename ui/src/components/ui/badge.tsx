import { cva, type VariantProps } from "class-variance-authority";
import * as React from "react";

import { cn } from "@/lib/utils";

// Coloured variants are the ink of a thread on its tint (`requirements/ui#General`): the meaning is
// in the text and icon the badge carries too. Badges are fully rounded; `outline` is the plain
// outline badge (a label, `technical`): text only, no colour.
const badgeVariants = cva(
  "inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-xs font-medium whitespace-nowrap transition-colors [&_svg]:size-3 [&_svg]:shrink-0",
  {
    variants: {
      variant: {
        default: "border-transparent bg-primary text-primary-foreground",
        secondary: "border-transparent bg-secondary text-secondary-foreground",
        outline: "border-input text-foreground",
        success: "border-transparent bg-sage-tint text-sage-ink",
        warning: "border-transparent bg-peach-tint text-peach-ink",
        error: "border-transparent bg-rose-tint text-rose-ink",
        info: "border-transparent bg-sky-tint text-sky-ink",
        lavender: "border-transparent bg-lavender-tint text-lavender-ink",
        muted: "border-transparent bg-muted text-ink-2",
      },
    },
    defaultVariants: { variant: "default" },
  },
);

export interface BadgeProps extends React.HTMLAttributes<HTMLDivElement>, VariantProps<typeof badgeVariants> {}

function Badge({ className, variant, ...props }: BadgeProps) {
  return <div className={cn(badgeVariants({ variant }), className)} {...props} />;
}

export { Badge, badgeVariants };
