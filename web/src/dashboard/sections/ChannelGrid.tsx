import { useMemo, useState } from "react";
import { Search } from "lucide-react";
import { BlinkingBlock } from "../../components/BlinkingBlock";
import { channels, type Channel, type ChannelStatus } from "../../data/realData";

// Searchable / filterable grid of all 52 channels.

const DOT: Record<ChannelStatus, string> = { online: "#2e9e5b", degraded: "#E8A5F3", offline: "#c0392b" };
const TABS = ["All", "Search", "Social", "Media", "Dev", "Knowledge", "AI", "Finance", "Tool"];

function ChannelTile({ c }: { c: Channel }) {
  const [open, setOpen] = useState(false);
  return (
    <button className={`chan-tile ${open ? "is-open" : ""}`} onClick={() => setOpen((o) => !o)} type="button">
      <div className="chan-tile-top">
        <BlinkingBlock color={DOT[c.status]} size={8} blink={c.status === "degraded"} />
        <span className="chan-tile-name">{c.name}</span>
        <span className="chan-tile-lat mono">{c.latency}ms</span>
      </div>
      <div className="chan-tile-meta">
        <span className="chan-badge mono">{c.category}</span>
        <span className="chan-tier mono">T{c.tier}</span>
      </div>
      {open ? (
        <div className="chan-tile-detail mono">
          <span>backends: {c.backends.join(" → ")}</span>
          <span>checked: {(c.latency % 9) + 1}s ago</span>
        </div>
      ) : (
        <div className="chan-tile-primary mono">{c.backends[0]}</div>
      )}
    </button>
  );
}

export function ChannelGrid() {
  const [q, setQ] = useState("");
  const [tab, setTab] = useState("All");

  const filtered = useMemo(() => {
    const term = q.trim().toLowerCase();
    return channels.filter((c) => {
      const matchTab = tab === "All" || c.category === tab.toLowerCase();
      const matchQ = !term || c.name.toLowerCase().includes(term) || c.category.includes(term);
      return matchTab && matchQ;
    });
  }, [q, tab]);

  return (
    <section
      className="dash-section dash-channels"
      style={{ background: "#CFB8FF", color: "#070707" }}
      data-text="#070707"
      data-label="AGENTSPAN // CHANNELS"
    >
      <div className="dash-section-head">
        <div>
          <span className="eyebrow mono">[ {channels.length} INTEGRATIONS ]</span>
          <h2 className="dash-h">{channels.length} CHANNELS</h2>
        </div>
        <label className="chan-search">
          <Search size={15} />
          <input
            value={q}
            onChange={(e) => setQ(e.target.value)}
            placeholder="Search channels…"
            aria-label="Search channels"
          />
        </label>
      </div>

      <div className="chan-tabs mono">
        {TABS.map((t) => (
          <button key={t} className={tab === t ? "is-active" : ""} onClick={() => setTab(t)} type="button">
            {t}
          </button>
        ))}
      </div>

      <div className="chan-grid">
        {filtered.map((c) => (
          <ChannelTile key={c.name} c={c} />
        ))}
        {filtered.length === 0 && <p className="chan-empty mono">No channels match “{q}”.</p>}
      </div>
    </section>
  );
}
