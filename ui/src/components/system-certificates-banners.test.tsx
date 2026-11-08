import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { SystemCertificatesStatus } from "@/bindings/SystemCertificatesStatus";
import { SystemCertificatesBanners, TURN_ON_TEXT } from "@/components/system-certificates-banners";
import { TooltipProvider } from "@/components/ui/tooltip";
import { call, on } from "@/lib/backend";

vi.mock("@/lib/backend", () => ({
  call: vi.fn(),
  on: vi.fn(),
  isAppError: (value: unknown) => typeof value === "object" && value !== null && "code" in value && "message" in value,
}));

const TODAY = new Date().toISOString();
const NOTICE = "System Certificates turned on";
const FAILURE = "System Certificates export failed";

const status = (patch: Partial<SystemCertificatesStatus> = {}): SystemCertificatesStatus => ({
  revision: 1,
  phase: "ready",
  automatic: true,
  count: 142,
  writtenAt: TODAY,
  error: null,
  turnOnNotice: false,
  failureBanner: false, failureReason: null,
  ...patch,
});

const failed = (patch: Partial<SystemCertificatesStatus> = {}) => {
  const shown = status({ phase: "failed", error: "no certificate authority found", failureBanner: true, revision: 2, ...patch });
  return { ...shown, failureReason: shown.failureBanner ? (patch.failureReason ?? shown.error) : null };
};

/** The backend of the app: it holds the status and tells every window what it is after each change. */
let held: SystemCertificatesStatus;
let listeners: ((status: SystemCertificatesStatus) => void)[];

function tell(next: SystemCertificatesStatus) {
  held = next;
  act(() => listeners.forEach((listener) => listener(held)));
}

beforeEach(() => {
  held = status();
  listeners = [];
  vi.mocked(on).mockImplementation(((name: string, handler: (status: SystemCertificatesStatus) => void) => {
    if (name === "system-certificates-changed") listeners.push(handler);
    return Promise.resolve(() => {});
  }) as unknown as typeof on);
  vi.mocked(call).mockReset();
  vi.mocked(call).mockImplementation(((name: string) => {
    switch (name) {
      case "system_certificates_status":
        return Promise.resolve(held);
      case "system_certificates_dismiss_notice":
        held = { ...held, turnOnNotice: false, revision: held.revision + 1 };
        listeners.forEach((listener) => listener(held));
        return Promise.resolve(held);
      case "system_certificates_dismiss_failure":
        held = { ...held, failureBanner: false, failureReason: null, revision: held.revision + 1 };
        listeners.forEach((listener) => listener(held));
        return Promise.resolve(held);
      default:
        return Promise.resolve(null);
    }
  }) as never);
});

function mount(onOpenSettings?: () => void) {
  return render(
    <TooltipProvider>
      <SystemCertificatesBanners onOpenSettings={onOpenSettings} />
    </TooltipProvider>,
  );
}

