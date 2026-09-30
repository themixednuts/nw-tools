//! `nw-tools` library surface.
//!
//! `nw-tools` is primarily the `nw-tools` CLI binary (`src/main.rs`); this lib
//! target exists so integration tests under `tests/` can exercise modules
//! (namely [`native_port`] and [`grep`]) without going through the CLI process.

pub mod cache;
pub mod extract;
pub mod fuzzy;
pub mod grep;
pub mod index;
pub mod native_port;
pub mod resources;
pub mod support;
pub mod ui;
