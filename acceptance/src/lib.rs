//! Acceptance tests for the job pattern, in Dave Farley's three layers:
//!
//! * `tests/specs`: executable specifications in domain language (no HTTP, no AWS).
//! * [`dsl`]: domain vocabulary; owns multi-step state; returns domain values, never asserts.
//! * [`adapters`]: protocol translation (SigV4-signed HTTP against the deployed API).
//!
//! The target stack is chosen by `STACK_NAME` (default `job-pattern-one-rust-dev`), so the same
//! specifications can verify the Node.js sibling (`job-pattern-one-dev`) for behavioral parity.

pub mod adapters;
pub mod dsl;
