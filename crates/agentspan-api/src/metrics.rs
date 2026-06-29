//! Process metrics exported in Prometheus text exposition format.

use std::sync::atomic::{AtomicU64, Ordering};

/// Lock-free request counters, rendered at `GET /metrics`.
#[derive(Debug, Default)]
pub struct Metrics {
    requests_total: AtomicU64,
    errors_total: AtomicU64,
    rejected_total: AtomicU64,
    latency_ms_sum: AtomicU64,
}

impl Metrics {
    /// Record a completed request.
    pub fn record(&self, status: u16, latency_ms: u64) {
        self.requests_total.fetch_add(1, Ordering::Relaxed);
        if status >= 400 {
            self.errors_total.fetch_add(1, Ordering::Relaxed);
        }
        self.latency_ms_sum.fetch_add(latency_ms, Ordering::Relaxed);
    }

    /// Record a request shed by the concurrency limiter (HTTP 503).
    pub fn record_rejected(&self) {
        self.rejected_total.fetch_add(1, Ordering::Relaxed);
    }

    /// Render the Prometheus text exposition (version 0.0.4).
    ///
    /// `channels` is the number of registered channels, exported as a gauge.
    pub fn render(&self, channels: usize) -> String {
        let requests = self.requests_total.load(Ordering::Relaxed);
        let errors = self.errors_total.load(Ordering::Relaxed);
        let rejected = self.rejected_total.load(Ordering::Relaxed);
        let latency_sum = self.latency_ms_sum.load(Ordering::Relaxed);

        let mut out = String::with_capacity(768);
        out.push_str("# HELP agentspan_up 1 if the gateway is serving requests.\n");
        out.push_str("# TYPE agentspan_up gauge\n");
        out.push_str("agentspan_up 1\n");

        out.push_str("# HELP agentspan_requests_total Total HTTP requests handled.\n");
        out.push_str("# TYPE agentspan_requests_total counter\n");
        out.push_str(&format!("agentspan_requests_total {requests}\n"));

        out.push_str(
            "# HELP agentspan_request_errors_total Requests that returned status >= 400.\n",
        );
        out.push_str("# TYPE agentspan_request_errors_total counter\n");
        out.push_str(&format!("agentspan_request_errors_total {errors}\n"));

        out.push_str(
            "# HELP agentspan_requests_rejected_total Requests shed by the concurrency limit.\n",
        );
        out.push_str("# TYPE agentspan_requests_rejected_total counter\n");
        out.push_str(&format!("agentspan_requests_rejected_total {rejected}\n"));

        out.push_str("# HELP agentspan_request_latency_ms_sum Cumulative request latency (ms).\n");
        out.push_str("# TYPE agentspan_request_latency_ms_sum counter\n");
        out.push_str(&format!("agentspan_request_latency_ms_sum {latency_sum}\n"));

        out.push_str("# HELP agentspan_channels Registered channels.\n");
        out.push_str("# TYPE agentspan_channels gauge\n");
        out.push_str(&format!("agentspan_channels {channels}\n"));

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_and_renders() {
        let m = Metrics::default();
        m.record(200, 5);
        m.record(500, 10);
        m.record_rejected();

        let text = m.render(24);
        assert!(text.contains("agentspan_requests_total 2"));
        assert!(text.contains("agentspan_request_errors_total 1"));
        assert!(text.contains("agentspan_requests_rejected_total 1"));
        assert!(text.contains("agentspan_request_latency_ms_sum 15"));
        assert!(text.contains("agentspan_channels 24"));
        assert!(text.contains("agentspan_up 1"));
        // Every metric line must be preceded by HELP/TYPE comments.
        assert_eq!(text.matches("# TYPE").count(), 6);
    }
}
