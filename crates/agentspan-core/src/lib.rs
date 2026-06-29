//! Core traits, types, errors, and configuration for AgentSpan.

pub mod analytics;
pub mod auth;
pub mod backend;
pub mod cache;
pub mod channel;
pub mod config;
pub mod error;
pub mod probe;
pub mod profiler;
pub mod types;

pub use analytics::{Analytics, ChannelStats, RequestRecord, Totals};
pub use config::Config;
pub use error::{BackendError, ChannelError, Error};
pub use profiler::{BackendSwap, PerformanceReport, Profiler};
pub use types::{
    BackendHealth, Content, OutputFormat, ProbeResult, ProbeStatus, ReadOptions, SearchOptions,
    SearchResult, Tier,
};
