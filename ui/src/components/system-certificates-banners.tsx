import { ShieldCheck, TriangleAlert, X } from "lucide-react";

import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { useSystemCertificates } from "@/hooks/use-system-certificates";
import { systemCertificatesLine } from "@/lib/system-certificates";

/** The text of the notice and of the system notification (`requirements/ui#system-certificates`). */
export const TURN_ON_TEXT =
  "Weaver turned on System Certificates because Zscaler is installed: the agent and its commands now trust the certificates your system trusts.";

function CloseButton({ onClick }: { onClick: () => void }) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button variant="ghost" size="icon" className="size-8 shrink-0" aria-label="Close" onClick={onClick}>
          <X aria-hidden />
        </Button>
      </TooltipTrigger>
      <TooltipContent>Close</TooltipContent>
    </Tooltip>
  );
}

/**
 * The two banners of System Certificates, at the top of every window next to the update banner:
 * the notice that Weaver turned the setting on by itself because Zscaler is installed, and the
 * failure of an automatic export. Whether each shows lives in the backend, in the status it
 * sends to every window, so a window that opens later shows what the others show and closing one
 * closes it everywhere.
 *
 * `onOpenSettings` shows the Settings view; a window without it (the welcome window, where the
 * switch is already under the status box of omp) has no `Open Settings`.
 */
export function SystemCertificatesBanners({ onOpenSettings }: { onOpenSettings?: () => void }) {
  const { status, dismissNotice, dismissFailure } = useSystemCertificates();
  const notice = status?.turnOnNotice === true;
  // The banner stays while another export runs: its text is that of the failed line, with the reason it keeps.
  const failure =
    status?.failureBanner === true && status.failureReason !== null
      ? systemCertificatesLine({ ...status, phase: "failed", error: status.failureReason }, new Date())
      : null;
  if (!notice && !failure) return null;

  return (
    <div className="flex flex-col gap-2 p-3">
      {notice ? (
        <Alert role="status" aria-label="System Certificates turned on">
          <ShieldCheck aria-hidden />
          <p className="min-w-0 flex-1">{TURN_ON_TEXT}</p>
          {onOpenSettings ? (
            <Button
              variant="outline"
              size="sm"
              onClick={() => {
                dismissNotice();
                onOpenSettings();
              }}
            >
              Open Settings
            </Button>
          ) : null}
          <CloseButton onClick={dismissNotice} />
        </Alert>
      ) : null}
      {failure ? (
        <Alert variant="warning" aria-label="System Certificates export failed">
          <TriangleAlert aria-hidden />
          <p className="min-w-0 flex-1">{failure.text}</p>
          {onOpenSettings ? (
            <Button variant="outline" size="sm" onClick={onOpenSettings}>
              Open Settings
            </Button>
          ) : null}
          <CloseButton onClick={dismissFailure} />
        </Alert>
      ) : null}
    </div>
  );
}
