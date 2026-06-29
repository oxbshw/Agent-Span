# Importing login cookies

Login-gated platforms (Twitter/X, XiaoHongShu, Bilibili, Xueqiu) need your
browser session. The most reliable, cross-platform way is the **Cookie-Editor**
extension export — the same flow Agent Reach recommends.

> ⚠️ Use a dedicated throwaway account, not your main one. Cookies equal full
> login access, and scripted access can get accounts flagged.

## Steps
1. Install [Cookie-Editor](https://cookie-editor.com/) in your browser.
2. Log into the target site (x.com, xiaohongshu.com, bilibili.com, xueqiu.com).
3. Open Cookie-Editor on that tab → **Export** → **JSON** (or **Header string**).
4. Import into AgentSpan:

```bash
# JSON export (recommended — carries cookie domains)
agentspan config cookies '[{"name":"auth_token","value":"...","domain":".x.com"}, ...]'

# or a header string for a single site
agentspan config cookies "auth_token=...; ct0=..."
```

AgentSpan extracts only the credentials each platform needs:
- Twitter/X: `auth_token` + `ct0`
- Bilibili: `SESSDATA` + `bili_jct`
- Xueqiu: requires `xq_a_token`
- XiaoHongShu: the full cookie set (JSON export only)

Cookies are stored in `~/.agentspan/config.yaml` with `0600` permissions and are
never uploaded.

## Browser auto-extraction
`agentspan config from-browser chrome` prints the guided Cookie-Editor flow.
Direct browser-DB decryption is available in the optional `browser-cookies`
build feature.

## Verify
```bash
agentspan doctor    # the platform should now show a healthy/active backend
```
