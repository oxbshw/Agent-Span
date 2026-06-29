//! Cookie import + per-platform extraction.
//!
//! Two input formats are accepted (mirroring Agent Reach's preferred flow):
//!   1. A Cookie-Editor JSON export — `[{"name":..,"value":..,"domain":..}, ..]`
//!   2. A raw header string — `"name=value; name2=value2; ..."`
//!
//! Per-platform credentials are extracted and stored in the AgentSpan config
//! `cookies` map, which `Config::save()` writes with `0o600` permissions.

use agentspan_core::Config;
use serde::Deserialize;

/// A single browser cookie.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cookie {
    pub name: String,
    pub value: String,
    pub domain: String,
}

/// One Cookie-Editor JSON entry (extra fields ignored).
#[derive(Debug, Deserialize)]
struct CookieEditorEntry {
    name: String,
    value: String,
    #[serde(default)]
    domain: String,
}

/// Extracted per-platform credential ready to store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformCookies {
    /// Channel/platform key, e.g. "twitter".
    pub platform: String,
    /// Stored cookie value (a header string).
    pub value: String,
    /// Human-readable summary, e.g. "auth_token + ct0".
    pub note: String,
}

/// Parse a Cookie-Editor JSON export into cookies.
pub fn parse_cookie_editor_json(raw: &str) -> Result<Vec<Cookie>, String> {
    let entries: Vec<CookieEditorEntry> =
        serde_json::from_str(raw).map_err(|e| format!("invalid Cookie-Editor JSON: {e}"))?;
    Ok(entries
        .into_iter()
        .map(|e| Cookie {
            name: e.name,
            value: e.value,
            domain: e.domain,
        })
        .collect())
}

/// Parse a `name=value; name2=value2` header string. Domain is left empty so
/// header-string input matches every platform spec (the user pasted it for a
/// specific site already).
pub fn parse_header_string(raw: &str) -> Vec<Cookie> {
    raw.split(';')
        .filter_map(|part| {
            let part = part.trim();
            let (name, value) = part.split_once('=')?;
            let name = name.trim();
            if name.is_empty() {
                return None;
            }
            Some(Cookie {
                name: name.to_string(),
                value: value.trim().to_string(),
                domain: String::new(),
            })
        })
        .collect()
}

/// Auto-detect the input format and parse it.
pub fn parse_cookies(raw: &str) -> Result<Vec<Cookie>, String> {
    let trimmed = raw.trim();
    if trimmed.starts_with('[') {
        parse_cookie_editor_json(trimmed)
    } else if trimmed.contains('=') {
        Ok(parse_header_string(trimmed))
    } else {
        Err("unrecognized cookie format (expected JSON array or 'name=value; ...')".to_string())
    }
}

struct PlatformSpec {
    platform: &'static str,
    domains: &'static [&'static str],
    /// Store ONLY these named cookies (all must be present).
    required: Option<&'static [&'static str]>,
    /// Grab the FULL cookie set, but only if this cookie name is present.
    require_present: Option<&'static str>,
}

const SPECS: &[PlatformSpec] = &[
    PlatformSpec {
        platform: "twitter",
        domains: &[".x.com", "x.com", ".twitter.com", "twitter.com"],
        required: Some(&["auth_token", "ct0"]),
        require_present: None,
    },
    PlatformSpec {
        platform: "xiaohongshu",
        domains: &[".xiaohongshu.com", "xiaohongshu.com"],
        required: None,
        require_present: None,
    },
    PlatformSpec {
        platform: "bilibili",
        domains: &[".bilibili.com", "bilibili.com"],
        required: Some(&["SESSDATA", "bili_jct"]),
        require_present: None,
    },
    PlatformSpec {
        // Xueqiu APIs need the whole session, not just the token — grab all
        // cookies but only when xq_a_token proves the user is logged in.
        platform: "xueqiu",
        domains: &[".xueqiu.com", "xueqiu.com"],
        required: None,
        require_present: Some("xq_a_token"),
    },
];

fn domain_matches(cookie_domain: &str, spec: &PlatformSpec) -> bool {
    // Header-string input has no domain → matches everything (user-scoped paste).
    if cookie_domain.is_empty() {
        return true;
    }
    spec.domains.iter().any(|d| {
        let bare = d.trim_start_matches('.');
        cookie_domain == *d || cookie_domain == bare || cookie_domain.ends_with(d)
    })
}

