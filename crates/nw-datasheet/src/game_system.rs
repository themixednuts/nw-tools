//! Transform-time GameSystem data table access.
//!
//! Native New World loads designer `.datasheet` files into
//! `Javelin::GameSystemDataManager<Manager, Row, AZ::Crc32>` instances and
//! looks up rows by a CRC key. This module keeps that shape over the current
//! parsed datasheet evidence for `nw-extract`. Runtime code should consume the
//! native game-data products emitted by the transform, not `.datasheet` bytes.
//! This crate is a New World project/transform crate, not an Azoth engine
//! gem; generic table/release contracts live in `gamedata`.

use std::{
    collections::HashMap,
    error::Error as StdError,
    fmt, fs, io,
    num::NonZeroU32,
    path::{Path, PathBuf},
};

use nw_asset::{ASSET_CATALOG_PATH, AssetId, Rasc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{CellValue, ColumnType, Datasheet, ParseError, is_datasheet_path};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Crc32(u32);

impl Crc32 {
    pub const ZERO: Self = Self(0);

    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn value(self) -> u32 {
        self.0
    }

    #[must_use]
    pub fn from_str_lower(value: &str) -> Self {
        let mut hasher = crc32fast::Hasher::new();
        for byte in value.bytes() {
            hasher.update(&[byte.to_ascii_lowercase()]);
        }
        Self(hasher.finalize())
    }
}

pub const ARCHETYPE_TABLE_NAME: &str = "ArchetypeDataTable";
pub const ARCHETYPE_TYPE_NAME: &str = "ArchetypeData";
pub const BACKSTORY_TABLE_NAME: &str = "Backstory";
pub const BACKSTORY_TYPE_NAME: &str = "BackstoryDefinition";
pub const ACHIEVEMENT_TABLE_NAME: &str = "AchievementDataTable";
pub const ACHIEVEMENT_TYPE_NAME: &str = "AchievementData";
pub const ARMOR_APPEARANCE_TABLE_NAME: &str = "ArmorAppearances";
pub const ARMOR_APPEARANCE_TYPE_NAME: &str = "ArmorAppearanceDefinitions";
pub const ARMOR_ITEM_TABLE_NAME: &str = "ArmorItemDefinitions";
pub const ARMOR_ITEM_TYPE_NAME: &str = "ArmorItemDefinitions";
pub const CONSUMABLE_ITEM_TABLE_NAME: &str = "ConsumableItemDefinitions";
pub const CONSUMABLE_ITEM_TYPE_NAME: &str = "ConsumableItemDefinitions";
pub const COOLDOWNS_PLAYER_TABLE_NAME: &str = "Cooldowns_Player";
pub const COOLDOWN_DATA_TYPE_NAME: &str = "CooldownData";
pub const MASTER_ITEM_TYPE_NAME: &str = "MasterItemDefinitions";
pub const PROGRESSION_POOLS_TABLE_NAME: &str = "ProgressionPools";
pub const PROGRESSION_POOL_TYPE_NAME: &str = "ProgressionPoolData";
pub const WEAPON_APPEARANCE_TABLE_NAME: &str = "WeaponAppearanceDefinitions";
pub const WEAPON_APPEARANCE_TYPE_NAME: &str = "WeaponAppearanceDefinitions";
pub const WEAPON_ITEM_TABLE_NAME: &str = "WeaponItemDefinitions";
pub const WEAPON_ITEM_TYPE_NAME: &str = "WeaponItemDefinitions";

const ABILITY_DATA_TYPE_NAME: &str = "AbilityData";
const PROFICIENCY_IMAGE_COLUMNS: [&str; 3] = [
    "ProficiencyImage_1",
    "ProficiencyImage_2",
    "ProficiencyImage_3",
];

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum GameSystemDataError {
    #[error("read datasheet {path:?}: {source}")]
    Read { path: PathBuf, source: io::Error },

    #[error("parse datasheet {path:?}: {source}")]
    Parse { path: PathBuf, source: ParseError },

    #[error("list game-system datasheet assets: {source}")]
    AssetSourceList {
        #[source]
        source: Box<dyn StdError + Send + Sync + 'static>,
    },

    #[error("read game-system datasheet asset {path:?}: {source}")]
    AssetSourceRead {
        path: PathBuf,
        #[source]
        source: Box<dyn StdError + Send + Sync + 'static>,
    },

    #[error("missing game system table `{name}`")]
    MissingTable { name: String },

    #[error(
        "cannot merge game system table `{name}` with incompatible schema: {first:?} and {second:?}"
    )]
    IncompatibleTableSchema {
        name: String,
        first: PathBuf,
        second: PathBuf,
    },

    #[error("table `{table}` has no column `{column}`")]
    MissingColumn { table: String, column: String },

    #[error("table `{table}` has no row for key crc 0x{key_crc:08x}")]
    MissingRow { table: String, key_crc: u32 },

    #[error("no archetype row points at backstory `{backstory_id}`")]
    MissingArchetypeForBackstory { backstory_id: String },

    #[error("table `{table}` has no row with `{column}` = `{value}`")]
    MissingRowByColumnValue {
        table: String,
        column: String,
        value: String,
    },

    #[error("no `{row_type}` row resolved item `{item_id}`")]
    MissingItemDefinition { row_type: String, item_id: String },

    #[error("table `{table}` column `{column}` expected {expected}, got {actual}")]
    WrongCellType {
        table: String,
        column: String,
        expected: &'static str,
        actual: &'static str,
    },

    #[error("table `{table}` column `{column}` value {value} is not a valid {expected}")]
    InvalidNumericValue {
        table: String,
        column: String,
        value: f32,
        expected: &'static str,
    },

    #[error("invalid `{column}` list entry `{entry}`: expected `name:amount`")]
    InvalidNameAmountEntry { column: String, entry: String },

    #[error("invalid `{column}` amount `{amount}` in entry `{entry}`")]
    InvalidNameAmount {
        column: String,
        entry: String,
        amount: String,
        #[source]
        source: std::num::ParseIntError,
    },

    #[error(
        "table `{table}` column `{column}` value `{value}` is not a valid non-negative integer"
    )]
    InvalidIntegerCell {
        table: String,
        column: String,
        value: String,
        #[source]
        source: std::num::ParseIntError,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct GameSystemTable {
    sources: Vec<GameSystemAsset>,
    name: String,
    name_crc: u32,
    type_name: String,
    type_crc: u32,
    columns: Vec<GameSystemColumn>,
    column_by_crc: HashMap<u32, usize>,
    column_by_name: HashMap<String, usize>,
    rows: Vec<GameSystemRow>,
    row_by_key_crc: HashMap<u32, usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameSystemAsset {
    path: PathBuf,
    asset_id: Option<AssetId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameSystemColumn {
    crc: u32,
    name: String,
    column_type: ColumnType,
}

#[derive(Debug, Clone, PartialEq)]
struct GameSystemRow {
    key_crc: u32,
    cells: Vec<GameSystemCell>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GameSystemCell {
    crc: u32,
    value: OwnedCellValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GameSystemTableKey {
    name_crc: u32,
    type_crc: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OwnedCellValue {
    String(String),
    Number(f32),
    Boolean(bool),
}

#[derive(Debug, Clone, Copy)]
pub struct GameSystemRowRef<'a> {
    table: &'a GameSystemTable,
    row_index: usize,
}

/// Object-safe manager surface for registries, reload, and diagnostics.
///
/// Typed row lookup intentionally lives on [`GameSystemDataManager`]. Keeping
/// row access off this trait means callers that know they need
/// `ArchetypeDataManager` keep compile-time row types, while generic registries
/// can still box heterogeneous managers.
pub trait ErasedGameSystemDataManager: fmt::Debug {
    fn table_key(&self) -> GameSystemTableKey;
    fn table_name(&self) -> &str;
    fn row_type_name(&self) -> &str;
    fn row_count(&self) -> usize;
}

/// Typed Native-style `Javelin::GameSystemDataManager` view.
///
/// Native New World specializes one manager per row type, e.g.
/// `GameSystemDataManager<ArchetypeDataManager, ArchetypeData, AZ::Crc32, 0>`.
/// This trait mirrors that shape: the manager owns table identity and turns
/// keyed rows into a concrete typed view.
pub trait GameSystemDataManager: ErasedGameSystemDataManager {
    type Row<'row>
    where
        Self: 'row;

    const TABLE_NAME: &'static str;
    const ROW_TYPE_NAME: &'static str;

    fn table(&self) -> &GameSystemTable;
    fn wrap_row<'row>(row: GameSystemRowRef<'row>) -> Self::Row<'row>
    where
        Self: 'row;

    #[must_use]
    fn row_by_key_crc(&self, key_crc: Crc32) -> Option<Self::Row<'_>> {
        self.table()
            .row_by_key_crc(key_crc.value())
            .map(Self::wrap_row)
    }

    #[must_use]
    fn row_by_key_name(&self, key: &str) -> Option<Self::Row<'_>> {
        self.row_by_key_crc(Crc32::from_str_lower(key))
    }

    /// Return a typed row by native `AZ::Crc32` row key.
    ///
    /// # Errors
    ///
    /// Returns [`GameSystemDataError::MissingRow`] when no row has `key_crc`.
    fn require_row_by_key_crc(&self, key_crc: Crc32) -> Result<Self::Row<'_>, GameSystemDataError> {
        self.table()
            .require_row_by_key_crc(key_crc.value())
            .map(Self::wrap_row)
    }

    /// Return a typed row by native `AZ::Crc32(key)`.
    ///
    /// # Errors
    ///
    /// Returns [`GameSystemDataError::MissingRow`] when no row matches `key`.
    fn require_row_by_key_name(&self, key: &str) -> Result<Self::Row<'_>, GameSystemDataError> {
        self.require_row_by_key_crc(Crc32::from_str_lower(key))
    }
}

/// Source of catalog-selected datasheet assets.
///
/// `newworld-datasheet` owns parsing and native-style game-system indexing;
/// it does not own asset discovery or file I/O. Transform callers adapt their
/// asset-catalog evidence to this surface.
pub trait GameSystemAssetSource {
    type Error: StdError + Send + Sync + 'static;

    /// Return catalog-selected datasheet assets that should be loaded.
    ///
    /// Implementations should use the asset catalog as the identity authority
    /// rather than walking the filesystem.
    fn datasheet_assets(&self) -> Result<Vec<GameSystemAsset>, Self::Error>;

    /// Read one asset's bytes using the same asset identity surface that
    /// produced the path.
    fn read_datasheet(&self, asset: &GameSystemAsset) -> Result<Vec<u8>, Self::Error>;
}

impl GameSystemAssetSource for nw_asset::AssetStore {
    type Error = nw_asset::AssetStoreError;

    fn datasheet_assets(&self) -> Result<Vec<GameSystemAsset>, Self::Error> {
        Ok(self
            .catalog()
            .into_iter()
            .flat_map(nw_asset::AssetCatalog::entries)
            .filter(|entry| is_datasheet_path(entry.relative_path()))
            .map(|entry| {
                GameSystemAsset::with_asset_id(
                    entry.relative_path().to_path_buf(),
                    entry.asset_id(),
                )
            })
            .collect())
    }

    fn read_datasheet(&self, asset: &GameSystemAsset) -> Result<Vec<u8>, Self::Error> {
        match asset.asset_id() {
            Some(asset_id) => self.read_required_id(asset_id),
            None => self.read_required_path(&asset.path().to_string_lossy()),
        }
    }
}

