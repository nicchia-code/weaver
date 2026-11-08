import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useRef, useState } from "react";
import { describe, expect, it, vi } from "vitest";

import { CardId } from "@/components/card-id";
import { ConfirmDialog, FormDialog } from "@/components/dialogs";
import { TooltipProvider } from "@/components/ui/tooltip";

function Confirm({ onConfirm }: { onConfirm: () => void }) {
  const [open, setOpen] = useState(false);
  return (
    <>
      <button onClick={() => setOpen(true)}>Delete T4</button>
      <ConfirmDialog
        open={open}
        onOpenChange={setOpen}
        title="Delete T4 for good?"
        description="The Task is removed."
        confirmLabel="Delete"
        destructive
        onConfirm={onConfirm}
      />
    </>
  );
}

describe("ConfirmDialog", () => {
  it("puts the initial focus on Cancel and has a destructive confirming button", async () => {
    const user = userEvent.setup();
    render(<Confirm onConfirm={() => {}} />);
    await user.click(screen.getByRole("button", { name: "Delete T4" }));
    const dialog = await screen.findByRole("alertdialog", { name: "Delete T4 for good?" });
    expect(dialog).toHaveAccessibleDescription("The Task is removed.");
    expect(screen.getByRole("button", { name: "Cancel" })).toHaveFocus();
    expect(screen.getByRole("button", { name: "Delete" }).className).toContain("text-destructive");
  });

  it("changes nothing when cancelled and gives the focus back to what opened it", async () => {
    const onConfirm = vi.fn();
    const user = userEvent.setup();
    render(<Confirm onConfirm={onConfirm} />);
    const opener = screen.getByRole("button", { name: "Delete T4" });
    await user.click(opener);
    await user.click(await screen.findByRole("button", { name: "Cancel" }));
    expect(onConfirm).not.toHaveBeenCalled();
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(opener).toHaveFocus();
  });

  it("closes with Esc without confirming, and confirms only with its button", async () => {
    const onConfirm = vi.fn();
    const user = userEvent.setup();
    render(<Confirm onConfirm={onConfirm} />);
    await user.click(screen.getByRole("button", { name: "Delete T4" }));
    await screen.findByRole("alertdialog");
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(onConfirm).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "Delete T4" }));
    await user.click(await screen.findByRole("button", { name: "Delete" }));
    expect(onConfirm).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
  });

  it("is not closed by a click outside", async () => {
    const user = userEvent.setup();
    render(<Confirm onConfirm={() => {}} />);
    await user.click(screen.getByRole("button", { name: "Delete T4" }));
    await screen.findByRole("alertdialog");
    // A modal dialog switches off the pointer events of the page: a click outside is an event on the overlay.
    fireEvent.pointerDown(document.body);
    fireEvent.click(document.body);
    expect(screen.getByRole("alertdialog")).toBeInTheDocument();
  });
});

function Details() {
  const [open, setOpen] = useState(false);
  const opener = useRef<HTMLButtonElement>(null);
  return (
    <>
      <button ref={opener} onClick={() => setOpen(true)}>
        Open details
      </button>
      <FormDialog open={open} onOpenChange={setOpen} title="Task T4" footer={<button>Run</button>} returnFocus={opener}>
        <p>body</p>
      </FormDialog>
    </>
  );
}

describe("FormDialog", () => {
  it("is at most 720 px wide, scrolls its body, and closes with Esc or its close button", async () => {
    const user = userEvent.setup();
    render(<Details />);
    await user.click(screen.getByRole("button", { name: "Open details" }));
    const dialog = await screen.findByRole("dialog", { name: "Task T4" });
    expect(dialog).toHaveClass("max-w-[720px]");
    expect(screen.getByText("body").parentElement).toHaveClass("overflow-y-auto");
    expect(screen.getByRole("button", { name: "Run" })).toBeInTheDocument();

    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Open details" })).toHaveFocus();

    await user.click(screen.getByRole("button", { name: "Open details" }));
    await user.click(await screen.findByRole("button", { name: "Close" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Open details" })).toHaveFocus();
  });
});

describe("CardId", () => {
  const wrap = (node: React.ReactNode) => <TooltipProvider>{node}</TooltipProvider>;

  it("shows the short number and not the permanent id, named as a temporary short number", () => {
    render(wrap(<CardId shortNumber="P3" permanentId="qmqh1o3p" />));
    const id = screen.getByRole("img", { name: "Short number (temporary) P3" });
    expect(id).toHaveTextContent("P3");
    expect(id).not.toHaveTextContent("qmqh1o3p");
    expect(id).toHaveClass("font-mono");
  });

  it("shows the permanent id when there is no short number", () => {
    render(wrap(<CardId shortNumber={null} permanentId="qmqh1o3p" />));
    const id = screen.getByRole("img", { name: "Permanent ID qmqh1o3p" });
    expect(id).toHaveTextContent("qmqh1o3p");
  });

  it("says the same in its tooltip", async () => {
    const user = userEvent.setup();
    render(wrap(<CardId shortNumber="T4" permanentId="o7f3fscl" />));
    await user.hover(screen.getByRole("img"));
    expect(await screen.findByRole("tooltip", undefined, { timeout: 2000 })).toHaveTextContent("Short number (temporary) T4");
  });
});
