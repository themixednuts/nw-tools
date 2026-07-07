use std::collections::BTreeMap;
use std::fmt::Write as _;

use anyhow::Result;

use crate::table::{
    EmittedTableColumn, EmittedTableModule, SOURCE_INDEX_CHUNK_SIZE, TableCodeTypeGroup,
};

#[derive(Debug, Clone)]
struct SourceIndexEntry<'a> {
    table: &'a EmittedTableModule,
    source_path: &'a str,
    product_path: String,
}

#[derive(Debug, Clone)]
struct ChunkedSourceIndexEntry<'a> {
    entry: SourceIndexEntry<'a>,
    chunk_name: String,
}

impl SourceIndexEntry<'_> {
    fn descriptor_source(&self, column_targets: &ColumnTargetIndex<'_>) -> Result<String> {
        let mut source = String::new();
        writeln!(source, "gamedata::TableSchemaDescriptor::new(")?;
        writeln!(
            source,
            "    {},",
            rust_string(&self.table.schema.table_name)
        )?;
        writeln!(
            source,
            "    {},",
            rust_string(&self.table.schema.row_type_name)
        )?;
        writeln!(
            source,
            "    gamedata::TableSourceRoute::new(\"gamedata\", {}),",
            rust_string(source_route_path(self.source_path))
        )?;
        writeln!(
            source,
            "    gamedata::TableProductRoute::new(\"tables\", {}),",
            rust_string(product_route_path(&self.product_path))
        )?;
        writeln!(source, "    &[")?;
        for column in &self.table.columns {
            writeln!(
                source,
                "        {},",
                descriptor_column_source(column, column_targets)?
            )?;
        }
        writeln!(source, "    ],")?;
        write!(source, ")")?;
        Ok(source)
    }
}

#[derive(Debug, Clone, Copy)]
struct ColumnTarget<'a> {
    table_name: &'a str,
    row_name: &'a str,
    column_name: &'a str,
}

type ColumnTargetIndex<'a> = BTreeMap<String, ColumnTarget<'a>>;

fn column_target_index<'a>(
    groups: &'a BTreeMap<String, TableCodeTypeGroup>,
) -> ColumnTargetIndex<'a> {
    let mut index = BTreeMap::new();
    for (type_module, group) in groups {
        for table in &group.tables {
            for column in &table.columns {
                index.insert(
                    format!(
                        "{type_module}::{}::{}",
                        table.module_name, column.field.rust_column_marker
                    ),
                    ColumnTarget {
                        table_name: &table.schema.table_name,
                        row_name: &table.schema.row_type_name,
                        column_name: &column.schema.name,
                    },
                );
            }
        }
    }
    index
}

fn source_index_entries(
    groups: &BTreeMap<String, TableCodeTypeGroup>,
) -> Vec<SourceIndexEntry<'_>> {
    groups
        .iter()
        .flat_map(|(type_module, group)| {
            group.tables.iter().flat_map(move |table| {
                table
                    .source_paths
                    .iter()
                    .map(move |source_path| SourceIndexEntry {
                        table,
                        source_path,
                        product_path: table_product_path(type_module, table),
                    })
            })
        })
        .collect()
}

fn chunked_source_index_entries<'a>(
    entries: &[SourceIndexEntry<'a>],
) -> Vec<ChunkedSourceIndexEntry<'a>> {
    let chunk_names = source_index_chunk_names(entries.len());
    chunk_names
        .into_iter()
        .zip(entries.chunks(SOURCE_INDEX_CHUNK_SIZE))
        .flat_map(|(chunk_name, chunk_entries)| {
            chunk_entries
                .iter()
                .cloned()
                .map(move |entry| ChunkedSourceIndexEntry {
                    entry,
                    chunk_name: chunk_name.clone(),
                })
        })
        .collect()
}

fn table_product_path(type_module: &str, table: &EmittedTableModule) -> String {
    format!("tables/{type_module}/{}.aztbl", table.module_name)
}

