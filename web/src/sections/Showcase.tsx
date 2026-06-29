import { WaveWordmark } from "../components/WaveWordmark";
import { PillCTA } from "../components/PillCTA";
import { showcaseStats, installSteps, site } from "../data/content";

// Charcoal section: stats, the AGENTSPAN waveform wordmark, and the install snippet.

export function Showcase() {
  return (
    <section
      id="showcase"
      className="mk-section showcase"
      data-bg="#383534"
      data-text="#FFDFC4"
      data-no-texture
      data-label="AGENTSPAN // SHOWCASE"
    >
      <div className="showcase-stats">
        {showcaseStats.map((s) => (
          <div key={s.label} className="showcase-stat">
            <span className="showcase-stat-val">{s.value}</span>
            <span className="showcase-stat-label mono">{s.label}</span>
          </div>
        ))}
      </div>

      <span className="eyebrow mono showcase-eyebrow">[ {site.name} // SIMPLE AS HTTP ]</span>

      <div className="showcase-wordmark">
        <WaveWordmark text="AGENTSPAN" />
      </div>

      <div className="showcase-install">
        <pre className="install-block">
          {installSteps.map((s, i) => (
            <span key={i} className="install-line">
              <span className="c">{s.c}</span>{"\n"}
              <span className="p">{s.p}</span>{s.rest}{i < installSteps.length - 1 ? "\n\n" : ""}
            </span>
          ))}
        </pre>
        <PillCTA href={site.githubUrl} variant="knob">INSTALL AGENTSPAN</PillCTA>
      </div>
    </section>
  );
}