describe("the turn-on notice", () => {
  it("says that Weaver turned System Certificates on because Zscaler is installed", async () => {
    held = status({ turnOnNotice: true });
    mount(() => {});
    const banner = await screen.findByRole("status", { name: NOTICE });
    expect(banner).toHaveTextContent(
      "Weaver turned on System Certificates because Zscaler is installed: the agent and its commands now trust the certificates your system trusts.",
    );
    expect(TURN_ON_TEXT).toBe(banner.querySelector("p")?.textContent);
    expect(within(banner).getByRole("button", { name: "Open Settings" })).toBeInTheDocument();
  });

  it("has an icon-only Close with the name and the tooltip Close", async () => {
    const user = userEvent.setup();
    held = status({ turnOnNotice: true });
    mount(() => {});
    const close = await screen.findByRole("button", { name: "Close" });
    expect(close.textContent).toBe("");
    await user.hover(close);
    expect(await screen.findByRole("tooltip")).toHaveTextContent("Close");
  });

  it("shows nothing before the backend answers and nothing when the notice is not due", async () => {
    const { container } = mount(() => {});
    await act(async () => {});
    expect(container).toBeEmptyDOMElement();
  });

  it("is also in a window that opens later, read with the status command", async () => {
    held = status({ turnOnNotice: true });
    mount(() => {});
    expect(await screen.findByRole("status", { name: NOTICE })).toBeInTheDocument();
    expect(call).toHaveBeenCalledWith("system_certificates_status");
  });

  it("is closed in every window when it is closed in one", async () => {
    const user = userEvent.setup();
    held = status({ turnOnNotice: true });
    const first = mount(() => {});
    const second = mount(() => {});
    const inFirst = within(first.container);
    const inSecond = within(second.container);
    await inFirst.findByRole("status", { name: NOTICE });
    await inSecond.findByRole("status", { name: NOTICE });
    await user.click(inFirst.getByRole("button", { name: "Close" }));
    expect(call).toHaveBeenCalledWith("system_certificates_dismiss_notice");
    await waitFor(() => expect(first.container).toBeEmptyDOMElement());
    expect(second.container).toBeEmptyDOMElement();
  });

  it("Open Settings shows the Settings view and closes the notice in every window", async () => {
    const user = userEvent.setup();
    const open = vi.fn();
    held = status({ turnOnNotice: true });
    const first = mount(open);
    const second = mount();
    await within(second.container).findByRole("status", { name: NOTICE });
    await user.click(await within(first.container).findByRole("button", { name: "Open Settings" }));
    expect(open).toHaveBeenCalledTimes(1);
    expect(call).toHaveBeenCalledWith("system_certificates_dismiss_notice");
    await waitFor(() => expect(first.container).toBeEmptyDOMElement());
    expect(second.container).toBeEmptyDOMElement();
  });

  it("has no Open Settings where the switch is already on the page (the welcome window)", async () => {
    held = status({ turnOnNotice: true });
    mount();
    const banner = await screen.findByRole("status", { name: NOTICE });
    expect(within(banner).queryByRole("button", { name: "Open Settings" })).toBeNull();
    expect(within(banner).getByRole("button", { name: "Close" })).toBeInTheDocument();
  });

  it("works from the keyboard: Tab reaches Open Settings then Close, Enter and Space act", async () => {
    const user = userEvent.setup();
    const open = vi.fn();
    held = status({ turnOnNotice: true });
    mount(open);
    await screen.findByRole("status", { name: NOTICE });
    await user.tab();
    expect(screen.getByRole("button", { name: "Open Settings" })).toHaveFocus();
    await user.tab();
    expect(screen.getByRole("button", { name: "Close" })).toHaveFocus();
    await user.keyboard(" ");
    expect(call).toHaveBeenCalledWith("system_certificates_dismiss_notice");
    expect(open).not.toHaveBeenCalled();
  });

  it("Enter on Open Settings does what a click does", async () => {
    const user = userEvent.setup();
    const open = vi.fn();
    held = status({ turnOnNotice: true });
    mount(open);
    await screen.findByRole("status", { name: NOTICE });
    await user.tab();
    await user.keyboard("{Enter}");
    expect(open).toHaveBeenCalledTimes(1);
  });
});

