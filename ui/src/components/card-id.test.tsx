import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { CardId } from "@/components/card-id";
import { TooltipProvider } from "@/components/ui/tooltip";

describe("CardId", () => {
  it("is monospace on a Paper tag with a 6 px radius, named for assistive technologies", () => {
    render(
      <TooltipProvider>
        <CardId shortNumber="T3" permanentId="qmqh1o3p" />
      </TooltipProvider>,
    );
    const tag = screen.getByRole("img", { name: "Short number (temporary) T3" });
    expect(tag).toHaveClass("font-mono", "bg-paper", "rounded-sm");
    expect(tag).toHaveTextContent("T3");
    expect(tag).not.toHaveTextContent("qmqh1o3p");
  });

  it("shows the permanent id, and only it, when the card has no short number", () => {
    render(
      <TooltipProvider>
        <CardId permanentId="qmqh1o3p" />
      </TooltipProvider>,
    );
    expect(screen.getByRole("img", { name: "Permanent ID qmqh1o3p" })).toHaveTextContent("qmqh1o3p");
  });
});
