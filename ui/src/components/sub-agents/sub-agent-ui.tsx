import { Bot, Check, Loader, Square, X } from "lucide-react";
import { createContext, useContext } from "react";

import type { AgentNode } from "@/bindings/AgentNode";
import type { Step } from "@/bindings/Step";
import type { SubAgent } from "@/bindings/SubAgent";
import type { SubAgentState } from "@/bindings/SubAgentState";
import { startedText } from "@/components/jobs/format";
import { samePath } from "@/components/sub-agents/api";

/**
 * What a sub-agent line in a conversation does when clicked: opens that sub-agent's conversation
 * and selects it in the list. `base` is the path of the conversation the line is in (empty for the
 * owner's own conversation). Without a provider, the lines are plain text.
 */
export interface SubAgentNav {
  base: readonly Step[];
  open: (path: Step[]) => void;
}

const NavContext = createContext<SubAgentNav | null>(null);

export const SubAgentNavProvider = NavContext.Provider;

/** A sub-agent's state as an icon and its word: `working`, `succeeded`, `failed`, `interrupted`. */
export function AgentState({ state }: { state: SubAgentState }) {
  const Icon = state === "working" ? Loader : state === "succeeded" ? Check : state === "failed" ? X : Square;
  const colour = state === "failed" ? "text-rose-ink" : state === "succeeded" ? "text-sage-ink" : state === "working" ? "text-sky-ink" : "text-muted-foreground";
  return (
    <span className={`flex shrink-0 items-center gap-1 text-xs ${colour}`}>
      <Icon aria-hidden className={`size-3 ${state === "working" ? "animate-spin motion-reduce:animate-none" : ""}`} />
      {state}
    </span>
  );
}

/** One sub-agent under the `task` row `call` of a conversation: a button that opens its conversation. */
export function SubAgentLine({ call, agent }: { call: string; agent: SubAgent }) {
  const nav = useContext(NavContext);
  const content = (
    <>
      <Bot aria-hidden className="size-3 shrink-0" />
      <span className="truncate" title={agent.name}>
        {agent.name}
      </span>
      <span className="ml-2">
        <AgentState state={agent.state} />
      </span>
    </>
  );
  if (!nav) return <div className="ml-8 flex items-center gap-2 text-xs">{content}</div>;
  return (
    <button
      type="button"
      className="ml-8 flex w-[calc(100%-2rem)] items-center gap-2 rounded-md px-1 text-left text-xs hover:bg-accent"
      onClick={() => nav.open([...nav.base, { call, id: agent.id }])}
    >
      {content}
    </button>
  );
}

/** The indented list of the sub-agents under the selected job's row. */
export function SubAgentList({
  nodes,
  selected,
  onSelect,
  parent = [],
}: {
  nodes: readonly AgentNode[];
  selected: readonly Step[] | null;
  onSelect: (path: Step[]) => void;
  parent?: readonly Step[];
}) {
  const sameName = (name: string) => nodes.filter((node) => node.name === name).length > 1;
  return (
    <ul aria-label={parent.length === 0 ? "Sub-agents" : undefined} className={parent.length === 0 ? "pb-1 pl-4" : "pl-4"}>
      {nodes.map((node) => {
        const path = [...parent, { call: node.call, id: node.id }];
        return (
          <li key={`${node.call}/${node.id}`}>
            <button
              type="button"
              aria-current={selected && samePath(selected, path) ? "true" : undefined}
              className="flex w-full items-center gap-2 rounded-md px-2 py-1 text-left text-sm hover:bg-accent aria-[current=true]:bg-accent"
              onClick={() => onSelect(path)}
            >
              <Bot aria-hidden className="size-3 shrink-0" />
              <span className="min-w-0 truncate" title={node.name}>
                {node.name}
              </span>
              {node.started && sameName(node.name) && (
                <span className="shrink-0 text-xs text-muted-foreground">{startedText(node.started).slice(-5)}</span>
              )}
              <span className="ml-auto">
                <AgentState state={node.state} />
              </span>
            </button>
            {node.children.length > 0 && <SubAgentList nodes={node.children} selected={selected} onSelect={onSelect} parent={path} />}
          </li>
        );
      })}
    </ul>
  );
}
