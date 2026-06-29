import { useState } from "react";
import { Copy, Check } from "lucide-react";

// Get started — code block with a copy-to-clipboard button.

const LINES: { t: string; cls?: string }[] = [
  { t: "cargo install agentspan", cls: "p" },
  { t: "agentspan serve", cls: "p" },
  { t: "# Gateway running at http://localhost:8080", cls: "c" },
];
const RAW = "cargo install agentspan\nagentspan serve\n# Gateway running at http://localhost:8080";

export function InstallSnippet() {
  const [copied, setCopied] = useState(false);

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(RAW);
    } catch {
      /* clipboard blocked — ignore */
    }
    setCopied(true);
    window.setTimeout(() => setCopied(false), 2000);
  };

  return (
    <section
      className="dash-section dash-install"
      style={{ background: "#383534", color: "#FFDFC4" }}
      data-text="#FFDFC4"
      data-no-texture
      data-label="AGENTSPAN // GET STARTED"
    >
      <div className="dash-section-head">
        <div>
          <span className="eyebrow mono" style={{ color: "#FFDFC4" }}>[ THREE COMMANDS ]</span>
          <h2 className="dash-h">GET STARTED</h2>
        </div>
      </div>

      <div className="code-card">
        <div className="code-card-bar mono">
          <span className="code-dots"><i /><i /><i /></span>
          <span>bash</span>
          <button className="code-copy mono" onClick={copy} type="button">
            {copied ? <Check size={13} /> : <Copy size={13} />}
            {copied ? "Copied!" : "Copy"}
          </button>
        </div>
        <pre className="code-card-body">
          {LINES.map((l, i) => (
            <div key={i} className={`code-line ${l.cls ?? ""}`}>{l.t}</div>
          ))}
        </pre>
      </div>
    </section>
  );
}
