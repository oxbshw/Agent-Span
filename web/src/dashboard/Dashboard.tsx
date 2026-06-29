import { useEffect, useRef, useState } from "react";
import Lenis from "lenis";
import gsap from "gsap";
import { ScrollTrigger } from "gsap/ScrollTrigger";
import { Chrome } from "../components/Chrome";
import { Nav } from "../components/Nav";
import { StatusHeader } from "./sections/StatusHeader";
import { ChannelGrid } from "./sections/ChannelGrid";
import { PerformanceCharts } from "./sections/PerformanceCharts";
import { HealthTable } from "./sections/HealthTable";
import { QuickActions } from "./sections/QuickActions";
import { InstallSnippet } from "./sections/InstallSnippet";

// The /status page — a read-only gateway overview that shares the landing's
// design language (same chrome, fonts, colors, animations). Single scrollable
// page; each band carries its own background, the chrome text flips on the dark
// sections via an IntersectionObserver.

gsap.registerPlugin(ScrollTrigger);

const STATIC = typeof location !== "undefined" && new URLSearchParams(location.search).has("static");

const DASH_REVEAL = [
  ".dash-status-head > *", ".dash-stat-row > *", ".dash-section-head", ".chan-search",
  ".chan-tabs", ".chan-grid", ".chart-card", ".action-card", ".code-card",
  ".health-alert", ".health-table-wrap", ".health-summary",
].join(", ");

export function Dashboard() {
  const [text, setText] = useState("#070707");
  const [label, setLabel] = useState("AGENTSPAN // STATUS");
  const rootRef = useRef<HTMLDivElement>(null);

  // Active-section observer → flips chrome text color + nav indicator.
  useEffect(() => {
    const sections = rootRef.current?.querySelectorAll<HTMLElement>(".dash-section");
    if (!sections) return;
    const obs = new IntersectionObserver(
      (entries) => {
        for (const e of entries) {
          if (e.isIntersecting) {
            const el = e.target as HTMLElement;
            setText(el.dataset.text || "#070707");
            setLabel(el.dataset.label || "AGENTSPAN");
          }
        }
      },
      { rootMargin: "-45% 0px -45% 0px", threshold: 0 },
    );
    sections.forEach((s) => obs.observe(s));
    return () => obs.disconnect();
  }, []);

  // Smooth scroll + scroll-reveal (skipped under ?static / ?nolenis).
  useEffect(() => {
    if (STATIC || new URLSearchParams(location.search).has("nolenis")) return;
    const lenis = new Lenis({ duration: 1.05, smoothWheel: true });
    lenis.on("scroll", ScrollTrigger.update);
    const tick = (t: number) => lenis.raf(t * 1000);
    gsap.ticker.add(tick);
    gsap.ticker.lagSmoothing(0);
    document.documentElement.classList.add("lenis", "lenis-smooth");

    const ctx = gsap.context(() => {
      gsap.set(DASH_REVEAL, { opacity: 0, y: 22 });
      ScrollTrigger.batch(DASH_REVEAL, {
        start: "top 90%",
        onEnter: (b) => gsap.to(b, { opacity: 1, y: 0, duration: 0.6, stagger: 0.06, ease: "power2.out", overwrite: true }),
      });
    }, rootRef);
    ScrollTrigger.refresh();

    return () => {
      ctx.revert();
      gsap.ticker.remove(tick);
      lenis.destroy();
      document.documentElement.classList.remove("lenis", "lenis-smooth");
    };
  }, []);

  return (
    <div
      ref={rootRef}
      className="dashboard"
      style={{ ["--mk-text" as string]: text, color: text }}
    >
      <Chrome />
      <Nav variant="dashboard" label={label} color={text} />

      <main className="dash-main">
        <StatusHeader />
        <ChannelGrid />
        <PerformanceCharts />
        <HealthTable />
        <QuickActions />
        <InstallSnippet />
      </main>
    </div>
  );
}
