import { describe, expect, it } from "vitest";

import type { SystemCertificatesStatus } from "@/bindings/SystemCertificatesStatus";
import { EXPORTING_TEXT, systemCertificatesLine } from "@/lib/system-certificates";

const NOW = new Date(2026, 10, 2, 12, 0);
const TODAY = new Date(2026, 10, 2, 7, 5).toISOString();
const OTHER_DAY = new Date(2026, 10, 1, 7, 5).toISOString();

const status = (patch: Partial<SystemCertificatesStatus>): SystemCertificatesStatus => ({
  revision: 1,
  phase: "ready",
  automatic: false,
  count: 142,
  writtenAt: TODAY,
  error: null,
  turnOnNotice: false,
  failureBanner: false, failureReason: null,
  ...patch,
});

describe("the status line of System Certificates", () => {
  it("is absent while the setting is off, even with a bundle", () => {
    expect(systemCertificatesLine(status({ phase: "off" }), NOW)).toBeNull();
  });

  it("says that an export runs, with or without an earlier bundle", () => {
    const expected = { kind: "exporting", text: EXPORTING_TEXT };
    expect(EXPORTING_TEXT).toBe("Exporting system certificates…");
    expect(systemCertificatesLine(status({ phase: "exporting" }), NOW)).toEqual(expected);
    expect(systemCertificatesLine(status({ phase: "exporting", count: null, writtenAt: null }), NOW)).toEqual(expected);
  });

  it("says how many certificates the bundle holds and when it was written: HH:MM today, D MMM HH:MM otherwise", () => {
    expect(systemCertificatesLine(status({}), NOW)).toEqual({ kind: "ready", text: "142 certificates, exported 07:05." });
    expect(systemCertificatesLine(status({ writtenAt: OTHER_DAY }), NOW)).toEqual({
      kind: "ready",
      text: "142 certificates, exported 1 Nov 07:05.",
    });
  });

  it("says why an export failed and that the earlier bundle is in use", () => {
    const failed = status({ phase: "failed", error: "the export did not finish within 30 s" });
    expect(systemCertificatesLine(failed, NOW)).toEqual({
      kind: "failed",
      text: "Could not export the system certificates (the export did not finish within 30 s). Using the ones exported 07:05.",
    });
    expect(systemCertificatesLine({ ...failed, writtenAt: OTHER_DAY }, NOW)?.text).toBe(
      "Could not export the system certificates (the export did not finish within 30 s). Using the ones exported 1 Nov 07:05.",
    );
  });

  it("says that the agent gets no certificates when a failed export has no bundle behind it", () => {
    const failed = status({ phase: "failed", count: null, writtenAt: null, error: "no certificate authority found" });
    expect(systemCertificatesLine(failed, NOW)).toEqual({
      kind: "failed",
      text: "Could not export the system certificates (no certificate authority found). The agent gets no certificates from Weaver.",
    });
  });
});
