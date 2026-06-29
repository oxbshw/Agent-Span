import { architecture, site } from "../data/content";

// A routing-orbit diagram (gateway core + pink orbits) beside the 9 Rust crates.

function OrbitDiagram() {
  const orbits = [44, 62, 80, 96];
  return (
    <svg className="orbit-svg" viewBox="0 0 220 220" aria-hidden="true">
      <defs>
        <radialGradient id="blob-grad" cx="0.4" cy="0.35" r="0.8">
          <stop offset="0" stopColor="#5a5550" />
          <stop offset="0.7" stopColor="#2f2c2a" />
          <stop offset="1" stopColor="#161413" />
        </radialGradient>
      </defs>
      <g className="orbit-rings">
        {orbits.map((r, i) => (
          <ellipse
            key={r}
            cx="110"
            cy="110"
            rx={r}
            ry={r * 0.52}
            fill="none"
            stroke="#E8A5F3"
            strokeWidth="0.7"
            opacity={0.7 - i * 0.12}
            transform={`rotate(${-22 + i * 6} 110 110)`}
          />
        ))}
      </g>
      {/* central gateway core (the blob) */}
      <circle cx="110" cy="110" r="30" fill="url(#blob-grad)" />
      <circle cx="110" cy="110" r="30" fill="none" stroke="#E8A5F3" strokeWidth="0.8" opacity="0.6" />
      {/* orbiting crate nodes — counter-rotate so they drift around the core */}
      <g className="orbit-nodes">
        {[0, 1, 2, 3, 4, 5].map((i) => {
          const a = (i / 6) * Math.PI * 2;
          const x = 110 + Math.cos(a) * 80;
          const y = 110 + Math.sin(a) * 42;
          return <circle key={i} cx={x} cy={y} r="3.4" fill="#39322d" />;
        })}
      </g>
    </svg>
  );
}

export function Architecture() {
  return (
    <section
      id="architecture"
      className="mk-section architecture"
      data-bg="#CFB8FF"
      data-text="#070707"
      data-label="AGENTSPAN // ARCHITECTURE"
    >
      <div className="arch-grid">
        <div className="arch-copy">
          <span className="eyebrow mono">[ {site.name} // UNDER THE HOOD ]</span>
          <h2 className="mk-display">BUILT IN RUST</h2>
          <p className="mk-lead">
            Nine async crates, one gateway. Every request is probed, routed to the healthiest
            backend, cached, and observed.
          </p>
          <ul className="crate-list">
            {architecture.map((c) => (
              <li key={c.name}>
                <span className="crate-name mono">{c.name}</span>
                <span className="crate-desc">{c.desc}</span>
              </li>
            ))}
          </ul>
        </div>
        <div className="arch-visual">
          <OrbitDiagram />
        </div>
      </div>
    </section>
  );
}
