import { BookOpen, Stethoscope, Code2, Github } from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { site } from "../../data/content";

// Quick actions — four styled link cards.

interface Action {
  icon: LucideIcon;
  title: string;
  hint: string;
  href: string;
}
const ACTIONS: Action[] = [
  { icon: BookOpen, title: "View Docs", hint: "docs.agentspan.io", href: site.docsUrl },
  { icon: Stethoscope, title: "Run Diagnostics", hint: "agentspan doctor", href: site.githubUrl },
  { icon: Code2, title: "API Reference", hint: "/api/v1/", href: `${site.githubUrl}/blob/main/docs/api-reference.md` },
  { icon: Github, title: "GitHub", hint: "oxbshw/Agent-Span", href: site.githubUrl },
];

export function QuickActions() {
  return (
    <section
      className="dash-section dash-actions"
      style={{ background: "#FFDFC4", color: "#070707" }}
      data-text="#070707"
      data-label="AGENTSPAN // ACTIONS"
    >
      <div className="dash-section-head">
        <div>
          <span className="eyebrow mono">[ SHORTCUTS ]</span>
          <h2 className="dash-h">QUICK ACTIONS</h2>
        </div>
      </div>

      <div className="action-grid">
        {ACTIONS.map((a) => {
          const Icon = a.icon;
          return (
            <a key={a.title} className="action-card" href={a.href} target="_blank" rel="noreferrer">
              <span className="action-ico"><Icon size={22} strokeWidth={1.6} /></span>
              <span className="action-title">{a.title}</span>
              <span className="action-hint mono">{a.hint}</span>
              <span className="action-arrow mono">›</span>
            </a>
          );
        })}
      </div>
    </section>
  );
}
