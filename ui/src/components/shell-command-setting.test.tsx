import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { ShellCommandOutcome } from "@/bindings/ShellCommandOutcome";
import type { ShellCommandResult } from "@/bindings/ShellCommandResult";
import { noteOf, ShellCommandSetting, shellCommandFailure } from "@/components/shell-command-setting";
import { ViewShownContext } from "@/components/view-shown";
import { call } from "@/lib/backend";

vi.mock("@/lib/backend", () => ({
  call: vi.fn(),
  isAppError: (value: unknown) => typeof value === "object" && value !== null && "code" in value && "message" in value,
}));

const PLACE = "/usr/local/bin/weaver";

function result(installed: boolean, outcome: ShellCommandOutcome = { kind: "done" }, patch: Partial<ShellCommandResult> = {}): ShellCommandResult {
  return { installed, place: PLACE, outcome, pathWarning: false, ...patch };
}

let state: ShellCommandResult;
let install: () => Promise<ShellCommandResult>;
let remove: () => Promise<ShellCommandResult>;
let status: () => Promise<ShellCommandResult>;

beforeEach(() => {
  state = result(false);
  status = () => Promise.resolve(state);
  install = () => Promise.resolve((state = result(true)));
  remove = () => Promise.resolve((state = result(false)));
  vi.mocked(call).mockReset();
  vi.mocked(call).mockImplementation(((name: string) => {
    if (name === "shell_command_status") return status();
    if (name === "shell_command_install") return install();
    if (name === "shell_command_remove") return remove();
    return Promise.reject(new Error(`unexpected command ${name}`));
  }) as unknown as typeof call);
});

describe("the words of an action's result", () => {
  it("words a refusal at install and at removal with the path found", () => {
    const refused = result(false, { kind: "refused", found: "/usr/local/bin/weaver" });
    expect(noteOf("install", refused)).toEqual({
      tone: "info",
      text: "/usr/local/bin/weaver already exists and is not Weaver's command; nothing was changed.",
    });
    expect(noteOf("remove", refused)).toEqual({
      tone: "info",
      text: "/usr/local/bin/weaver is not Weaver's command; nothing was removed.",
    });
  });

  it("words a failure with its reason and one full stop", () => {
    const failed = result(false, { kind: "failed", reason: "permission denied." });
    expect(noteOf("install", failed)?.text).toBe("Could not install the weaver command: permission denied.");
    expect(noteOf("remove", failed)?.text).toBe("Could not remove the weaver command: permission denied.");
    expect(shellCommandFailure("remove", "busy")).toBe("Could not remove the weaver command: busy.");
  });

  it("says nothing for a cancelled password request, and warns about the PATH only after installing", () => {
    expect(noteOf("install", result(false, { kind: "cancelled" }))).toBeNull();
    expect(noteOf("remove", result(true, { kind: "cancelled" }))).toBeNull();
    const warn = result(true, { kind: "done" }, { pathWarning: true });
    expect(noteOf("install", warn)).toEqual({ tone: "warning", text: "~/.local/bin may not be in your PATH; see the README." });
    expect(noteOf("remove", warn)).toBeNull();
  });
});

