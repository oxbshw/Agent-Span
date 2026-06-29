// Small squares that blink on an alternating delay — live/status indicators.

interface Props {
  color?: string;
  size?: number;
  count?: number;
  blink?: boolean;
}

export function BlinkingBlock({ color = "var(--ink)", size = 9, count = 1, blink = true }: Props) {
  return (
    <span className="blinking-decor-block-wrap" style={{ display: "inline-flex", gap: 4, alignItems: "center" }}>
      {Array.from({ length: count }).map((_, i) => (
        <span
          key={i}
          className={blink ? "blinking-decor-block" : ""}
          style={{ width: size, height: size, borderRadius: 2, background: color, display: "inline-block" }}
        />
      ))}
    </span>
  );
}