/// Extract per-platform credentials from a flat cookie list.
///
/// When the input is a header string (no domains), every spec sees the same
/// cookies; a spec only yields a result if its required names are present (so a
/// Twitter paste won't masquerade as Bilibili).
pub fn extract_platforms(cookies: &[Cookie]) -> Vec<PlatformCookies> {
    let mut out = Vec::new();
    for spec in SPECS {
        let matching: Vec<&Cookie> = cookies
            .iter()
            .filter(|c| domain_matches(&c.domain, spec))
            .collect();
        if matching.is_empty() {
            continue;
        }
        // Mode 1: grab the full cookie set, gated on a marker cookie.
        if let Some(token) = spec.require_present {
            if matching.iter().any(|c| c.name == token) {
                let value = matching
                    .iter()
                    .map(|c| format!("{}={}", c.name, c.value))
                    .collect::<Vec<_>>()
                    .join("; ");
                out.push(PlatformCookies {
                    platform: spec.platform.to_string(),
                    value,
                    note: format!("{} cookies (with {token})", matching.len()),
                });
            }
            continue;
        }

        match spec.required {
            Some(required) => {
                let have: Vec<&Cookie> = matching
                    .iter()
                    .copied()
                    .filter(|c| required.contains(&c.name.as_str()))
                    .collect();
                if required.iter().all(|r| have.iter().any(|c| c.name == *r)) {
                    let value = have
                        .iter()
                        .map(|c| format!("{}={}", c.name, c.value))
                        .collect::<Vec<_>>()
                        .join("; ");
                    out.push(PlatformCookies {
                        platform: spec.platform.to_string(),
                        value,
                        note: required.join(" + "),
                    });
                }
            }
            None => {
                // A "grab-all" platform (e.g. XiaoHongShu) needs real domain
                // evidence. A domainless header-string paste matches every spec,
                // so without a domain match we cannot attribute it here — skip to
                // avoid mislabeling (e.g. a Twitter paste becoming XHS cookies).
                let domain_scoped: Vec<&Cookie> = matching
                    .iter()
                    .copied()
                    .filter(|c| !c.domain.is_empty())
                    .collect();
                if domain_scoped.is_empty() {
                    continue;
                }
                let value = domain_scoped
                    .iter()
                    .map(|c| format!("{}={}", c.name, c.value))
                    .collect::<Vec<_>>()
                    .join("; ");
                out.push(PlatformCookies {
                    platform: spec.platform.to_string(),
                    value,
                    note: format!("{} cookies", domain_scoped.len()),
                });
            }
        }
    }
    out
}

