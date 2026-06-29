import { Link } from "react-router-dom";
import { Logo } from "./Logo";
import { site } from "../data/content";

// Fixed top nav: logo left, a live section indicator centre, links right. Shared
// between the landing (`/`) and the dashboard (`/status`); the active link
// reflects the current route.

interface Props {
  label: string;
  color: string;
  variant?: "landing" | "dashboard";
}

export function Nav({ label, color, variant = "landing" }: Props) {
  const onDash = variant === "dashboard";

  return (
    <nav className="site-nav" style={{ color }}>
      {onDash ? (
        <Link className="site-nav-brand" to="/"><Logo size={18} color={color} /></Link>
      ) : (
        <a className="site-nav-brand" href="#hero"><Logo size={24} color={color} /></a>
      )}

      <span className="site-nav-indicator mono">{label}</span>

      <div className="site-nav-right mono">
        {onDash ? (
          <>
            <Link to="/">HOME</Link>
            <Link to="/status" className="is-active">STATUS</Link>
            <a href={site.githubUrl} target="_blank" rel="noreferrer">GITHUB</a>
          </>
        ) : (
          <>
            <Link to="/status">STATUS</Link>
            <a href={site.githubUrl} target="_blank" rel="noreferrer">GITHUB</a>
            <a href={site.docsUrl} target="_blank" rel="noreferrer">DOCS</a>
          </>
        )}
      </div>
    </nav>
  );
}