/// Headless asset-catalog source for transform-time game-system datasheets.
///
/// This is the `nw-extract` adapter for New World's shipped
/// `assetcatalog.catalog`.
#[derive(Debug)]
pub struct RascGameSystemAssetSource {
    asset_root: PathBuf,
    catalog: Rasc,
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum RascGameSystemAssetSourceError {
    #[error("load asset catalog {path:?}: {source}")]
    Catalog {
        path: PathBuf,
        #[source]
        source: nw_asset::Error,
    },

    #[error("read asset {path:?}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

impl RascGameSystemAssetSource {
    /// Open `assetcatalog.catalog` under a New World asset root.
    ///
    /// # Errors
    ///
    /// Returns catalog read/parse errors for `assetcatalog.catalog`.
    pub fn open(asset_root: impl AsRef<Path>) -> Result<Self, RascGameSystemAssetSourceError> {
        let asset_root = asset_root.as_ref().to_path_buf();
        let catalog_path = asset_root.join(ASSET_CATALOG_PATH);
        let catalog_bytes =
            fs::read(&catalog_path).map_err(|source| RascGameSystemAssetSourceError::Read {
                path: catalog_path.clone(),
                source,
            })?;
        let catalog = Rasc::parse(&catalog_bytes).map_err(|source| {
            RascGameSystemAssetSourceError::Catalog {
                path: catalog_path,
                source,
            }
        })?;
        Ok(Self {
            asset_root,
            catalog,
        })
    }

    /// Asset catalog used to resolve datasheet paths and ids.
    #[must_use]
    pub fn catalog(&self) -> &Rasc {
        &self.catalog
    }
}

impl GameSystemAssetSource for RascGameSystemAssetSource {
    type Error = RascGameSystemAssetSourceError;

    fn datasheet_assets(&self) -> Result<Vec<GameSystemAsset>, Self::Error> {
        Ok(self
            .catalog
            .iter()
            .filter(|info| is_datasheet_path(info.relative_path()))
            .map(|info| {
                GameSystemAsset::with_asset_id(info.relative_path().to_path_buf(), info.asset_id())
            })
            .collect())
    }

    fn read_datasheet(&self, asset: &GameSystemAsset) -> Result<Vec<u8>, Self::Error> {
        let disk_path = self.asset_root.join(asset.path());
        fs::read(&disk_path).map_err(|source| RascGameSystemAssetSourceError::Read {
            path: disk_path,
            source,
        })
    }
}

impl<T> ErasedGameSystemDataManager for T
where
    T: GameSystemDataManager + fmt::Debug,
{
    fn table_key(&self) -> GameSystemTableKey {
        self.table().key()
    }

    fn table_name(&self) -> &str {
        Self::TABLE_NAME
    }

    fn row_type_name(&self) -> &str {
        Self::ROW_TYPE_NAME
    }

    fn row_count(&self) -> usize {
        self.table().len()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ArchetypeDataManager<'a> {
    table: &'a GameSystemTable,
}

#[derive(Debug, Clone, Copy)]
pub struct StaticBackstoryDataManager<'a> {
    table: &'a GameSystemTable,
}

#[derive(Debug, Clone)]
pub struct CharacterCreationTemplateResolver<'a> {
    archetypes: ArchetypeDataManager<'a>,
    backstories: StaticBackstoryDataManager<'a>,
    game_system: StarterGameSystemManagers<'a>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameAmount {
    pub name: String,
    pub amount: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CharacterCreationTemplate {
    pub archetype_id: String,
    pub archetype_crc: u32,
    pub backstory_id: String,
    pub backstory_crc: u32,
    pub backstory_name: String,
    pub level: u32,
    pub force_ftue: bool,
    pub core_attributes: CoreAttributes,
    pub inventory_items: Vec<NameAmount>,
    pub consumable_items: Vec<NameAmount>,
    pub weapon_masteries: Vec<NameAmount>,
    pub categorical_progression: Vec<NameAmount>,
    pub objective_unlock_override: String,
    pub achievement_unlock_override: String,
    pub achievement_bitset_byte_len: u32,
    pub ftue_loot_bag_game_event: String,
    pub stat_multipliers: Option<Vec<CharacterCreationStatMultiplierTemplate>>,
    pub objective_id_entries: Option<Vec<CharacterCreationObjectiveIdTemplate>>,
    pub active_objectives: Option<Vec<CharacterCreationActiveObjectiveTemplate>>,
    pub objective_task_states: Option<Vec<CharacterCreationObjectiveTaskStateTemplate>>,
    pub game_events: Option<Vec<CharacterCreationGameEventTemplate>>,
    pub items: Vec<CharacterCreationItemTemplate>,
    pub cooldowns: Vec<CharacterCreationCooldownTemplate>,
    pub abilities: Vec<CharacterCreationAbilityTemplate>,
    pub achievement_unlocks: Vec<CharacterCreationAchievementTemplate>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CharacterCreationTemplateTransform {
    pub template: CharacterCreationTemplate,
    pub diagnostics: Vec<GameSystemValidationDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameSystemValidationDiagnostic {
    pub source_table: String,
    pub source_column: String,
    pub source_row: String,
    pub source_row_key_crc: u32,
    pub value: String,
    pub occurrences: u32,
    pub kind: GameSystemValidationDiagnosticKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GameSystemValidationDiagnosticKind {
    MissingForeignKey {
        target_table: String,
        target_column: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharacterCreationItemSource {
    InventoryItem,
    ConsumableItem,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CharacterCreationItemTemplate {
    pub source: CharacterCreationItemSource,
    pub item_id: String,
    pub item_crc: u32,
    pub amount: u32,
    pub item_type: String,
    pub item_class: String,
    pub item_stats_ref: String,
    pub cooldown_id: String,
    pub item_key_name: u32,
    pub item_count: u16,
    pub durability: u32,
    pub paperdoll_slot: Option<u16>,
    pub appearance_id: String,
    pub appearance_crc: u32,
    pub is_non_removable_from_player: bool,
    pub is_bound_to_player: bool,
    pub is_bind_on_pickup: bool,
    pub is_bind_on_equip: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterCreationCooldownTemplate {
    pub cooldown_id: String,
    pub cooldown_crc: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterCreationAbilityTemplate {
    pub ability_id: String,
    pub ability_crc: u32,
    pub initial_points: Option<u32>,
    pub values: Vec<CharacterCreationAbilityValueTemplate>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterCreationAbilityValueTemplate {
    pub key: u32,
    pub value: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterCreationAchievementTemplate {
    pub achievement_id: String,
    pub achievement_index: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterCreationStatMultiplierTemplate {
    pub table_kind: u8,
    pub map_key: u32,
    pub amount: u32,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterCreationObjectiveIdTemplate {
    pub field_kind: u8,
    pub wire_index: u32,
    pub value: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterCreationActiveObjectiveTemplate {
    pub wire_index: u32,
    pub objective_type: u32,
    pub objective_id: u64,
    pub objective_crc: u32,
    pub objective_uuid: Vec<u8>,
    pub parent_objective_id: u64,
    pub objective_task_id: u16,
    pub available: bool,
    pub visible: bool,
    pub tracked: bool,
    pub complete: bool,
    pub poi_entity_id: u64,
    pub has_poi: bool,
    pub task_indices: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterCreationObjectiveTaskStateTemplate {
    pub wire_index: u32,
    pub objective_id: u64,
    pub task_id: u8,
    pub state: u32,
    pub count: u32,
    pub flags: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CharacterCreationGameEventTemplate {
    pub ordinal: u32,
    pub event_id: u64,
    pub phase_id: u32,
    pub start_offset: f32,
    pub end_offset: f32,
    pub state: u32,
    pub active: bool,
    pub score: u32,
    pub rank: u32,
    pub objective_id: u32,
    pub tier: u8,
    pub category: u8,
    pub reward_id: u32,
    pub difficulty: u16,
    pub completion_count: u32,
    pub flags: u32,
}

#[derive(Debug, Clone)]
struct StarterGameSystemManagers<'a> {
    master_items: MasterItemDefinitionsManager<'a>,
    armor_appearances: ArmorAppearanceDefinitionsManager<'a>,
    consumable_items: ConsumableItemDefinitionsManager<'a>,
    weapon_appearances: WeaponAppearanceDefinitionsManager<'a>,
    cooldowns: CooldownsPlayerManager<'a>,
    achievements: AchievementDataManager<'a>,
    progression_pools: ProgressionPoolsManager<'a>,
}

#[derive(Debug, Clone, Copy)]
pub struct MasterItemDefinitionsManager<'a> {
    data_tables: &'a GameSystemDataTables,
}

#[derive(Debug, Clone, Copy)]
pub struct ArmorAppearanceDefinitionsManager<'a> {
    table: &'a GameSystemTable,
}

#[derive(Debug, Clone, Copy)]
pub struct ArmorItemDefinitionsManager<'a> {
    table: &'a GameSystemTable,
}

#[derive(Debug, Clone, Copy)]
pub struct ConsumableItemDefinitionsManager<'a> {
    table: &'a GameSystemTable,
}

#[derive(Debug, Clone, Copy)]
pub struct WeaponAppearanceDefinitionsManager<'a> {
    table: &'a GameSystemTable,
}

#[derive(Debug, Clone, Copy)]
pub struct WeaponItemDefinitionsManager<'a> {
    table: &'a GameSystemTable,
}

#[derive(Debug, Clone, Copy)]
pub struct CooldownsPlayerManager<'a> {
    table: &'a GameSystemTable,
}

#[derive(Debug, Clone, Copy)]
pub struct AchievementDataManager<'a> {
    table: &'a GameSystemTable,
}

#[derive(Debug, Clone, Copy)]
pub struct ProgressionPoolsManager<'a> {
    table: &'a GameSystemTable,
}

#[derive(Debug, Clone, Copy)]
pub struct DefaultAbilityDataManager<'a> {
    data_tables: &'a GameSystemDataTables,
}

#[derive(Debug, Clone, Copy)]
pub struct MasterItemDefinition<'a> {
    row: GameSystemRowRef<'a>,
}

#[derive(Debug, Clone, Copy)]
pub struct ArmorAppearanceDefinition<'a> {
    row: GameSystemRowRef<'a>,
}

#[derive(Debug, Clone, Copy)]
pub struct ArmorItemDefinition<'a> {
    #[allow(dead_code)]
    row: GameSystemRowRef<'a>,
}

#[derive(Debug, Clone, Copy)]
pub struct ConsumableItemDefinition<'a> {
    row: GameSystemRowRef<'a>,
}

#[derive(Debug, Clone, Copy)]
pub struct WeaponAppearanceDefinition<'a> {
    row: GameSystemRowRef<'a>,
}

#[derive(Debug, Clone, Copy)]
pub struct WeaponItemDefinition<'a> {
    #[allow(dead_code)]
    row: GameSystemRowRef<'a>,
}

#[derive(Debug, Clone, Copy)]
pub struct CooldownData<'a> {
    row: GameSystemRowRef<'a>,
}

#[derive(Debug, Clone, Copy)]
pub struct AchievementData<'a> {
    row: GameSystemRowRef<'a>,
}

#[derive(Debug, Clone, Copy)]
pub struct ProgressionPoolData<'a> {
    row: GameSystemRowRef<'a>,
}

#[derive(Debug, Clone, Copy)]
pub struct AbilityData<'a> {
    row: GameSystemRowRef<'a>,
}

#[derive(Debug, Clone, Copy)]
pub struct ArchetypeData<'a> {
    row: GameSystemRowRef<'a>,
}

#[derive(Debug, Clone, Copy)]
pub struct StaticBackstoryData<'a> {
    row: GameSystemRowRef<'a>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoreAttributes {
    pub constitution: f32,
    pub dexterity: f32,
    pub focus: f32,
    pub intelligence: f32,
    pub strength: f32,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct GameSystemDataTables {
    tables: Vec<GameSystemTable>,
    table_by_name: HashMap<String, usize>,
    table_by_crc: HashMap<(u32, u32), usize>,
}

/// Build a typed manager/resolver from loaded game-system data tables.
///
/// This mirrors the native `GameSystemDataManager<Manager, Row, Key, ...>`
/// pattern: callers ask for a manager type, and the data tables supply the
/// rows behind that manager.
pub trait FromGameSystemDataTables<'a>: Sized {
    /// Build `Self` from `data_tables`.
    ///
    /// # Errors
    ///
    /// Returns [`GameSystemDataError::MissingTable`] when the manager's native
    /// table is absent, or the manager-specific validation error.
    fn from_data_tables(data_tables: &'a GameSystemDataTables)
    -> Result<Self, GameSystemDataError>;
}

