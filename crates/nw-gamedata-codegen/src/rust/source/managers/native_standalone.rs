//! Schema-row-backed native manager augmentation for the standalone Rust target.
//!
//! This module deliberately emits fragments for the existing direct-manager
//! wrapper instead of rendering another manager runtime.  Generated indexes own
//! `RowRef`s into decoded `RowCollection`s, so they do not depend on an engine,
//! generated table views, or a second representation of datasheet rows.

use std::collections::BTreeSet;

use anyhow::{Result, anyhow, bail};
use nw_datasheet::ColumnType;

use crate::game_system_schema::GameSystemDataTablesSchemaReport;
use crate::manager::{
    NativeCrcIndexLookupMethod, NativeCrcIndexLookupParameterKind, NativeDuplicateKeyPolicy,
    NativeEnumLookupMethod, NativeEnumLookupParameterKind, NativeManagerShape,
    NativeNumericKeyType, NativeNumericLookupMethod, NativeNumericLookupParameterKind,
    NativeOneTableCostumeChangeManager, NativeOneTableCrcKeyProjectionManager,
    NativeStringLookupMethod, NativeStringLookupParameterKind, NativeStringLookupTarget,
    NativeTableFamilyTable,
};
use crate::manager_records::DirectManagerSurface;

use super::{
    RustStandaloneSchemaField, RustStandaloneSchemaRow, rust_direct_row_field_name,
    rust_direct_row_specs, rust_direct_table_type_for_row,
};

#[cfg(test)]
mod coverage_tests;
mod families_special;
mod indexed;
mod seasons;

/// Source fragments spliced into a standalone direct-manager wrapper.
///
/// `initializers` run after the wrapper's `RowCollection`s are built and before
/// `Self` is constructed. `field_values` belong in that `Self` expression.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct RustNativeManagerAugmentation {
    pub(crate) declarations: String,
    pub(crate) fields: String,
    pub(crate) field_values: String,
    pub(crate) initializers: String,
    pub(crate) methods: String,
    pub(crate) rows_type: String,
    pub(crate) rows_method: String,
}

impl RustNativeManagerAugmentation {
    fn append(&mut self, other: Self) {
        self.declarations.push_str(&other.declarations);
        self.fields.push_str(&other.fields);
        self.field_values.push_str(&other.field_values);
        self.initializers.push_str(&other.initializers);
        self.methods.push_str(&other.methods);
        if !other.rows_type.is_empty() {
            debug_assert!(
                self.rows_type.is_empty(),
                "multiple semantic Rows contracts"
            );
            self.rows_type = other.rows_type;
            self.rows_method = other.rows_method;
        }
    }

    fn merged(values: impl IntoIterator<Item = Self>) -> Self {
        let mut merged = Self::default();
        for value in values {
            merged.append(value);
        }
        merged
    }
}

/// Builds the standalone Rust augmentation for one native manager shape.
///
/// This dispatcher is intentionally exhaustive. A newly added shape must make
/// an explicit support decision here; unsupported semantics return an error and
/// never degrade to an unaugmented direct manager.
pub(crate) fn augment_native_manager(
    manager: &DirectManagerSurface,
    shape: &NativeManagerShape,
    schema_report: &GameSystemDataTablesSchemaReport,
) -> Result<RustNativeManagerAugmentation> {
    let augmentation = build_manager_augmentation(manager, shape, schema_report)?;
    let emitted = format!(
        "{}{}{}{}{}",
        augmentation.declarations,
        augmentation.fields,
        augmentation.field_values,
        augmentation.initializers,
        augmentation.methods
    );
    if emitted.contains("native") || emitted.contains("Native") {
        bail!(
            "standalone Rust manager `{}` leaked an implementation-history marker",
            manager.manager_name
        );
    }
    if augmentation.rows_type.is_empty() != augmentation.rows_method.is_empty() {
        bail!(
            "standalone Rust manager `{}` emitted an incomplete Rows contract",
            manager.manager_name
        );
    }
    if !augmentation.rows_method.is_empty()
        && !augmentation
            .methods
            .contains(&format!("fn {}(", augmentation.rows_method))
    {
        bail!(
            "standalone Rust manager `{}` Rows contract references missing method `{}`",
            manager.manager_name,
            augmentation.rows_method
        );
    }
    Ok(augmentation)
}

