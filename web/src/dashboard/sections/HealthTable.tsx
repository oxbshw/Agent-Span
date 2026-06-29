import { AlertTriangle } from "lucide-react";
import { BlinkingBlock } from "../../components/BlinkingBlock";
import {
  healthSnapshots, onlineCount, degradedCount, offlineCount, type ChannelStatus,
} from "../../data/realData";

// Backend health monitor — compact table with fallback status + alert.

const DOT: Record<ChannelStatus, string> = { online: "#43d17f", degraded: "#E8A5F3", offline: "#ff6b6b" };
const LABEL: Record<ChannelStatus, string> = { online: "Online", degraded: "Degraded", offline: "Offline" };

export function HealthTable() {
  const rows = healthSnapshots.slice(0, 10);
  const degraded = healthSnapshots.filter((s) => s.status === "degraded").map((s) => s.channel);

  return (
    <section
      className="dash-section dash-health"
      style={{ background: "#383534", color: "#FFDFC4" }}
      data-text="#FFDFC4"
      data-no-texture
      data-label="AGENTSPAN // HEALTH"
    >
      <div className="dash-section-head">
        <div>
          <span className="eyebrow mono" style={{ color: "#FFDFC4" }}>[ BACKEND MONITOR ]</span>
          <h2 className="dash-h">HEALTH MONITOR</h2>
        </div>
      </div>

      {degraded.length > 0 && (
        <div className="health-alert mono">
          <AlertTriangle size={15} />
          <span>{degraded.join(", ")} {degraded.length > 1 ? "are" : "is"} running on a fallback backend</span>
        </div>
      )}

      <div className="health-table-wrap">
        <table className="health-table">
          <thead className="mono">
            <tr>
              <th>Channel</th><th>Status</th><th>Primary Backend</th><th>Fallback Active?</th><th>Last Check</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((r) => (
              <tr key={r.channel}>
                <td className="health-chan">{r.channel}</td>
                <td>
                  <span className="health-status">
                    <BlinkingBlock color={DOT[r.status]} size={8} blink={r.status === "degraded"} />
                    {LABEL[r.status]}
                  </span>
                </td>
                <td className="mono health-backend">{r.primary}</td>
                <td>
                  <span className={`health-badge ${r.fallbackActive ? "is-yes" : "is-no"} mono`}>
                    {r.fallbackActive ? "YES" : "no"}
                  </span>
                </td>
                <td className="mono health-check">{r.lastCheck}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <div className="health-summary mono">
        <span><b>{onlineCount}</b> online</span>
        <span className="sep">·</span>
        <span><b>{degradedCount}</b> degraded</span>
        <span className="sep">·</span>
        <span><b>{offlineCount}</b> offline</span>
      </div>
    </section>
  );
}