impl GameSystemDataTables {
    /// Build a typed native-style manager or resolver from these data tables.
    ///
    /// # Errors
    ///
    /// Returns the manager construction error.
    pub fn manager<'a, M>(&'a self) -> Result<M, GameSystemDataError>
    where
        M: FromGameSystemDataTables<'a>,
    {
        M::from_data_tables(self)
    }

    /// Load every `.datasheet` under `root`, recursively, into table objects.
    ///
    /// The path walk is sorted to keep duplicate-table diagnostics stable.
    ///
    /// # Errors
    ///
    /// Returns an error when a datasheet cannot be read or parsed, or when two
    /// files declare the same sheet name/table id.
    pub fn load_dir(root: impl AsRef<Path>) -> Result<Self, GameSystemDataError> {
        let mut paths = Vec::new();
        collect_datasheet_paths(root.as_ref(), &mut paths)?;
        paths.sort();

        let mut data_tables = Self::default();
        for path in paths {
            data_tables.insert(GameSystemTable::load_file(&path)?)?;
        }
        Ok(data_tables)
    }

    /// Load catalog-selected datasheets through an asset source.
    ///
    /// The source owns asset discovery and byte reads. This keeps transform
    /// code catalog-driven instead of passing raw filesystem roots into the
    /// game-system table layer.
    ///
    /// # Errors
    ///
    /// Returns an error when asset enumeration/read fails, when a datasheet
    /// cannot be parsed, or when duplicate table schemas are incompatible.
    pub fn load_from_source<S>(source: &S) -> Result<Self, GameSystemDataError>
    where
        S: GameSystemAssetSource + ?Sized,
    {
        let mut assets = source
            .datasheet_assets()
            .map_err(|source| GameSystemDataError::AssetSourceList {
                source: Box::new(source),
            })?
            .into_iter()
            .filter(|asset| is_datasheet_path(asset.path()))
            .collect::<Vec<_>>();
        assets.sort_by(|a, b| a.path().cmp(b.path()).then(a.asset_id().cmp(&b.asset_id())));

        let mut data_tables = Self::default();
        for asset in assets {
            let bytes = source.read_datasheet(&asset).map_err(|source| {
                GameSystemDataError::AssetSourceRead {
                    path: asset.path().to_path_buf(),
                    source: Box::new(source),
                }
            })?;
            data_tables.insert(GameSystemTable::parse_asset(asset, &bytes)?)?;
        }
        Ok(data_tables)
    }

    /// Insert a table into the table set.
    ///
    /// # Errors
    ///
    /// Returns [`GameSystemDataError::IncompatibleTableSchema`] when another
    /// table with the same sheet-name/type CRC pair has a different column
    /// layout.
    pub fn insert(&mut self, table: GameSystemTable) -> Result<(), GameSystemDataError> {
        let table_key = (table.name_crc, table.type_crc);
        if let Some(existing) = self.table_by_crc.get(&table_key).copied() {
            return self.tables[existing].merge_rows_from(table);
        }

        let index = self.tables.len();
        self.table_by_name
            .entry(table.name.clone())
            .or_insert(index);
        self.table_by_crc.insert(table_key, index);
        self.tables.push(table);
        Ok(())
    }

    #[must_use]
    #[inline]
    pub fn tables(&self) -> &[GameSystemTable] {
        &self.tables
    }

    #[must_use]
    pub fn table(&self, name: &str) -> Option<&GameSystemTable> {
        self.table_by_name
            .get(name)
            .copied()
            .or_else(|| {
                let name_crc = Crc32::from_str_lower(name).value();
                self.tables
                    .iter()
                    .position(|table| table.name_crc == name_crc)
            })
            .and_then(|index| self.tables.get(index))
    }

    /// Return a table by native table name.
    ///
    /// # Errors
    ///
    /// Returns [`GameSystemDataError::MissingTable`] when the table set does not
    /// contain `name`.
    pub fn require_table(&self, name: &str) -> Result<&GameSystemTable, GameSystemDataError> {
        self.table(name)
            .ok_or_else(|| GameSystemDataError::MissingTable {
                name: name.to_owned(),
            })
    }

    #[must_use]
    pub fn table_by_key(&self, key: GameSystemTableKey) -> Option<&GameSystemTable> {
        self.table_by_crc
            .get(&(key.name_crc, key.type_crc))
            .copied()
            .and_then(|index| self.tables.get(index))
    }

    /// Return a table by native `(sheet name CRC, row type CRC)`.
    ///
    /// # Errors
    ///
    /// Returns [`GameSystemDataError::MissingTable`] when the table set does not
    /// contain the matching table/type pair.
    pub fn require_table_by_key(
        &self,
        key: GameSystemTableKey,
    ) -> Result<&GameSystemTable, GameSystemDataError> {
        self.table_by_key(key)
            .ok_or_else(|| GameSystemDataError::MissingTable {
                name: format!(
                    "name_crc=0x{:08x}, type_crc=0x{:08x}",
                    key.name_crc, key.type_crc
                ),
            })
    }

    /// Native `ArchetypeDataTable` manager.
    ///
    /// # Errors
    ///
    /// Returns [`GameSystemDataError::MissingTable`] when the table set has not
    /// loaded the archetype table.
    pub fn archetypes(&self) -> Result<ArchetypeDataManager<'_>, GameSystemDataError> {
        self.manager()
    }

    /// Native `Backstory` manager.
    ///
    /// # Errors
    ///
    /// Returns [`GameSystemDataError::MissingTable`] when the table set has not
    /// loaded the backstory table.
    pub fn backstories(&self) -> Result<StaticBackstoryDataManager<'_>, GameSystemDataError> {
        self.manager()
    }

    /// Native starter-item manager over every `MasterItemDefinitions*`
    /// table. New World spreads the row type across many source tables, so
    /// lookup is by row type plus native row key rather than one sheet name.
    #[must_use]
    pub const fn master_item_definitions(&self) -> MasterItemDefinitionsManager<'_> {
        MasterItemDefinitionsManager::new(self)
    }

    /// Native `ArmorAppearances` manager.
    ///
    /// # Errors
    ///
    /// Returns [`GameSystemDataError::MissingTable`] when the table set has not
    /// loaded the armor appearance table.
    pub fn armor_appearance_definitions(
        &self,
    ) -> Result<ArmorAppearanceDefinitionsManager<'_>, GameSystemDataError> {
        self.manager()
    }

    /// Native `ArmorItemDefinitions` manager.
    ///
    /// # Errors
    ///
    /// Returns [`GameSystemDataError::MissingTable`] when the table set has not
    /// loaded the armor item table.
    pub fn armor_item_definitions(
        &self,
    ) -> Result<ArmorItemDefinitionsManager<'_>, GameSystemDataError> {
        self.manager()
    }

    /// Native `ConsumableItemDefinitions` manager.
    ///
    /// # Errors
    ///
    /// Returns [`GameSystemDataError::MissingTable`] when the table set has not
    /// loaded the consumable item table.
    pub fn consumable_item_definitions(
        &self,
    ) -> Result<ConsumableItemDefinitionsManager<'_>, GameSystemDataError> {
        self.manager()
    }

    /// Native `WeaponAppearanceDefinitions` manager.
    ///
    /// # Errors
    ///
    /// Returns [`GameSystemDataError::MissingTable`] when the table set has not
    /// loaded the weapon appearance table.
    pub fn weapon_appearance_definitions(
        &self,
    ) -> Result<WeaponAppearanceDefinitionsManager<'_>, GameSystemDataError> {
        self.manager()
    }

    /// Native `WeaponItemDefinitions` manager.
    ///
    /// # Errors
    ///
    /// Returns [`GameSystemDataError::MissingTable`] when the table set has not
    /// loaded the weapon item table.
    pub fn weapon_item_definitions(
        &self,
    ) -> Result<WeaponItemDefinitionsManager<'_>, GameSystemDataError> {
        self.manager()
    }

    /// Native `Cooldowns_Player` manager.
    ///
    /// # Errors
    ///
    /// Returns [`GameSystemDataError::MissingTable`] when the table set has not
    /// loaded the player cooldown table.
    pub fn player_cooldowns(&self) -> Result<CooldownsPlayerManager<'_>, GameSystemDataError> {
        self.manager()
    }

    /// Native `AchievementDataTable` manager.
    ///
    /// # Errors
    ///
    /// Returns [`GameSystemDataError::MissingTable`] when the table set has not
    /// loaded the achievement table.
    pub fn achievements(&self) -> Result<AchievementDataManager<'_>, GameSystemDataError> {
        self.manager()
    }

    /// Native `ProgressionPools` manager.
    ///
    /// # Errors
    ///
    /// Returns [`GameSystemDataError::MissingTable`] when the table set has not
    /// loaded the progression-pool table.
    pub fn progression_pools(&self) -> Result<ProgressionPoolsManager<'_>, GameSystemDataError> {
        self.manager()
    }

    /// Data-driven view over all `AbilityData` row tables.
    #[must_use]
    pub const fn default_abilities(&self) -> DefaultAbilityDataManager<'_> {
        DefaultAbilityDataManager::new(self)
    }

    /// Resolver for character-creation/FTUE template data.
    ///
    /// # Errors
    ///
    /// Returns [`GameSystemDataError::MissingTable`] when one of the required
    /// native managers is absent.
    pub fn character_creation_template_resolver(
        &self,
    ) -> Result<CharacterCreationTemplateResolver<'_>, GameSystemDataError> {
        self.manager()
    }

    /// Built-in managers exposed through the object-safe diagnostics surface.
    ///
    /// # Errors
    ///
    /// Returns [`GameSystemDataError::MissingTable`] when one of the required
    /// native tables is absent.
    pub fn core_managers(
        &self,
    ) -> Result<Vec<Box<dyn ErasedGameSystemDataManager + '_>>, GameSystemDataError> {
        Ok(vec![
            Box::new(self.archetypes()?),
            Box::new(self.backstories()?),
        ])
    }
}

fn require_manager_table<'a>(
    data_tables: &'a GameSystemDataTables,
    table_name: &'static str,
    row_type_name: &'static str,
) -> Result<&'a GameSystemTable, GameSystemDataError> {
    data_tables.require_table_by_key(GameSystemTableKey::from_names(table_name, row_type_name))
}

macro_rules! impl_table_backed_manager {
    ($manager:ident, $table_name:expr, $row_type_name:expr) => {
        impl<'a> FromGameSystemDataTables<'a> for $manager<'a> {
            fn from_data_tables(
                data_tables: &'a GameSystemDataTables,
            ) -> Result<Self, GameSystemDataError> {
                require_manager_table(data_tables, $table_name, $row_type_name).map(Self::new)
            }
        }
    };
}

impl_table_backed_manager!(
    ArchetypeDataManager,
    ARCHETYPE_TABLE_NAME,
    ARCHETYPE_TYPE_NAME
);
impl_table_backed_manager!(
    StaticBackstoryDataManager,
    BACKSTORY_TABLE_NAME,
    BACKSTORY_TYPE_NAME
);
impl_table_backed_manager!(
    ArmorAppearanceDefinitionsManager,
    ARMOR_APPEARANCE_TABLE_NAME,
    ARMOR_APPEARANCE_TYPE_NAME
);
impl_table_backed_manager!(
    ArmorItemDefinitionsManager,
    ARMOR_ITEM_TABLE_NAME,
    ARMOR_ITEM_TYPE_NAME
);
impl_table_backed_manager!(
    ConsumableItemDefinitionsManager,
    CONSUMABLE_ITEM_TABLE_NAME,
    CONSUMABLE_ITEM_TYPE_NAME
);
impl_table_backed_manager!(
    WeaponAppearanceDefinitionsManager,
    WEAPON_APPEARANCE_TABLE_NAME,
    WEAPON_APPEARANCE_TYPE_NAME
);
impl_table_backed_manager!(
    WeaponItemDefinitionsManager,
    WEAPON_ITEM_TABLE_NAME,
    WEAPON_ITEM_TYPE_NAME
);
impl_table_backed_manager!(
    CooldownsPlayerManager,
    COOLDOWNS_PLAYER_TABLE_NAME,
    COOLDOWN_DATA_TYPE_NAME
);
impl_table_backed_manager!(
    AchievementDataManager,
    ACHIEVEMENT_TABLE_NAME,
    ACHIEVEMENT_TYPE_NAME
);
impl_table_backed_manager!(
    ProgressionPoolsManager,
    PROGRESSION_POOLS_TABLE_NAME,
    PROGRESSION_POOL_TYPE_NAME
);

impl<'a> FromGameSystemDataTables<'a> for MasterItemDefinitionsManager<'a> {
    fn from_data_tables(
        data_tables: &'a GameSystemDataTables,
    ) -> Result<Self, GameSystemDataError> {
        Ok(Self::new(data_tables))
    }
}

impl<'a> FromGameSystemDataTables<'a> for DefaultAbilityDataManager<'a> {
    fn from_data_tables(
        data_tables: &'a GameSystemDataTables,
    ) -> Result<Self, GameSystemDataError> {
        Ok(Self::new(data_tables))
    }
}

impl<'a> FromGameSystemDataTables<'a> for CharacterCreationTemplateResolver<'a> {
    fn from_data_tables(
        data_tables: &'a GameSystemDataTables,
    ) -> Result<Self, GameSystemDataError> {
        Ok(Self {
            archetypes: data_tables.manager()?,
            backstories: data_tables.manager()?,
            game_system: StarterGameSystemManagers::from_data_tables(data_tables)?,
        })
    }
}

impl GameSystemAsset {
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            asset_id: None,
        }
    }

    #[must_use]
    pub fn with_asset_id(path: impl Into<PathBuf>, asset_id: AssetId) -> Self {
        Self {
            path: path.into(),
            asset_id: Some(asset_id),
        }
    }

    #[must_use]
    #[inline]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    #[inline]
    pub const fn asset_id(&self) -> Option<AssetId> {
        self.asset_id
    }
}

impl GameSystemTable {
    /// Load and parse a single `.datasheet` into an owned table.
    ///
    /// # Errors
    ///
    /// Returns read/parse errors from the source file.
    pub fn load_file(path: impl AsRef<Path>) -> Result<Self, GameSystemDataError> {
        let path = path.as_ref();
        let bytes = fs::read(path).map_err(|source| GameSystemDataError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        let sheet = Datasheet::parse(&bytes).map_err(|source| GameSystemDataError::Parse {
            path: path.to_path_buf(),
            source,
        })?;
        Ok(Self::from_datasheet(&sheet).with_source(path))
    }

    /// Parse a catalog-relative datasheet asset from bytes.
    ///
    /// `asset_path` is retained as the table's source path so diagnostics use
    /// the same path vocabulary as the asset catalog and native logs.
    ///
    /// # Errors
    ///
    /// Returns parse errors from the asset bytes.
    pub fn parse_asset_bytes(
        asset_path: impl AsRef<Path>,
        bytes: &[u8],
    ) -> Result<Self, GameSystemDataError> {
        Self::parse_asset(
            GameSystemAsset::new(asset_path.as_ref().to_path_buf()),
            bytes,
        )
    }

    /// Parse a catalog-selected datasheet asset from bytes.
    ///
    /// The asset descriptor is retained so transform diagnostics can report
    /// both the catalog path and the original `AZ::Data::AssetId`.
    ///
    /// # Errors
    ///
    /// Returns parse errors from the asset bytes.
    pub fn parse_asset(asset: GameSystemAsset, bytes: &[u8]) -> Result<Self, GameSystemDataError> {
        let asset_path = asset.path();
        debug_assert!(
            !asset_path.is_absolute(),
            "asset catalog paths must be relative"
        );
        let sheet = Datasheet::parse(bytes).map_err(|source| GameSystemDataError::Parse {
            path: asset_path.to_path_buf(),
            source,
        })?;
        Ok(Self::from_datasheet(&sheet).with_source_asset(asset))
    }

    #[must_use]
    pub fn from_datasheet(sheet: &Datasheet<'_>) -> Self {
        let columns = sheet
            .columns()
            .iter()
            .map(|column| GameSystemColumn {
                crc: column.crc(),
                name: column.name().to_owned(),
                column_type: column.column_type(),
            })
            .collect::<Vec<_>>();

        Self::from_native_columns(
            sheet.name(),
            sheet.name_crc(),
            sheet.type_name(),
            sheet.type_crc(),
            columns,
            sheet.rows().map(|row| {
                let cells = row
                    .cells()
                    .iter()
                    .map(|cell| GameSystemCell {
                        crc: cell.crc(),
                        value: OwnedCellValue::from(*cell.value()),
                    })
                    .collect::<Vec<_>>();
                let key_crc = cells.first().map_or(0, native_row_key_crc);
                (key_crc, cells)
            }),
        )
    }

    /// Build a table from decoded native GameData column/row payloads.
    #[must_use]
    pub fn from_native_columns(
        name: impl Into<String>,
        name_crc: u32,
        type_name: impl Into<String>,
        type_crc: u32,
        columns: Vec<GameSystemColumn>,
        rows: impl IntoIterator<Item = (u32, Vec<GameSystemCell>)>,
    ) -> Self {
        let mut column_by_crc = HashMap::with_capacity(columns.len());
        let mut column_by_name = HashMap::with_capacity(columns.len());
        for (index, column) in columns.iter().enumerate() {
            column_by_crc.insert(column.crc, index);
            column_by_name.insert(column.name.clone(), index);
        }

        let mut table_rows = Vec::new();
        let mut row_by_key_crc = HashMap::new();
        for (key_crc, cells) in rows {
            let index = table_rows.len();
            row_by_key_crc.entry(key_crc).or_insert(index);
            table_rows.push(GameSystemRow { key_crc, cells });
        }

        Self {
            sources: Vec::new(),
            name: name.into(),
            name_crc,
            type_name: type_name.into(),
            type_crc,
            columns,
            column_by_crc,
            column_by_name,
            rows: table_rows,
            row_by_key_crc,
        }
    }

    #[must_use]
    #[inline]
    pub const fn key(&self) -> GameSystemTableKey {
        GameSystemTableKey {
            name_crc: self.name_crc,
            type_crc: self.type_crc,
        }
    }

    #[must_use]
    pub fn with_source(mut self, source: impl AsRef<Path>) -> Self {
        self.sources
            .push(GameSystemAsset::new(source.as_ref().to_path_buf()));
        self
    }

    #[must_use]
    pub fn with_source_asset(mut self, source: GameSystemAsset) -> Self {
        self.sources.push(source);
        self
    }

    /// Return the first source path for compatibility with single-file callers.
    ///
    /// Tables can aggregate rows from multiple datasheet assets. Use
    /// [`Self::sources`] when source completeness matters.
    #[must_use]
    #[inline]
    pub fn source(&self) -> Option<&Path> {
        self.sources.first().map(GameSystemAsset::path)
    }

    /// Return all datasheet assets that contributed rows to this table.
    #[must_use]
    #[inline]
    pub fn sources(&self) -> &[GameSystemAsset] {
        &self.sources
    }

    /// Return the first source asset for compatibility with single-file callers.
    ///
    /// Tables can aggregate rows from multiple datasheet assets. Use
    /// [`Self::sources`] when source completeness matters.
    #[must_use]
    #[inline]
    pub fn source_asset(&self) -> Option<&GameSystemAsset> {
        self.sources.first()
    }

    /// Return the first source asset id for compatibility with single-file callers.
    ///
    /// Tables can aggregate rows from multiple datasheet assets. Use
    /// [`Self::source_asset_ids`] when source completeness matters.
    #[must_use]
    #[inline]
    pub fn source_asset_id(&self) -> Option<AssetId> {
        self.source_asset().and_then(GameSystemAsset::asset_id)
    }

    #[inline]
    pub fn source_asset_ids(&self) -> impl Iterator<Item = AssetId> + '_ {
        self.sources.iter().filter_map(GameSystemAsset::asset_id)
    }

