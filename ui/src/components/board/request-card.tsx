import { SendHorizontal, Trash2 } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";

import { registerCardRenderer, type BoardCardOf } from "@/components/board/card-registry";
import { CardShell } from "@/components/board/card-shell";
import { ConfirmDialog } from "@/components/dialogs";
import { GuardedButton } from "@/components/guarded-button";
import { startedText } from "@/components/jobs/format";
import { useProject } from "@/components/project-context";
import { useRequests } from "@/components/requests/use-requests";
import { call, isAppError } from "@/lib/backend";

type RequestCardData = BoardCardOf<"request">;

/** `Send` (primary) and `Discard` (outline, confirmed): the request editor's controls, on the card. */
function RequestControls({ card }: { card: RequestCardData }) {
  const { projectId } = useProject();
  const [confirming, setConfirming] = useState(false);

  const send = () => {
    call("request_send", { projectId, requestId: card.id }).catch((error: unknown) => {
      toast.error(isAppError(error) ? error.message : "The request could not be sent.");
    });
  };
  const discard = () => {
    call("request_discard", { projectId, requestId: card.id }).catch((error: unknown) => {
      toast.error(isAppError(error) ? error.message : "The request could not be discarded.");
    });
  };

  return (
    <>
      <GuardedButton size="sm" reasons={[card.empty && "Write the request first."]} onClick={send}>
        <SendHorizontal aria-hidden />
        Send
      </GuardedButton>
      <GuardedButton variant="outline" size="sm" onClick={() => setConfirming(true)}>
        <Trash2 aria-hidden />
        Discard
      </GuardedButton>
      <div onClick={(event) => event.stopPropagation()}>
        <ConfirmDialog
          open={confirming}
          onOpenChange={setConfirming}
          title="Discard this request?"
          description="Its file and images are deleted."
          confirmLabel="Discard"
          destructive
          onConfirm={discard}
        />
      </div>
    </>
  );
}

/**
 * The card of a request being written (`requirements/ui#design-request`): the tag `Request` where
 * the other cards show their id, the title, `Not sent · <time>` in Muted and the controls. It has
 * no details dialog: a click on it, or Enter on its title, opens the request in the editor.
 */
function RequestCard({ card }: { card: RequestCardData }) {
  const { openEditor } = useRequests();
  return (
    <CardShell
      cardId={card.id}
      shortNumber={null}
      tag="Request"
      title={card.title}
      summary={<p className="text-xs text-muted-foreground">Not sent · {startedText(card.created)}</p>}
      controls={<RequestControls card={card} />}
      onOpen={() => openEditor(card.id)}
    />
  );
}

registerCardRenderer("request", { Card: RequestCard });
