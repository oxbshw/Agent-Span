import { Globe, Cpu, HeartPulse, Database, Shield, Brain } from "lucide-react";
import { features, site } from "../data/content";

// The six capabilities on the pink panel.

const ICONS: Record<string, typeof Globe> = { Globe, Cpu, HeartPulse, Database, Shield, Brain };

export function Features() {
  return (
    <section
      id="features"
      className="mk-section features"
      data-bg="#E8A5F3"
      data-text="#070707"
      data-no-texture
      data-label="AGENTSPAN // FEATURES"
    >
      <div className="features-head">
        <span className="eyebrow mono">[ {site.name} // CAPABILITIES ]</span>
        <h2 className="mk-display">READ THE WEB</h2>
        <p className="mk-lead">
          Everything an agent needs to reach the web and keep reaching it — one REST API, one MCP server,
          9 client SDKs, and a router that heals itself the moment a backend breaks.
        </p>
      </div>

      <div className="feature-grid">
        {features.map((f, i) => {
          const Icon = ICONS[f.icon] ?? Globe;
          return (
            <article key={f.title} className="feature-tile">
              <span className="feature-index mono">0{i + 1}</span>
              <span className="feature-ico"><Icon size={22} strokeWidth={1.6} /></span>
              <h3>{f.title}</h3>
              <p>{f.description}</p>
            </article>
          );
        })}
      </div>
    </section>
  );
}
