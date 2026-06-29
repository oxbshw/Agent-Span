interface Props {
  size?: number;
  color?: string;
  wordmark?: boolean;
}

export function Logo({ size = 26, color = "var(--ink)", wordmark = true }: Props) {
  return (
    <span style={{ display: "inline-flex", alignItems: "center", gap: 9 }}>
      <svg width={size} height={size} viewBox="0 0 46 48" fill="none" aria-hidden="true">
        <defs>
          <linearGradient id="ag-logo-grad" x1="0" y1="0" x2="1" y2="1">
            <stop offset="0" stopColor="#E8A5F3" />
            <stop offset="1" stopColor="#FFDFC4" />
          </linearGradient>
        </defs>
        <g stroke="url(#ag-logo-grad)" strokeWidth={3} strokeLinecap="round" strokeLinejoin="round">
          <path d="M11 39 L23 9 L35 39" />
          <path d="M16 29 L30 29" />
          <path d="M23 9 L23 2" />
          <path d="M11 39 L5 44" />
          <path d="M35 39 L41 44" />
        </g>
        <circle cx="23" cy="9" r="4.2" fill="url(#ag-logo-grad)" />
        <circle cx="5" cy="44" r="2.6" fill="url(#ag-logo-grad)" />
        <circle cx="41" cy="44" r="2.6" fill="url(#ag-logo-grad)" />
        <circle cx="23" cy="2" r="2.4" fill="url(#ag-logo-grad)" />
      </svg>
      {wordmark && (
        <span style={{ fontWeight: 900, fontSize: size * 0.72, letterSpacing: "-0.03em", color }}>AgentSpan</span>
      )}
    </span>
  );
}
