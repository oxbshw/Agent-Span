import { Cpu } from "lucide-react";
import { GatewayOrb } from "../../components/GatewayOrb";
import { DashStat } from "../DashStat";
import { onlineCount, totalChannels, mcpTools, avgLatency, successRate } from "../../data/realData";

// Gateway status header with count-up stat cards.
export function StatusHeader() {
  return (
    <section
      className="dash-section dash-status"
      style={{ background: "#FFDFC4", color: "#070707" }}
      data-text="#070707"
      data-label="AGENTSPAN // STATUS"
    >
      <div className="dash-status-orb" aria-hidden="true">
        <GatewayOrb size={420} pulse={false} />
      </div>

      <div className="dash-status-head">
        <span className="eyebrow mono">[ AGENTSPAN // LIVE STATUS ]</span>
        <h1 className="mk-display">AGENTSPAN GATEWAY</h1>
        <p className="mk-lead">
          52 channels · 91 MCP tools · self-healing · Rust-powered. A live, read-only view of the
          gateway right now.
        </p>
      </div>

      <div className="dash-stat-row">
        <DashStat label="Channels Online" value={onlineCount} suffix={` / ${totalChannels}`} dot="online" />
        <DashStat label="MCP Tools" value={mcpTools} icon={Cpu} />
        <DashStat label="Avg Latency" value={avgLatency} suffix="ms" trend="up" />
        <DashStat label="Uptime" value={successRate} decimals={1} suffix="%" dot="online" />
      </div>
    </section>
  );
}
