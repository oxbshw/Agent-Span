import { useEffect, useRef, useState } from "react";
import Lenis from "lenis";
import gsap from "gsap";
import { ScrollTrigger } from "gsap/ScrollTrigger";
import { Preloader } from "../components/Preloader";
import { Chrome } from "../components/Chrome";
import { Nav } from "../components/Nav";
import { Hero } from "./Hero";
import { Channels } from "./Channels";
import { Network } from "./Network";
import { Architecture } from "./Architecture";
import { Features } from "./Features";
import { Showcase } from "./Showcase";
import { Footer } from "./Footer";

// The whole site is the landing experience: preloader → hero → channels →
// network → architecture → features → showcase → footer. A `.change-bg` layer
// cross-fades the section colors (1s) and a centered IntersectionObserver drives
// the active background, text color and nav indicator.

const STATIC = typeof location !== "undefined" && new URLSearchParams(location.search).has("static");

gsap.registerPlugin(ScrollTrigger);

// Elements that fade + rise into view as their section scrolls in (staggered).
const REVEAL_SELECTOR = [
  ".channels-top > *", ".channel-cards > *", ".chan-markers",
  ".net-head > *", ".network-cell",
  ".arch-copy > *", ".crate-list > li", ".arch-visual",
  ".features-head > *", ".feature-grid > *",
  ".showcase > *", ".footer-top", ".footer-links", ".footer-base",
].join(", ");

export function MarketingSite() {
  const [entered, setEntered] = useState(STATIC);
  const [bg, setBg] = useState("#FFDFC4");
  const [text, setText] = useState("#070707");
  const [label, setLabel] = useState("AGENTSPAN // INDEX");
  const rootRef = useRef<HTMLDivElement>(null);

  // Smooth scroll (Lenis) + GSAP ScrollTrigger (scroll reveals + pinned
  // Architecture), started once the preloader is gone.
  useEffect(() => {
    if (!entered) return;
    // Escape hatch: ?nolenis / ?static render with native scroll and no scroll
    // animations (everything visible) — used for screenshots/verification.
    if (STATIC || new URLSearchParams(location.search).has("nolenis")) return;

    const lenis = new Lenis({ duration: 1.1, smoothWheel: true });
    lenis.on("scroll", ScrollTrigger.update);
    const tick = (time: number) => lenis.raf(time * 1000);
    gsap.ticker.add(tick);
    gsap.ticker.lagSmoothing(0);
    document.documentElement.classList.add("lenis", "lenis-smooth");

    // Route in-page anchor clicks through Lenis for smooth section jumps.
    const onClick = (e: MouseEvent) => {
      const a = (e.target as HTMLElement).closest<HTMLAnchorElement>('a[href^="#"]');
      if (!a) return;
      const href = a.getAttribute("href");
      if (!href || href === "#") return;
      const target = document.querySelector(href);
      if (target) {
        e.preventDefault();
        lenis.scrollTo(target as HTMLElement, { offset: 0 });
      }
    };
    document.addEventListener("click", onClick);

    const ctx = gsap.context(() => {
      // Staggered scroll-reveal for every section after the hero.
      gsap.set(REVEAL_SELECTOR, { opacity: 0, y: 26 });
      ScrollTrigger.batch(REVEAL_SELECTOR, {
        start: "top 88%",
        onEnter: (b) =>
          gsap.to(b, { opacity: 1, y: 0, duration: 0.7, stagger: 0.07, ease: "power2.out", overwrite: true }),
      });
      // Pin the Architecture section for an extra viewport (cinematic dwell).
      ScrollTrigger.create({ trigger: ".architecture", start: "top top", end: "+=100%", pin: true, pinSpacing: true });
    }, rootRef);

    ScrollTrigger.refresh();

    return () => {
      ctx.revert();
      gsap.ticker.remove(tick);
      lenis.destroy();
      document.removeEventListener("click", onClick);
      document.documentElement.classList.remove("lenis", "lenis-smooth");
    };
  }, [entered]);

  // Deep-link: scroll to #section on load once the page is laid out.
  useEffect(() => {
    if (!entered || !location.hash) return;
    const id = setTimeout(() => document.querySelector(location.hash)?.scrollIntoView(), 120);
    return () => clearTimeout(id);
  }, [entered]);

  // Active-section observer: the section crossing the viewport center wins.
  useEffect(() => {
    if (!entered) return;
    const sections = rootRef.current?.querySelectorAll<HTMLElement>(".mk-section");
    if (!sections) return;
    const obs = new IntersectionObserver(
      (entries) => {
        for (const e of entries) {
          if (e.isIntersecting) {
            const el = e.target as HTMLElement;
            setBg(el.dataset.bg || "#FFDFC4");
            setText(el.dataset.text || "#070707");
            setLabel(el.dataset.label || "AGENTSPAN");
          }
        }
      },
      { rootMargin: "-50% 0px -50% 0px", threshold: 0 },
    );
    sections.forEach((s) => obs.observe(s));
    return () => obs.disconnect();
  }, [entered]);

  return (
    <>
      {!entered && <Preloader onDone={() => setEntered(true)} />}

      <div
        ref={rootRef}
        className="marketing"
        style={{ ["--mk-text" as string]: text, ["--mk-bg" as string]: bg, color: text }}
      >
        <div className="change-bg" style={{ backgroundColor: bg }} />
        <Chrome />
        <Nav label={label} color={text} />

        <main className="mk-main">
          <Hero />
          <Channels />
          <Network />
          <Architecture />
          <Features />
          <Showcase />
          <Footer />
        </main>
      </div>
    </>
  );
}