    #[must_use]
    #[inline]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    #[inline]
    pub const fn name_crc(&self) -> u32 {
        self.name_crc
    }

    #[must_use]
    #[inline]
    pub fn type_name(&self) -> &str {
        &self.type_name
    }

    #[must_use]
    #[inline]
    pub const fn type_crc(&self) -> u32 {
        self.type_crc
    }

    #[must_use]
    #[inline]
    pub fn columns(&self) -> &[GameSystemColumn] {
        &self.columns
    }

    #[must_use]
    #[inline]
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    #[must_use]
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    #[must_use]
    pub fn column_index(&self, name: &str) -> Option<usize> {
        self.column_by_name.get(name).copied().or_else(|| {
            self.column_by_crc
                .get(&Crc32::from_str_lower(name).value())
                .copied()
        })
    }

    #[must_use]
    pub fn column_index_by_crc(&self, crc: u32) -> Option<usize> {
        self.column_by_crc.get(&crc).copied()
    }

    #[must_use]
    pub fn row_by_key_crc(&self, key_crc: u32) -> Option<GameSystemRowRef<'_>> {
        self.row_by_key_crc
            .get(&key_crc)
            .copied()
            .map(|row_index| GameSystemRowRef {
                table: self,
                row_index,
            })
    }

    #[must_use]
    #[inline]
    pub fn row_by_key_name(&self, key: &str) -> Option<GameSystemRowRef<'_>> {
        self.row_by_key_crc(Crc32::from_str_lower(key).value())
    }

    /// Return a row by native `AZ::Crc32` row key.
    ///
    /// # Errors
    ///
    /// Returns [`GameSystemDataError::MissingRow`] when no row has `key_crc`.
    pub fn require_row_by_key_crc(
        &self,
        key_crc: u32,
    ) -> Result<GameSystemRowRef<'_>, GameSystemDataError> {
        self.row_by_key_crc(key_crc)
            .ok_or_else(|| GameSystemDataError::MissingRow {
                table: self.name.clone(),
                key_crc,
            })
    }

    /// Return a row by native `AZ::Crc32` of `key`.
    ///
    /// # Errors
    ///
    /// Returns [`GameSystemDataError::MissingRow`] when no row matches `key`.
    pub fn require_row_by_key_name(
        &self,
        key: &str,
    ) -> Result<GameSystemRowRef<'_>, GameSystemDataError> {
        self.require_row_by_key_crc(Crc32::from_str_lower(key).value())
    }

    fn row(&self, row_index: usize) -> &GameSystemRow {
        &self.rows[row_index]
    }

    #[must_use]
    pub fn row_at_index(&self, row_index: usize) -> Option<GameSystemRowRef<'_>> {
        (row_index < self.rows.len()).then_some(GameSystemRowRef {
            table: self,
            row_index,
        })
    }

    pub fn row_refs(&self) -> impl Iterator<Item = GameSystemRowRef<'_>> {
        (0..self.rows.len()).map(|row_index| GameSystemRowRef {
            table: self,
            row_index,
        })
    }

    fn merge_rows_from(&mut self, other: Self) -> Result<(), GameSystemDataError> {
        if self.columns != other.columns {
            return Err(GameSystemDataError::IncompatibleTableSchema {
                name: self.name.clone(),
                first: self.source().map(Path::to_path_buf).unwrap_or_default(),
                second: other.source().map(Path::to_path_buf).unwrap_or_default(),
            });
        }

        for row in other.rows {
            let index = self.rows.len();
            self.row_by_key_crc.entry(row.key_crc).or_insert(index);
            self.rows.push(row);
        }
        self.sources.extend(other.sources);
        Ok(())
    }
}

impl GameSystemTableKey {
    #[must_use]
    #[inline]
    pub const fn new(name_crc: u32, type_crc: u32) -> Self {
        Self { name_crc, type_crc }
    }

    #[must_use]
    #[inline]
    pub fn from_names(name: &str, type_name: &str) -> Self {
        Self {
            name_crc: Crc32::from_str_lower(name).value(),
            type_crc: Crc32::from_str_lower(type_name).value(),
        }
    }

    #[must_use]
    #[inline]
    pub const fn name_crc(self) -> u32 {
        self.name_crc
    }

    #[must_use]
    #[inline]
    pub const fn type_crc(self) -> u32 {
        self.type_crc
    }
}

impl GameSystemColumn {
    #[must_use]
    pub fn new(crc: u32, name: impl Into<String>, column_type: ColumnType) -> Self {
        Self {
            crc,
            name: name.into(),
            column_type,
        }
    }
}

impl GameSystemCell {
    #[must_use]
    pub fn new(crc: u32, value: OwnedCellValue) -> Self {
        Self { crc, value }
    }
}

impl GameSystemColumn {
    #[must_use]
    #[inline]
    pub const fn crc(&self) -> u32 {
        self.crc
    }

    #[must_use]
    #[inline]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    #[inline]
    pub const fn column_type(&self) -> ColumnType {
        self.column_type
    }
}

impl GameSystemCell {
    #[must_use]
    #[inline]
    pub const fn crc(&self) -> u32 {
        self.crc
    }

    #[must_use]
    #[inline]
    pub const fn value(&self) -> &OwnedCellValue {
        &self.value
    }
}

impl OwnedCellValue {
    #[must_use]
    #[inline]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value),
            Self::Number(_) | Self::Boolean(_) => None,
        }
    }

    #[must_use]
    #[inline]
    pub const fn as_f32(&self) -> Option<f32> {
        match self {
            Self::Number(value) => Some(*value),
            Self::String(_) | Self::Boolean(_) => None,
        }
    }

    #[must_use]
    #[inline]
    pub const fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Boolean(value) => Some(*value),
            Self::String(_) | Self::Number(_) => None,
        }
    }

    #[must_use]
    pub const fn type_name(&self) -> &'static str {
        match self {
            Self::String(_) => "string",
            Self::Number(_) => "number",
            Self::Boolean(_) => "boolean",
        }
    }
}

impl fmt::Display for OwnedCellValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::String(value) => f.write_str(value),
            Self::Number(value) => value.fmt(f),
            Self::Boolean(value) => value.fmt(f),
        }
    }
}

impl From<CellValue<'_>> for OwnedCellValue {
    fn from(value: CellValue<'_>) -> Self {
        match value {
            CellValue::String(value) => Self::String(value.to_owned()),
            CellValue::Number(value) => Self::Number(value),
            CellValue::Boolean(value) => Self::Boolean(value),
        }
    }
}

impl<'a> GameSystemRowRef<'a> {
    #[must_use]
    #[inline]
    pub fn table(&self) -> &'a GameSystemTable {
        self.table
    }

    #[must_use]
    #[inline]
    pub const fn index(&self) -> usize {
        self.row_index
    }

    #[must_use]
    #[inline]
    pub fn key_crc(&self) -> u32 {
        self.table.row(self.row_index).key_crc
    }

    #[must_use]
    #[inline]
    pub fn cells(&self) -> &'a [GameSystemCell] {
        &self.table.row(self.row_index).cells
    }

    #[must_use]
    pub fn cell(&self, column: &str) -> Option<&'a GameSystemCell> {
        let index = self.table.column_index(column)?;
        self.table.row(self.row_index).cells.get(index)
    }

    /// Return a required string cell.
    ///
    /// # Errors
    ///
    /// Returns missing-column or wrong-type errors for invalid access.
    pub fn required_str(&self, column: &str) -> Result<&'a str, GameSystemDataError> {
        let cell = self.required_cell(column)?;
        cell.value
            .as_str()
            .ok_or_else(|| self.wrong_type(column, "string", cell))
    }

    /// Return a required number cell.
    ///
    /// # Errors
    ///
    /// Returns missing-column or wrong-type errors for invalid access.
    pub fn required_f32(&self, column: &str) -> Result<f32, GameSystemDataError> {
        let cell = self.required_cell(column)?;
        cell.value
            .as_f32()
            .ok_or_else(|| self.wrong_type(column, "number", cell))
    }

    /// Return a required boolean cell.
    ///
    /// # Errors
    ///
    /// Returns missing-column or wrong-type errors for invalid access.
    pub fn required_bool(&self, column: &str) -> Result<bool, GameSystemDataError> {
        let cell = self.required_cell(column)?;
        cell.value
            .as_bool()
            .ok_or_else(|| self.wrong_type(column, "boolean", cell))
    }

    fn required_cell(&self, column: &str) -> Result<&'a GameSystemCell, GameSystemDataError> {
        let index =
            self.table
                .column_index(column)
                .ok_or_else(|| GameSystemDataError::MissingColumn {
                    table: self.table.name.clone(),
                    column: column.to_owned(),
                })?;
        Ok(&self.table.row(self.row_index).cells[index])
    }

    fn wrong_type(
        &self,
        column: &str,
        expected: &'static str,
        cell: &GameSystemCell,
    ) -> GameSystemDataError {
        GameSystemDataError::WrongCellType {
            table: self.table.name.clone(),
            column: column.to_owned(),
            expected,
            actual: cell.value.type_name(),
        }
    }
}

impl<'a> GameSystemDataManager for ArchetypeDataManager<'a> {
    type Row<'row>
        = ArchetypeData<'row>
    where
        Self: 'row;

    const TABLE_NAME: &'static str = ARCHETYPE_TABLE_NAME;
    const ROW_TYPE_NAME: &'static str = ARCHETYPE_TYPE_NAME;

    fn table(&self) -> &GameSystemTable {
        self.table
    }

    fn wrap_row<'row>(row: GameSystemRowRef<'row>) -> Self::Row<'row>
    where
        Self: 'row,
    {
        ArchetypeData::from_row(row)
    }
}

impl<'a> ArchetypeDataManager<'a> {
    #[must_use]
    #[inline]
    pub const fn new(table: &'a GameSystemTable) -> Self {
        Self { table }
    }

    #[must_use]
    #[inline]
    pub const fn table_ref(&self) -> &'a GameSystemTable {
        self.table
    }

    pub fn rows(&self) -> impl Iterator<Item = ArchetypeData<'_>> {
        self.table.row_refs().map(ArchetypeData::from_row)
    }

    /// Find the archetype row whose `BackstoryId` matches `backstory_id`.
    ///
    /// # Errors
    ///
    /// Returns an access error for malformed rows or
    /// [`GameSystemDataError::MissingArchetypeForBackstory`] when no row
    /// points at the requested backstory.
    pub fn require_row_by_backstory_id(
        &self,
        backstory_id: &str,
    ) -> Result<ArchetypeData<'_>, GameSystemDataError> {
        self.row_by_backstory_id(backstory_id)?.ok_or_else(|| {
            GameSystemDataError::MissingArchetypeForBackstory {
                backstory_id: backstory_id.to_owned(),
            }
        })
    }

    /// Find the archetype row whose `BackstoryId` matches `backstory_id`.
    ///
    /// # Errors
    ///
    /// Returns an access error for malformed rows.
    pub fn row_by_backstory_id(
        &self,
        backstory_id: &str,
    ) -> Result<Option<ArchetypeData<'_>>, GameSystemDataError> {
        for row in self.rows() {
            if row.backstory_id()? == backstory_id {
                return Ok(Some(row));
            }
        }
        Ok(None)
    }
}

impl<'a> GameSystemDataManager for StaticBackstoryDataManager<'a> {
    type Row<'row>
        = StaticBackstoryData<'row>
    where
        Self: 'row;

    const TABLE_NAME: &'static str = BACKSTORY_TABLE_NAME;
    const ROW_TYPE_NAME: &'static str = BACKSTORY_TYPE_NAME;

    fn table(&self) -> &GameSystemTable {
        self.table
    }

    fn wrap_row<'row>(row: GameSystemRowRef<'row>) -> Self::Row<'row>
    where
        Self: 'row,
    {
        StaticBackstoryData::from_row(row)
    }
}

impl<'a> StaticBackstoryDataManager<'a> {
    #[must_use]
    #[inline]
    pub const fn new(table: &'a GameSystemTable) -> Self {
        Self { table }
    }

    #[must_use]
    #[inline]
    pub const fn table_ref(&self) -> &'a GameSystemTable {
        self.table
    }

    pub fn rows(&self) -> impl Iterator<Item = StaticBackstoryData<'_>> {
        self.table.row_refs().map(StaticBackstoryData::from_row)
    }

    /// Find a backstory row by its display/name column (`BackstoryName`).
    ///
    /// # Errors
    ///
    /// Returns an access error for malformed rows or
    /// [`GameSystemDataError::MissingRowByColumnValue`] when no row has the
    /// requested name.
    pub fn require_row_by_backstory_name(
        &self,
        backstory_name: &str,
    ) -> Result<StaticBackstoryData<'_>, GameSystemDataError> {
        for row in self.rows() {
            if row.backstory_name()? == backstory_name {
                return Ok(row);
            }
        }
        Err(GameSystemDataError::MissingRowByColumnValue {
            table: self.table.name().to_owned(),
            column: "BackstoryName".to_owned(),
            value: backstory_name.to_owned(),
        })
    }
}