fn build_manager_augmentation(
    manager: &DirectManagerSurface,
    shape: &NativeManagerShape,
    schema_report: &GameSystemDataTablesSchemaReport,
) -> Result<RustNativeManagerAugmentation> {
    let rows = rust_direct_row_specs(manager, schema_report);
    let context = AugmentationContext {
        manager,
        schema_report,
        rows: &rows,
    };

    if let Some(augmentation) = families_special::augmentation(&context, shape) {
        return augmentation;
    }
    if let Some(augmentation) = indexed::augmentation(&context, shape) {
        return augmentation;
    }
    if let Some(augmentation) = seasons::augmentation(&context, shape)? {
        return Ok(augmentation);
    }

    match shape {
        NativeManagerShape::RequirementsOnly => Ok(RustNativeManagerAugmentation::default()),
        NativeManagerShape::OneTableCrcIndex(shape) => context.crc_index(
            shape.row_type_name().as_str(),
            shape.key_column().as_str(),
            shape.reject_zero_crc(),
            NativeDuplicateKeyPolicy::FirstWins,
            shape.methods(),
            "one_table_crc",
        ),
        NativeManagerShape::TableFamilyCrcIndex(shape) => {
            let row_type = context.only_row_type("TableFamilyCrcIndex")?;
            context.crc_index(
                row_type,
                shape.key_column().as_str(),
                shape.reject_zero_crc(),
                NativeDuplicateKeyPolicy::FirstWins,
                shape.methods(),
                "table_family_crc",
            )
        }
        NativeManagerShape::OneTableOwnedStringCrcIndex(shape) => context.crc_index(
            shape.row_type_name().as_str(),
            shape.key_column().as_str(),
            true,
            shape.duplicate_key_policy(),
            shape.methods(),
            "one_table_owned_string_crc",
        ),
        NativeManagerShape::TableFamilyOwnedStringCrcIndex(shape) => {
            let row_type = one_family_row_type(shape.tables(), "TableFamilyOwnedStringCrcIndex")?;
            context.crc_index(
                row_type,
                shape.key_column().as_str(),
                true,
                shape.duplicate_key_policy(),
                shape.methods(),
                "table_family_owned_string_crc",
            )
        }
        NativeManagerShape::OneTableCrcKeyProjection(shape) => {
            context.crc_projection(shape, "one_table_crc_projection")
        }
        NativeManagerShape::StaticBackstoryData(shape) => {
            context.crc_projection(shape.projection(), "one_table_crc_projection")
        }
        NativeManagerShape::MultiTableCrcKeyProjection(shape) => {
            let values = shape
                .projections()
                .iter()
                .enumerate()
                .map(|(index, projection)| {
                    context.crc_projection(projection, &format!("multi_table_crc_{index}"))
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(RustNativeManagerAugmentation::merged(values))
        }
        NativeManagerShape::OneTableNumericKeyProjection(shape) => context.numeric_index(
            shape.row_type_name().as_str(),
            shape.key_column().as_str(),
            shape.key_type(),
            shape.duplicate_key_policy(),
            shape.methods(),
            "one_table_numeric",
        ),
        NativeManagerShape::TableFamilyNumericKeyProjection(shape) => {
            let row_type = one_family_row_type(shape.tables(), "TableFamilyNumericKeyProjection")?;
            context.numeric_index(
                row_type,
                shape.key_column().as_str(),
                shape.key_type(),
                shape.duplicate_key_policy(),
                shape.methods(),
                "table_family_numeric",
            )
        }
        NativeManagerShape::OneTableEnumKeyProjection(shape) => {
            if shape.secondary_crc_index().is_some() {
                return context.unsupported(
                    "OneTableEnumKeyProjection",
                    "secondary CRC indexes require projected enum records",
                );
            }
            context.enum_index(
                shape.row_type_name().as_str(),
                shape.key_column().as_str(),
                shape.skip_empty_key(),
                shape.trim_key(),
                shape.duplicate_key_policy(),
                shape.key_type().as_str(),
                &shape
                    .invalid_key_variants()
                    .iter()
                    .map(|variant| variant.as_str())
                    .collect::<Vec<_>>(),
                shape.methods(),
                "one_table_enum",
            )
        }
        NativeManagerShape::OneTableStringKeyProjection(shape) => context.string_index(
            shape.row_type_name().as_str(),
            shape.key_column().as_str(),
            shape.skip_empty_key(),
            shape.duplicate_key_policy(),
            shape.methods(),
            "one_table_string",
        ),
        NativeManagerShape::OneTableRowProjection(shape) => context.row_aliases(
            shape.row_type_name().as_str(),
            shape.source_row_method().map(|value| value.as_str()),
            shape.rows_method().map(|value| value.as_str()),
        ),
        NativeManagerShape::OneTableCostumeChange(shape) => context.costume_change(shape),

        _ => unreachable!("shape was not handled by its standalone semantic dispatcher"),
    }
}

struct AugmentationContext<'a> {
    manager: &'a DirectManagerSurface,
    schema_report: &'a GameSystemDataTablesSchemaReport,
    rows: &'a [RustStandaloneSchemaRow],
}

impl AugmentationContext<'_> {
    fn unsupported<T>(&self, shape: &str, reason: &str) -> Result<T> {
        bail!(
            "standalone Rust native augmentation for manager `{}` does not support `{shape}`: {reason}",
            self.manager.manager_name
        )
    }

    fn row(&self, source_row_type: &str) -> Result<&RustStandaloneSchemaRow> {
        self.rows
            .iter()
            .find(|row| row.source_row_type == source_row_type)
            .ok_or_else(|| {
                anyhow!(
                    "standalone Rust native augmentation for manager `{}` requires schema row `{source_row_type}`",
                    self.manager.manager_name
                )
            })
    }

    fn find_first_row(&self, candidates: &[&str]) -> Result<&RustStandaloneSchemaRow> {
        candidates
            .iter()
            .find_map(|candidate| self.rows.iter().find(|row| row.source_row_type == *candidate))
            .ok_or_else(|| {
                anyhow!(
                    "standalone Rust native augmentation for manager `{}` requires one of schema rows {:?}",
                    self.manager.manager_name,
                    candidates
                )
            })
    }

    fn field<'a>(
        &self,
        row: &'a RustStandaloneSchemaRow,
        column: &str,
    ) -> Result<&'a RustStandaloneSchemaField> {
        row.fields
            .iter()
            .find(|field| field.source_name.eq_ignore_ascii_case(column))
            .ok_or_else(|| {
                anyhow!(
                    "standalone Rust native augmentation for manager `{}` requires column `{column}` on schema row `{}`",
                    self.manager.manager_name,
                    row.source_row_type
                )
            })
    }

    fn only_row_type(&self, shape: &str) -> Result<&str> {
        if self.rows.len() != 1 {
            return self.unsupported(shape, "the family contains more than one schema row type");
        }
        Ok(self.rows[0].source_row_type.as_str())
    }

    fn row_parts<'a>(&self, row: &'a RustStandaloneSchemaRow) -> RowParts<'a> {
        RowParts {
            row_type: &row.type_name,
            row_field: rust_direct_row_field_name(&row.source_row_type),
            table_type: rust_direct_table_type_for_row(self.manager, self.schema_report, row),
        }
    }

    fn simple_crc(
        &self,
        source_row_type: &str,
        key_column: &str,
        crc_methods: &[&str],
        string_methods: &[&str],
        tag: &str,
    ) -> Result<RustNativeManagerAugmentation> {
        let methods = crc_methods
            .iter()
            .map(|name| CrcMethodSpec::crc32(name))
            .chain(
                string_methods
                    .iter()
                    .map(|name| CrcMethodSpec::str_ref(name)),
            )
            .collect::<Vec<_>>();
        self.crc_index_with_specs(
            source_row_type,
            key_column,
            true,
            NativeDuplicateKeyPolicy::FirstWins,
            &methods,
            tag,
        )
    }

    fn crc_index(
        &self,
        source_row_type: &str,
        key_column: &str,
        reject_zero_crc: bool,
        duplicate_policy: NativeDuplicateKeyPolicy,
        methods: &[NativeCrcIndexLookupMethod],
        tag: &str,
    ) -> Result<RustNativeManagerAugmentation> {
        let methods = methods
            .iter()
            .map(|method| CrcMethodSpec {
                name: method.name().as_str(),
                parameter: method.parameter().name().as_str(),
                kind: method.parameter().kind(),
            })
            .collect::<Vec<_>>();
        self.crc_index_with_specs(
            source_row_type,
            key_column,
            reject_zero_crc,
            duplicate_policy,
            &methods,
            tag,
        )
    }

    fn crc_projection(
        &self,
        shape: &NativeOneTableCrcKeyProjectionManager,
        tag: &str,
    ) -> Result<RustNativeManagerAugmentation> {
        if !shape.row_filters().is_empty()
            || !shape.secondary_indexes().is_empty()
            || !shape.descending_f32_indexes().is_empty()
            || !shape.dependency_lookup_methods().is_empty()
            || shape.schema_validation_fields().is_some()
        {
            return self.unsupported(
                "OneTableCrcKeyProjection",
                "filters, secondary indexes, dependency lookups, and validation fields require projected records",
            );
        }
        self.crc_index(
            shape.row_type_name().as_str(),
            shape.key_column().as_str(),
            shape.reject_zero_crc(),
            shape.duplicate_key_policy(),
            shape.methods(),
            tag,
        )
    }

    fn crc_index_with_specs(
        &self,
        source_row_type: &str,
        key_column: &str,
        reject_zero_crc: bool,
        duplicate_policy: NativeDuplicateKeyPolicy,
        methods: &[CrcMethodSpec<'_>],
        tag: &str,
    ) -> Result<RustNativeManagerAugmentation> {
        let row = self.row(source_row_type)?;
        let key = self.field(row, key_column)?;
        require_string_field(self.manager, row, key)?;
        let parts = self.row_parts(row);
        let index = format!("{}_by_crc", rust_fragment_ident(tag));
        let key_value = string_value_expression(key, "source.row")?;
        let zero_guard = if reject_zero_crc {
            "            if key == Crc32::ZERO { continue; }\n"
        } else {
            ""
        };
        let insert = duplicate_insert(
            duplicate_policy,
            &index,
            "key",
            "source.reference.clone()",
            source_row_type,
        );
        let methods = methods
            .iter()
            .map(|method| crc_lookup_method(method, &index, &parts))
            .collect::<String>();

        Ok(RustNativeManagerAugmentation {
            fields: format!(
                "    {index}: HashMap<Crc32, RowRef<{}, {}>>,\n",
                parts.table_type, parts.row_type
            ),
            field_values: format!("            {index},\n"),
            initializers: format!(
                "        let mut {index} = HashMap::new();\n        for source in {row_field}.rows() {{\n            let key_text = {key_value}.trim();\n            if key_text.is_empty() {{ continue; }}\n            let key = Crc32::from_str_lower(key_text);\n{zero_guard}{insert}        }}\n",
                row_field = parts.row_field,
            ),
            methods,
            ..RustNativeManagerAugmentation::default()
        })
    }

    fn numeric_index(
        &self,
        source_row_type: &str,
        key_column: &str,
        key_type: NativeNumericKeyType,
        duplicate_policy: NativeDuplicateKeyPolicy,
        methods: &[NativeNumericLookupMethod],
        tag: &str,
    ) -> Result<RustNativeManagerAugmentation> {
        let methods = methods
            .iter()
            .map(|method| NumericMethodSpec {
                name: method.name().as_str(),
                parameter: method.parameter().as_str(),
                kind: method.parameter_kind(),
            })
            .collect::<Vec<_>>();
        self.numeric_index_with_specs(
            source_row_type,
            key_column,
            key_type,
            duplicate_policy,
            &methods,
            tag,
        )
    }

    fn numeric_index_with_specs(
        &self,
        source_row_type: &str,
        key_column: &str,
        key_type: NativeNumericKeyType,
        duplicate_policy: NativeDuplicateKeyPolicy,
        methods: &[NumericMethodSpec<'_>],
        tag: &str,
    ) -> Result<RustNativeManagerAugmentation> {
        let row = self.row(source_row_type)?;
        let key = self.field(row, key_column)?;
        if key.column_type != ColumnType::Number {
            bail!(
                "standalone Rust numeric index for manager `{}` requires numeric column `{key_column}` on `{source_row_type}`",
                self.manager.manager_name
            );
        }
        let parts = self.row_parts(row);
        let index = format!("{}_by_number", rust_fragment_ident(tag));
        let map_type = numeric_key_type(key_type);
        let raw = numeric_value_expression(key, "source.row")?;
        let conversion = numeric_conversion(key_type, &raw, source_row_type, key_column);
        let insert = duplicate_insert(
            duplicate_policy,
            &index,
            "key",
            "source.reference.clone()",
            source_row_type,
        );
        let methods = methods
            .iter()
            .map(|method| numeric_lookup_method(method, key_type, &index, &parts))
            .collect::<Result<String>>()?;

        Ok(RustNativeManagerAugmentation {
            fields: format!(
                "    {index}: HashMap<{map_type}, RowRef<{}, {}>>,\n",
                parts.table_type, parts.row_type
            ),
            field_values: format!("            {index},\n"),
            initializers: format!(
                "        let mut {index} = HashMap::new();\n        for source in {row_field}.rows() {{\n{conversion}{insert}        }}\n",
                row_field = parts.row_field,
            ),
            methods,
            ..RustNativeManagerAugmentation::default()
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn enum_index(
        &self,
        source_row_type: &str,
        key_column: &str,
        skip_empty: bool,
        trim_key: bool,
        duplicate_policy: NativeDuplicateKeyPolicy,
        key_type: &str,
        invalid_key_variants: &[&str],
        methods: &[NativeEnumLookupMethod],
        tag: &str,
    ) -> Result<RustNativeManagerAugmentation> {
        let row = self.row(source_row_type)?;
        let key = self.field(row, key_column)?;
        require_string_field(self.manager, row, key)?;
        let parts = self.row_parts(row);
        let index = format!("{}_by_enum", rust_fragment_ident(tag));
        let value = string_value_expression(key, "source.row")?;
        let normalized = if trim_key {
            format!("{value}.trim()")
        } else {
            value
        };
        let empty_guard = if skip_empty {
            "            if key_text.is_empty() { continue; }\n"
        } else {
            ""
        };
        let invalid_guard = if invalid_key_variants.is_empty() {
            String::new()
        } else {
            let patterns = invalid_key_variants
                .iter()
                .map(|variant| format!("{key_type}::{variant}"))
                .collect::<Vec<_>>()
                .join(" | ");
            format!("            if matches!(key, {patterns}) {{ continue; }}\n")
        };
        let insert = duplicate_insert(
            duplicate_policy,
            &index,
            "key",
            "source.reference.clone()",
            source_row_type,
        );
        let methods = methods
            .iter()
            .map(|method| enum_lookup_method(method, &index, &parts, key_type))
            .collect::<String>();
        Ok(RustNativeManagerAugmentation {
            fields: format!(
                "    {index}: HashMap<{key_type}, RowRef<{}, {}>>,\n",
                parts.table_type, parts.row_type
            ),
            field_values: format!("            {index},\n"),
            initializers: format!(
                "        let mut {index} = HashMap::new();\n        for source in {row_field}.rows() {{\n            let key_text = {normalized};\n{empty_guard}            let Ok(key) = {key_type}::try_from(key_text) else {{ continue; }};\n{invalid_guard}{insert}        }}\n",
                row_field = parts.row_field,
            ),
            methods,
            ..RustNativeManagerAugmentation::default()
        })
    }

    fn string_index(
        &self,
        source_row_type: &str,
        key_column: &str,
        skip_empty: bool,
        duplicate_policy: NativeDuplicateKeyPolicy,
        methods: &[NativeStringLookupMethod],
        tag: &str,
    ) -> Result<RustNativeManagerAugmentation> {
        if methods
            .iter()
            .any(|method| !matches!(method.target(), NativeStringLookupTarget::Key))
        {
            return self.unsupported(
                "OneTableStringKeyProjection",
                "lookup targets other than the primary key require secondary projected fields",
            );
        }
        let row = self.row(source_row_type)?;
        let key = self.field(row, key_column)?;
        require_string_field(self.manager, row, key)?;
        let parts = self.row_parts(row);
        let index = format!("{}_by_string", rust_fragment_ident(tag));
        let value = string_value_expression(key, "source.row")?;
        let empty_guard = if skip_empty {
            "            if key.is_empty() { continue; }\n"
        } else {
            ""
        };
        let insert = duplicate_insert(
            duplicate_policy,
            &index,
            "key.to_owned()",
            "source.reference.clone()",
            source_row_type,
        );
        let methods = methods
            .iter()
            .map(|method| string_lookup_method(method, &index, &parts))
            .collect::<String>();
        Ok(RustNativeManagerAugmentation {
            fields: format!(
                "    {index}: HashMap<String, RowRef<{}, {}>>,\n",
                parts.table_type, parts.row_type
            ),
            field_values: format!("            {index},\n"),
            initializers: format!(
                "        let mut {index} = HashMap::new();\n        for source in {row_field}.rows() {{\n            let key = {value};\n{empty_guard}{insert}        }}\n",
                row_field = parts.row_field,
            ),
            methods,
            ..RustNativeManagerAugmentation::default()
        })
    }

    fn row_aliases(
        &self,
        source_row_type: &str,
        source_row_method: Option<&str>,
        rows_method: Option<&str>,
    ) -> Result<RustNativeManagerAugmentation> {
        let row = self.row(source_row_type)?;
        let parts = self.row_parts(row);
        let mut methods = String::new();
        if let Some(name) = source_row_method {
            methods.push_str(&format!(
                "    pub fn {name}(&self, slot: &RowSlot<{}, {}>) -> Option<&{}> {{\n        self.{}.row_by_index(slot)\n    }}\n\n",
                parts.table_type, parts.row_type, parts.row_type, parts.row_field
            ));
        }
        if let Some(name) = rows_method {
            methods.push_str(&named_rows_method(name, &parts));
        }
        Ok(RustNativeManagerAugmentation {
            methods,
            ..RustNativeManagerAugmentation::default()
        })
    }

    fn named_rows(
        &self,
        source_row_type: &str,
        method: &str,
    ) -> Result<RustNativeManagerAugmentation> {
        let row = self.row(source_row_type)?;
        Ok(RustNativeManagerAugmentation {
            methods: named_rows_method(method, &self.row_parts(row)),
            ..RustNativeManagerAugmentation::default()
        })
    }

    fn costume_change(
        &self,
        shape: &NativeOneTableCostumeChangeManager,
    ) -> Result<RustNativeManagerAugmentation> {
        let row = self.row(shape.row_type_name().as_str())?;
        let parts = self.row_parts(row);
        let key = self.field(row, shape.key_column().as_str())?;
        let mesh = self.field(row, shape.mesh_column().as_str())?;
        let matches_skeleton = self.field(row, shape.matches_skeleton_column().as_str())?;
        let z_offset = self.field(row, shape.z_offset_column().as_str())?;
        require_string_field(self.manager, row, key)?;
        require_string_field(self.manager, row, mesh)?;

        let data_type = shape.data_type().as_str();
        let slot_type = shape.slot_type().as_str();
        let override_type = shape.override_type().as_str();
        let entries = shape.entries_field().as_str();
        let by_id = shape.index_field().as_str();
        let by_source = shape.source_row_index_field().as_str();
        let source_row = shape.source_row_field().as_str();
        let key_field = shape.key_field().as_str();
        let key_text = shape.key_text_field().as_str();
        let mesh_field = shape.mesh_field().as_str();
        let skeleton_field = shape.matches_skeleton_field().as_str();
        let z_offset_field = shape.z_offset_field().as_str();
        let audio_overrides = shape.audio_overrides_field().as_str();
        let slot_count = shape.slots().len();

        let slot_variants = shape
            .slots()
            .iter()
            .enumerate()
            .map(|(index, slot)| format!("    {} = {index},\n", slot.variant().as_str()))
            .collect::<String>();
        let slot_names = shape
            .slots()
            .iter()
            .map(|slot| {
                format!(
                    "            Self::{} => {:?},\n",
                    slot.variant().as_str(),
                    slot.display().as_str()
                )
            })
            .collect::<String>();
        let audio_values = shape
            .slots()
            .iter()
            .map(|slot| {
                let left = self
                    .field(row, slot.left_column().as_str())
                    .and_then(|field| string_value_expression(field, "source.row"))?;
                let right = self
                    .field(row, slot.right_column().as_str())
                    .and_then(|field| string_value_expression(field, "source.row"))?;
                Ok(format!(
                    "                {override_type}::new(costume_audio_override_id({left}), costume_audio_override_id({right})),\n"
                ))
            })
            .collect::<Result<String>>()?;

        let key_value = string_value_expression(key, "source.row")?;
        let mesh_value = string_value_expression(mesh, "source.row")?;
        let skeleton_value = boolean_value_expression(matches_skeleton, "source.row")?;
        let z_offset_value = numeric_value_or_default_expression(z_offset, "source.row", "0.0")?;

        let declarations = format!(
            r#"#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum {slot_type} {{
{slot_variants}}}

impl {slot_type} {{
    pub const COUNT: usize = {slot_count};

    #[must_use]
    pub const fn as_str(self) -> &'static str {{
        match self {{
{slot_names}        }}
    }}

    #[must_use]
    pub const fn index(self) -> usize {{ self as usize }}
}}

impl AsRef<str> for {slot_type} {{
    fn as_ref(&self) -> &str {{ self.as_str() }}
}}

impl std::fmt::Display for {slot_type} {{
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{
        formatter.write_str(self.as_str())
    }}
}}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct {override_type} {{
    left: Option<Crc32>,
    right: Option<Crc32>,
}}

impl {override_type} {{
    #[must_use]
    pub const fn new(left: Option<Crc32>, right: Option<Crc32>) -> Self {{ Self {{ left, right }} }}
    #[must_use]
    pub const fn left(self) -> Option<Crc32> {{ self.left }}
    #[must_use]
    pub const fn right(self) -> Option<Crc32> {{ self.right }}
    #[must_use]
    pub const fn is_empty(self) -> bool {{ self.left.is_none() && self.right.is_none() }}
}}

#[derive(Debug, Clone, PartialEq)]
pub struct {data_type} {{
    {source_row}: RowRef<{table_type}, {row_type}>,
    {key_field}: Crc32,
    {key_text}: String,
    {mesh_field}: String,
    {skeleton_field}: bool,
    {z_offset_field}: f32,
    {audio_overrides}: [{override_type}; {slot_count}],
}}

impl {data_type} {{
    #[must_use]
    pub const fn {source_row}(&self) -> &RowRef<{table_type}, {row_type}> {{ &self.{source_row} }}
    #[must_use]
    pub const fn {key_field}(&self) -> Crc32 {{ self.{key_field} }}
    #[must_use]
    pub fn {key_text}(&self) -> &str {{ &self.{key_text} }}
    #[must_use]
    pub fn {mesh_field}(&self) -> &str {{ &self.{mesh_field} }}
    #[must_use]
    pub const fn {skeleton_field}(&self) -> bool {{ self.{skeleton_field} }}
    #[must_use]
    pub const fn {z_offset_field}(&self) -> f32 {{ self.{z_offset_field} }}
    #[must_use]
    pub const fn audio_override(&self, slot: {slot_type}) -> {override_type} {{ self.{audio_overrides}[slot.index()] }}
    #[must_use]
    pub const fn {audio_overrides}(&self) -> &[{override_type}; {slot_count}] {{ &self.{audio_overrides} }}
}}

fn costume_audio_override_id(value: &str) -> Option<Crc32> {{
    let value = value.trim();
    (!value.is_empty()).then(|| Crc32::from_str_lower(value))
}}

"#,
            table_type = parts.table_type,
            row_type = parts.row_type,
        );

        let methods = format!(
            r#"    #[must_use]
    pub fn {lookup_from_id}(&self, costume_change_id: Crc32) -> Option<&{data_type}> {{
        self.{entries}.get(*self.{by_id}.get(&costume_change_id)?)
    }}

    #[must_use]
    pub fn {lookup}(&self, costume_change_id: impl AsRef<str>) -> Option<&{data_type}> {{
        self.{lookup_from_id}(Crc32::from_str_lower(costume_change_id.as_ref()))
    }}

    #[must_use]
    pub fn {lookup_by_key}(&self, key: impl AsRef<str>) -> Option<&{data_type}> {{ self.{lookup}(key) }}

    #[must_use]
    pub fn {source_row_method}(&self, source: &RowRef<{table_type}, {row_type}>) -> Option<&{data_type}> {{
        self.{entries}.get(*self.{by_source}.get(source)?)
    }}

    #[must_use]
    pub fn {audio_from_id}(&self, costume_change_id: Crc32, slot: {slot_type}) -> Option<{override_type}> {{
        Some(self.{lookup_from_id}(costume_change_id)?.audio_override(slot))
    }}

    #[must_use]
    pub fn {audio}(&self, costume_change_id: impl AsRef<str>, slot: {slot_type}) -> Option<{override_type}> {{
        self.{audio_from_id}(Crc32::from_str_lower(costume_change_id.as_ref()), slot)
    }}

    pub fn {rows_method}(&self) -> impl ExactSizeIterator<Item = &{data_type}> + '_ {{ self.{entries}.iter() }}
    #[must_use]
    pub fn {len_method}(&self) -> usize {{ self.{entries}.len() }}
    #[must_use]
    pub fn {is_empty_method}(&self) -> bool {{ self.{entries}.is_empty() }}

"#,
            lookup_from_id = shape.lookup_from_id_method().as_str(),
            lookup = shape.lookup_method().as_str(),
            lookup_by_key = shape.lookup_by_key_method().as_str(),
            source_row_method = shape.source_row_method().as_str(),
            audio_from_id = shape.audio_override_from_id_method().as_str(),
            audio = shape.audio_override_method().as_str(),
            rows_method = shape.rows_method().as_str(),
            len_method = shape.len_method().as_str(),
            is_empty_method = shape.is_empty_method().as_str(),
            table_type = parts.table_type,
            row_type = parts.row_type,
        );

        Ok(RustNativeManagerAugmentation {
            declarations,
            fields: format!(
                "    {entries}: Vec<{data_type}>,\n    {by_id}: HashMap<Crc32, usize>,\n    {by_source}: HashMap<RowRef<{}, {}>, usize>,\n",
                parts.table_type, parts.row_type
            ),
            field_values: format!(
                "            {entries},\n            {by_id},\n            {by_source},\n"
            ),
            initializers: format!(
                r#"        let mut {entries} = Vec::new();
        let mut {by_id} = HashMap::new();
        let mut {by_source} = HashMap::new();
        for source in {row_field}.rows() {{
            let key_text_value = {key_value}.trim();
            if key_text_value.is_empty() {{ continue; }}
            let key = Crc32::from_str_lower(key_text_value);
            if key == Crc32::ZERO || {by_id}.contains_key(&key) {{ continue; }}
            let data = {data_type} {{
                {source_row}: source.reference.clone(),
                {key_field}: key,
                {key_text}: key_text_value.to_owned(),
                {mesh_field}: {mesh_value}.to_owned(),
                {skeleton_field}: {skeleton_value},
                {z_offset_field}: {z_offset_value},
                {audio_overrides}: [
{audio_values}                ],
            }};
            let index = {entries}.len();
            {by_id}.insert(key, index);
            {by_source}.insert(source.reference.clone(), index);
            {entries}.push(data);
        }}
"#,
                row_field = parts.row_field,
            ),
            methods,
            rows_type: data_type.to_owned(),
            rows_method: shape.rows_method().as_str().to_owned(),
        })
    }

    fn contribution(&self) -> Result<RustNativeManagerAugmentation> {
        let row = self.row("ContributionData")?;
        let id = self.field(row, "ContributionID")?;
        let category = self.field(row, "Category")?;
        require_string_field(self.manager, row, id)?;
        require_string_field(self.manager, row, category)?;
        let parts = self.row_parts(row);
        let id_value = string_value_expression(id, "source.row")?;
        let category_value = string_value_expression(category, "source.row")?;
        let key_type = format!("{}ContributionKey", self.manager.manager_class_name);
        let index = "contributions";
        Ok(RustNativeManagerAugmentation {
            declarations: format!(
                "#[derive(Debug, Clone, PartialEq, Eq, Hash)]\nstruct {key_type} {{\n    table: {},\n    id: Crc32,\n    category: String,\n}}\n\n",
                parts.table_type
            ),
            fields: format!(
                "    {index}: HashMap<{key_type}, RowRef<{}, {}>>,\n",
                parts.table_type, parts.row_type
            ),
            field_values: format!("            {index},\n"),
            initializers: format!(
                "        let mut {index} = HashMap::new();\n        for source in {row_field}.rows() {{\n            let id_text = {id_value}.trim();\n            if id_text.is_empty() {{ continue; }}\n            let key = {key_type} {{\n                table: *source.reference.table(),\n                id: Crc32::from_str_lower(id_text),\n                category: {category_value}.trim().to_ascii_lowercase(),\n            }};\n            {index}.insert(key, source.reference.clone());\n        }}\n",
                row_field = parts.row_field,
            ),
            methods: format!(
                "    pub fn contribution_data(&self, table: {}, contribution_id: Crc32, category: &str) -> Option<&{}> {{\n        let key = {key_type} {{ table, id: contribution_id, category: category.trim().to_ascii_lowercase() }};\n        self.{}.get(self.{index}.get(&key)?)\n    }}\n\n    pub fn contribution_data_by_key(&self, table: {}, contribution_id: &str, category: &str) -> Option<&{}> {{\n        self.contribution_data(table, Crc32::from_str_lower(contribution_id), category)\n    }}\n\n",
                parts.table_type, parts.row_type, parts.row_field, parts.table_type, parts.row_type,
            ),
            ..RustNativeManagerAugmentation::default()
        })
    }
}

