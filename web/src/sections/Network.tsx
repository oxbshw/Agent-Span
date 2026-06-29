import { ChannelProfile } from "../components/ChannelProfile";
import { BlinkingBlock } from "../components/BlinkingBlock";
import { channels, type ChannelStatus } from "../data/realData";
import { site } from "../data/content";

const DOT: Record<ChannelStatus, string> = { online: "#43d17f", degraded: "#E8A5F3", offline: "#ff6b6b" };

export function Network() {
  return (
    <section
      id="network"
      className="mk-section network"
      data-bg="#383534"
      data-text="#FFDFC4"
      data-no-texture
      data-label="AGENTSPAN // NETWORK"
    >
      <div className="net-head">
        <span className="eyebrow mono">[ {channels.length} CHANNELS · ONE GATEWAY ]</span>
        <h2 className="mk-display">THE NETWORK</h2>
        <span className="net-plus mono" aria-hidden="true">+++ ———— +++</span>
      </div>

      <div className="network-grid">
        {channels.map((c) => (
          <button
            key={c.name}
            className="network-cell"
            type="button"
            onClick={() =>
              window.alert(
                `${c.name.toUpperCase()}\n\nTier ${c.tier} · ${c.category}\nStatus: ${c.status} · ${c.latency}ms\nBackends: ${c.backends.join(" → ")}`,
              )
            }
          >
            <span className="network-av"><ChannelProfile channel={c.name} size={104} /></span>
            <span className="network-name mono">{c.name}</span>
            <span className="network-meta">
              <BlinkingBlock color={DOT[c.status]} size={7} blink={c.status === "degraded"} />
              <span className="network-cat mono">{c.category}</span>
            </span>
          </button>
        ))}
      </div>

      <div className="net-foot mono">{site.name.toUpperCase()} · {channels.length} UNIQUE ENDPOINTS</div>
    </section>
  );
}
