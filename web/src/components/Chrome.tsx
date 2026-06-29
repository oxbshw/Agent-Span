import { useEffect, useRef, useState } from "react";

// Persistent HUD overlay: corner brackets, a live mouse-coordinate readout, and
// a sound toggle. Drawn above the sections; pointer-events off except the toggle.

function Bracket({ pos }: { pos: "tl" | "tr" | "bl" | "br" }) {
  return <span className={`corner-frame is-${pos}`} aria-hidden="true" />;
}

export function Chrome() {
  const xRef = useRef<HTMLSpanElement>(null);
  const yRef = useRef<HTMLSpanElement>(null);
  const [sound, setSound] = useState(true);

  useEffect(() => {
    const fmt = (n: number) => String(Math.round(n)).padStart(4, "0");
    const onMove = (e: MouseEvent) => {
      if (xRef.current) xRef.current.textContent = fmt(e.clientX);
      if (yRef.current) yRef.current.textContent = fmt(e.clientY);
    };
    window.addEventListener("mousemove", onMove, { passive: true });
    return () => window.removeEventListener("mousemove", onMove);
  }, []);

  return (
    <div className="chrome" aria-hidden="false">
      <Bracket pos="tl" />
      <Bracket pos="tr" />
      <Bracket pos="bl" />
      <Bracket pos="br" />

      <button
        className={`sound-toggle ${sound ? "is-on" : ""}`}
        onClick={() => setSound((s) => !s)}
        type="button"
      >
        <span className="sound-bars" aria-hidden="true">
          <i /><i /><i /><i />
        </span>
        <span>{sound ? "SND / ON" : "SND / OFF"}</span>
      </button>

      <div className="coord-hud">
        X: <span ref={xRef}>0000</span>  //  Y: <span ref={yRef}>0000</span>
      </div>
    </div>
  );
}