struct RowParts<'a> {
    row_type: &'a str,
    row_field: String,
    table_type: String,
}

struct CrcMethodSpec<'a> {
    name: &'a str,
    parameter: &'a str,
    kind: NativeCrcIndexLookupParameterKind,
}

impl<'a> CrcMethodSpec<'a> {
    const fn crc32(name: &'a str) -> Self {
        Self {
            name,
            parameter: "id",
            kind: NativeCrcIndexLookupParameterKind::Crc32,
        }
    }

    const fn str_ref(name: &'a str) -> Self {
        Self {
            name,
            parameter: "key",
            kind: NativeCrcIndexLookupParameterKind::StrRef,
        }
    }
}

struct NumericMethodSpec<'a> {
    name: &'a str,
    parameter: &'a str,
    kind: NativeNumericLookupParameterKind,
}

fn one_family_row_type<'a>(tables: &'a [NativeTableFamilyTable], shape: &str) -> Result<&'a str> {
    let row_types = tables
        .iter()
        .map(|table| table.row_type_name().as_str())
        .collect::<BTreeSet<_>>();
    if row_types.len() != 1 {
        bail!(
            "standalone Rust native augmentation does not support `{shape}` with heterogeneous schema row types: {:?}",
            row_types
        );
    }
    row_types
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("`{shape}` has no table-family rows"))
}