fn source_index_chunk_names(entry_count: usize) -> Vec<String> {
    let chunk_count = entry_count.div_ceil(SOURCE_INDEX_CHUNK_SIZE);
    (0..chunk_count)
        .map(|index| format!("chunk_{index:03}"))
        .collect()
}

fn descriptor_column_source(
    column: &EmittedTableColumn,
    column_targets: &ColumnTargetIndex<'_>,
) -> Result<String> {
    let mut source = format!(
        "gamedata::ColumnSchemaDescriptor::new({}, {}, {})",
        rust_string(&column.field.rust_name),
        rust_string(&column.schema.name),
        cell_type_source(column.cell_type)
    );

    if column.schema.row_key {
        source.push_str(".row_key(true)");
    }
    if !column.schema.required {
        source.push_str(".required(false)");
    }

    let enum_variants = enum_variant_sources(column);
    if !enum_variants.is_empty() {
        source.push_str(".with_enum_variants(&[");
        source.push_str(&enum_variants.join(", "));
        source.push_str("])");
    }

    let foreign_key_targets = foreign_key_target_sources(column, column_targets)?;
    if !foreign_key_targets.is_empty() {
        source.push_str(".with_foreign_key_targets(&[");
        source.push_str(&foreign_key_targets.join(", "));
        source.push_str("])");
    }

    Ok(source)
}

fn enum_variant_sources(column: &EmittedTableColumn) -> Vec<String> {
    let Some(enum_shape) = column_enum_shape(column) else {
        return Vec::new();
    };
    enum_shape
        .variants
        .iter()
        .map(|variant| {
            let source_tokens = variant
                .source_tokens
                .iter()
                .map(|token| rust_string(token))
                .collect::<Vec<_>>();
            format!(
                "gamedata::EnumVariantDescriptor::new({}, &[{}], {})",
                rust_string(&variant.name),
                source_tokens.join(", "),
                signed_i64_literal(variant.discriminant)
            )
        })
        .collect()
}

fn column_enum_shape(
    column: &EmittedTableColumn,
) -> Option<&crate::game_system_schema::GameSystemEnumShape> {
    match &column.schema.value_shape {
        crate::game_system_schema::GameSystemColumnValueShape::Enum { enum_shape } => {
            Some(enum_shape)
        }
        crate::game_system_schema::GameSystemColumnValueShape::String {
            list: Some(list), ..
        } => match list.element_shape.as_ref() {
            Some(crate::game_system_schema::GameSystemListElementShape::Enum { enum_shape }) => {
                Some(enum_shape)
            }
            Some(
                crate::game_system_schema::GameSystemListElementShape::Boolean
                | crate::game_system_schema::GameSystemListElementShape::Color { .. }
                | crate::game_system_schema::GameSystemListElementShape::Number { .. }
                | crate::game_system_schema::GameSystemListElementShape::Range { .. }
                | crate::game_system_schema::GameSystemListElementShape::Crc32
                | crate::game_system_schema::GameSystemListElementShape::Pair { .. }
                | crate::game_system_schema::GameSystemListElementShape::String,
            )
            | None => None,
        },
        crate::game_system_schema::GameSystemColumnValueShape::Boolean
        | crate::game_system_schema::GameSystemColumnValueShape::Color { .. }
        | crate::game_system_schema::GameSystemColumnValueShape::Crc32
        | crate::game_system_schema::GameSystemColumnValueShape::Number { .. }
        | crate::game_system_schema::GameSystemColumnValueShape::Range { .. }
        | crate::game_system_schema::GameSystemColumnValueShape::String { list: None, .. } => None,
    }
}

fn foreign_key_target_sources(
    column: &EmittedTableColumn,
    column_targets: &ColumnTargetIndex<'_>,
) -> Result<Vec<String>> {
    column
        .field
        .foreign_key_meta_columns
        .iter()
        .map(|target| {
            let Some(target) = column_targets.get(&target.rust_marker) else {
                anyhow::bail!(
                    "missing descriptor foreign-key target metadata for {}",
                    target.rust_marker
                );
            };
            Ok(format!(
                "gamedata::ForeignKeyTargetDescriptor::new({}, {}, {})",
                rust_string(target.table_name),
                rust_string(target.row_name),
                rust_string(target.column_name)
            ))
        })
        .collect()
}