impl<'a> CharacterCreationTemplateResolver<'a> {
    /// Build a template resolver from loaded game-system data tables.
    ///
    /// # Errors
    ///
    /// Returns [`GameSystemDataError::MissingTable`] when archetype or
    /// backstory managers are absent.
    pub fn from_data_tables(
        data_tables: &'a GameSystemDataTables,
    ) -> Result<Self, GameSystemDataError> {
        data_tables.manager()
    }

    #[must_use]
    #[inline]
    pub const fn archetypes(&self) -> ArchetypeDataManager<'a> {
        self.archetypes
    }

    #[must_use]
    #[inline]
    pub const fn backstories(&self) -> StaticBackstoryDataManager<'a> {
        self.backstories
    }

    /// Resolve starter state from an archetype row key such as `mercenary`.
    ///
    /// # Errors
    ///
    /// Returns row access/list parse errors when the required table rows are
    /// absent or malformed.
    pub fn resolve_archetype(
        &self,
        archetype_id: &str,
    ) -> Result<CharacterCreationTemplate, GameSystemDataError> {
        let archetype = self.archetypes.require_row_by_key_name(archetype_id)?;
        self.template_from_archetype(archetype)
    }

    /// Resolve starter state for transform output, retaining validation
    /// diagnostics for authored references that are absent from their target
    /// tables.
    ///
    /// The returned template contains only validated foreign-key expansions.
    /// Diagnostics are transform-time evidence and are intentionally kept out of
    /// runtime GameData projections.
    ///
    /// # Errors
    ///
    /// Returns row access/list parse errors when the required table rows are
    /// absent or malformed.
    pub fn resolve_archetype_for_transform(
        &self,
        archetype_id: &str,
    ) -> Result<CharacterCreationTemplateTransform, GameSystemDataError> {
        let archetype = self.archetypes.require_row_by_key_name(archetype_id)?;
        self.template_from_archetype_for_transform(archetype)
    }

    /// Resolve starter state from a backstory id such as `Archetype_Soldier`.
    ///
    /// Character-creation payloads and replicated player state sometimes carry
    /// backstory ids where the archetype manager is keyed by short archetype
    /// ids. This keeps the translation in the datasheet layer.
    ///
    /// # Errors
    ///
    /// Returns row access/list parse errors when the required table rows are
    /// absent or malformed.
    pub fn resolve_backstory(
        &self,
        backstory_id: &str,
    ) -> Result<CharacterCreationTemplate, GameSystemDataError> {
        let backstory = self.backstories.require_row_by_key_name(backstory_id)?;
        let archetype = self.archetypes.row_by_backstory_id(backstory_id)?;
        self.template_from_backstory(backstory, archetype)
    }

    /// Resolve starter state from a backstory id for transform output.
    ///
    /// Missing optional authored references become diagnostics; required table
    /// shape, numeric, and item-resolution errors still fail the transform.
    ///
    /// # Errors
    ///
    /// Returns row access/list parse errors when the required table rows are
    /// absent or malformed.
    pub fn resolve_backstory_for_transform(
        &self,
        backstory_id: &str,
    ) -> Result<CharacterCreationTemplateTransform, GameSystemDataError> {
        let backstory = self.backstories.require_row_by_key_name(backstory_id)?;
        let archetype = self.archetypes.row_by_backstory_id(backstory_id)?;
        self.template_from_backstory_for_transform(backstory, archetype)
    }

    /// Resolve starter state from a `BackstoryName` such as
    /// `Archetype_Mercenary`.
    ///
    /// # Errors
    ///
    /// Returns row access/list parse errors when the required table rows are
    /// absent or malformed.
    pub fn resolve_backstory_name(
        &self,
        backstory_name: &str,
    ) -> Result<CharacterCreationTemplate, GameSystemDataError> {
        let backstory = self
            .backstories
            .require_row_by_backstory_name(backstory_name)?;
        let archetype = self
            .archetypes
            .row_by_backstory_id(backstory.backstory_id()?)?;
        self.template_from_backstory(backstory, archetype)
    }

    /// Resolve starter state from either an archetype key or a backstory id.
    ///
    /// # Errors
    ///
    /// Returns the archetype lookup error unless that lookup misses and a
    /// backstory-id lookup succeeds.
    pub fn resolve_archetype_or_backstory(
        &self,
        key: &str,
    ) -> Result<CharacterCreationTemplate, GameSystemDataError> {
        match self.resolve_archetype(key) {
            Ok(template) => Ok(template),
            Err(GameSystemDataError::MissingRow { .. }) => match self.resolve_backstory(key) {
                Ok(template) => Ok(template),
                Err(GameSystemDataError::MissingRow { .. }) => self.resolve_backstory_name(key),
                Err(err) => Err(err),
            },
            Err(err) => Err(err),
        }
    }

    fn template_from_archetype(
        &self,
        archetype: ArchetypeData<'_>,
    ) -> Result<CharacterCreationTemplate, GameSystemDataError> {
        let backstory_id = archetype.backstory_id()?;
        let backstory = self.backstories.require_row_by_key_name(backstory_id)?;
        self.template_from_backstory(backstory, Some(archetype))
    }

    fn template_from_archetype_for_transform(
        &self,
        archetype: ArchetypeData<'_>,
    ) -> Result<CharacterCreationTemplateTransform, GameSystemDataError> {
        let backstory_id = archetype.backstory_id()?;
        let backstory = self.backstories.require_row_by_key_name(backstory_id)?;
        self.template_from_backstory_for_transform(backstory, Some(archetype))
    }

    fn template_from_backstory(
        &self,
        backstory: StaticBackstoryData<'_>,
        archetype: Option<ArchetypeData<'_>>,
    ) -> Result<CharacterCreationTemplate, GameSystemDataError> {
        self.template_from_backstory_impl(backstory, archetype, None)
    }

    fn template_from_backstory_for_transform(
        &self,
        backstory: StaticBackstoryData<'_>,
        archetype: Option<ArchetypeData<'_>>,
    ) -> Result<CharacterCreationTemplateTransform, GameSystemDataError> {
        let mut diagnostics = Vec::new();
        let template =
            self.template_from_backstory_impl(backstory, archetype, Some(&mut diagnostics))?;
        Ok(CharacterCreationTemplateTransform {
            template,
            diagnostics,
        })
    }

    fn template_from_backstory_impl(
        &self,
        backstory: StaticBackstoryData<'_>,
        archetype: Option<ArchetypeData<'_>>,
        mut diagnostics: Option<&mut Vec<GameSystemValidationDiagnostic>>,
    ) -> Result<CharacterCreationTemplate, GameSystemDataError> {
        let level_override = backstory.level_override()?;
        let level = positive_integer_value(
            self.backstories.table().name(),
            "LevelOverride",
            level_override,
        )?;
        let inventory_items = backstory.inventory_items()?;
        let consumable_items = archetype
            .map(|archetype| archetype.consumable_item_list())
            .transpose()?
            .unwrap_or_default();
        let achievement_unlock_override = backstory.achievement_unlock_override()?.to_owned();
        let objective_unlock_override = backstory.objective_unlock_override()?.to_owned();
        let items = if let Some(diagnostics) = diagnostics.as_mut() {
            self.game_system.resolve_items_for_transform(
                &inventory_items,
                &consumable_items,
                self.backstories.table().name(),
                backstory.backstory_id()?,
                backstory.row.key_crc(),
                diagnostics,
            )?
        } else {
            self.game_system
                .resolve_items(&inventory_items, &consumable_items)?
        };
        let cooldowns = self.game_system.cooldowns.rows()?.collect();
        let achievement_unlocks = if let Some(diagnostics) = diagnostics.as_mut() {
            self.game_system.achievement_unlocks_for_transform(
                &achievement_unlock_override,
                self.backstories.table().name(),
                "AchievementUnlockOverride",
                backstory.backstory_id()?,
                backstory.row.key_crc(),
                diagnostics,
            )?
        } else {
            self.game_system
                .achievement_unlocks(&achievement_unlock_override)?
        };
        let achievement_bitset_byte_len = self.game_system.achievement_bitset_byte_len()?;
        let abilities = self.game_system.default_abilities()?;
        let categorical_progression = backstory.categorical_progression()?;
        let archetype_id = archetype
            .map(|archetype| archetype.archetype_id())
            .transpose()?
            .unwrap_or(backstory.backstory_id()?)
            .to_owned();
        let archetype_crc = archetype
            .map(|archetype| archetype.row.key_crc())
            .unwrap_or(backstory.row.key_crc());
        let ftue_loot_bag_game_event = archetype
            .map(|archetype| archetype.ftue_loot_bag_game_event())
            .transpose()?
            .unwrap_or_default()
            .to_owned();
        Ok(CharacterCreationTemplate {
            archetype_id,
            archetype_crc,
            backstory_id: backstory.backstory_id()?.to_owned(),
            backstory_crc: backstory.row.key_crc(),
            backstory_name: backstory.backstory_name()?.to_owned(),
            level,
            force_ftue: backstory.force_ftue()?,
            core_attributes: backstory.core_attributes()?,
            inventory_items,
            consumable_items,
            weapon_masteries: backstory.weapon_masteries()?,
            categorical_progression,
            objective_unlock_override,
            achievement_unlock_override,
            achievement_bitset_byte_len,
            ftue_loot_bag_game_event,
            stat_multipliers: None,
            objective_id_entries: None,
            active_objectives: None,
            objective_task_states: None,
            game_events: None,
            items,
            cooldowns,
            abilities,
            achievement_unlocks,
        })
    }
}

impl<'a> ArchetypeData<'a> {
    #[must_use]
    #[inline]
    pub const fn from_row(row: GameSystemRowRef<'a>) -> Self {
        Self { row }
    }

    /// `ArchetypeId`.
    ///
    /// # Errors
    ///
    /// Returns an access error if the source table is missing or malformed.
    pub fn archetype_id(&self) -> Result<&'a str, GameSystemDataError> {
        self.row.required_str("ArchetypeId")
    }

    /// `BackstoryId`; this is the key into the `Backstory` table.
    ///
    /// # Errors
    ///
    /// Returns an access error if the source table is missing or malformed.
    pub fn backstory_id(&self) -> Result<&'a str, GameSystemDataError> {
        self.row.required_str("BackstoryId")
    }

    /// `FTUELootBagGameEvent`.
    ///
    /// # Errors
    ///
    /// Returns an access error if the source table is missing or malformed.
    pub fn ftue_loot_bag_game_event(&self) -> Result<&'a str, GameSystemDataError> {
        self.row.required_str("FTUELootBagGameEvent")
    }

    /// `ProficiencyImage_1..3`, ignoring blank image slots.
    ///
    /// The native archetype row converter reads these fields beside the
    /// tooltip fields (`NewWorld+0x4d6b6c0`). They are exposed for UI/audit
    /// parity only; starter categorical progression comes from the backstory
    /// `CategoricalProgression` field unless a later native path proves an
    /// additional transform projection.
    ///
    /// # Errors
    ///
    /// Returns an access error if the source table is missing or malformed.
    pub fn proficiency_images(&self) -> Result<Vec<&'a str>, GameSystemDataError> {
        let mut images = Vec::new();
        for column in PROFICIENCY_IMAGE_COLUMNS {
            let image = self.row.required_str(column)?.trim();
            if !image.is_empty() {
                images.push(image);
            }
        }
        Ok(images)
    }

    /// `ItemExclusionList`, parsed as `name:amount` entries.
    ///
    /// # Errors
    ///
    /// Returns an access or list parse error.
    pub fn item_exclusion_list(&self) -> Result<Vec<NameAmount>, GameSystemDataError> {
        parse_name_amount_list(
            "ItemExclusionList",
            self.row.required_str("ItemExclusionList")?,
        )
    }

    /// `ConsumableItemList`, parsed as `name:amount` entries.
    ///
    /// # Errors
    ///
    /// Returns an access or list parse error.
    pub fn consumable_item_list(&self) -> Result<Vec<NameAmount>, GameSystemDataError> {
        parse_name_amount_list(
            "ConsumableItemList",
            self.row.required_str("ConsumableItemList")?,
        )
    }
}

impl<'a> StaticBackstoryData<'a> {
    #[must_use]
    #[inline]
    pub const fn from_row(row: GameSystemRowRef<'a>) -> Self {
        Self { row }
    }

    /// `BackstoryID`.
    ///
    /// # Errors
    ///
    /// Returns an access error if the source table is missing or malformed.
    pub fn backstory_id(&self) -> Result<&'a str, GameSystemDataError> {
        self.row.required_str("BackstoryID")
    }

    /// `BackstoryName`.
    ///
    /// # Errors
    ///
    /// Returns an access error if the source table is missing or malformed.
    pub fn backstory_name(&self) -> Result<&'a str, GameSystemDataError> {
        self.row.required_str("BackstoryName")
    }

    /// `LevelOverride`.
    ///
    /// # Errors
    ///
    /// Returns an access error if the source table is missing or malformed.
    pub fn level_override(&self) -> Result<f32, GameSystemDataError> {
        self.row.required_f32("LevelOverride")
    }

    /// Core attribute template columns.
    ///
    /// # Errors
    ///
    /// Returns an access error if the source table is missing or malformed.
    pub fn core_attributes(&self) -> Result<CoreAttributes, GameSystemDataError> {
        Ok(CoreAttributes {
            constitution: self.row.required_f32("Constitution")?,
            dexterity: self.row.required_f32("Dexterity")?,
            focus: self.row.required_f32("Focus")?,
            intelligence: self.row.required_f32("Intelligence")?,
            strength: self.row.required_f32("Strength")?,
        })
    }

    /// `InventoryItem`, parsed as `name:amount` entries.
    ///
    /// # Errors
    ///
    /// Returns an access or list parse error.
    pub fn inventory_items(&self) -> Result<Vec<NameAmount>, GameSystemDataError> {
        parse_name_amount_list("InventoryItem", self.row.required_str("InventoryItem")?)
    }

    /// `WeaponMasteries`, parsed as `name:amount` entries.
    ///
    /// # Errors
    ///
    /// Returns an access or list parse error.
    pub fn weapon_masteries(&self) -> Result<Vec<NameAmount>, GameSystemDataError> {
        parse_name_amount_list("WeaponMasteries", self.row.required_str("WeaponMasteries")?)
    }

    /// `CategoricalProgression`, parsed as `name:amount` entries.
    ///
    /// # Errors
    ///
    /// Returns an access or list parse error.
    pub fn categorical_progression(&self) -> Result<Vec<NameAmount>, GameSystemDataError> {
        parse_name_amount_list(
            "CategoricalProgression",
            self.row.required_str("CategoricalProgression")?,
        )
    }

    /// `ObjectiveUnlockOverride`.
    ///
    /// # Errors
    ///
    /// Returns an access error if the source table is missing or malformed.
    pub fn objective_unlock_override(&self) -> Result<&'a str, GameSystemDataError> {
        self.row.required_str("ObjectiveUnlockOverride")
    }

    /// `AchievementUnlockOverride`.
    ///
    /// # Errors
    ///
    /// Returns an access error if the source table is missing or malformed.
    pub fn achievement_unlock_override(&self) -> Result<&'a str, GameSystemDataError> {
        self.row.required_str("AchievementUnlockOverride")
    }

    /// `ForceFTUE`.
    ///
    /// # Errors
    ///
    /// Returns an access error if the source table is missing or malformed.
    pub fn force_ftue(&self) -> Result<bool, GameSystemDataError> {
        self.row.required_bool("ForceFTUE")
    }
}

