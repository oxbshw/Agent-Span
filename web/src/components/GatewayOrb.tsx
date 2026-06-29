import { useId, type ReactNode } from "react";

// A pink→purple→coral sphere wrapped in concentric ring lines with a soft glow,
// in pure SVG/CSS. Used by the preloader and the dashboard header.

interface Props {
  size?: number;
  rings?: number;
  spin?: boolean;
  pulse?: boolean;
  children?: ReactNode;
}

export function GatewayOrb({ size = 360, rings = 16, spin = true, pulse = true, children }: Props) {
  const uid = useId().replace(/:/g, "");
  const g = `orb-grad-${uid}`;
  const glow = `orb-glow-${uid}`;
  const sphere = `orb-sphere-${uid}`;
  const cx = 100;
  const cy = 100;
  const rOuter = 96;

  const ringEls = [];
  for (let i = 0; i < rings; i++) {
    const r = rOuter * (0.34 + (0.66 * i) / (rings - 1));
    const op = 0.16 + 0.5 * (i / (rings - 1));
    ringEls.push(<circle key={i} cx={cx} cy={cy} r={r} fill="none" stroke={`url(#${g})`} strokeWidth={0.6} opacity={op} />);
  }

  return (
    <div className="gateway-orb" style={{ width: size, height: size }}>
      <svg viewBox="0 0 200 200" className={spin ? "orb-svg is-spin" : "orb-svg"} aria-hidden="true">
        <defs>
          <linearGradient id={g} x1="0" y1="0" x2="1" y2="1">
            <stop offset="0" stopColor="#E8A5F3" />
            <stop offset="0.45" stopColor="#C5C4FF" />
            <stop offset="1" stopColor="#FFDFC4" />
          </linearGradient>
          <radialGradient id={sphere} cx="0.4" cy="0.36" r="0.75">
            <stop offset="0" stopColor="#FFE9D6" />
            <stop offset="0.5" stopColor="#E8A5F3" />
            <stop offset="1" stopColor="#7B6BC9" />
          </radialGradient>
          <radialGradient id={glow} cx="0.5" cy="0.5" r="0.5">
            <stop offset="0" stopColor="#FFDFC4" stopOpacity="0.5" />
            <stop offset="1" stopColor="#FFDFC4" stopOpacity="0" />
          </radialGradient>
        </defs>

        <circle cx={cx} cy={cy} r={rOuter} fill={`url(#${glow})`} />
        <circle cx={cx} cy={cy} r={rOuter * 0.33} fill={`url(#${sphere})`} opacity={0.92} />
        <g>{ringEls}</g>

        {/* expanding pulse rings — radiate outward + fade (the WebGL orb's pulse) */}
        {pulse && (
          <g className="orb-pulse">
            <circle cx={cx} cy={cy} r={rOuter * 0.4} fill="none" stroke={`url(#${g})`} strokeWidth={1.2} />
            <circle cx={cx} cy={cy} r={rOuter * 0.4} fill="none" stroke={`url(#${g})`} strokeWidth={1.2} />
            <circle cx={cx} cy={cy} r={rOuter * 0.4} fill="none" stroke={`url(#${g})`} strokeWidth={1.2} />
          </g>
        )}

        {/* orbiting node — the planet in the ring-head */}
        <g className="orb-node">
          <circle cx={cx + rOuter * 0.62} cy={cy} r={3.4} fill="#FFEBB0" />
        </g>
      </svg>
      {children && <div className="orb-center">{children}</div>}
    </div>
  );
}
