import { GatewayOrb } from "../components/GatewayOrb";
import { PillCTA } from "../components/PillCTA";
import { channelCategories, site } from "../data/content";
import { channels } from "../data/realData";

export function Channels() {
  const online = channels.filter((c) => c.status === "online").length;

  return (
    <section
      id="channels"
      className="mk-section channels"
      data-bg="#FFDFC4"
      data-text="#070707"
      data-label="AGENTSPAN // CHANNELS"
    >
      <div className="chan-decor is-left" aria-hidden="true">
        <span className="cd-square" /><span className="cd-square is-fill" /><span className="cd-square" />
        <span className="cd-cross">+</span>
        <span className="cd-line" />
      </div>
      <div className="chan-decor is-right" aria-hidden="true">
        {Array.from({ length: 7 }).map((_, i) => <span key={i} className="cd-bar" style={{ width: `${30 + (i % 4) * 18}px` }} />)}
      </div>

      <div className="channels-top">
        <div className="channel-portal">
          <div className="channel-portal-orb"><GatewayOrb size={420} pulse={false} /></div>
          <span className="channel-portal-ring" />
        </div>
        <div className="channels-head">
          <span className="eyebrow mono">[ {site.name} // INTEGRATIONS ]</span>
          <h2 className="mk-display">52 CHANNELS</h2>
          <p className="mk-lead">
            A <em>channel</em> is one unified endpoint for a web service. AgentSpan puts {channels.length} of
            them — search engines, social platforms, cloud providers, dev tools and knowledge bases — behind
            a single HTTP API and MCP server, so your agent calls them all the same way.
            <span className="mono"> {online}/{channels.length} ONLINE</span>.
          </p>
          <PillCTA href="#network" variant="knob">EXPLORE</PillCTA>
        </div>
      </div>

      <div className="chan-markers mono">
        {channelCategories.map((c) => <span key={c.no}>{c.no}</span>)}
      </div>

      <div className="channel-cards">
        {channelCategories.map((c) => (
          <article key={c.no} className="channel-card">
            <header>
              <span className="channel-card-no mono">{c.no}</span>
              <span className="channel-card-count mono">{c.count} CH</span>
            </header>
            <h3>{c.name}</h3>
            <ul className="channel-card-list mono">
              {c.list.map((n) => <li key={n}>{n}</li>)}
            </ul>
            <span className="channel-card-bar" />
          </article>
        ))}
      </div>
    </section>
  );
}