impl<'a> StarterGameSystemManagers<'a> {
    fn from_data_tables(
        data_tables: &'a GameSystemDataTables,
    ) -> Result<Self, GameSystemDataError> {
        let armor_appearances = data_tables.manager()?;
        let weapon_appearances = data_tables.manager()?;
        Ok(Self {
            master_items: data_tables.manager()?,
            armor_appearances,
            consumable_items: data_tables.manager()?,
            weapon_appearances,
            cooldowns: data_tables.manager()?,
            achievements: data_tables.manager()?,
            progression_pools: data_tables.manager()?,
        })
    }

    fn resolve_items(
        &self,
        inventory_items: &[NameAmount],
        consumable_items: &[NameAmount],
    ) -> Result<Vec<CharacterCreationItemTemplate>, GameSystemDataError> {
        let mut out = Vec::new();
        for entry in inventory_items {
            if entry.amount == 0 {
                continue;
            }
            out.push(self.resolve_item(CharacterCreationItemSource::InventoryItem, entry)?);
        }
        for entry in consumable_items {
            if entry.amount == 0 {
                continue;
            }
            out.push(self.resolve_item(CharacterCreationItemSource::ConsumableItem, entry)?);
        }
        Ok(out)
    }

    fn resolve_items_for_transform(
        &self,
        inventory_items: &[NameAmount],
        consumable_items: &[NameAmount],
        source_table: &str,
        source_row: &str,
        source_row_key_crc: u32,
        diagnostics: &mut Vec<GameSystemValidationDiagnostic>,
    ) -> Result<Vec<CharacterCreationItemTemplate>, GameSystemDataError> {
        let mut out = Vec::new();
        for entry in inventory_items {
            self.resolve_item_for_transform(
                CharacterCreationItemSource::InventoryItem,
                "InventoryItem",
                entry,
                source_table,
                source_row,
                source_row_key_crc,
                diagnostics,
                &mut out,
            )?;
        }
        for entry in consumable_items {
            self.resolve_item_for_transform(
                CharacterCreationItemSource::ConsumableItem,
                "ConsumableItemList",
                entry,
                source_table,
                source_row,
                source_row_key_crc,
                diagnostics,
                &mut out,
            )?;
        }
        Ok(out)
    }

    #[allow(clippy::too_many_arguments)]
    fn resolve_item_for_transform(
        &self,
        source: CharacterCreationItemSource,
        source_column: &str,
        entry: &NameAmount,
        source_table: &str,
        source_row: &str,
        source_row_key_crc: u32,
        diagnostics: &mut Vec<GameSystemValidationDiagnostic>,
        out: &mut Vec<CharacterCreationItemTemplate>,
    ) -> Result<(), GameSystemDataError> {
        if entry.amount == 0 {
            return Ok(());
        }

        match self.resolve_item_with_diagnostics(
            source,
            entry,
            source_table,
            source_column,
            source_row,
            source_row_key_crc,
            diagnostics,
        ) {
            Ok(item) => out.push(item),
            Err(GameSystemDataError::MissingItemDefinition { row_type, item_id }) => {
                diagnostics.push(GameSystemValidationDiagnostic {
                    source_table: source_table.to_owned(),
                    source_column: source_column.to_owned(),
                    source_row: source_row.to_owned(),
                    source_row_key_crc,
                    value: item_id,
                    occurrences: 1,
                    kind: GameSystemValidationDiagnosticKind::MissingForeignKey {
                        target_table: row_type,
                        target_column: "ItemID".to_owned(),
                    },
                });
            }
            Err(err) => return Err(err),
        }
        Ok(())
    }

    fn resolve_item(
        &self,
        source: CharacterCreationItemSource,
        entry: &NameAmount,
    ) -> Result<CharacterCreationItemTemplate, GameSystemDataError> {
        self.resolve_item_impl(source, entry, None)
    }

    #[allow(clippy::too_many_arguments)]
    fn resolve_item_with_diagnostics(
        &self,
        source: CharacterCreationItemSource,
        entry: &NameAmount,
        source_table: &str,
        source_column: &str,
        source_row: &str,
        source_row_key_crc: u32,
        diagnostics: &mut Vec<GameSystemValidationDiagnostic>,
    ) -> Result<CharacterCreationItemTemplate, GameSystemDataError> {
        self.resolve_item_impl(
            source,
            entry,
            Some((
                source_table,
                source_column,
                source_row,
                source_row_key_crc,
                diagnostics,
            )),
        )
    }

    fn resolve_item_impl(
        &self,
        source: CharacterCreationItemSource,
        entry: &NameAmount,
        diagnostics: Option<(
            &str,
            &str,
            &str,
            u32,
            &mut Vec<GameSystemValidationDiagnostic>,
        )>,
    ) -> Result<CharacterCreationItemTemplate, GameSystemDataError> {
        let master = self.master_items.require_row_by_item_id(&entry.name)?;
        let item_type = master.item_type()?.to_owned();
        let item_class = master.item_class()?.to_owned();
        let item_stats_ref = master.item_stats_ref()?.to_owned();
        let item_crc = Crc32::from_str_lower(&entry.name).value();
        let gear_score = match generated_item_instance_id(&entry.name) {
            Some(instance) => instance.gear_score.get(),
            None => master.gear_score()?,
        };
        let durability = master.durability()?;
        let paperdoll_slot = paperdoll_slot_for_item_class(&item_class);
        let appearance_id = match diagnostics {
            Some((source_table, source_column, source_row, source_row_key_crc, diagnostics)) => {
                match self.resolve_item_appearance_id(&item_type, &master) {
                    Ok(appearance_id) => appearance_id,
                    Err(GameSystemDataError::MissingRowByColumnValue {
                        table,
                        column,
                        value,
                    }) => {
                        diagnostics.push(GameSystemValidationDiagnostic {
                            source_table: source_table.to_owned(),
                            source_column: source_column.to_owned(),
                            source_row: source_row.to_owned(),
                            source_row_key_crc,
                            value,
                            occurrences: 1,
                            kind: GameSystemValidationDiagnosticKind::MissingForeignKey {
                                target_table: table,
                                target_column: column,
                            },
                        });
                        String::new()
                    }
                    Err(err) => return Err(err),
                }
            }
            None => self.resolve_item_appearance_id(&item_type, &master)?,
        };
        let appearance_crc = if appearance_id.is_empty() {
            0
        } else {
            Crc32::from_str_lower(&appearance_id).value()
        };
        let cooldown_id = if item_type == "Consumable" {
            let consumable_lookup_id = if item_stats_ref.is_empty() {
                entry.name.as_str()
            } else {
                item_stats_ref.as_str()
            };
            self.consumable_items
                .row_by_item_id(consumable_lookup_id)
                .and_then(|row| row.cooldown_id().ok())
                .unwrap_or_default()
                .to_owned()
        } else {
            String::new()
        };
        let stack_or_score = if item_type == "Consumable" {
            u16::try_from(entry.amount).map_err(|_| GameSystemDataError::InvalidNumericValue {
                table: "CharacterCreationTemplate".to_owned(),
                column: "ConsumableItemList".to_owned(),
                value: entry.amount as f32,
                expected: "u16 stack count",
            })?
        } else {
            u16::try_from(gear_score).map_err(|_| GameSystemDataError::InvalidNumericValue {
                table: master.row.table().name().to_owned(),
                column: "GearScoreOverride".to_owned(),
                value: gear_score as f32,
                expected: "u16 gear score",
            })?
        };
        Ok(CharacterCreationItemTemplate {
            source,
            item_id: entry.name.clone(),
            item_crc,
            amount: entry.amount,
            item_type,
            item_class,
            item_stats_ref,
            cooldown_id,
            item_key_name: durability,
            item_count: stack_or_score,
            durability,
            paperdoll_slot,
            appearance_id,
            appearance_crc,
            is_non_removable_from_player: master.nonremovable().unwrap_or(false),
            is_bound_to_player: master.bind_on_pickup().unwrap_or(false),
            is_bind_on_pickup: master.bind_on_pickup().unwrap_or(false),
            is_bind_on_equip: master.bind_on_equip().unwrap_or(false),
        })
    }

    fn achievement_unlocks(
        &self,
        achievement_ids: &str,
    ) -> Result<Vec<CharacterCreationAchievementTemplate>, GameSystemDataError> {
        let mut out = Vec::new();
        for achievement_id in legacy_list_entries(achievement_ids) {
            if achievement_id.is_empty() {
                continue;
            }
            for achievement in self
                .achievements
                .rows_by_achievement_id_or_prefix(achievement_id)?
            {
                out.push(CharacterCreationAchievementTemplate {
                    achievement_id: achievement.achievement_id()?.to_owned(),
                    achievement_index: achievement.achievement_index()?,
                });
            }
        }
        Ok(out)
    }

    fn achievement_unlocks_for_transform(
        &self,
        achievement_ids: &str,
        source_table: &str,
        source_column: &str,
        source_row: &str,
        source_row_key_crc: u32,
        diagnostics: &mut Vec<GameSystemValidationDiagnostic>,
    ) -> Result<Vec<CharacterCreationAchievementTemplate>, GameSystemDataError> {
        let mut out = Vec::new();
        for achievement_id in legacy_list_entries(achievement_ids) {
            if achievement_id.is_empty() {
                continue;
            }

            let rows = match self
                .achievements
                .rows_by_achievement_id_or_prefix(achievement_id)
            {
                Ok(rows) => rows,
                Err(GameSystemDataError::MissingRowByColumnValue {
                    table,
                    column,
                    value,
                }) => {
                    diagnostics.push(GameSystemValidationDiagnostic {
                        source_table: source_table.to_owned(),
                        source_column: source_column.to_owned(),
                        source_row: source_row.to_owned(),
                        source_row_key_crc,
                        value,
                        occurrences: 1,
                        kind: GameSystemValidationDiagnosticKind::MissingForeignKey {
                            target_table: table,
                            target_column: column,
                        },
                    });
                    continue;
                }
                Err(err) => return Err(err),
            };

            for achievement in rows {
                out.push(CharacterCreationAchievementTemplate {
                    achievement_id: achievement.achievement_id()?.to_owned(),
                    achievement_index: achievement.achievement_index()?,
                });
            }
        }
        Ok(out)
    }

    fn achievement_bitset_byte_len(&self) -> Result<u32, GameSystemDataError> {
        self.achievements.bitset_byte_len()
    }

    fn default_abilities(
        &self,
    ) -> Result<Vec<CharacterCreationAbilityTemplate>, GameSystemDataError> {
        DefaultAbilityDataManager::new(self.master_items.data_tables)
            .default_abilities_with_pools(&self.progression_pools)
    }

    fn resolve_item_appearance_id(
        &self,
        item_type: &str,
        master: &MasterItemDefinition<'a>,
    ) -> Result<String, GameSystemDataError> {
        match item_type {
            "Armor" => {
                let appearance_id = master.armor_appearance_id()?.unwrap_or_default();
                if appearance_id.is_empty() {
                    return Ok(String::new());
                }
                self.armor_appearances
                    .require_row_by_item_id(appearance_id)?;
                Ok(appearance_id.to_owned())
            }
            "Weapon" => {
                let appearance_id = master.weapon_appearance_id()?.unwrap_or_default();
                if appearance_id.is_empty() {
                    return Ok(String::new());
                }
                self.weapon_appearances
                    .require_row_by_appearance_id(appearance_id)?;
                Ok(appearance_id.to_owned())
            }
            _ => Ok(String::new()),
        }
    }
}

impl<'a> MasterItemDefinitionsManager<'a> {
    #[must_use]
    #[inline]
    pub const fn new(data_tables: &'a GameSystemDataTables) -> Self {
        Self { data_tables }
    }

