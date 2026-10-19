//! Umbra core. See `docs/core-api.md`.
//!
//! * [`api`]: the flutter_rust_bridge surface (plain types + functions).
//! * [`engine`]: identity, crypto, store and networking, usable directly from Rust.

pub mod api;
pub mod engine;
mod frb_generated;
mod mock;