describe("the export failure banner", () => {
  it("says what the failed status line says, with an earlier bundle", async () => {
    held = failed();
    mount(() => {});
    const banner = await screen.findByRole("alert", { name: FAILURE });
    expect(banner.querySelector("p")?.textContent).toMatch(
      /^Could not export the system certificates \(no certificate authority found\)\. Using the ones exported \d{2}:\d{2}\.$/,
    );
    expect(within(banner).getByRole("button", { name: "Open Settings" })).toBeInTheDocument();
    expect(within(banner).getByRole("button", { name: "Close" })).toBeInTheDocument();
  });

  it("says that the agent gets no certificates when there is no earlier bundle", async () => {
    held = failed({ count: null, writtenAt: null });
    mount(() => {});
    expect(await screen.findByRole("alert", { name: FAILURE })).toHaveTextContent(
      "Could not export the system certificates (no certificate authority found). The agent gets no certificates from Weaver.",
    );
  });

  it("names no product", async () => {
    held = failed();
    mount(() => {});
    const banner = await screen.findByRole("alert", { name: FAILURE });
    expect(banner.textContent).not.toMatch(/zscaler/i);
  });

  it("shows nothing while the status line alone speaks (an export that was asked)", async () => {
    held = failed({ failureBanner: false, failureReason: null });
    const { container } = mount(() => {});
    await act(async () => {});
    expect(container).toBeEmptyDOMElement();
  });

  it("stays while another export runs, with the text of the failure it speaks of", async () => {
    held = failed();
    mount(() => {});
    await screen.findByRole("alert", { name: FAILURE });
    // A Refresh or the daily export starts: the status line says Exporting, the banner stays.
    tell(status({ phase: "exporting", error: null, failureBanner: true, failureReason: "no certificate authority found", revision: 3 }));
    expect(screen.getByRole("alert", { name: FAILURE })).toHaveTextContent("(no certificate authority found)");
  });

  it("a newer failure replaces the text, and a successful export removes it", async () => {
    held = failed();
    mount(() => {});
    await screen.findByRole("alert", { name: FAILURE });
    tell(failed({ error: "could not read the certificates", revision: 3 }));
    expect(screen.getByRole("alert", { name: FAILURE })).toHaveTextContent("(could not read the certificates)");
    tell(status({ revision: 4 }));
    expect(screen.queryByRole("alert", { name: FAILURE })).toBeNull();
  });

  it("turning the setting off removes it", async () => {
    held = failed();
    mount(() => {});
    await screen.findByRole("alert", { name: FAILURE });
    tell(status({ phase: "off", failureBanner: false, failureReason: null, revision: 3 }));
    expect(screen.queryByRole("alert", { name: FAILURE })).toBeNull();
  });

  it("is closed in every window when it is closed in one, and a newer failure shows it again", async () => {
    const user = userEvent.setup();
    held = failed();
    const first = mount(() => {});
    const second = mount();
    await within(first.container).findByRole("alert", { name: FAILURE });
    await within(second.container).findByRole("alert", { name: FAILURE });
    await user.click(within(second.container).getByRole("button", { name: "Close" }));
    expect(call).toHaveBeenCalledWith("system_certificates_dismiss_failure");
    await waitFor(() => expect(first.container).toBeEmptyDOMElement());
    expect(second.container).toBeEmptyDOMElement();
    tell(failed({ error: "boom", revision: 10 }));
    expect(within(first.container).getByRole("alert", { name: FAILURE })).toHaveTextContent("(boom)");
    expect(within(second.container).getByRole("alert", { name: FAILURE })).toBeInTheDocument();
  });

  it("Open Settings shows the Settings view and only in a project window", async () => {
    const user = userEvent.setup();
    const open = vi.fn();
    held = failed();
    const project = mount(open);
    const welcome = mount();
    await within(welcome.container).findByRole("alert", { name: FAILURE });
    expect(within(welcome.container).queryByRole("button", { name: "Open Settings" })).toBeNull();
    await user.click(await within(project.container).findByRole("button", { name: "Open Settings" }));
    expect(open).toHaveBeenCalledTimes(1);
    expect(call).not.toHaveBeenCalledWith("system_certificates_dismiss_failure");
  });

  it("can be closed from the keyboard", async () => {
    const user = userEvent.setup();
    held = failed();
    const { container } = mount();
    await screen.findByRole("alert", { name: FAILURE });
    await user.tab();
    expect(screen.getByRole("button", { name: "Close" })).toHaveFocus();
    await user.keyboard("{Enter}");
    await waitFor(() => expect(container).toBeEmptyDOMElement());
  });

  it("shows both banners together, the notice first", async () => {
    held = failed({ turnOnNotice: true });
    mount(() => {});
    const notice = await screen.findByRole("status", { name: NOTICE });
    const failure = screen.getByRole("alert", { name: FAILURE });
    expect(notice.compareDocumentPosition(failure) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });
});
