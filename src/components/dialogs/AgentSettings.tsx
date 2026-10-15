import { useEffect, useState } from "react";
import { api } from "../../lib/api";
import { attempt, useStore } from "../../lib/store";
import type { Agent, AgentStatus } from "../../lib/types";
import { Button, SettingRow } from "../ui";

const NAMES: Record<Agent, string> = { claude: "Claude Code", codex: "Codex", grok: "Grok Build" };

/** Connects coding agents on this Mac to Redrule's notes, and gives cloud agents a shared folder's address. */
export function AgentSettings() {
  const [statuses, setStatuses] = useState<AgentStatus[]>([]);
  const [connecting, setConnecting] = useState<Agent | null>(null);
  const folders = useStore((s) => s.app?.folders ?? []).filter((f) => !f.unavailable);
  useEffect(() => void attempt(() => api.agentStatuses().then(setStatuses)), []);

  const connect = async (agent: Agent) => {
    setConnecting(agent);
    await attempt(() => api.connectAgent(agent).then(setStatuses), `${NAMES[agent]} can now read your notes`);
    setConnecting(null);
  };

  return (
    <div>
      <p className="pb-1 text-[12px] leading-relaxed text-graphite">
        Agents can list, search and read your notes and transcripts. They can't change or delete anything.
      </p>
      <div className="divide-y divide-rule">
        {statuses.map(({ agent, connected }) => (
          <SettingRow key={agent} title={NAMES[agent]} detail={connected ? "Connected" : "Not connected"}>
            <Button size="sm" variant="ghost" onClick={() => void attempt(() => api.copyAgentCommand(agent), "Command copied")}>
              Copy Command
            </Button>
            <Button size="sm" disabled={connected || connecting !== null} onClick={() => void connect(agent)}>
              {connecting === agent ? "Connecting…" : connected ? "Connected" : "Connect"}
            </Button>
          </SettingRow>
        ))}
      </div>

      <h3 className="mt-5 text-[11.5px] font-semibold text-graphite">Grok Bot and other cloud agents</h3>
      <p className="py-1 text-[12px] leading-relaxed text-graphite">
        Cloud agents can't reach this Mac, but they can read a shared folder at any time. Copy a folder's address and add it
        as a custom connector at grok.com/connectors. Anyone with the address can read the folder.
      </p>
      <div className="divide-y divide-rule">
        {folders.length === 0 && <SettingRow title="No shared folders" detail="Create a shared folder in the sidebar, then move meetings into it." />}
        {folders.map((folder) => (
          <SettingRow key={folder.id} title={folder.name}>
            <Button size="sm" onClick={() => void attempt(() => api.copyFolderConnector(folder.id), "Connector address copied")}>
              Copy Address
            </Button>
          </SettingRow>
        ))}
      </div>
    </div>
  );
}