fn require_string_field(
    manager: &DirectManagerSurface,
    row: &RustStandaloneSchemaRow,
    field: &RustStandaloneSchemaField,
) -> Result<()> {
    if field.column_type != ColumnType::String {
        bail!(
            "standalone Rust native augmentation for manager `{}` requires string column `{}` on `{}`",
            manager.manager_name,
            field.source_name,
            row.source_row_type
        );
    }
    Ok(())
}

fn string_value_expression(field: &RustStandaloneSchemaField, receiver: &str) -> Result<String> {
    match (field.column_type, field.required) {
        (ColumnType::String, true) => Ok(format!("{receiver}.{}.as_str()", field.field_name)),
        (ColumnType::String, false) => Ok(format!(
            "{receiver}.{}.as_deref().unwrap_or_default()",
            field.field_name
        )),
        _ => bail!("column `{}` is not a string", field.source_name),
    }
}

fn numeric_value_expression(field: &RustStandaloneSchemaField, receiver: &str) -> Result<String> {
    match (field.column_type, field.required) {
        (ColumnType::Number, true) => Ok(format!("{receiver}.{}", field.field_name)),
        (ColumnType::Number, false) => Ok(format!(
            "match {receiver}.{} {{ Some(value) => value, None => continue }}",
            field.field_name
        )),
        (ColumnType::String, true) => Ok(format!(
            "match {receiver}.{}.trim().parse::<f32>() {{ Ok(value) if value.is_finite() => value, _ => continue }}",
            field.field_name
        )),
        (ColumnType::String, false) => Ok(format!(
            "match {receiver}.{}.as_deref().and_then(|value| value.trim().parse::<f32>().ok()).filter(|value| value.is_finite()) {{ Some(value) => value, None => continue }}",
            field.field_name
        )),
        _ => bail!("column `{}` is not numeric", field.source_name),
    }
}

