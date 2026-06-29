import { useEffect, useRef } from "react";
import gsap from "gsap";
import { HeroDistortion } from "../components/HeroDistortion";
import { PillCTA } from "../components/PillCTA";
import { heroLabels, site } from "../data/content";

// Scattered coordinate tags (G.001…G.050), positioned around the figure.
const GRID_TAGS: { n: number; cls: string }[] = [
  { n: 2, cls: "t0" }, { n: 21, cls: "t1" }, { n: 5, cls: "t2" }, { n: 13, cls: "t3" },
  { n: 4, cls: "t4" }, { n: 15, cls: "t5" }, { n: 33, cls: "t6" }, { n: 41, cls: "t7" },
  { n: 9, cls: "t8" }, { n: 28, cls: "t9" }, { n: 47, cls: "t10" }, { n: 19, cls: "t11" },
];

// Screenshot / verification mode: render everything visible, no entrance tweens
// (headless browsers throttle rAF, which freezes gsap.from() mid-fade).
const STATIC =
  typeof location !== "undefined" &&
  (new URLSearchParams(location.search).has("static") ||
    new URLSearchParams(location.search).has("nolenis"));

export function Hero() {
  const ref = useRef<HTMLElement>(null);

  useEffect(() => {
    if (!ref.current || STATIC) return;
    const ctx = gsap.context(() => {
      gsap.from(".hero-canvas, .hero-photo", { scale: 1.08, opacity: 0, duration: 1.6, ease: "expo.out" });
      gsap.from(".hero-headline", { y: -24, opacity: 0, duration: 1, ease: "expo.out", delay: 0.2 });
      gsap.from(".hero-kicker", { opacity: 0, duration: 0.9, ease: "power2.out", delay: 0.5 });
      gsap.from(".hero-pitch > *", { y: 22, opacity: 0, duration: 0.8, stagger: 0.1, ease: "expo.out", delay: 0.55 });
      gsap.from(".hero-flank, .grid-tag, .plus-mark", { opacity: 0, duration: 1, delay: 0.7 });
    }, ref);
    return () => ctx.revert();
  }, []);

  return (
    <section ref={ref} id="hero" className="mk-section hero" data-bg="#FFDFC4" data-text="#070707" data-label="AGENTSPAN // INDEX">
      <HeroDistortion
        src="/hero/figure.jpg"
        alt="The AgentSpan gateway — a surreal coral figure whose head is an open ring routing a request through the sky"
      />
      <div className="hero-scrim" aria-hidden="true" />

      {GRID_TAGS.map((t) => (
        <span key={t.cls} className={`grid-tag mono ${t.cls}`}>G.{String(t.n).padStart(3, "0")}</span>
      ))}
      <span className="plus-mark pm-left" aria-hidden="true">+++</span>
      <span className="plus-mark pm-right" aria-hidden="true">+++</span>

      {/* Headline framed across the TOP so the figure stays clear */}
      <div className="hero-top">
        <h1 className="hero-headline">Web Access Gateway</h1>
        <p className="hero-kicker mono">THE OPEN-SOURCE GATEWAY THAT LETS AI AGENTS READ &amp; SEARCH THE WEB</p>
      </div>

      <div className="hero-flank is-left">
        {heroLabels.left.map((l) => <span key={l} className="mono">{l}</span>)}
      </div>
      <div className="hero-flank is-right">
        {heroLabels.right.map((l) => <span key={l} className="mono">{l}</span>)}
      </div>

      {/* Pitch + CTAs along the BOTTOM */}
      <div className="hero-pitch">
        <p className="hero-tagline">{site.description}</p>
        <div className="hero-cta-row">
          <PillCTA href="#channels">EXPLORE</PillCTA>
          <PillCTA href={site.githubUrl} variant="knob">GITHUB</PillCTA>
        </div>
      </div>
    </section>
  );
}
