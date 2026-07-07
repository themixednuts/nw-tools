use anyhow::{Context, Result};
use nw_asset::AssetId;
use gamedata::release::{
    GameDataDependency, GameDataRelease, GameDataReleaseId, ProjectionHash, SchemaHash,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::game_system_schema::GameSystemDataTablesSchemaReport;

#[derive(Debug, Clone, Serialize)]
pub(super) struct TableAssetIndex {
    pub(super) tables: Vec<TableAssetEntry>,
    pub(super) dependencies: Vec<GameDataDependency>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct TableAssetEntry {
    pub(super) logical_name: String,
    pub(super) source_path: String,
    pub(super) asset_id: AssetId,
    pub(super) schema_hash: SchemaHash,
    pub(super) row_count: u32,
}

pub(super) fn table_set_fingerprint(
    schema_report: &GameSystemDataTablesSchemaReport,
) -> Result<String> {
    let encoded = serde_json::to_vec(schema_report).context("serialize game-data schema report")?;
    Ok(format!("{:x}", Sha256::digest(encoded)))
}

pub(super) fn table_index_projection_hash(table_index: &TableAssetIndex) -> Result<ProjectionHash> {
    let encoded = serde_json::to_vec(table_index).context("serialize table index projection")?;
    Ok(ProjectionHash(Sha256::digest(encoded).into()))
}

pub(super) fn table_index_catalog_hash(table_index: &TableAssetIndex) -> Result<String> {
    let encoded = serde_json::to_vec(&table_index.tables)
        .context("serialize table asset ids for catalog fingerprint")?;
    Ok(format!("{:x}", Sha256::digest(encoded)))
}

pub(super) fn deterministic_release_id(
    release: &GameDataRelease,
    table_index: &TableAssetIndex,
) -> Result<GameDataReleaseId> {
    let encoded = serde_json::to_vec(&(
        &release.game_build,
        &release.asset_catalog_hash,
        &release.table_set_hash,
        release.projection_hash,
        &table_index.tables,
        &table_index.dependencies,
    ))
    .context("serialize game-data release id inputs")?;
    let hash: [u8; 32] = Sha256::digest(encoded).into();
    let mut uuid_bytes = [0u8; 16];
    uuid_bytes.copy_from_slice(&hash[..16]);
    uuid_bytes[6] = (uuid_bytes[6] & 0x0f) | 0x50;
    uuid_bytes[8] = (uuid_bytes[8] & 0x3f) | 0x80;
    Ok(GameDataReleaseId::from_uuid(Uuid::from_bytes(uuid_bytes)))
}

pub(super) fn default_release() -> GameDataRelease {
    GameDataRelease {
        id: GameDataReleaseId::from_uuid(Uuid::nil()),
        game_build: "nw-extract".into(),
        asset_catalog_hash: String::new(),
        table_set_hash: String::new(),
        projection_hash: ProjectionHash([0; 32]),
    }
}