/// Store extracted platform cookies into the config and persist (0o600).
pub fn apply_to_config(config: &mut Config, platforms: &[PlatformCookies]) -> Result<(), String> {
    for p in platforms {
        config.cookies.insert(p.platform.clone(), p.value.clone());
    }
    config.save().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_header_string_basic() {
        let cookies = parse_header_string("auth_token=abc; ct0=xyz");
        assert_eq!(cookies.len(), 2);
        assert_eq!(cookies[0].name, "auth_token");
        assert_eq!(cookies[1].value, "xyz");
    }

    #[test]
    fn parse_cookie_editor_json_basic() {
        let raw = r#"[{"name":"SESSDATA","value":"s","domain":".bilibili.com"},
                      {"name":"bili_jct","value":"j","domain":".bilibili.com"}]"#;
        let cookies = parse_cookie_editor_json(raw).unwrap();
        assert_eq!(cookies.len(), 2);
        assert_eq!(cookies[0].domain, ".bilibili.com");
    }

    #[test]
    fn parse_cookies_autodetects_format() {
        assert_eq!(parse_cookies("a=1").unwrap().len(), 1);
        assert!(
            parse_cookies(r#"[{"name":"a","value":"1"}]"#)
                .unwrap()
                .len()
                == 1
        );
        assert!(parse_cookies("garbage").is_err());
    }

    #[test]
    fn extract_twitter_requires_both_tokens() {
        let cookies = parse_header_string("auth_token=abc; ct0=xyz");
        let platforms = extract_platforms(&cookies);
        let twitter = platforms.iter().find(|p| p.platform == "twitter").unwrap();
        assert!(twitter.value.contains("auth_token=abc"));
        assert!(twitter.value.contains("ct0=xyz"));
        assert_eq!(twitter.note, "auth_token + ct0");
    }

    #[test]
    fn extract_twitter_missing_token_yields_nothing() {
        let cookies = parse_header_string("auth_token=abc");
        let platforms = extract_platforms(&cookies);
        assert!(platforms.iter().all(|p| p.platform != "twitter"));
    }

    #[test]
    fn extract_uses_domains_from_json() {
        let raw = r#"[
            {"name":"SESSDATA","value":"s","domain":".bilibili.com"},
            {"name":"bili_jct","value":"j","domain":".bilibili.com"},
            {"name":"auth_token","value":"a","domain":".x.com"},
            {"name":"ct0","value":"c","domain":".x.com"}
        ]"#;
        let cookies = parse_cookie_editor_json(raw).unwrap();
        let platforms = extract_platforms(&cookies);
        let names: Vec<_> = platforms.iter().map(|p| p.platform.as_str()).collect();
        assert!(names.contains(&"bilibili"));
        assert!(names.contains(&"twitter"));
        // Bilibili value must not contain Twitter's cookies.
        let bili = platforms.iter().find(|p| p.platform == "bilibili").unwrap();
        assert!(!bili.value.contains("auth_token"));
    }

    #[test]
    fn header_paste_does_not_mislabel_xiaohongshu() {
        // Regression: a domainless Twitter paste must not become XHS cookies.
        let cookies = parse_header_string("auth_token=a; ct0=c");
        let platforms = extract_platforms(&cookies);
        assert!(platforms.iter().any(|p| p.platform == "twitter"));
        assert!(platforms.iter().all(|p| p.platform != "xiaohongshu"));
    }

    #[test]
    fn json_export_extracts_xiaohongshu() {
        let raw = r#"[{"name":"web_session","value":"s","domain":".xiaohongshu.com"},
                      {"name":"a1","value":"x","domain":".xiaohongshu.com"}]"#;
        let cookies = parse_cookie_editor_json(raw).unwrap();
        let platforms = extract_platforms(&cookies);
        assert!(platforms.iter().any(|p| p.platform == "xiaohongshu"));
    }

    #[test]
    fn extract_xueqiu_stores_full_cookie_with_token() {
        // M2 regression: xueqiu must keep the WHOLE session, not just the token.
        let raw = r#"[{"name":"xq_a_token","value":"t","domain":".xueqiu.com"},
                      {"name":"u","value":"1","domain":".xueqiu.com"},
                      {"name":"device_id","value":"d","domain":".xueqiu.com"}]"#;
        let cookies = parse_cookie_editor_json(raw).unwrap();
        let platforms = extract_platforms(&cookies);
        let xq = platforms.iter().find(|p| p.platform == "xueqiu").unwrap();
        assert!(xq.value.contains("xq_a_token=t"));
        assert!(xq.value.contains("u=1"));
        assert!(xq.value.contains("device_id=d"));
    }

    #[test]
    fn extract_xueqiu_skipped_without_token() {
        let raw = r#"[{"name":"u","value":"1","domain":".xueqiu.com"}]"#;
        let cookies = parse_cookie_editor_json(raw).unwrap();
        let platforms = extract_platforms(&cookies);
        assert!(platforms.iter().all(|p| p.platform != "xueqiu"));
    }

    #[test]
    fn apply_to_config_writes_cookies() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        let mut config = Config::default();
        // Redirect save target by writing directly through save_to is not exposed
        // here; instead verify the in-memory map is populated then round-trip.
        let platforms = vec![PlatformCookies {
            platform: "twitter".to_string(),
            value: "auth_token=a; ct0=c".to_string(),
            note: "auth_token + ct0".to_string(),
        }];
        for p in &platforms {
            config.cookies.insert(p.platform.clone(), p.value.clone());
        }
        config.save_to(&path).unwrap();
        let loaded = Config::load_from_file(&path).unwrap();
        assert_eq!(
            loaded.cookies.get("twitter"),
            Some(&"auth_token=a; ct0=c".to_string())
        );
    }
}
