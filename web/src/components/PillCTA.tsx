import type { ReactNode } from "react";

// Rounded capsule CTA — solid ink, or outlined with a yellow knob — with a
// 3-chevron arrow that flickers (staggered) on hover.

type Variant = "ink" | "knob";

interface Props {
  children: ReactNode;
  href?: string;
  onClick?: () => void;
  variant?: Variant;
}

function Chevrons() {
  return (
    <span className="cta-arrows" aria-hidden="true">
      <span className="cta-chev is-1">›</span>
      <span className="cta-chev is-2">›</span>
      <span className="cta-chev is-3">›</span>
    </span>
  );
}

export function PillCTA({ children, href, onClick, variant = "ink" }: Props) {
  const inner =
    variant === "knob" ? (
      <>
        <span className="cta-label">{children}</span>
        <span className="cta-knob"><Chevrons /></span>
      </>
    ) : (
      <>
        <span className="cta-label">{children}</span>
        <Chevrons />
      </>
    );

  const className = `pill-cta is-${variant}`;
  if (href) {
    return (
      <a className={className} href={href} target="_blank" rel="noreferrer">
        {inner}
      </a>
    );
  }
  return (
    <button className={className} onClick={onClick} type="button">
      {inner}
    </button>
  );
}
