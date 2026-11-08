import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { toast } from "sonner";
import { afterEach, describe, expect, it } from "vitest";

import { ThemeProvider } from "@/components/theme-provider";
import { Toaster, TOAST_BOTTOM_EXTRA, TOAST_DURATION_MS, VISIBLE_TOASTS } from "@/components/ui/sonner";
import { TooltipProvider } from "@/components/ui/tooltip";

function mount() {
  render(
    <ThemeProvider preference="light">
      <TooltipProvider>
        <Toaster />
      </TooltipProvider>
    </ThemeProvider>,
  );
}

afterEach(() => {
  act(() => {
    toast.dismiss();
  });
});

const toastOf = async (text: string) => (await screen.findByText(text)).closest<HTMLElement>("[data-sonner-toast]")!;

describe("Toaster", () => {
  it("lasts 4 s and shows at most 3 toasts", () => {
    expect(TOAST_DURATION_MS).toBe(4000);
    expect(VISIBLE_TOASTS).toBe(3);
  });

  it("gives every kind of toast an icon-only close button named Close that removes it at once", async () => {
    mount();
    act(() => {
      toast("An info");
      toast.warning("A warning");
      toast.error("An error");
    });
    for (const text of ["An info", "A warning", "An error"]) {
      expect((await toastOf(text)).querySelector("[data-close-button]")).toHaveAccessibleName("Close");
    }
    const user = userEvent.setup();
    await user.click((await toastOf("An error")).querySelector<HTMLElement>("[data-close-button]")!);
    await waitFor(() => expect(screen.queryByText("An error")).not.toBeInTheDocument());
    expect(screen.getByText("A warning")).toBeInTheDocument();
  });

  it("shows a close tooltip when the close button is focused with the keyboard", async () => {
    mount();
    act(() => {
      toast.error("An error");
    });
    (await toastOf("An error")).querySelector<HTMLElement>("[data-close-button]")!.focus();
    expect(await screen.findByRole("tooltip")).toHaveTextContent("Close");
  });

  it("draws an icon for warnings and errors, in their colour, and none for info", async () => {
    mount();
    act(() => {
      toast("An info");
      toast.warning("A warning");
      toast.error("An error");
    });
    expect((await toastOf("An info")).querySelector("[data-icon]")).toBeNull();
    const warning = await toastOf("A warning");
    expect(warning.querySelector("[data-icon] svg.lucide-triangle-alert")).not.toBeNull();
    expect(warning.className).toMatch(/!border-peach/);
    expect(warning.className).toMatch(/\[&_\[data-icon\]\]:text-peach-ink/);
    const error = await toastOf("An error");
    expect(error.querySelector("[data-icon] svg.lucide-circle-x")).not.toBeNull();
    expect(error.className).toMatch(/!border-rose/);
    expect(error.className).toMatch(/\[&_\[data-icon\]\]:text-rose-ink/);
  });

  it("is placed at the bottom right, and Alt+T focuses the toasts", async () => {
    mount();
    act(() => {
      toast.error("An error");
    });
    await toastOf("An error");
    const list = document.querySelector<HTMLElement>("[data-sonner-toaster]")!;
    expect(list).toHaveAttribute("data-y-position", "bottom");
    expect(list).toHaveAttribute("data-x-position", "right");
    await userEvent.setup().keyboard("{Alt>}t{/Alt}");
    expect(document.activeElement === document.querySelector("section ol") || document.querySelector("section ol")?.contains(document.activeElement)).toBe(true);
  });

  it("sits 24 px above the bottom, plus what a bar at the bottom of the window takes while one shows", async () => {
    mount();
    act(() => {
      toast.error("An error");
    });
    await toastOf("An error");
    const list = document.querySelector<HTMLElement>("[data-sonner-toaster]")!;
    expect(list.style.getPropertyValue("--offset-bottom")).toBe(`calc(24px + var(${TOAST_BOTTOM_EXTRA}, 0px))`);
    expect(list.style.getPropertyValue("--offset-right")).toBe("24px");
    expect(list.style.getPropertyValue("--mobile-offset-bottom")).toBe(`calc(16px + var(${TOAST_BOTTOM_EXTRA}, 0px))`);
  });

  it("keeps the toast when Esc is pressed", async () => {
    mount();
    act(() => {
      toast.error("An error");
    });
    const item = await toastOf("An error");
    await userEvent.setup().keyboard("{Escape}");
    // Dismissing marks the toast removed at once; it must still be live.
    expect(item).toHaveAttribute("data-removed", "false");
    expect(screen.getByText("An error")).toBeInTheDocument();
  });
});
