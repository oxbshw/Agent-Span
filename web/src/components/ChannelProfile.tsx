import { useId } from "react";
import { avatarMap, type AvatarConfig } from "../data/realData";
import { brandIcons } from "../data/brandIcons";
import { customGlyphs } from "../data/customGlyphs";

// A branded "profile" avatar in the style of the supplied sample: the classic
// head-and-shoulders silhouette, but filled with each channel's real brand
// gradient + a glossy frosted sheen, a soft brand glow, and the brand monogram
// on the chest — so every company reads as itself (Google ≠ Reddit ≠ Spotify).

const FALLBACK: AvatarConfig = { shape: "circle", gradient: ["#E8A5F3", "#C5C4FF"], pattern: "solid", symbol: "?" };

// head circle + rounded-shoulder bust — the default-avatar silhouette
const HEAD = { cx: 50, cy: 34, r: 16 };
const BUST = "M19,95 C19,72 32,59 50,59 C68,59 81,72 81,95 Z";

export function ChannelProfile({ channel, size = 96 }: { channel: string; size?: number }) {
  const uid = useId().replace(/:/g, "");
  const cfg = avatarMap[channel] ?? FALLBACK;
  const [c1, c2] = cfg.gradient;
  const g = `cp-g-${uid}`;
  const sheen = `cp-s-${uid}`;
  const glow = `cp-glow-${uid}`;
  const clip = `cp-clip-${uid}`;

  return (
    <svg className="channel-profile" width={size} height={size} viewBox="0 0 100 100" role="img" aria-label={`${channel} profile`}>
      <defs>
        <linearGradient id={g} x1="0.12" y1="0.05" x2="0.88" y2="1">
          <stop offset="0" stopColor={c1} />
          <stop offset="1" stopColor={c2} />
        </linearGradient>
        <radialGradient id={sheen} cx="0.32" cy="0.24" r="0.75">
          <stop offset="0" stopColor="#fff" stopOpacity="0.6" />
          <stop offset="0.45" stopColor="#fff" stopOpacity="0.1" />
          <stop offset="1" stopColor="#fff" stopOpacity="0" />
        </radialGradient>
        <radialGradient id={glow} cx="0.5" cy="0.52" r="0.5">
          <stop offset="0" stopColor={c1} stopOpacity="0.45" />
          <stop offset="1" stopColor={c1} stopOpacity="0" />
        </radialGradient>
        <clipPath id={clip}>
          <circle cx={HEAD.cx} cy={HEAD.cy} r={HEAD.r} />
          <path d={BUST} />
        </clipPath>
      </defs>

      {/* soft brand glow behind the silhouette */}
      <circle cx="50" cy="54" r="50" fill={`url(#${glow})`} />

      {/* the silhouette, filled with the brand gradient */}
      <circle cx={HEAD.cx} cy={HEAD.cy} r={HEAD.r} fill={`url(#${g})`} />
      <path d={BUST} fill={`url(#${g})`} />

      {/* glossy frosted sheen, clipped to the silhouette */}
      <g clipPath={`url(#${clip})`}>
        <rect x="0" y="0" width="100" height="100" fill={`url(#${sheen})`} />
        {/* faint top rim light on the head */}
        <ellipse cx="44" cy="26" rx="9" ry="5" fill="#fff" opacity="0.28" />
      </g>

      {/* the channel mark on the chest: real brand logo, else a hand-drawn glyph,
          else a clean monogram — every channel resolves to a crisp white mark */}
      {brandIcons[channel] ? (
        <g transform="translate(36.5, 61.5) scale(1.125)">
          <path d={brandIcons[channel].path} fill="#fff" fillOpacity="0.97" />
        </g>
      ) : customGlyphs[channel] ? (
        <g transform="translate(36.5, 61.5) scale(1.125)">
          <path
            d={customGlyphs[channel].path}
            fillRule={customGlyphs[channel].fillRule ?? "nonzero"}
            fill="#fff"
            fillOpacity="0.97"
          />
        </g>
      ) : (
        <text
          x="50" y="76" textAnchor="middle" dominantBaseline="central"
          fontFamily="'Space Grotesk', sans-serif" fontWeight="700"
          fontSize={cfg.symbol.length > 2 ? 13 : 16}
          fill="#fff" fillOpacity="0.96" letterSpacing="0.02em"
        >
          {cfg.symbol}
        </text>
      )}
    </svg>
  );
}