    /// Find an item in any `MasterItemDefinitions*` table by native row key.
    ///
    /// # Errors
    ///
    /// Returns [`GameSystemDataError::MissingItemDefinition`] when no loaded
    /// master-item table has a matching row.
    pub fn require_row_by_item_id(
        &self,
        item_id: &str,
    ) -> Result<MasterItemDefinition<'a>, GameSystemDataError> {
        self.row_by_item_id(item_id)
            .or_else(|| {
                generated_item_instance_id(item_id).and_then(|instance| {
                    self.row_by_item_id(instance.base_item_id)
                        .or_else(|| {
                            self.row_by_tier_family_gear_score(
                                instance.base_item_id,
                                instance.gear_score,
                            )
                        })
                        .or_else(|| self.row_by_nearest_defined_tier(instance.base_item_id))
                })
            })
            .or_else(|| {
                base_item_id_from_modifier_instance_id(item_id)
                    .and_then(|base_item_id| self.row_by_item_id(base_item_id))
            })
            .or_else(|| self.row_by_nearest_defined_tier(item_id))
            .ok_or_else(|| GameSystemDataError::MissingItemDefinition {
                row_type: MASTER_ITEM_TYPE_NAME.to_owned(),
                item_id: item_id.to_owned(),
            })
    }

    fn row_by_item_id(&self, item_id: &str) -> Option<MasterItemDefinition<'a>> {
        self.data_tables
            .tables()
            .iter()
            .filter(|table| table.type_name() == MASTER_ITEM_TYPE_NAME)
            .find_map(|table| table.row_by_key_name(item_id))
            .map(MasterItemDefinition::from_row)
    }

    fn row_by_nearest_defined_tier(&self, item_id: &str) -> Option<MasterItemDefinition<'a>> {
        let (stem, tier) = split_trailing_tier(item_id)?;
        let mut matches = (1u8..=9)
            .filter_map(|candidate_tier| {
                let candidate = format!("{stem}T{candidate_tier}");
                self.row_by_item_id(&candidate)
                    .map(|row| (candidate_tier, row))
            })
            .collect::<Vec<_>>();
        matches.sort_by_key(|(candidate_tier, _)| {
            ((*candidate_tier).abs_diff(tier), *candidate_tier < tier)
        });
        matches.into_iter().map(|(_, row)| row).next()
    }

    fn row_by_tier_family_gear_score(
        &self,
        item_id: &str,
        gear_score: NonZeroU32,
    ) -> Option<MasterItemDefinition<'a>> {
        let (stem, tier) = split_trailing_tier(item_id)?;
        let mut matches = (1u8..=9)
            .filter_map(|candidate_tier| {
                let candidate = format!("{stem}T{candidate_tier}");
                let row = self.row_by_item_id(&candidate)?;
                (row.gear_score().ok() == Some(gear_score.get())).then_some((candidate_tier, row))
            })
            .collect::<Vec<_>>();
        matches.sort_by_key(|(candidate_tier, _)| (*candidate_tier).abs_diff(tier));
        matches.into_iter().map(|(_, row)| row).next()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GeneratedItemInstanceId<'a> {
    base_item_id: &'a str,
    gear_score: NonZeroU32,
}

fn generated_item_instance_id(item_id: &str) -> Option<GeneratedItemInstanceId<'_>> {
    for (dash_index, _) in item_id.match_indices('-') {
        let base_item_id = item_id[..dash_index].trim();
        if base_item_id.is_empty() {
            continue;
        }

        let score_and_suffix = &item_id[dash_index + 1..];
        let digit_len = score_and_suffix
            .bytes()
            .take_while(u8::is_ascii_digit)
            .count();
        if digit_len == 0 {
            continue;
        }
        match score_and_suffix.as_bytes().get(digit_len) {
            Some(b'-') | None => {}
            Some(_) => continue,
        }
        let gear_score = NonZeroU32::new(score_and_suffix[..digit_len].parse::<u32>().ok()?)?;
        return Some(GeneratedItemInstanceId {
            base_item_id,
            gear_score,
        });
    }
    None
}

fn base_item_id_from_modifier_instance_id(item_id: &str) -> Option<&str> {
    let (base_item_id, _) = item_id.split_once("-PerkID_")?;
    let base_item_id = base_item_id.trim();
    (!base_item_id.is_empty()).then_some(base_item_id)
}

#[cfg(test)]
fn base_item_id_from_instance_id(item_id: &str) -> Option<&str> {
    generated_item_instance_id(item_id)
        .map(|instance| instance.base_item_id)
        .or_else(|| base_item_id_from_modifier_instance_id(item_id))
}

fn split_trailing_tier(item_id: &str) -> Option<(&str, u8)> {
    let tier_start = item_id.rfind('T')?;
    let suffix = item_id.get(tier_start + 1..)?;
    if suffix.is_empty() || !suffix.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let tier = suffix.parse::<u8>().ok()?;
    Some((&item_id[..tier_start], tier))
}

fn starts_with_ignore_ascii_case(value: &str, prefix: &str) -> bool {
    value
        .get(..prefix.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
}

impl<'a> ArmorAppearanceDefinitionsManager<'a> {
    #[must_use]
    #[inline]
    pub const fn new(table: &'a GameSystemTable) -> Self {
        Self { table }
    }

    /// Find an armor appearance by native `ItemID`.
    ///
    /// # Errors
    ///
    /// Returns [`GameSystemDataError::MissingRowByColumnValue`] when the
    /// appearance row is absent.
    pub fn require_row_by_item_id(
        &self,
        item_id: &str,
    ) -> Result<ArmorAppearanceDefinition<'a>, GameSystemDataError> {
        self.table
            .row_by_key_name(item_id)
            .map(ArmorAppearanceDefinition::from_row)
            .ok_or_else(|| GameSystemDataError::MissingRowByColumnValue {
                table: self.table.name().to_owned(),
                column: "ItemID".to_owned(),
                value: item_id.to_owned(),
            })
    }
}

impl<'a> ArmorItemDefinitionsManager<'a> {
    #[must_use]
    #[inline]
    pub const fn new(table: &'a GameSystemTable) -> Self {
        Self { table }
    }

    #[must_use]
    pub fn row_by_item_id(&self, item_id: &str) -> Option<ArmorItemDefinition<'a>> {
        self.table
            .row_by_key_name(item_id)
            .map(ArmorItemDefinition::from_row)
    }
}

impl<'a> ConsumableItemDefinitionsManager<'a> {
    #[must_use]
    #[inline]
    pub const fn new(table: &'a GameSystemTable) -> Self {
        Self { table }
    }

    #[must_use]
    pub fn row_by_item_id(&self, item_id: &str) -> Option<ConsumableItemDefinition<'a>> {
        self.table
            .row_by_key_name(item_id)
            .map(ConsumableItemDefinition::from_row)
    }
}

impl<'a> WeaponAppearanceDefinitionsManager<'a> {
    #[must_use]
    #[inline]
    pub const fn new(table: &'a GameSystemTable) -> Self {
        Self { table }
    }

    /// Find a weapon appearance by native `WeaponAppearanceID`.
    ///
    /// # Errors
    ///
    /// Returns [`GameSystemDataError::MissingRowByColumnValue`] when the
    /// appearance row is absent.
    pub fn require_row_by_appearance_id(
        &self,
        appearance_id: &str,
    ) -> Result<WeaponAppearanceDefinition<'a>, GameSystemDataError> {
        self.table
            .row_by_key_name(appearance_id)
            .map(WeaponAppearanceDefinition::from_row)
            .ok_or_else(|| GameSystemDataError::MissingRowByColumnValue {
                table: self.table.name().to_owned(),
                column: "WeaponAppearanceID".to_owned(),
                value: appearance_id.to_owned(),
            })
    }
}

impl<'a> WeaponItemDefinitionsManager<'a> {
    #[must_use]
    #[inline]
    pub const fn new(table: &'a GameSystemTable) -> Self {
        Self { table }
    }

    #[must_use]
    pub fn row_by_item_id(&self, item_id: &str) -> Option<WeaponItemDefinition<'a>> {
        self.table
            .row_by_key_name(item_id)
            .map(WeaponItemDefinition::from_row)
    }
}

impl<'a> CooldownsPlayerManager<'a> {
    #[must_use]
    #[inline]
    pub const fn new(table: &'a GameSystemTable) -> Self {
        Self { table }
    }

    pub fn rows(
        &self,
    ) -> Result<impl Iterator<Item = CharacterCreationCooldownTemplate> + 'a, GameSystemDataError>
    {
        Ok(self.table.row_refs().map(|row| {
            let cooldown = CooldownData::from_row(row);
            let id = cooldown.ability_id().unwrap_or_default().to_owned();
            CharacterCreationCooldownTemplate {
                cooldown_crc: Crc32::from_str_lower(&id).value(),
                cooldown_id: id,
            }
        }))
    }
}

impl<'a> AchievementDataManager<'a> {
    #[must_use]
    #[inline]
    pub const fn new(table: &'a GameSystemTable) -> Self {
        Self { table }
    }

    /// Find an achievement row by `AchievementID`.
    ///
    /// # Errors
    ///
    /// Returns access errors for malformed rows or
    /// [`GameSystemDataError::MissingRowByColumnValue`] when the achievement
    /// is absent.
    pub fn require_row_by_achievement_id(
        &self,
        achievement_id: &str,
    ) -> Result<AchievementData<'a>, GameSystemDataError> {
        self.row_by_achievement_id(achievement_id)?.ok_or_else(|| {
            GameSystemDataError::MissingRowByColumnValue {
                table: self.table.name().to_owned(),
                column: "AchievementID".to_owned(),
                value: achievement_id.to_owned(),
            }
        })
    }

    /// Resolve an authored achievement id or a deterministic prefix family.
    ///
    /// Some legacy starter rows use a family id such as
    /// `FactionIntro_Covenant_Vector` while the table contains only concrete
    /// numbered rows. The transform expands that into validated concrete FKs;
    /// runtime products never carry the family alias.
    ///
    /// # Errors
    ///
    /// Returns access errors for malformed rows or
    /// [`GameSystemDataError::MissingRowByColumnValue`] when neither an exact
    /// row nor a concrete prefixed family exists.
    pub fn rows_by_achievement_id_or_prefix(
        &self,
        achievement_id: &str,
    ) -> Result<Vec<AchievementData<'a>>, GameSystemDataError> {
        if let Some(achievement) = self.row_by_achievement_id(achievement_id)? {
            return Ok(vec![achievement]);
        }

        let prefix = format!("{achievement_id}_");
        let mut rows = Vec::new();
        for row in self.table.row_refs() {
            let achievement = AchievementData::from_row(row);
            let current_id = achievement.achievement_id()?;
            if starts_with_ignore_ascii_case(current_id, &prefix) {
                rows.push((current_id.to_owned(), achievement));
            }
        }

        if rows.is_empty() {
            return Err(GameSystemDataError::MissingRowByColumnValue {
                table: self.table.name().to_owned(),
                column: "AchievementID".to_owned(),
                value: achievement_id.to_owned(),
            });
        }

        rows.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(rows
            .into_iter()
            .map(|(_, achievement)| achievement)
            .collect())
    }

    fn row_by_achievement_id(
        &self,
        achievement_id: &str,
    ) -> Result<Option<AchievementData<'a>>, GameSystemDataError> {
        for row in self.table.row_refs() {
            let achievement = AchievementData::from_row(row);
            if achievement
                .achievement_id()?
                .eq_ignore_ascii_case(achievement_id)
            {
                return Ok(Some(achievement));
            }
        }
        Ok(None)
    }

    /// Byte length of the native `AchievementStates` bitset.
    ///
    /// The wire state is a dense `ReplicatedContainer<Vec<u8>>` indexed by
    /// `AchievementIndex`. A newly-created character only sets a few bits,
    /// but live still sends the full-width zeroed vector.
    pub fn bitset_byte_len(&self) -> Result<u32, GameSystemDataError> {
        let max_index = self
            .table
            .row_refs()
            .map(|row| AchievementData::from_row(row).achievement_index())
            .try_fold(None, |max_index: Option<u32>, index| {
                index.map(|index| Some(max_index.map_or(index, |max| max.max(index))))
            })?;
        Ok(max_index.map_or(0, |index| (index / 8).saturating_add(1)))
    }
}

impl<'a> GameSystemDataManager for ProgressionPoolsManager<'a> {
    type Row<'row>
        = ProgressionPoolData<'row>
    where
        Self: 'row;

    const TABLE_NAME: &'static str = PROGRESSION_POOLS_TABLE_NAME;
    const ROW_TYPE_NAME: &'static str = PROGRESSION_POOL_TYPE_NAME;

    fn table(&self) -> &GameSystemTable {
        self.table
    }

    fn wrap_row<'row>(row: GameSystemRowRef<'row>) -> Self::Row<'row>
    where
        Self: 'row,
    {
        ProgressionPoolData::from_row(row)
    }
}

impl<'a> ProgressionPoolsManager<'a> {
    #[must_use]
    #[inline]
    pub const fn new(table: &'a GameSystemTable) -> Self {
        Self { table }
    }

    #[must_use]
    pub fn row_by_pool_id(&self, pool_id: &str) -> Option<ProgressionPoolData<'a>> {
        self.table
            .row_by_key_name(pool_id)
            .map(ProgressionPoolData::from_row)
    }
}

impl<'a> DefaultAbilityDataManager<'a> {
    #[must_use]
    #[inline]
    pub const fn new(data_tables: &'a GameSystemDataTables) -> Self {
        Self { data_tables }
    }

    /// Return the default persistent ability-table entries.
    ///
    /// New World's initial `persistentAbilityData` snapshot carries one entry
    /// per loaded `AbilityData` table. The entry id is the table name CRC,
    /// while weapon-table additional values come from `ProgressionPools`.
    ///
    /// # Errors
    ///
    /// Returns row access errors for malformed progression-pool rows.
    pub fn default_abilities(
        &self,
    ) -> Result<Vec<CharacterCreationAbilityTemplate>, GameSystemDataError> {
        let progression_pools = self.data_tables.manager()?;
        self.default_abilities_with_pools(&progression_pools)
    }

    fn default_abilities_with_pools(
        &self,
        progression_pools: &ProgressionPoolsManager<'_>,
    ) -> Result<Vec<CharacterCreationAbilityTemplate>, GameSystemDataError> {
        let mut out = Vec::new();
        for table in self
            .data_tables
            .tables()
            .iter()
            .filter(|table| table.type_name() == ABILITY_DATA_TYPE_NAME)
        {
            let initial_points = match progression_pools.row_by_pool_id(table.name()) {
                Some(pool) => Some(pool.initial_points()?),
                None => None,
            };
            out.push(CharacterCreationAbilityTemplate {
                ability_id: table.name().to_owned(),
                ability_crc: table.name_crc(),
                initial_points,
                values: Vec::new(),
            });
        }
        out.sort_by_key(|entry| entry.ability_crc);
        out.dedup_by_key(|entry| entry.ability_crc);
        Ok(out)
    }
}

impl<'a> MasterItemDefinition<'a> {
    #[must_use]
    #[inline]
    pub const fn from_row(row: GameSystemRowRef<'a>) -> Self {
        Self { row }
    }

