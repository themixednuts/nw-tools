//! Project-owned native GameData manager facts.
//!
//! This crate holds the ported native-manager declarations — manager shapes,
//! table family lists, and key policies recovered from the New World binary —
//! as project data, per `docs/subsystems/gamedata-ownership.md` §2. The
//! `nw-gamedata-codegen` tool consumes these specs as pure mechanism; the
//! future editor reads manager shapes from here.
//!
//! The `Native*` vocabulary types in [`manager`] are a stopgap: the follow-up
//! convergence step translates them onto the az-rs engine descriptors
//! (`GameDataManagerShape`, `KeyPolicy`, `DuplicateKeyPolicy`,
//! `TableFamilyDescriptor`), keeping emitter-only details as a thin annex.

pub mod manager;
pub mod naming;
pub mod native;
pub mod symbols;
