import { ChevronDown } from "lucide-react";
import { useMemo, useState } from "react";

import { activeReasons, GuardedButton, type Reason } from "@/components/guarded-button";
import { modelCatalogOf } from "@/components/omp-status";
import { ModelSetting } from "@/components/settings-view";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { useOmp } from "@/hooks/use-omp";
import { useSettings } from "@/hooks/use-settings";

const DEFAULT_MODEL = "Default model";
const TOOLTIP = "The model of Chat and of Ask more; it applies to the next message.";

/**
 * The model control of the message box: a ghost button with the analysis model of the Settings
 * (`Default model` when it is empty) that opens the same choice as the `Analysis` setting. A choice
 * is that setting's change, saved at once for every project; the agent of Chat takes it at its next
 * start. `reasons` say why it is disabled, one per line.
 */
export function ModelControl({ reasons }: { reasons: readonly Reason[] }) {
  const { snapshot } = useSettings();
  const { status } = useOmp();
  const catalog = useMemo(() => modelCatalogOf(status), [status]);
  const [open, setOpen] = useState(false);
  const stored = snapshot?.settings.models.analysis.trim() ?? "";
  const blocked = activeReasons(reasons).length > 0;
  return (
    <Popover open={open && !blocked} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <GuardedButton variant="ghost" size="sm" className="max-w-64 gap-1" tooltip={TOOLTIP} reasons={reasons}>
          <span className="truncate">{stored === "" ? DEFAULT_MODEL : stored}</span>
          <ChevronDown aria-hidden className="!size-3" />
        </GuardedButton>
      </PopoverTrigger>
      <PopoverContent align="end" aria-label="Model of Chat" className="w-[420px]">
        <ModelSetting settingKey="models.analysis" catalog={catalog} />
      </PopoverContent>
    </Popover>
  );
}