fn numeric_value_or_default_expression(
    field: &RustStandaloneSchemaField,
    receiver: &str,
    default: &str,
) -> Result<String> {
    match (field.column_type, field.required) {
        (ColumnType::Number, true) => Ok(format!("{receiver}.{}", field.field_name)),
        (ColumnType::Number, false) => Ok(format!(
            "{receiver}.{}.unwrap_or({default})",
            field.field_name
        )),
        (ColumnType::String, true) => Ok(format!(
            "{receiver}.{}.trim().parse::<f32>().ok().filter(|value| value.is_finite()).unwrap_or({default})",
            field.field_name
        )),
        (ColumnType::String, false) => Ok(format!(
            "{receiver}.{}.as_deref().and_then(|value| value.trim().parse::<f32>().ok()).filter(|value| value.is_finite()).unwrap_or({default})",
            field.field_name
        )),
        _ => bail!("column `{}` is not numeric", field.source_name),
    }
}

fn boolean_value_expression(field: &RustStandaloneSchemaField, receiver: &str) -> Result<String> {
    match (field.column_type, field.required) {
        (ColumnType::Boolean, true) => Ok(format!("{receiver}.{}", field.field_name)),
        (ColumnType::Boolean, false) => {
            Ok(format!("{receiver}.{}.unwrap_or(false)", field.field_name))
        }
        _ => bail!("column `{}` is not boolean", field.source_name),
    }
}

