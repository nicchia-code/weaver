// What the interface says about the export of the system certificates
// (`requirements/ui#system-certificates`). The status line of the switch and the banner of a failed
// export share it, so the same failure reads the same everywhere.
import type { SystemCertificatesStatus } from "@/bindings/SystemCertificatesStatus";
import { formatStarted } from "@/lib/format";

/** The words shown while an export runs; also the tooltip of the disabled `Refresh`. */
export const EXPORTING_TEXT = "Exporting system certificates…";

export type SystemCertificatesLineKind = "exporting" | "ready" | "failed";

export interface SystemCertificatesLine {
  kind: SystemCertificatesLineKind;
  text: string;
}

/**
 * The text of the status line for `status`, or `null` while the setting is off. `<date and time>`
 * is when the bundle in use was written: `HH:MM` today, else `D MMM HH:MM`.
 */
export function systemCertificatesLine(status: SystemCertificatesStatus, now: Date | number): SystemCertificatesLine | null {
  switch (status.phase) {
    case "off":
      return null;
    case "exporting":
      return { kind: "exporting", text: EXPORTING_TEXT };
    case "ready":
      return {
        kind: "ready",
        text: `${status.count ?? 0} certificates, exported ${written(status, now)}.`,
      };
    case "failed": {
      const failure = `Could not export the system certificates (${status.error ?? "the export failed"}).`;
      const earlier = status.count !== null && status.writtenAt !== null;
      return {
        kind: "failed",
        text: earlier ? `${failure} Using the ones exported ${written(status, now)}.` : `${failure} The agent gets no certificates from Weaver.`,
      };
    }
  }
}

function written(status: SystemCertificatesStatus, now: Date | number): string {
  return status.writtenAt === null ? "" : formatStarted(new Date(status.writtenAt), now);
}
