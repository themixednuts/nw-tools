use serde::{Deserialize, Serialize};

use nw_datasheet::{ColumnType, game_system::GameSystemValidationDiagnostic};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameSystemDataTablesSchemaReport {
    pub tables: Vec<GameSystemTableSchema>,
    pub diagnostics: Vec<GameSystemValidationDiagnostic>,
    pub type_affinities: Vec<GameSystemColumnTypeAffinity>,
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameSystemTableSchema {
    pub table_name: String,
    pub table_name_crc: u32,
    pub row_type_name: String,
    pub row_type_crc: u32,
    pub row_count: usize,
    pub sources: Vec<String>,
    pub columns: Vec<GameSystemColumnSchema>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameSystemColumnSchema {
    pub name: String,
    pub crc: u32,
    pub declared_type: ColumnType,
    pub row_key: bool,
    pub required: bool,
    pub non_empty_rows: usize,
    pub empty_rows: usize,
    pub distinct_values: usize,
    pub value_shape: GameSystemColumnValueShape,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "shape", rename_all = "snake_case")]
pub enum GameSystemColumnValueShape {
    Boolean,
    Color {
        color_shape: GameSystemColorShape,
    },
    Crc32,
    Enum {
        enum_shape: GameSystemEnumShape,
    },
    Number {
        number_shape: GameSystemNumberShape,
    },
    Range {
        bounds: GameSystemRangeBounds,
        number_shape: GameSystemNumberShape,
    },
    String {
        identifier_like: bool,
        localized_key_like: bool,
        asset_path_like: bool,
        expression_like: bool,
        #[serde(default, skip_serializing_if = "is_false")]
        qualified_reference_like: bool,
        list: Option<GameSystemListShape>,
        foreign_keys: Vec<GameSystemForeignKeyCandidate>,
    },
}

impl GameSystemColumnValueShape {
    /// Whether this string shape contains an authored boolean expression.
    #[must_use]
    pub fn is_expression_like(&self) -> bool {
        matches!(
            self,
            Self::String {
                expression_like: true,
                ..
            }
        )
    }

    /// Whether this string shape contains an identifier with an authored
    /// numeric qualifier, such as `PvP_XP:5`.
    #[must_use]
    pub fn is_qualified_reference_like(&self) -> bool {
        matches!(
            self,
            Self::String {
                qualified_reference_like: true,
                ..
            }
        )
    }

    /// Whether this string shape carries authored syntax that cannot be
    /// replaced by an inferred scalar type without losing information.
    #[must_use]
    pub fn requires_authored_string(&self) -> bool {
        self.is_expression_like() || self.is_qualified_reference_like()
    }

    /// Whether every authored value can be represented losslessly as a plain
    /// foreign-key key or list of keys.
    #[must_use]
    pub fn supports_lossless_foreign_key(&self) -> bool {
        !self.requires_authored_string() && matches!(self, Self::String { .. })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameSystemColorShape {
    LinearRgba,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameSystemNumberShape {
    Float,
    Integer,
    NonNegativeInteger,
    PositiveInteger,
    U8,
    NonZeroU8,
    U16,
    NonZeroU16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameSystemRangeBounds {
    Exclusive,
    Inclusive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameSystemEnumRepresentation {
    U8,
    I32,
    U32,
    Crc32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameSystemEnumVariant {
    pub name: String,
    pub discriminant: i64,
    #[serde(default)]
    pub source_tokens: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameSystemEnumShape {
    pub name: String,
    pub representation: GameSystemEnumRepresentation,
    pub variants: Vec<GameSystemEnumVariant>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "shape", rename_all = "snake_case")]
pub enum GameSystemListAtomShape {
    Boolean,
    Color {
        color_shape: GameSystemColorShape,
    },
    Number {
        number_shape: GameSystemNumberShape,
    },
    Range {
        bounds: GameSystemRangeBounds,
        number_shape: GameSystemNumberShape,
    },
    Crc32,
    Enum {
        enum_shape: GameSystemEnumShape,
    },
    String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "shape", rename_all = "snake_case")]
pub enum GameSystemListElementShape {
    Boolean,
    Color {
        color_shape: GameSystemColorShape,
    },
    Number {
        number_shape: GameSystemNumberShape,
    },
    Range {
        bounds: GameSystemRangeBounds,
        number_shape: GameSystemNumberShape,
    },
    Crc32,
    Enum {
        enum_shape: GameSystemEnumShape,
    },
    Pair {
        separator: char,
        first: GameSystemListAtomShape,
        second: GameSystemListAtomShape,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        default_second_source_token: Option<String>,
    },
    String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameSystemColumnTypeAffinity {
    pub table_name: String,
    pub row_type_name: String,
    pub column_name: String,
    pub declared_type: ColumnType,
    pub observed_row_key: bool,
    pub effective_row_key: bool,
    pub observed_shape: GameSystemColumnValueShape,
    pub effective_shape: GameSystemColumnValueShape,
    pub confidence: f64,
    pub repairable: bool,
    pub repairs: Vec<GameSystemColumnTypeRepair>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameSystemColumnTypeRepair {
    pub kind: GameSystemColumnTypeRepairKind,
    pub from: GameSystemColumnValueShape,
    pub to: GameSystemColumnValueShape,
    pub confidence: f64,
    pub reason: String,
    pub row_index: Option<usize>,
    pub value: Option<String>,
    pub adjacent_column: Option<String>,
    pub adjacent_direction: Option<GameSystemAdjacentColumnDirection>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameSystemColumnTypeRepairKind {
    SemanticName,
    SemanticColor,
    Family,
    ForeignKey,
    NativeNumericText,
    NativeRangeText,
    NativeBooleanText,
    NativeText,
    AdjacentColumn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameSystemAdjacentColumnDirection {
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameSystemListShape {
    pub separators: Vec<String>,
    pub rows_with_lists: usize,
    pub total_entries: usize,
    #[serde(default, skip_serializing_if = "is_false")]
    pub preserve_empty_entries: bool,
    #[serde(default)]
    pub element_shape: Option<GameSystemListElementShape>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameSystemForeignKeyCandidate {
    pub target_table: String,
    pub target_column: String,
    pub checked_values: usize,
    pub matched_values: usize,
    pub missing_values: usize,
    pub confidence: f64,
}
