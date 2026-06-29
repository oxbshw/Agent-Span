import * as vscode from "vscode";
import {
  AgentSpanClient,
  AgentSpanError,
  FederatedResult,
  ReadResponse,
  SearchResult,
} from "./api";

const HEALTH_POLL_INTERVAL_MS = 30_000;

let client: AgentSpanClient;
let output: vscode.OutputChannel;
let statusBar: vscode.StatusBarItem;
let healthTimer: ReturnType<typeof setInterval> | undefined;

export function activate(context: vscode.ExtensionContext): void {
  client = new AgentSpanClient();
  output = vscode.window.createOutputChannel("AgentSpan");

  statusBar = vscode.window.createStatusBarItem(
    vscode.StatusBarAlignment.Right,
    100
  );
  statusBar.command = "agentspan.searchWeb";
  context.subscriptions.push(statusBar, output);

  context.subscriptions.push(
    vscode.commands.registerCommand("agentspan.searchWeb", () =>
      runCommand(searchWeb)
    ),
    vscode.commands.registerCommand("agentspan.readUrl", () =>
      runCommand(readUrl)
    ),
    vscode.commands.registerCommand("agentspan.federatedSearch", () =>
      runCommand(federatedSearch)
    )
  );

  // Start health polling and refresh immediately.
  void refreshHealth();
  healthTimer = setInterval(() => void refreshHealth(), HEALTH_POLL_INTERVAL_MS);
  context.subscriptions.push({
    dispose: () => {
      if (healthTimer) {
        clearInterval(healthTimer);
        healthTimer = undefined;
      }
    },
  });
  statusBar.show();
}

export function deactivate(): void {
  if (healthTimer) {
    clearInterval(healthTimer);
    healthTimer = undefined;
  }
}

/** Wraps a command body with uniform error handling. */
async function runCommand(fn: () => Promise<void>): Promise<void> {
  try {
    await fn();
  } catch (err) {
    reportError(err);
  }
}

function reportError(err: unknown): void {
  const message =
    err instanceof AgentSpanError
      ? err.message
      : err instanceof Error
        ? err.message
        : String(err);
  output.appendLine(`[error] ${message}`);
  void vscode.window.showErrorMessage(`AgentSpan: ${message}`);
}

// ---------------------------------------------------------------------------
// Health status bar
// ---------------------------------------------------------------------------

async function refreshHealth(): Promise<void> {
  try {
    const health = await client.health();
    const ok = health.status?.toLowerCase() === "ok";
    if (ok) {
      statusBar.text = "$(check) AgentSpan";
      statusBar.tooltip = "AgentSpan gateway is healthy. Click to search the web.";
      statusBar.backgroundColor = undefined;
    } else {
      statusBar.text = "$(x) AgentSpan";
      statusBar.tooltip = `AgentSpan reported status: ${health.status}`;
      statusBar.backgroundColor = new vscode.ThemeColor(
        "statusBarItem.warningBackground"
      );
    }
  } catch (err) {
    statusBar.text = "$(x) AgentSpan";
    statusBar.tooltip =
      err instanceof Error
        ? `AgentSpan gateway unreachable: ${err.message}`
        : "AgentSpan gateway unreachable.";
    statusBar.backgroundColor = new vscode.ThemeColor(
      "statusBarItem.errorBackground"
    );
  }
}

// ---------------------------------------------------------------------------
// Command: Search Web (single channel)
// ---------------------------------------------------------------------------

async function searchWeb(): Promise<void> {
  const query = await vscode.window.showInputBox({
    title: "AgentSpan: Search Web",
    prompt: "Enter a search query",
    placeHolder: "e.g. rust async runtime comparison",
    ignoreFocusOut: true,
  });
  if (!query) {
    return;
  }

  const channel = await pickChannel();
  if (!channel) {
    return;
  }

  const response = await vscode.window.withProgress(
    {
      location: vscode.ProgressLocation.Notification,
      title: `AgentSpan: searching "${channel}"...`,
    },
    () => client.searchChannel(channel, query)
  );

  if (response.results.length === 0) {
    void vscode.window.showInformationMessage(
      `AgentSpan: no results for "${query}" on ${channel}.`
    );
    return;
  }

  await presentResults(
    response.results.map((r) => ({ ...r, channels: [response.channel] })),
    `Results on ${response.channel} for "${query}"`
  );
}