fn cell_type_source(cell_type: gamedata::CellType) -> String {
    match cell_type {
        gamedata::CellType::Scalar(scalar) => {
            format!("gamedata::CellType::Scalar({})", scalar_type_source(scalar))
        }
        gamedata::CellType::Range(range) => {
            format!("gamedata::CellType::Range({})", range_type_source(range))
        }
        gamedata::CellType::List(element) => {
            format!(
                "gamedata::CellType::List({})",
                list_element_type_source(element)
            )
        }
    }
}

fn list_element_type_source(element: gamedata::ListElementType) -> String {
    match element {
        gamedata::ListElementType::Scalar(scalar) => {
            format!(
                "gamedata::ListElementType::Scalar({})",
                scalar_type_source(scalar)
            )
        }
        gamedata::ListElementType::Range(range) => {
            format!(
                "gamedata::ListElementType::Range({})",
                range_type_source(range)
            )
        }
        gamedata::ListElementType::Pair(pair) => {
            format!(
                "gamedata::ListElementType::Pair(gamedata::PairType::new({}, {}))",
                atom_type_source(pair.first),
                atom_type_source(pair.second)
            )
        }
    }
}

fn atom_type_source(atom: gamedata::AtomType) -> String {
    match atom {
        gamedata::AtomType::Scalar(scalar) => {
            format!("gamedata::AtomType::Scalar({})", scalar_type_source(scalar))
        }
        gamedata::AtomType::Range(range) => {
            format!("gamedata::AtomType::Range({})", range_type_source(range))
        }
    }
}

fn range_type_source(range: gamedata::RangeType) -> String {
    format!(
        "gamedata::RangeType {{ bounds: {}, endpoint: {} }}",
        range_bounds_source(range.bounds),
        range_endpoint_type_source(range.endpoint)
    )
}

fn range_bounds_source(bounds: gamedata::RangeBounds) -> &'static str {
    match bounds {
        gamedata::RangeBounds::Inclusive => "gamedata::RangeBounds::Inclusive",
        gamedata::RangeBounds::Exclusive => "gamedata::RangeBounds::Exclusive",
    }
}

fn range_endpoint_type_source(endpoint: gamedata::RangeEndpointType) -> &'static str {
    match endpoint {
        gamedata::RangeEndpointType::I32 => "gamedata::RangeEndpointType::I32",
        gamedata::RangeEndpointType::U32 => "gamedata::RangeEndpointType::U32",
        gamedata::RangeEndpointType::F32 => "gamedata::RangeEndpointType::F32",
    }
}

fn scalar_type_source(scalar: gamedata::ScalarType) -> &'static str {
    match scalar {
        gamedata::ScalarType::Bool => "gamedata::ScalarType::Bool",
        gamedata::ScalarType::I8 => "gamedata::ScalarType::I8",
        gamedata::ScalarType::I16 => "gamedata::ScalarType::I16",
        gamedata::ScalarType::I32 => "gamedata::ScalarType::I32",
        gamedata::ScalarType::I64 => "gamedata::ScalarType::I64",
        gamedata::ScalarType::NonZeroI8 => "gamedata::ScalarType::NonZeroI8",
        gamedata::ScalarType::NonZeroI16 => "gamedata::ScalarType::NonZeroI16",
        gamedata::ScalarType::NonZeroI32 => "gamedata::ScalarType::NonZeroI32",
        gamedata::ScalarType::NonZeroI64 => "gamedata::ScalarType::NonZeroI64",
        gamedata::ScalarType::U8 => "gamedata::ScalarType::U8",
        gamedata::ScalarType::U16 => "gamedata::ScalarType::U16",
        gamedata::ScalarType::U32 => "gamedata::ScalarType::U32",
        gamedata::ScalarType::U64 => "gamedata::ScalarType::U64",
        gamedata::ScalarType::NonZeroU8 => "gamedata::ScalarType::NonZeroU8",
        gamedata::ScalarType::NonZeroU16 => "gamedata::ScalarType::NonZeroU16",
        gamedata::ScalarType::NonZeroU32 => "gamedata::ScalarType::NonZeroU32",
        gamedata::ScalarType::NonZeroU64 => "gamedata::ScalarType::NonZeroU64",
        gamedata::ScalarType::F32 => "gamedata::ScalarType::F32",
        gamedata::ScalarType::F64 => "gamedata::ScalarType::F64",
        gamedata::ScalarType::Crc32 => "gamedata::ScalarType::Crc32",
        gamedata::ScalarType::RowIndex => "gamedata::ScalarType::RowIndex",
        gamedata::ScalarType::RowKey => "gamedata::ScalarType::RowKey",
        gamedata::ScalarType::ForeignKey => "gamedata::ScalarType::ForeignKey",
        gamedata::ScalarType::String => "gamedata::ScalarType::String",
        gamedata::ScalarType::LinearRgba => "gamedata::ScalarType::LinearRgba",
    }
}

