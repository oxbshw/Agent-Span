//! Integration test crate: boots the real AgentSpan HTTP API and exercises it
//! end-to-end. These tests catch wiring bugs that handler-level unit tests
//! cannot (middleware order, route collisions, body limits, auth propagation).
//!
//! Shared helpers live in `tests/common/mod.rs`.
