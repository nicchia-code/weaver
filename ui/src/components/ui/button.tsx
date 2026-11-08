import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";
import * as React from "react";

import { cn } from "@/lib/utils";

// Ink is the only colour of actions: primary buttons are Ink with inverted text, secondary and outline
// buttons are Surface with a 1 px border, ghost buttons have no background and Ink 2 text, destructive
// buttons are Surface with Rose ink text and a Rose line. Focus ring: 2 px, `ring` token (Ink, 3:1 or
// more against both backgrounds). A control that is
// disabled for a reason uses `aria-disabled` (see GuardedButton); `disabled` is for controls that
// have no reason to explain.
const buttonVariants = cva(
  "inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-md text-[13px] font-medium transition-colors duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background disabled:pointer-events-none disabled:opacity-50 aria-disabled:cursor-not-allowed aria-disabled:opacity-50 [&_svg]:pointer-events-none [&_svg]:size-4 [&_svg]:shrink-0",
  {
    variants: {
      variant: {
        default: "bg-primary text-primary-foreground hover:bg-primary/90 aria-disabled:hover:bg-primary",
        destructive:
          "border border-destructive-line bg-card text-destructive hover:bg-rose-tint aria-disabled:hover:bg-card",
        secondary: "border border-input bg-card text-foreground hover:bg-accent aria-disabled:hover:bg-card",
        outline: "border border-input bg-card text-foreground hover:bg-accent aria-disabled:hover:bg-card",
        ghost: "text-ink-2 hover:bg-accent hover:text-foreground aria-disabled:hover:bg-transparent aria-disabled:hover:text-ink-2",
      },
      size: {
        default: "h-9 px-4 py-2",
        sm: "h-8 rounded-md px-3 text-xs",
        icon: "size-9",
      },
    },
    defaultVariants: { variant: "default", size: "default" },
  },
);

export interface ButtonProps
  extends React.ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof buttonVariants> {
  asChild?: boolean;
}

const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(
  ({ className, variant, size, asChild = false, type, ...props }, ref) => {
    const Comp = asChild ? Slot : "button";
    return (
      <Comp
        className={cn(buttonVariants({ variant, size, className }))}
        ref={ref}
        {...(asChild ? {} : { type: type ?? "button" })}
        {...props}
      />
    );
  },
);
Button.displayName = "Button";

export { Button, buttonVariants };
