import { Logo } from "../components/Logo";
import { site, stats } from "../data/content";

// Charcoal footer: brand, stats, links, sign-off.

export function Footer() {
  return (
    <section
      id="footer"
      className="mk-section footer"
      data-bg="#383534"
      data-text="#FFDFC4"
      data-no-texture
      data-label="AGENTSPAN // BOTTOM"
    >
      <div className="footer-top">
        <div className="footer-brand">
          <Logo size={26} color="#FFDFC4" />
          <p>{site.tagline}</p>
        </div>
        <div className="footer-stats">
          {stats.map((s) => (
            <div key={s.label}>
              <span className="footer-stat-val">{s.value}</span>
              <span className="footer-stat-label mono">{s.label}</span>
            </div>
          ))}
        </div>
      </div>

      <div className="footer-links mono">
        <a href={site.githubUrl} target="_blank" rel="noreferrer">GITHUB ›</a>
        <a href={site.docsUrl} target="_blank" rel="noreferrer">DOCS ›</a>
        <a href={site.cratesUrl} target="_blank" rel="noreferrer">CRATES.IO ›</a>
        <a href="#hero">BACK TO TOP ›</a>
      </div>

      <div className="footer-base mono">
        <span>MIT LICENSED — BUILT WITH RUST</span>
        <span>© {site.year} {site.name.toUpperCase()} — V{site.version}</span>
      </div>
    </section>
  );
}
