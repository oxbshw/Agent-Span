//! Local-vs-server environment detection (drives `install --env auto`).

/// The kind of machine AgentSpan is running on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    /// A developer's desktop/laptop (has a display, browser sessions).
    Local,
    /// A headless server / container / cloud VM.
    Server,
}

impl Environment {
    /// Lowercase label for display.
    pub fn label(self) -> &'static str {
        match self {
            Environment::Local => "local",
            Environment::Server => "server",
        }
    }
}

/// Observable signals that distinguish a server from a local machine.
#[derive(Debug, Clone, Copy, Default)]
pub struct EnvSignals {
    pub ssh: bool,
    pub container: bool,
    pub headless: bool,
    pub cloud: bool,
}

/// Classify an environment from weighted signals (mirrors Agent Reach's scoring:
/// two or more "server points" ⇒ server).
pub fn classify(signals: &EnvSignals) -> Environment {
    let mut score = 0;
    if signals.ssh {
        score += 2;
    }
    if signals.container {
        score += 2;
    }
    if signals.cloud {
        score += 2;
    }
    if signals.headless {
        score += 1;
    }
    if score >= 2 {
        Environment::Server
    } else {
        Environment::Local
    }
}

/// Gather signals from the current process environment / filesystem.
pub fn current_signals() -> EnvSignals {
    let ssh =
        std::env::var_os("SSH_CONNECTION").is_some() || std::env::var_os("SSH_CLIENT").is_some();

    let container = std::path::Path::new("/.dockerenv").exists()
        || std::path::Path::new("/run/.containerenv").exists();

    // Headless only matters on unix-likes; Windows desktops always have a session.
    let headless = if cfg!(windows) {
        false
    } else {
        std::env::var_os("DISPLAY").is_none() && std::env::var_os("WAYLAND_DISPLAY").is_none()
    };

    let cloud = detect_cloud();

    EnvSignals {
        ssh,
        container,
        headless,
        cloud,
    }
}

fn detect_cloud() -> bool {
    for path in ["/sys/class/dmi/id/product_name", "/sys/hypervisor/uuid"] {
        if let Ok(content) = std::fs::read_to_string(path) {
            let lower = content.to_lowercase();
            if [
                "amazon",
                "google",
                "microsoft",
                "digitalocean",
                "linode",
                "vultr",
                "hetzner",
            ]
            .iter()
            .any(|v| lower.contains(v))
            {
                return true;
            }
        }
    }
    false
}

/// Detect the current environment.
pub fn detect() -> Environment {
    classify(&current_signals())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ssh_alone_is_server() {
        let s = EnvSignals {
            ssh: true,
            ..Default::default()
        };
        assert_eq!(classify(&s), Environment::Server);
    }

    #[test]
    fn headless_alone_is_local() {
        let s = EnvSignals {
            headless: true,
            ..Default::default()
        };
        assert_eq!(classify(&s), Environment::Local);
    }

    #[test]
    fn container_is_server() {
        let s = EnvSignals {
            container: true,
            ..Default::default()
        };
        assert_eq!(classify(&s), Environment::Server);
    }

    #[test]
    fn clean_desktop_is_local() {
        assert_eq!(classify(&EnvSignals::default()), Environment::Local);
    }

    #[test]
    fn headless_plus_cloud_is_server() {
        let s = EnvSignals {
            headless: true,
            cloud: true,
            ..Default::default()
        };
        assert_eq!(classify(&s), Environment::Server);
    }

    #[test]
    fn labels() {
        assert_eq!(Environment::Local.label(), "local");
        assert_eq!(Environment::Server.label(), "server");
    }
}