fn duplicate_insert(
    policy: NativeDuplicateKeyPolicy,
    index: &str,
    key: &str,
    value: &str,
    row_type: &str,
) -> String {
    match policy {
        NativeDuplicateKeyPolicy::FirstWins => {
            format!("            {index}.entry({key}).or_insert_with(|| {value});\n")
        }
        NativeDuplicateKeyPolicy::Overwrite => {
            format!("            {index}.insert({key}, {value});\n")
        }
        NativeDuplicateKeyPolicy::Error => format!(
            "            if {index}.insert({key}, {value}).is_some() {{\n                bail!(\"duplicate {row_type} semantic key\");\n            }}\n"
        ),
    }
}

fn crc_lookup_method(method: &CrcMethodSpec<'_>, index: &str, row: &RowParts<'_>) -> String {
    let (parameter_type, key) = match method.kind {
        NativeCrcIndexLookupParameterKind::StrRef => (
            "&str",
            format!("Crc32::from_str_lower({})", method.parameter),
        ),
        NativeCrcIndexLookupParameterKind::AsRefStr => (
            "impl AsRef<str>",
            format!("Crc32::from_str_lower({}.as_ref())", method.parameter),
        ),
        NativeCrcIndexLookupParameterKind::Crc32 => ("Crc32", method.parameter.to_owned()),
        NativeCrcIndexLookupParameterKind::IntoCrc32 => (
            "impl IntoCrc32Key",
            format!("{}.into_crc32_key()", method.parameter),
        ),
    };
    format!(
        "    pub fn {name}(&self, {parameter}: {parameter_type}) -> Option<&{row_type}> {{\n        let reference = self.{index}.get(&{key})?;\n        self.{row_field}.get(reference)\n    }}\n\n",
        name = method.name,
        parameter = method.parameter,
        row_type = row.row_type,
        row_field = row.row_field,
    )
}

