//! Shared native GameData manager evidence and semantic contracts.
//!
//! This crate owns manager shapes, table families, key and duplicate policies,
//! manager dependencies, and Ghidra evidence recovered from New World. The
//! self-contained Rust, TypeScript, and Go emitters consume the same contracts;
//! repository-integrated targets adapt them at their asset/runtime boundary.

pub mod manager;
pub mod naming;
pub mod native;
pub mod symbols;
