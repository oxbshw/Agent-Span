import { useEffect, useRef, useState } from "react";
import gsap from "gsap";
import { GatewayOrb } from "./GatewayOrb";

// Black intro: the gradient orb at centre, a 0→100 counter in coral, corner
// brackets, and boot readouts down both flanks. Auto-dismisses at 100% (with a
// hard fallback) and on click.

interface Props {
  onDone: () => void;
}

const LEFT = ["C://AGENTSPAN", "_PROTOCOL_HTTP", "/////_2026", "<ACCESS GRANTED>"];
const RIGHT = ["D://GATEWAY_CORE", "__52_CHANNELS", "<91 MCP TOOLS>", "<SELF-HEALING>"];

export function Preloader({ onDone }: Props) {
  const [count, setCount] = useState(0);
  const [active, setActive] = useState(false);
  const [leaving, setLeaving] = useState(false);
  const doneRef = useRef(false);
  const rootRef = useRef<HTMLDivElement>(null);

  const leave = () => {
    if (doneRef.current) return;
    doneRef.current = true;
    setLeaving(true);
    window.setTimeout(onDone, 700);
  };

  // GSAP entrance: the orb scales/fades in, the counter drops in, and the boot
  // readouts stagger in from the flanks.
  useEffect(() => {
    if (!rootRef.current) return;
    const ctx = gsap.context(() => {
      gsap.from(".gateway-orb", { scale: 0.7, opacity: 0, duration: 1.1, ease: "expo.out" });
      gsap.from(".preloader-count", { y: -18, opacity: 0, duration: 0.8, ease: "expo.out", delay: 0.15 });
      gsap.from(".preloader-readout.is-left span", { x: -24, opacity: 0, duration: 0.6, stagger: 0.08, ease: "power2.out", delay: 0.2 });
      gsap.from(".preloader-readout.is-right span", { x: 24, opacity: 0, duration: 0.6, stagger: 0.08, ease: "power2.out", delay: 0.2 });
      gsap.from(".preloader-foot, .preloader-corner", { opacity: 0, duration: 0.8, delay: 0.5 });
    }, rootRef);
    return () => ctx.revert();
  }, []);

  useEffect(() => {
    let n = 0;
    const tick = window.setInterval(() => {
      n = Math.min(100, n + 3);
      setCount(n);
      if (n >= 100) {
        setActive(true);
        window.clearInterval(tick);
        window.setTimeout(leave, 650);
      }
    }, 70);
    const fallback = window.setTimeout(leave, 4200);
    return () => {
      window.clearInterval(tick);
      window.clearTimeout(fallback);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return (
    <div ref={rootRef} className={`preloader ${leaving ? "is-leaving" : ""}`} onClick={active ? leave : undefined}>
      <span className="preloader-corner is-tl" />
      <span className="preloader-corner is-tr" />
      <span className="preloader-corner is-bl" />
      <span className="preloader-corner is-br" />

      <div className="preloader-count mono">{count} %</div>

      <div className="preloader-readout is-left mono">
        {LEFT.map((l) => <span key={l}>{l}</span>)}
      </div>
      <div className="preloader-readout is-right mono">
        {RIGHT.map((l) => <span key={l}>{l}</span>)}
      </div>

      <GatewayOrb size={420}>
        <span className="preloader-orb-top mono">ROUTE // HEAL // SCALE</span>
        <span className="preloader-orb-main">{active ? "‹‹‹ ENTER ›››" : "‹‹‹ BOOTING ›››"}</span>
      </GatewayOrb>

      <div className="preloader-foot mono">AGENTSPAN GATEWAY — INNOVATION // PERFECTION</div>
    </div>
  );
}
