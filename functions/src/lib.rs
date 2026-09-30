//! Job pattern: decouple a potentially long-running action from the caller who requested it.
//!
//! `domain` is pure (no I/O) and holds every behavioral rule. `adapters` wrap AWS services.
//! The binaries in `src/bin` are thin shells: parse the trigger, call the domain, call an adapter.

pub mod adapters;
pub mod domain;
pub mod runtime;