fn source_route_path(source_path: &str) -> &str {
    source_path.strip_prefix("gamedata/").unwrap_or(source_path)
}

fn product_route_path(product_path: &str) -> &str {
    product_path.strip_prefix("tables/").unwrap_or(product_path)
}

fn signed_i64_literal(value: i64) -> String {
    if value == i64::MIN {
        "i64::MIN".to_owned()
    } else {
        format!("{value}_i64")
    }
}

fn rust_string(value: &str) -> String {
    format!("{value:?}")
}

pub(crate) fn render_source_index_mod(
    groups: &BTreeMap<String, TableCodeTypeGroup>,
) -> Result<String> {
    let entries = source_index_entries(groups);
    let chunk_names = source_index_chunk_names(entries.len());
    let mut source = String::new();
    writeln!(source, "#![allow(dead_code)]")?;
    writeln!(source)?;
    for chunk_name in &chunk_names {
        writeln!(source, "pub(crate) mod {chunk_name};")?;
    }
    writeln!(source)?;
    writeln!(source, "#[derive(Debug, Clone, Copy)]")?;
    writeln!(source, "pub(crate) struct TableSourceEntry {{")?;
    writeln!(source, "    source_path: &'static str,")?;
    writeln!(source, "    product_path: &'static str,")?;
    writeln!(source, "    schema: gamedata::TableSchemaDescriptor,")?;
    writeln!(source, "}}")?;
    writeln!(source)?;
    writeln!(source, "impl TableSourceEntry {{")?;
    writeln!(source, "    pub(super) const fn new(")?;
    writeln!(source, "        source_path: &'static str,")?;
    writeln!(source, "        product_path: &'static str,")?;
    writeln!(source, "        schema: gamedata::TableSchemaDescriptor,")?;
    writeln!(source, "    ) -> Self {{")?;
    writeln!(
        source,
        "        Self {{ source_path, product_path, schema }}"
    )?;
    writeln!(source, "    }}")?;
    writeln!(source)?;
    writeln!(
        source,
        "    pub(crate) const fn source_path(self) -> &'static str {{"
    )?;
    writeln!(source, "        self.source_path")?;
    writeln!(source, "    }}")?;
    writeln!(source)?;
    writeln!(
        source,
        "    pub(crate) const fn product_path(self) -> &'static str {{"
    )?;
    writeln!(source, "        self.product_path")?;
    writeln!(source, "    }}")?;
    writeln!(source)?;
    writeln!(
        source,
        "    pub(crate) const fn schema(&self) -> &gamedata::TableSchemaDescriptor {{"
    )?;
    writeln!(source, "        &self.schema")?;
    writeln!(source, "    }}")?;
    writeln!(source, "}}")?;
    writeln!(source)?;
    writeln!(
        source,
        "pub(crate) fn tables() -> impl Iterator<Item = &'static TableSourceEntry> {{"
    )?;
    writeln!(source, "    [")?;
    for chunk_name in &chunk_names {
        writeln!(source, "        {chunk_name}::TABLES,")?;
    }
    writeln!(source, "    ]")?;
    writeln!(source, "    .into_iter()")?;
    writeln!(source, "    .flatten()")?;
    writeln!(source, "}}")?;
    writeln!(source)?;
    writeln!(source, "pub const fn table_count() -> usize {{")?;
    write!(source, "    0")?;
    for chunk_name in &chunk_names {
        write!(source, " + {chunk_name}::TABLES.len()")?;
    }
    writeln!(source)?;
    writeln!(source, "}}")?;
    writeln!(source)?;
    writeln!(
        source,
        "pub fn table_source_paths() -> impl Iterator<Item = &'static str> {{"
    )?;
    writeln!(source, "    tables().map(|table| table.source_path())")?;
    writeln!(source, "}}")?;
    writeln!(source)?;
    writeln!(
        source,
        "pub fn table_product_paths() -> impl Iterator<Item = (gamedata::TableRequirement, &'static str)> {{"
    )?;
    writeln!(
        source,
        "    tables().map(|table| (table.schema().requirement(), table.product_path()))"
    )?;
    writeln!(source, "}}")?;
    writeln!(source)?;
    writeln!(
        source,
        "pub fn table_product_path(requirement: gamedata::TableRequirement) -> Option<&'static str> {{"
    )?;
    writeln!(source, "    tables()")?;
    writeln!(
        source,
        "        .find(|table| table.schema().requirement() == requirement)"
    )?;
    writeln!(source, "        .map(|table| table.product_path())")?;
    writeln!(source, "}}")?;
    writeln!(source)?;
    writeln!(
        source,
        "pub fn table_for_source_path(source_path: &str) -> Option<&'static gamedata::TableSchemaDescriptor> {{"
    )?;
    writeln!(source, "    tables()")?;
    writeln!(
        source,
        "        .find(|table| table.source_path() == source_path)"
    )?;
    writeln!(source, "        .map(|table| table.schema())")?;
    writeln!(source, "}}")?;
    writeln!(source)?;
    writeln!(
        source,
        "pub fn foreign_key_source_path(foreign_key: gamedata::ForeignKeyTargetDescriptor) -> Option<&'static str> {{"
    )?;
    writeln!(source, "    tables()")?;
    writeln!(source, "        .find(|table| {{")?;
    writeln!(
        source,
        "            table.schema().table_name_crc() == foreign_key.target_table_crc()"
    )?;
    writeln!(
        source,
        "                && table.schema().row_name_crc() == foreign_key.target_row_crc()"
    )?;
    writeln!(source, "        }})")?;
    writeln!(source, "        .map(|table| table.source_path())")?;
    writeln!(source, "}}")?;
    Ok(source)
}

