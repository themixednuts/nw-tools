//! Extract-time legacy asset port: deterministic legacy-to-native mapper,
//! asset-identity planner, and native-source writer (ADR 0027 + ADR 0028).
//!
//! This module is the New World legacy-knowledge home for the offline port. It
//! maps legacy paths to native ones, assigns/preserves asset identity, and
//! writes native source files plus their `.azmeta.json` sidecars into an output
//! `assets/` tree. The engine's asset processor then scans, cooks, and catalogs
//! those native sources; `nw-tools` emits no import manifest or handoff protocol
//! of its own.
//!
//! - [`cli`] — the `nw-tools port` CLI subcommand (pass-1 inventory, pass-2 publish).
//! - [`route`] — the closed ADR-0027 legacy-to-native route registry.
//! - [`exception`] — the reviewed mapping-exception table.
//! - [`identity`] — catalog-preserved / `CreateName`-fallback asset identity
//!   (via [`nw_asset::uuid::AzUuidExt`]).
//! - [`inventory`] — pass-1 mapping over a batch, accumulating a diagnostic report.
//! - [`sidecar`] — the `azoth.source-meta/v1` sidecar contract.
//! - [`writer`] — pass-2 emission of native sources and their sidecars.
//!
//! # Repo boundary
//!
//! Per ADR 0028 `nw-tools` never depends on `az-rs`. The engine owns the
//! `AZ::Uuid::CreateName` derivation and the `azoth.source-meta/v1` sidecar
//! contract; this module ports/mirrors them here (duplication is intended) and
//! the byte-for-byte sidecar fixtures prove the two agree.

mod cli;
mod exception;
mod identity;
mod inventory;
mod normalize;
mod route;
mod sidecar;
pub mod writer;

pub use cli::Cmd;
pub use exception::{Exception, ExceptionOutcome, ExceptionTable, Publication};
pub use identity::{
    AssetId, IdentitySource, assign_asset_id, fallback_asset_id, normalize_source_path, source_guid,
};
pub use inventory::{
    BlockedSource, Collision, CollisionKind, Inventory, MapFailure, MappedSource, Mapper,
    SourceInput,
};
pub use normalize::{NormalizeError, normalize_legacy_path};
pub use route::{DecodedClass, LEGACY_TREES, RouteError, map_native_path};
pub use sidecar::{SIDECAR_SUFFIX, SOURCE_META_SPEC, SourceAssetMeta, serialize_sidecar};
pub use writer::{
    ASSETS_DIR, PayloadReader, WriteError, WriteSummary, sidecar_path, write_native_sources,
};