// ---------------------------------------------------------------------------
// Command: Federated Search (multiple channels)
// ---------------------------------------------------------------------------

async function federatedSearch(): Promise<void> {
  const query = await vscode.window.showInputBox({
    title: "AgentSpan: Federated Search",
    prompt: "Enter a search query (searched across multiple channels)",
    placeHolder: "e.g. latest typescript release notes",
    ignoreFocusOut: true,
  });
  if (!query) {
    return;
  }

  const channels = await pickChannels();
  // `undefined` => let the gateway choose its default set of channels.

  const response = await vscode.window.withProgress(
    {
      location: vscode.ProgressLocation.Notification,
      title: "AgentSpan: running federated search...",
    },
    () =>
      client.federatedSearch({
        query,
        channels: channels && channels.length > 0 ? channels : undefined,
      })
  );

  if (response.results.length === 0) {
    void vscode.window.showInformationMessage(
      `AgentSpan: no federated results for "${query}".`
    );
    return;
  }

  const searched = response.searched?.length
    ? ` (searched: ${response.searched.join(", ")})`
    : "";
  await presentResults(response.results, `Federated results for "${query}"${searched}`);
}

// ---------------------------------------------------------------------------
// Command: Read URL
// ---------------------------------------------------------------------------

async function readUrl(): Promise<void> {
  const target = await vscode.window.showInputBox({
    title: "AgentSpan: Read URL",
    prompt: "Enter a URL to read",
    placeHolder: "https://example.com/article",
    ignoreFocusOut: true,
    validateInput: (value) => {
      if (!value.trim()) {
        return "A URL is required.";
      }
      try {
        // eslint-disable-next-line no-new
        new URL(value.trim());
        return undefined;
      } catch {
        return "Enter a valid absolute URL (including https://).";
      }
    },
  });
  if (!target) {
    return;
  }

  const response = await vscode.window.withProgress(
    {
      location: vscode.ProgressLocation.Notification,
      title: `AgentSpan: reading ${target}...`,
    },
    () => client.read(target.trim())
  );

  await openReadResult(response);
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

interface PickableResult extends SearchResult {
  channels?: string[];
}

/** Lets the user pick one channel; returns `undefined` if cancelled. */
async function pickChannel(): Promise<string | undefined> {
  const { channels } = await vscode.window.withProgress(
    { location: vscode.ProgressLocation.Window, title: "AgentSpan: loading channels..." },
    () => client.channels()
  );
  if (channels.length === 0) {
    void vscode.window.showWarningMessage("AgentSpan: the gateway reports no channels.");
    return undefined;
  }

  const picked = await vscode.window.showQuickPick(
    channels.map((c) => ({
      label: c.name,
      description: c.tier,
      detail: c.description,
    })),
    {
      title: "AgentSpan: choose a channel",
      placeHolder: "Select a channel to search",
      matchOnDescription: true,
      matchOnDetail: true,
    }
  );
  return picked?.label;
}

/**
 * Lets the user pick zero or more channels for federated search.
 * Returns `undefined` if the picker was cancelled, an empty array if the user
 * confirmed with nothing selected (meaning "use gateway defaults").
 */
async function pickChannels(): Promise<string[] | undefined> {
  let channels;
  try {
    ({ channels } = await client.channels());
  } catch (err) {
    // Federated search can run without an explicit channel list, so degrade
    // gracefully if the channel listing is unavailable.
    output.appendLine(
      `[warn] could not list channels, falling back to gateway defaults: ${
        err instanceof Error ? err.message : String(err)
      }`
    );
    return undefined;
  }

  if (channels.length === 0) {
    return undefined;
  }

  const picked = await vscode.window.showQuickPick(
    channels.map((c) => ({
      label: c.name,
      description: c.tier,
      detail: c.description,
    })),
    {
      title: "AgentSpan: choose channels (none = gateway defaults)",
      placeHolder: "Select channels to search across, or confirm with none",
      canPickMany: true,
      matchOnDescription: true,
      matchOnDetail: true,
    }
  );
  if (picked === undefined) {
    return undefined;
  }
  return picked.map((p) => p.label);
}

/** Shows results in a QuickPick; the chosen one is read & opened. */
async function presentResults(
  results: PickableResult[],
  title: string
): Promise<void> {
  output.appendLine(`\n=== ${title} (${results.length}) ===`);
  for (const r of results) {
    const channelTag = r.channels?.length ? ` [${r.channels.join(", ")}]` : "";
    output.appendLine(`- ${r.title}${channelTag}`);
    output.appendLine(`  ${r.url}`);
    if (r.snippet) {
      output.appendLine(`  ${r.snippet}`);
    }
  }

  const items: (vscode.QuickPickItem & { result: PickableResult })[] =
    results.map((r) => ({
      label: r.title || r.url,
      description: r.channels?.length ? r.channels.join(", ") : undefined,
      detail: r.snippet || r.url,
      result: r,
    }));

  const picked = await vscode.window.showQuickPick(items, {
    title,
    placeHolder: "Select a result to read (Esc to just view the list in Output)",
    matchOnDescription: true,
    matchOnDetail: true,
  });
  if (!picked) {
    output.show(true);
    return;
  }

  const response = await vscode.window.withProgress(
    {
      location: vscode.ProgressLocation.Notification,
      title: `AgentSpan: reading ${picked.result.url}...`,
    },
    () => client.read(picked.result.url)
  );
  await openReadResult(response);
}

/** Renders a read result in a webview panel. */
async function openReadResult(response: ReadResponse): Promise<void> {
  const content = response.content ?? { url: response.url };
  const title = content.title?.trim() || response.url;

  const panel = vscode.window.createWebviewPanel(
    "agentspanReader",
    truncate(title, 60),
    vscode.ViewColumn.Active,
    { enableScripts: false }
  );
  panel.webview.html = renderHtml(response, title);
}

function renderHtml(response: ReadResponse, title: string): string {
  const content = response.content ?? { url: response.url };
  const body = content.body ?? "";
  const safeTitle = escapeHtml(title);
  const safeUrl = escapeHtml(response.url);
  const safeChannel = escapeHtml(response.channel ?? "");
  const safeBody = escapeHtml(body).replace(/\n/g, "<br>");

  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <meta http-equiv="Content-Security-Policy"
        content="default-src 'none'; style-src 'unsafe-inline';">
  <title>${safeTitle}</title>
  <style>
    body {
      font-family: var(--vscode-font-family, sans-serif);
      color: var(--vscode-editor-foreground);
      line-height: 1.6;
      padding: 1.5rem 2rem;
      max-width: 60rem;
      margin: 0 auto;
    }
    h1 { font-size: 1.5rem; margin-bottom: 0.25rem; }
    .meta {
      font-size: 0.85rem;
      opacity: 0.75;
      margin-bottom: 1.5rem;
      word-break: break-all;
    }
    .meta a { color: var(--vscode-textLink-foreground); }
    .channel {
      display: inline-block;
      font-size: 0.75rem;
      padding: 0.1rem 0.5rem;
      border-radius: 0.5rem;
      background: var(--vscode-badge-background);
      color: var(--vscode-badge-foreground);
      margin-left: 0.5rem;
    }
    .body { font-size: 0.95rem; }
    .empty { opacity: 0.6; font-style: italic; }
    hr { border: none; border-top: 1px solid var(--vscode-editorWidget-border); }
  </style>
</head>
<body>
  <h1>${safeTitle}${safeChannel ? `<span class="channel">${safeChannel}</span>` : ""}</h1>
  <div class="meta"><a href="${safeUrl}">${safeUrl}</a></div>
  <hr>
  <div class="body">${safeBody || '<p class="empty">No body content returned.</p>'}</div>
</body>
</html>`;
}

// ---------------------------------------------------------------------------
// Small utilities
// ---------------------------------------------------------------------------

function escapeHtml(value: string): string {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#039;");
}

function truncate(value: string, max: number): string {
  return value.length > max ? `${value.slice(0, max - 1)}…` : value;
}

// Re-export for potential test usage / type clarity.
export type { FederatedResult };