    pub fn item_type(&self) -> Result<&'a str, GameSystemDataError> {
        self.row.required_str("ItemType")
    }

    pub fn item_class(&self) -> Result<&'a str, GameSystemDataError> {
        self.row.required_str("ItemClass")
    }

    pub fn item_stats_ref(&self) -> Result<&'a str, GameSystemDataError> {
        self.row.required_str("ItemStatsRef")
    }

    pub fn bind_on_pickup(&self) -> Result<bool, GameSystemDataError> {
        Ok(truthy_cell(
            self.row.cell("BindOnPickup").map(GameSystemCell::value),
        ))
    }

    pub fn bind_on_equip(&self) -> Result<bool, GameSystemDataError> {
        Ok(truthy_cell(
            self.row.cell("BindOnEquip").map(GameSystemCell::value),
        ))
    }

    pub fn nonremovable(&self) -> Result<bool, GameSystemDataError> {
        Ok(truthy_cell(
            self.row.cell("Nonremovable").map(GameSystemCell::value),
        ))
    }

    pub fn gear_score(&self) -> Result<u32, GameSystemDataError> {
        for column in ["GearScoreOverride", "MaxGearScore", "MinGearScore"] {
            if let Some(value) = optional_nonzero_u32(&self.row, column)? {
                return Ok(value);
            }
        }
        Ok(0)
    }

    pub fn durability(&self) -> Result<u32, GameSystemDataError> {
        let value = optional_number(&self.row, "Durability").unwrap_or(0.0);
        positive_integer_value(self.row.table().name(), "Durability", value)
    }

    pub fn armor_appearance_id(&self) -> Result<Option<&'a str>, GameSystemDataError> {
        Ok(optional_str(&self.row, "ArmorAppearanceM"))
    }

    pub fn weapon_appearance_id(&self) -> Result<Option<&'a str>, GameSystemDataError> {
        Ok(optional_str(&self.row, "WeaponAppearanceOverride"))
    }
}

impl<'a> ArmorAppearanceDefinition<'a> {
    #[must_use]
    #[inline]
    pub const fn from_row(row: GameSystemRowRef<'a>) -> Self {
        Self { row }
    }

    pub fn item_id(&self) -> Result<&'a str, GameSystemDataError> {
        self.row.required_str("ItemID")
    }

    pub fn appearance_name(&self) -> Result<&'a str, GameSystemDataError> {
        self.row.required_str("AppearanceName")
    }
}

impl<'a> ArmorItemDefinition<'a> {
    #[must_use]
    #[inline]
    pub const fn from_row(row: GameSystemRowRef<'a>) -> Self {
        Self { row }
    }
}

impl<'a> ConsumableItemDefinition<'a> {
    #[must_use]
    #[inline]
    pub const fn from_row(row: GameSystemRowRef<'a>) -> Self {
        Self { row }
    }

    pub fn cooldown_id(&self) -> Result<&'a str, GameSystemDataError> {
        self.row.required_str("CooldownId")
    }
}

impl<'a> WeaponAppearanceDefinition<'a> {
    #[must_use]
    #[inline]
    pub const fn from_row(row: GameSystemRowRef<'a>) -> Self {
        Self { row }
    }

    pub fn weapon_appearance_id(&self) -> Result<&'a str, GameSystemDataError> {
        self.row.required_str("WeaponAppearanceID")
    }

    pub fn appearance(&self) -> Result<&'a str, GameSystemDataError> {
        self.row.required_str("Appearance")
    }

    pub fn item_class(&self) -> Result<&'a str, GameSystemDataError> {
        self.row.required_str("ItemClass")
    }
}

impl<'a> WeaponItemDefinition<'a> {
    #[must_use]
    #[inline]
    pub const fn from_row(row: GameSystemRowRef<'a>) -> Self {
        Self { row }
    }
}

impl<'a> CooldownData<'a> {
    #[must_use]
    #[inline]
    pub const fn from_row(row: GameSystemRowRef<'a>) -> Self {
        Self { row }
    }

    pub fn ability_id(&self) -> Result<&'a str, GameSystemDataError> {
        self.row.required_str("AbilityID")
    }
}

impl<'a> AchievementData<'a> {
    #[must_use]
    #[inline]
    pub const fn from_row(row: GameSystemRowRef<'a>) -> Self {
        Self { row }
    }

    pub fn achievement_id(&self) -> Result<&'a str, GameSystemDataError> {
        self.row.required_str("AchievementID")
    }

    pub fn achievement_index(&self) -> Result<u32, GameSystemDataError> {
        positive_integer_value(
            self.row.table().name(),
            "AchievementIndex",
            self.row.required_f32("AchievementIndex")?,
        )
    }
}

impl<'a> ProgressionPoolData<'a> {
    #[must_use]
    #[inline]
    pub const fn from_row(row: GameSystemRowRef<'a>) -> Self {
        Self { row }
    }

    pub fn progression_pool_id(&self) -> Result<&'a str, GameSystemDataError> {
        self.row.required_str("ProgressionPoolId")
    }

    pub fn initial_points(&self) -> Result<u32, GameSystemDataError> {
        u32_cell_default_zero(&self.row, "InitialPoints")
    }
}

impl<'a> AbilityData<'a> {
    #[must_use]
    #[inline]
    pub const fn from_row(row: GameSystemRowRef<'a>) -> Self {
        Self { row }
    }

    pub fn ability_id(&self) -> Result<&'a str, GameSystemDataError> {
        self.row.required_str("AbilityID")
    }

    #[must_use]
    pub fn tree_row_position(&self) -> Option<u32> {
        optional_str(&self.row, "TreeRowPosition")
            .and_then(|value| value.parse::<u32>().ok())
            .or_else(|| {
                optional_number(&self.row, "TreeRowPosition").and_then(|value| {
                    (value.is_finite() && value >= 0.0 && value.fract() == 0.0)
                        .then_some(value as u32)
                })
            })
    }
}

fn paperdoll_slot_for_item_class(item_class: &str) -> Option<u16> {
    let classes = item_class.split('+');
    for class in classes {
        match class {
            "EquippableHead" => return Some(0),
            "EquippableChest" => return Some(1),
            "EquippableHands" => return Some(2),
            "EquippableLegs" => return Some(3),
            "EquippableFeet" => return Some(4),
            "EquippableMainHand" => return Some(24),
            "EquippableOffHand" => return Some(27),
            _ => {}
        }
    }
    None
}

fn u32_cell_default_zero(
    row: &GameSystemRowRef<'_>,
    column: &str,
) -> Result<u32, GameSystemDataError> {
    let cell = row.required_cell(column)?;
    match cell.value() {
        OwnedCellValue::Number(value) => positive_integer_value(row.table().name(), column, *value),
        OwnedCellValue::String(value) => {
            let value = value.trim();
            if value.is_empty() {
                return Ok(0);
            }
            value
                .parse::<u32>()
                .map_err(|source| GameSystemDataError::InvalidIntegerCell {
                    table: row.table().name().to_owned(),
                    column: column.to_owned(),
                    value: value.to_owned(),
                    source,
                })
        }
        OwnedCellValue::Boolean(_) => Err(row.wrong_type(column, "string or number", cell)),
    }
}

fn optional_str<'a>(row: &GameSystemRowRef<'a>, column: &str) -> Option<&'a str> {
    row.cell(column).and_then(|cell| cell.value().as_str())
}

fn optional_number(row: &GameSystemRowRef<'_>, column: &str) -> Option<f32> {
    row.cell(column).and_then(|cell| cell.value().as_f32())
}

fn optional_nonzero_u32(
    row: &GameSystemRowRef<'_>,
    column: &str,
) -> Result<Option<u32>, GameSystemDataError> {
    let Some(value) = optional_number(row, column) else {
        return Ok(None);
    };
    let value = positive_integer_value(row.table().name(), column, value)?;
    Ok(NonZeroU32::new(value).map(NonZeroU32::get))
}

fn truthy_cell(value: Option<&OwnedCellValue>) -> bool {
    match value {
        Some(OwnedCellValue::Boolean(value)) => *value,
        Some(OwnedCellValue::Number(value)) => *value != 0.0,
        Some(OwnedCellValue::String(value)) => {
            let value = value.trim();
            value.eq_ignore_ascii_case("true") || value.eq_ignore_ascii_case("yes") || value == "1"
        }
        None => false,
    }
}

/// Parse datasheet cells of the form `Name:42,OtherName:1`.
///
/// Empty or whitespace-only cells produce an empty list.
///
/// # Errors
///
/// Entries may be `name:amount` or a bare `name`; bare names default to amount
/// `1`. Returns an error if any amount is non-integer.
pub fn parse_name_amount_list(
    column: &str,
    value: &str,
) -> Result<Vec<NameAmount>, GameSystemDataError> {
    let mut out = Vec::new();
    for entry in legacy_list_entries(value) {
        if entry.is_empty() {
            continue;
        }

        let Some((name, amount)) = entry.split_once(':') else {
            out.push(NameAmount {
                name: entry.to_owned(),
                amount: 1,
            });
            continue;
        };

        let amount_text = amount
            .split(':')
            .rev()
            .map(|part| part.trim().trim_start_matches('+'))
            .find(|part| !part.is_empty())
            .unwrap_or("1");
        let amount = amount_text.parse::<u32>().map_err(|source| {
            GameSystemDataError::InvalidNameAmount {
                column: column.to_owned(),
                entry: entry.to_owned(),
                amount: amount_text.to_owned(),
                source,
            }
        })?;
        out.push(NameAmount {
            name: normalize_legacy_list_entry(name).to_owned(),
            amount,
        });
    }
    Ok(out)
}

fn legacy_list_entries(value: &str) -> Vec<&str> {
    let raw_entries: Vec<&str> = if value.contains(',') {
        value.split(',').collect()
    } else {
        value.split_whitespace().collect()
    };

    raw_entries
        .into_iter()
        .map(normalize_legacy_list_entry)
        .filter(|entry| !entry.is_empty())
        .collect()
}

fn normalize_legacy_list_entry(entry: &str) -> &str {
    entry.trim().trim_start_matches('+').trim()
}

fn collect_datasheet_paths(root: &Path, out: &mut Vec<PathBuf>) -> Result<(), GameSystemDataError> {
    let entries = fs::read_dir(root).map_err(|source| GameSystemDataError::Read {
        path: root.to_path_buf(),
        source,
    })?;

    for entry in entries {
        let entry = entry.map_err(|source| GameSystemDataError::Read {
            path: root.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if path.is_dir() {
            collect_datasheet_paths(&path, out)?;
        } else if is_datasheet_path(&path) {
            out.push(path);
        }
    }
    Ok(())
}

fn native_row_key_crc(cell: &GameSystemCell) -> u32 {
    match cell.value() {
        OwnedCellValue::String(value) => Crc32::from_str_lower(value).value(),
        OwnedCellValue::Number(value) if value.is_finite() && value.fract() == 0.0 => *value as u32,
        OwnedCellValue::Number(_) | OwnedCellValue::Boolean(_) => cell.crc(),
    }
}

fn positive_integer_value(
    table: &str,
    column: &str,
    value: f32,
) -> Result<u32, GameSystemDataError> {
    if value.is_finite() && value >= 0.0 && value.fract() == 0.0 {
        Ok(value as u32)
    } else {
        Err(GameSystemDataError::InvalidNumericValue {
            table: table.to_owned(),
            column: column.to_owned(),
            value,
            expected: "non-negative integer",
        })
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU32;

    use super::{
        NameAmount, base_item_id_from_instance_id, generated_item_instance_id,
        parse_name_amount_list, split_trailing_tier,
    };

    #[test]
    fn base_item_id_from_instance_id_strips_score_and_perks() {
        assert_eq!(
            base_item_id_from_instance_id("1hGauntletVoidT4-455"),
            Some("1hGauntletVoidT4")
        );
        assert_eq!(
            base_item_id_from_instance_id("2hGreatAxeT4-455-PerkID_Stat_TwoHandSoldier"),
            Some("2hGreatAxeT4")
        );
        assert_eq!(
            base_item_id_from_instance_id(
                "2hBlunderbussAncientT4-400-PerkID_Weapon_DmgCorrupted-PerkID_Stat_TwoHandZealot",
            ),
            Some("2hBlunderbussAncientT4")
        );
        assert_eq!(
            base_item_id_from_instance_id(
                "Omega_FishingPole_T5-600-FishLineStrengthFresh3-FishLineStrengthSalt3",
            ),
            Some("Omega_FishingPole_T5")
        );
        assert_eq!(
            base_item_id_from_instance_id("MaxedBackstory_DescriptionItem-PerkID_Gem_EmptyGemSlot"),
            Some("MaxedBackstory_DescriptionItem")
        );
    }

    #[test]
    fn base_item_id_from_instance_id_keeps_non_instance_ids() {
        assert_eq!(base_item_id_from_instance_id("1hSwordT2_FTUE"), None);
        assert_eq!(base_item_id_from_instance_id("Faction-Token"), None);
        assert_eq!(base_item_id_from_instance_id("WeaponT5-0"), None);
    }

    #[test]
    fn generated_item_instance_id_extracts_score() {
        let instance = generated_item_instance_id(
            "ClothingChest_AlchemistT5-500-PerkID_Stat_ArmorScholar-PerkID_Armor_ResistBlight",
        )
        .expect("generated item instance");
        assert_eq!(instance.base_item_id, "ClothingChest_AlchemistT5");
        assert_eq!(instance.gear_score, NonZeroU32::new(500).unwrap());
    }

    #[test]
    fn split_trailing_tier_requires_final_tier_digits() {
        assert_eq!(split_trailing_tier("1hShieldBT2"), Some(("1hShieldB", 2)));
        assert_eq!(
            split_trailing_tier("PlaytestBoxT51"),
            Some(("PlaytestBox", 51))
        );
        assert_eq!(split_trailing_tier("Playtest_Ring_T5_625"), None);
        assert_eq!(split_trailing_tier("PotionT"), None);
    }

    #[test]
    fn parse_name_amount_list_accepts_legacy_spacing_and_colon_tails() {
        assert_eq!(
            parse_name_amount_list("InventoryItem", "SwordT5:1: BowT5:+2").unwrap(),
            vec![
                NameAmount {
                    name: "SwordT5".to_owned(),
                    amount: 1,
                },
                NameAmount {
                    name: "BowT5".to_owned(),
                    amount: 2,
                },
            ]
        );
    }
}
