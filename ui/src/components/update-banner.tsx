import { Download } from "lucide-react";
import { useEffect, useState } from "react";

import type { UpdateStatus } from "@/bindings/UpdateStatus";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { call, isAppError, on } from "@/lib/backend";

const NOTHING: UpdateStatus = { available: null, downloading: false };

/**
 * The update the backend found, with its version and notes. It installs only when the developer
 * chooses **Install and restart** (the backend then quits Weaver through its usual quit path, with
 * its confirmation when jobs are working); **Later** hides it until the next check. A check that
 * failed is never shown: the backend only logs it.
 */
export function UpdateBanner() {
  const [status, setStatus] = useState<UpdateStatus>(NOTHING);
  const [failure, setFailure] = useState<string | null>(null);

  useEffect(() => {
    let current = true;
    // Subscribe first, then ask: a change between the two is not lost.
    const unlisten = on("update-changed", (incoming) => current && setStatus(incoming));
    call("update_status")
      .then((known) => current && setStatus(known?.available !== undefined ? known : NOTHING))
      .catch(() => {});
    return () => {
      current = false;
      unlisten.then((stop) => stop()).catch(() => {});
    };
  }, []);

  const update = status.available;
  if (!update) return null;

  const install = () => {
    setFailure(null);
    call("update_install").catch((error: unknown) =>
      setFailure(isAppError(error) ? error.message : "The update could not be installed."),
    );
  };
  const later = () => {
    setFailure(null);
    void call("update_later").catch(() => {});
  };

  return (
    <div className="flex flex-col gap-2 p-3">
      <Alert role="status" aria-label="Update available">
        <Download aria-hidden />
        <div className="flex min-w-0 flex-1 flex-col gap-1">
          <p className="font-medium">{`Weaver ${update.version} is available.`}</p>
          {update.notes ? (
            <p className="max-h-32 overflow-auto whitespace-pre-wrap text-muted-foreground">{update.notes}</p>
          ) : null}
          {failure ? <p className="text-rose-ink">{failure}</p> : null}
        </div>
        <Button size="sm" aria-disabled={status.downloading} onClick={() => !status.downloading && install()}>
          {status.downloading ? "Downloading…" : "Install and restart"}
        </Button>
        <Button variant="outline" size="sm" aria-disabled={status.downloading} onClick={() => !status.downloading && later()}>
          Later
        </Button>
      </Alert>
    </div>
  );
}
