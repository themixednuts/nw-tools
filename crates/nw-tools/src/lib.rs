//! `nw-tools` library surface.
//!
//! `nw-tools` is primarily the `nw-tools` CLI binary (`src/main.rs`); this lib
//! target exists so integration tests under `tests/` can exercise modules
//! (namely [`native_port`]) without going through the CLI process.

pub mod native_port;
pub mod ui;
