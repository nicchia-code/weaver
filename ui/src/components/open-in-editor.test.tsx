import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { toast } from "sonner";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { OpenInEditor } from "@/components/open-in-editor";
import { ProjectContext } from "@/components/project-context";
import { call } from "@/lib/backend";

vi.mock("@/lib/backend", () => ({
  call: vi.fn(),
  isAppError: (value: unknown) => typeof value === "object" && value !== null && "code" in value && "message" in value,
}));
vi.mock("sonner", () => ({ toast: { error: vi.fn() }, Toaster: () => null }));

beforeEach(() => {
  vi.mocked(call).mockResolvedValue(null as never);
});

describe("Open in editor", () => {
  it("opens the file at the line", async () => {
    render(<OpenInEditor file="/work/alpha/.weaver/model/cart.md" line={12} />);
    await userEvent.setup().click(screen.getByRole("button", { name: "Open in editor" }));
    expect(call).toHaveBeenCalledWith("open_in_editor", { file: "/work/alpha/.weaver/model/cart.md", line: 12, projectId: null });
  });

  it("takes the project of the window for a file relative to it", async () => {
    render(
      <ProjectContext.Provider value={{ projectId: "0123456789abcdef", showView: vi.fn(), openJob: vi.fn(), openedJob: null, openedJobRequest: 0 }}>
        <OpenInEditor file=".weaver/model/cart.md" />
      </ProjectContext.Provider>,
    );
    await userEvent.setup().click(screen.getByRole("button", { name: "Open in editor" }));
    expect(call).toHaveBeenCalledWith("open_in_editor", { file: ".weaver/model/cart.md", line: null, projectId: "0123456789abcdef" });
  });

  it("works with the keyboard", async () => {
    const user = userEvent.setup();
    render(<OpenInEditor file="/a.md" line={1} />);
    await user.tab();
    expect(screen.getByRole("button", { name: "Open in editor" })).toHaveFocus();
    await user.keyboard("{Enter}");
    expect(call).toHaveBeenCalledTimes(1);
  });

  it("gives a notice with the reason when the editor cannot start", async () => {
    vi.mocked(call).mockRejectedValue({ code: "invalid-input", message: "The editor `code` was not found." } as never);
    render(<OpenInEditor file="/a.md" line={1} />);
    await userEvent.setup().click(screen.getByRole("button", { name: "Open in editor" }));
    await vi.waitFor(() =>
      expect(toast.error).toHaveBeenCalledWith("Could not open the file in the editor.", { description: "The editor `code` was not found." }),
    );
  });
});