pub(crate) fn render_source_index_chunks(
    groups: &BTreeMap<String, TableCodeTypeGroup>,
) -> Result<Vec<(String, String)>> {
    let entries = source_index_entries(groups);
    let column_targets = column_target_index(groups);
    let chunk_names = source_index_chunk_names(entries.len());
    let chunked_entries = chunked_source_index_entries(&entries);

    let mut chunks = Vec::new();
    for chunk_name in chunk_names {
        let mut source = String::new();
        writeln!(source, "#![allow(dead_code)]")?;
        writeln!(source, "#![allow(clippy::struct_excessive_bools)]")?;
        writeln!(source)?;
        writeln!(source, "use super::TableSourceEntry;")?;
        writeln!(source)?;
        writeln!(source, "pub(super) const TABLES: &[TableSourceEntry] = &[")?;
        for chunk_entry in chunked_entries
            .iter()
            .filter(|entry| entry.chunk_name.as_str() == chunk_name.as_str())
        {
            writeln!(source, "    TableSourceEntry::new(")?;
            writeln!(
                source,
                "        {},",
                rust_string(chunk_entry.entry.source_path)
            )?;
            writeln!(
                source,
                "        {},",
                rust_string(&chunk_entry.entry.product_path)
            )?;
            writeln!(
                source,
                "        {},",
                chunk_entry.entry.descriptor_source(&column_targets)?
            )?;
            writeln!(source, "    ),")?;
        }
        writeln!(source, "];")?;
        chunks.push((chunk_name, source));
    }
    Ok(chunks)
}
