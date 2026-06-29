import { useEffect, useRef, useState } from "react";

// Counts a number up from 0 → target the first time the element scrolls into
// view (easeOutCubic). Returns a ref to attach and the formatted string.
export function useCountUp(target: number, decimals = 0, duration = 1100) {
  const [val, setVal] = useState(0);
  const ref = useRef<HTMLSpanElement>(null);
  const started = useRef(false);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const io = new IntersectionObserver(
      (entries) => {
        for (const e of entries) {
          if (e.isIntersecting && !started.current) {
            started.current = true;
            const start = performance.now();
            const tick = (now: number) => {
              const p = Math.min(1, (now - start) / duration);
              setVal(target * (1 - Math.pow(1 - p, 3)));
              if (p < 1) requestAnimationFrame(tick);
            };
            requestAnimationFrame(tick);
          }
        }
      },
      { threshold: 0.4 },
    );
    io.observe(el);
    return () => io.disconnect();
  }, [target, duration]);

  const shown = decimals > 0 ? val.toFixed(decimals) : Math.round(val).toLocaleString();
  return { ref, shown };
}
