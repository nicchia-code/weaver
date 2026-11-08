import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";

export interface CardIdProps {
  /** The temporary short number (`P3`, `T3`, `D3`), when the card has one. */
  shortNumber?: string | null;
  /** The 8-character permanent id. */
  permanentId: string;
  className?: string;
}

/** What the id is called to assistive technologies and in the tooltip. */
export function idName(shortNumber: string | null | undefined, permanentId: string): string {
  return shortNumber ? `Short number (temporary) ${shortNumber}` : `Permanent ID ${permanentId}`;
}

/**
 * The id of a card: its short number when it has one, otherwise its permanent id, never both, in
 * monospace on a Paper tag (6 px radius). The accessible name and the tooltip say which of the two
 * it is.
 */
export function CardId({ shortNumber, permanentId, className }: CardIdProps) {
  const shown = shortNumber || permanentId;
  const name = idName(shortNumber, permanentId);
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <span role="img" aria-label={name} className={cn("inline-block rounded-sm bg-paper px-1.5 py-0.5 font-mono text-xs", className)}>
          <span aria-hidden>{shown}</span>
        </span>
      </TooltipTrigger>
      <TooltipContent>{name}</TooltipContent>
    </Tooltip>
  );
}
