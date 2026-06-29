import { useId } from "react";

// Wordmark rendered as audio-waveform line-art: a field of thin horizontal
// lines, with a denser/brighter set clipped to the letterforms so the word
// emerges from the waveform. Gold/peach lines on charcoal.

interface Props {
  text?: string;
  color?: string;
}

export function WaveWordmark({ text = "AGENTSPAN", color = "#FFEBB0" }: Props) {
  const uid = useId().replace(/:/g, "");
  const W = 1200;
  const H = 360;
  const clip = `wm-clip-${uid}`;

  // Faint full-panel line field + dense in-letter field.
  const fieldLines = [];
  for (let y = 10; y < H; y += 9) {
    fieldLines.push(<line key={`f${y}`} x1={0} y1={y} x2={W} y2={y} stroke={color} strokeWidth={1} opacity={0.14} />);
  }
  const denseLines = [];
  for (let y = 6; y < H; y += 5) {
    denseLines.push(<line key={`d${y}`} x1={0} y1={y} x2={W} y2={y} stroke={color} strokeWidth={1.4} opacity={0.95} />);
  }

  return (
    <svg className="wave-wordmark" viewBox={`0 0 ${W} ${H}`} preserveAspectRatio="xMidYMid meet" role="img" aria-label={text}>
      <defs>
        <clipPath id={clip}>
          <text
            x="50%"
            y="54%"
            textAnchor="middle"
            dominantBaseline="middle"
            style={{ fontFamily: "'Space Grotesk', sans-serif", fontWeight: 700, fontSize: 230, letterSpacing: "-0.04em" }}
          >
            {text}
          </text>
        </clipPath>
      </defs>

      {/* faint waveform field across the whole panel */}
      <g>{fieldLines}</g>

      {/* dense bright waveform inside the letterforms */}
      <g clipPath={`url(#${clip})`}>{denseLines}</g>

      {/* thin keyline tracing the wordmark */}
      <text
        x="50%"
        y="54%"
        textAnchor="middle"
        dominantBaseline="middle"
        fill="none"
        stroke={color}
        strokeWidth={1}
        opacity={0.5}
        style={{ fontFamily: "'Space Grotesk', sans-serif", fontWeight: 700, fontSize: 230, letterSpacing: "-0.04em" }}
      >
        {text}
      </text>
    </svg>
  );
}