describe("the Command line setting", () => {
  it("says what weaver open does, with the command in a monospace font", async () => {
    render(<ShellCommandSetting />);
    await screen.findByText("Not installed");
    const command = screen.getByText("weaver open <folder>");
    expect(command.tagName).toBe("CODE");
    expect(command).toHaveClass("font-mono");
    expect(command.parentElement).toHaveTextContent("Open a folder in Weaver from a terminal with weaver open <folder>.");
  });

  it("shows `Not installed` and the Install weaver command button", async () => {
    render(<ShellCommandSetting />);
    expect(await screen.findByText("Not installed")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Install weaver command" })).toBeEnabled();
    expect(screen.queryByRole("button", { name: "Remove" })).not.toBeInTheDocument();
  });

  it("shows `Installed at <place>` with the place in a monospace font, and Remove", async () => {
    state = result(true);
    render(<ShellCommandSetting />);
    const place = await screen.findByText(PLACE);
    expect(place.tagName).toBe("CODE");
    expect(place).toHaveClass("font-mono");
    expect(place.parentElement).toHaveTextContent(`Installed at ${PLACE}`);
    expect(screen.getByRole("button", { name: "Remove" })).toBeEnabled();
    expect(screen.queryByRole("button", { name: "Install weaver command" })).not.toBeInTheDocument();
  });

  it("reads the state again each time Settings shows", async () => {
    const view = (shown: boolean) => (
      <ViewShownContext.Provider value={shown}>
        <ShellCommandSetting />
      </ViewShownContext.Provider>
    );
    const { rerender } = render(view(true));
    await screen.findByText("Not installed");
    expect(call).toHaveBeenCalledTimes(1);

    rerender(view(false));
    state = result(true);
    expect(call).toHaveBeenCalledTimes(1);

    rerender(view(true));
    expect(await screen.findByText(`Installed at`, { exact: false })).toBeInTheDocument();
    expect(call).toHaveBeenCalledTimes(2);
    expect(screen.getByRole("button", { name: "Remove" })).toBeInTheDocument();
  });

  it("installs: the button spins and is disabled meanwhile, then the state is the answer's", async () => {
    const user = userEvent.setup();
    let finish: (value: ShellCommandResult) => void = () => {};
    install = () => new Promise((resolve) => (finish = resolve));
    render(<ShellCommandSetting />);
    const button = await screen.findByRole("button", { name: "Install weaver command" });
    await user.click(button);
    expect(button).toBeDisabled();
    expect(button.querySelector("svg")).toHaveClass("animate-spin", "motion-reduce:animate-none");

    await act(async () => finish(result(true)));
    expect(screen.getByRole("button", { name: "Remove" })).toBeEnabled();
    expect(screen.getByText(PLACE)).toBeInTheDocument();
  });

  it("removes: the button spins and is disabled meanwhile, then it is `Not installed` again", async () => {
    const user = userEvent.setup();
    state = result(true);
    let finish: (value: ShellCommandResult) => void = () => {};
    remove = () => new Promise((resolve) => (finish = resolve));
    render(<ShellCommandSetting />);
    const button = await screen.findByRole("button", { name: "Remove" });
    await user.click(button);
    expect(button).toBeDisabled();
    expect(button.querySelector("svg")).toHaveClass("animate-spin");

    await act(async () => finish(result(false)));
    expect(screen.getByText("Not installed")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Install weaver command" })).toBeEnabled();
  });

  it("refuses at install with the path found and keeps the text until the next action", async () => {
    const user = userEvent.setup();
    install = () => Promise.resolve(result(false, { kind: "refused", found: PLACE }));
    render(<ShellCommandSetting />);
    await user.click(await screen.findByRole("button", { name: "Install weaver command" }));
    expect(await screen.findByText(`${PLACE} already exists and is not Weaver's command; nothing was changed.`)).toBeInTheDocument();
    expect(screen.getByText("Not installed")).toBeInTheDocument();

    install = () => new Promise(() => {});
    await user.click(screen.getByRole("button", { name: "Install weaver command" }));
    expect(screen.queryByText(/already exists/)).not.toBeInTheDocument();
  });

  it("refuses at removal with the path found", async () => {
    const user = userEvent.setup();
    state = result(true);
    remove = () => Promise.resolve(result(false, { kind: "refused", found: PLACE }));
    render(<ShellCommandSetting />);
    await user.click(await screen.findByRole("button", { name: "Remove" }));
    expect(await screen.findByText(`${PLACE} is not Weaver's command; nothing was removed.`)).toBeInTheDocument();
  });

  it("says why an installation failed, in the error colour", async () => {
    const user = userEvent.setup();
    install = () => Promise.resolve(result(false, { kind: "failed", reason: "permission denied" }));
    render(<ShellCommandSetting />);
    await user.click(await screen.findByRole("button", { name: "Install weaver command" }));
    expect(await screen.findByText("Could not install the weaver command: permission denied.")).toHaveClass("text-rose-ink");
    expect(screen.getByText("Not installed")).toBeInTheDocument();
  });

  it("says why a removal failed, in the error colour, and stays installed", async () => {
    const user = userEvent.setup();
    state = result(true);
    remove = () => Promise.resolve(result(true, { kind: "failed", reason: "in use" }));
    render(<ShellCommandSetting />);
    await user.click(await screen.findByRole("button", { name: "Remove" }));
    expect(await screen.findByText("Could not remove the weaver command: in use.")).toHaveClass("text-rose-ink");
    expect(screen.getByRole("button", { name: "Remove" })).toBeEnabled();
  });

  it("says why when the backend refuses the action with an error", async () => {
    const user = userEvent.setup();
    install = () => Promise.reject({ code: "refused", message: "another action on the weaver command is already running" });
    render(<ShellCommandSetting />);
    await user.click(await screen.findByRole("button", { name: "Install weaver command" }));
    expect(
      await screen.findByText("Could not install the weaver command: another action on the weaver command is already running."),
    ).toHaveClass("text-rose-ink");
    expect(screen.getByRole("button", { name: "Install weaver command" })).toBeEnabled();
  });

  it("shows nothing for a cancelled administrator password request", async () => {
    const user = userEvent.setup();
    install = () => Promise.resolve(result(false, { kind: "cancelled" }));
    render(<ShellCommandSetting />);
    await user.click(await screen.findByRole("button", { name: "Install weaver command" }));
    await screen.findByRole("button", { name: "Install weaver command" });
    expect(screen.getByRole("status")).toBeEmptyDOMElement();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(screen.getByText("Not installed")).toBeInTheDocument();
  });

  it("warns about the PATH after installing, in the warning colour", async () => {
    const user = userEvent.setup();
    install = () => Promise.resolve(result(true, { kind: "done" }, { place: "~/.local/bin/weaver", pathWarning: true }));
    render(<ShellCommandSetting />);
    await user.click(await screen.findByRole("button", { name: "Install weaver command" }));
    expect(await screen.findByText("~/.local/bin may not be in your PATH; see the README.")).toHaveClass("text-peach-ink");
    expect(screen.getByText("~/.local/bin/weaver")).toBeInTheDocument();
  });

  it("says when the state cannot be read", async () => {
    status = () => Promise.reject({ code: "internal", message: "operation `shell_command_status` failed unexpectedly" });
    render(<ShellCommandSetting />);
    expect(await screen.findByRole("alert")).toHaveTextContent("Could not check the weaver command: operation `shell_command_status` failed unexpectedly.");
  });
});
