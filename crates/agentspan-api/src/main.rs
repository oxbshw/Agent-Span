//! Axum server entry point.

use agentspan_api::AppState;
use agentspan_core::Config;

const USAGE: &str = "usage: agentspan-api [--host <addr>] [--port <port>]

Starts the AgentSpan gateway API. Host and port default to the loaded
configuration (agentspan.yaml / ~/.agentspan/config.yaml / AGENTSPAN_*
environment variables); these flags override it.";

/// CLI overrides for the standalone API binary.
///
/// The binary used to ignore its arguments entirely, so
/// `agentspan-api --port 18080` silently started on the configured port
/// instead — callers (including our own load-test workflow) had no way to
/// notice. Unknown arguments are now an error.
#[derive(Debug, Default, PartialEq)]
struct Args {
    host: Option<String>,
    port: Option<u16>,
}

/// Parse `--host`/`--port`, accepting both `--flag value` and `--flag=value`.
fn parse_args<I>(args: I) -> Result<Args, String>
where
    I: IntoIterator<Item = String>,
{
    let mut out = Args::default();
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        let (flag, inline) = match arg.split_once('=') {
            Some((f, v)) => (f.to_string(), Some(v.to_string())),
            None => (arg, None),
        };
        match flag.as_str() {
            "--port" | "-p" => {
                let v = inline
                    .or_else(|| iter.next())
                    .ok_or_else(|| "--port requires a value".to_string())?;
                let port = v
                    .parse()
                    .map_err(|_| format!("invalid --port value: {v}"))?;
                out.port = Some(port);
            }
            "--host" => {
                out.host = Some(
                    inline
                        .or_else(|| iter.next())
                        .ok_or_else(|| "--host requires a value".to_string())?,
                );
            }
            other => return Err(format!("unknown argument: {other}\n\n{USAGE}")),
        }
    }
    Ok(out)
}

#[tokio::main]
async fn main() {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    if raw.iter().any(|a| a == "--help" || a == "-h") {
        println!("{USAGE}");
        return;
    }
    let args = match parse_args(raw) {
        Ok(args) => args,
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(2);
        }
    };

    // Defaults bind to 127.0.0.1. In permissive (require_api_key=false) mode,
    // admin routes are gated off entirely; enable require_api_key for a real
    // multi-user deployment before binding to a public address.
    let mut config = Config::load().unwrap_or_default();
    if let Some(host) = args.host {
        config.server.host = host;
    }
    if let Some(port) = args.port {
        config.server.port = port;
    }
    let addr = format!("{}:{}", config.server.host, config.server.port);

    let state = AppState::with_config(config);

    // Start the background self-healing monitor: it probes every channel on an
    // interval and feeds the shared healing snapshots.
    let _healer = state.spawn_healer();
    tracing::info!("self-healing monitor started");

    let app = state.clone().router();

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    println!("AgentSpan API listening on http://{addr}");
    println!("  self-healing monitor running (probes every 30s)");
    if !addr.starts_with("127.") && !addr.starts_with("localhost") {
        println!(
            "  warning: bound to a non-local address — set auth.require_api_key=true \
             to protect admin routes"
        );
    }
    tracing::info!(%addr, "AgentSpan API listening");
    axum::serve(listener, app).await.unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(args: &[&str]) -> Vec<String> {
        args.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn no_args_means_no_overrides() {
        assert_eq!(parse_args(v(&[])).unwrap(), Args::default());
    }

    #[test]
    fn parses_port_in_space_equals_and_short_forms() {
        assert_eq!(
            parse_args(v(&["--port", "18080"])).unwrap().port,
            Some(18080)
        );
        assert_eq!(parse_args(v(&["--port=18080"])).unwrap().port, Some(18080));
        assert_eq!(parse_args(v(&["-p", "9090"])).unwrap().port, Some(9090));
    }

    #[test]
    fn parses_host_and_port_together() {
        let args = parse_args(v(&["--host", "0.0.0.0", "--port", "80"])).unwrap();
        assert_eq!(args.host.as_deref(), Some("0.0.0.0"));
        assert_eq!(args.port, Some(80));
    }

    #[test]
    fn rejects_unknown_missing_and_invalid() {
        // Regression: these used to be silently ignored.
        assert!(parse_args(v(&["--verbose"])).is_err());
        assert!(parse_args(v(&["--port"])).is_err());
        assert!(parse_args(v(&["--port", "not-a-port"])).is_err());
        assert!(parse_args(v(&["--port", "99999"])).is_err()); // > u16::MAX
    }
}