fn numeric_key_type(key_type: NativeNumericKeyType) -> &'static str {
    match key_type {
        NativeNumericKeyType::NonZeroU32 => "std::num::NonZeroU32",
        NativeNumericKeyType::NonZeroU8 => "std::num::NonZeroU8",
        NativeNumericKeyType::U16 | NativeNumericKeyType::U16FromNonZeroU32 => "u16",
    }
}

fn numeric_conversion(
    key_type: NativeNumericKeyType,
    raw: &str,
    row_type: &str,
    column: &str,
) -> String {
    let validation = format!(
        "            let raw = {raw};\n            if !raw.is_finite() || raw.fract() != 0.0 || raw < 0.0 {{\n                bail!(\"{row_type}.{column} contains non-integral numeric key {{raw}}\");\n            }}\n"
    );
    let conversion = match key_type {
        NativeNumericKeyType::NonZeroU32 => format!(
            "            let key = u32::try_from(raw as u64).ok().and_then(std::num::NonZeroU32::new).ok_or_else(|| anyhow::anyhow!(\"{row_type}.{column} key {{raw}} is not a non-zero u32\"))?;\n"
        ),
        NativeNumericKeyType::NonZeroU8 => format!(
            "            let key = u8::try_from(raw as u64).ok().and_then(std::num::NonZeroU8::new).ok_or_else(|| anyhow::anyhow!(\"{row_type}.{column} key {{raw}} is not a non-zero u8\"))?;\n"
        ),
        NativeNumericKeyType::U16 => format!(
            "            let key = u16::try_from(raw as u64).map_err(|_| anyhow::anyhow!(\"{row_type}.{column} key {{raw}} exceeds u16\"))?;\n"
        ),
        NativeNumericKeyType::U16FromNonZeroU32 => format!(
            "            let non_zero = u32::try_from(raw as u64).ok().and_then(std::num::NonZeroU32::new).ok_or_else(|| anyhow::anyhow!(\"{row_type}.{column} key {{raw}} is not a non-zero u32\"))?;\n            let key = u16::try_from(non_zero.get()).map_err(|_| anyhow::anyhow!(\"{row_type}.{column} key {{raw}} exceeds u16\"))?;\n"
        ),
    };
    format!("{validation}{conversion}")
}

fn numeric_lookup_method(
    method: &NumericMethodSpec<'_>,
    key_type: NativeNumericKeyType,
    index: &str,
    row: &RowParts<'_>,
) -> Result<String> {
    let (parameter_type, key) = match (key_type, method.kind) {
        (NativeNumericKeyType::NonZeroU32, NativeNumericLookupParameterKind::NonZeroU32) => {
            ("std::num::NonZeroU32", method.parameter.to_owned())
        }
        (NativeNumericKeyType::NonZeroU8, NativeNumericLookupParameterKind::NonZeroU8) => {
            ("std::num::NonZeroU8", method.parameter.to_owned())
        }
        (NativeNumericKeyType::U16, NativeNumericLookupParameterKind::U16) => {
            ("u16", method.parameter.to_owned())
        }
        (NativeNumericKeyType::U16FromNonZeroU32, NativeNumericLookupParameterKind::NonZeroU32) => {
            (
                "std::num::NonZeroU32",
                format!("u16::try_from({}.get()).ok()?", method.parameter),
            )
        }
        (storage, parameter) => bail!(
            "standalone Rust numeric index cannot use `{parameter:?}` lookup with `{storage:?}` storage"
        ),
    };
    Ok(format!(
        "    pub fn {name}(&self, {parameter}: {parameter_type}) -> Option<&{row_type}> {{\n        let reference = self.{index}.get(&{key})?;\n        self.{row_field}.get(reference)\n    }}\n\n",
        name = method.name,
        parameter = method.parameter,
        row_type = row.row_type,
        row_field = row.row_field,
    ))
}

fn enum_lookup_method(
    method: &NativeEnumLookupMethod,
    index: &str,
    row: &RowParts<'_>,
    key_type: &str,
) -> String {
    let name = method.name().as_str();
    let parameter = method.parameter().as_str();
    let (parameter_type, key) = match method.parameter_kind() {
        NativeEnumLookupParameterKind::Enum => (key_type, parameter.to_owned()),
        NativeEnumLookupParameterKind::AsRefStr => (
            "impl AsRef<str>",
            format!("{key_type}::try_from({parameter}.as_ref()).ok()?"),
        ),
    };
    format!(
        "    pub fn {name}(&self, {parameter}: {parameter_type}) -> Option<&{row_type}> {{\n        let reference = self.{index}.get(&{key})?;\n        self.{row_field}.get(reference)\n    }}\n\n",
        row_type = row.row_type,
        row_field = row.row_field,
    )
}

fn string_lookup_method(
    method: &NativeStringLookupMethod,
    index: &str,
    row: &RowParts<'_>,
) -> String {
    let name = method.name().as_str();
    let parameter = method.parameter().as_str();
    let (parameter_type, key) = match method.parameter_kind() {
        NativeStringLookupParameterKind::StrRef => ("&str", parameter.to_owned()),
        NativeStringLookupParameterKind::AsRefStr => {
            ("impl AsRef<str>", format!("{parameter}.as_ref()"))
        }
    };
    format!(
        "    pub fn {name}(&self, {parameter}: {parameter_type}) -> Option<&{row_type}> {{\n        let reference = self.{index}.get({key})?;\n        self.{row_field}.get(reference)\n    }}\n\n",
        row_type = row.row_type,
        row_field = row.row_field,
    )
}

fn named_rows_method(name: &str, row: &RowParts<'_>) -> String {
    format!(
        "    pub fn {name}(&self) -> impl ExactSizeIterator<Item = &{row_type}> + '_ {{\n        self.{row_field}.rows().map(|source| &source.row)\n    }}\n\n",
        row_type = row.row_type,
        row_field = row.row_field,
    )
}

fn rust_fragment_ident(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut previous_separator = false;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            output.push(character.to_ascii_lowercase());
            previous_separator = false;
        } else if !previous_separator && !output.is_empty() {
            output.push('_');
            previous_separator = true;
        }
    }
    while output.ends_with('_') {
        output.pop();
    }
    if output.is_empty() {
        "index".to_owned()
    } else {
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merging_augmentations_preserves_splice_order() {
        let merged = RustNativeManagerAugmentation::merged([
            RustNativeManagerAugmentation {
                fields: "first-field\n".to_owned(),
                methods: "first-method\n".to_owned(),
                ..RustNativeManagerAugmentation::default()
            },
            RustNativeManagerAugmentation {
                fields: "second-field\n".to_owned(),
                methods: "second-method\n".to_owned(),
                ..RustNativeManagerAugmentation::default()
            },
        ]);

        assert_eq!(merged.fields, "first-field\nsecond-field\n");
        assert_eq!(merged.methods, "first-method\nsecond-method\n");
    }

    #[test]
    fn generated_lookup_fragments_are_standalone() {
        let row = RowParts {
            row_type: "ExampleDataSchemaRow",
            row_field: "example_data_rows".to_owned(),
            table_type: "ExampleDataTable".to_owned(),
        };
        let method = crc_lookup_method(
            &CrcMethodSpec::str_ref("example_data"),
            "example_by_crc",
            &row,
        );

        assert!(method.contains("example_by_crc"));
        assert!(!method.contains("native"));
        assert!(method.contains("ExampleDataSchemaRow"));
    }

    #[test]
    fn fragment_ident_is_stable_and_ascii() {
        assert_eq!(
            rust_fragment_ident("Multi Table/CRC-0"),
            "multi_table_crc_0"
        );
        assert_eq!(rust_fragment_ident("---"), "index");
    }
}
