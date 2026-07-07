use std::path::Path;

use crate::game_system_schema::{
    GameSystemColumnSchema, GameSystemColumnValueShape, GameSystemListElementShape,
    GameSystemListShape, GameSystemNumberShape, GameSystemRangeBounds, GameSystemTableSchema,
};
use anyhow::{Context, Result};
use nw_datasheet::game_system::{GameSystemTable, OwnedCellValue};
use serde::Serialize;

use super::model::RustField;
use super::native_value::{NativeDevCellValue, NativeDevScalarValue, render_native_dev_cell_value};

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
enum RonTransformAuditSeverity {
    Info,
    Warn,
}

#[derive(Debug, Clone, Serialize)]
pub struct RonTransformAuditEntry {
    severity: RonTransformAuditSeverity,
    table_name: String,
    row_type_name: String,
    source_path: Option<String>,
    ron_path: String,
    row_index: usize,
    row_number: usize,
    row_key: Option<String>,
    column_name: String,
    field_name: String,
    source_value: String,
    emitted_value: Option<String>,
    reason: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct RonTransformAuditReport {
    info_count: usize,
    warnings: Vec<RonTransformAuditEntry>,
}

#[derive(Debug, Clone)]
pub(super) enum RonTransformAudit {
    Info,
    Warning(RonTransformAuditEntry),
}

impl RonTransformAuditReport {
    pub(super) fn record(&mut self, audit: RonTransformAudit) {
        match audit {
            RonTransformAudit::Info => self.info_count += 1,
            RonTransformAudit::Warning(entry) => self.warnings.push(entry),
        }
    }

    pub(super) fn extend(&mut self, other: Self) {
        self.info_count += other.info_count;
        self.warnings.extend(other.warnings);
    }

    pub(super) fn total_count(&self) -> usize {
        self.info_count + self.warnings.len()
    }

    fn is_empty(&self) -> bool {
        self.info_count == 0 && self.warnings.is_empty()
    }

    fn warnings(&self) -> &[RonTransformAuditEntry] {
        &self.warnings
    }
}

pub(super) fn report_ron_transform_audits(path: &Path, audits: &RonTransformAuditReport) {
    if audits.is_empty() {
        return;
    }

    if audits.info_count != 0 {
        eprintln!(
            "info: recorded {} intentional RON transforms in {}",
            audits.info_count,
            path.display()
        );
    }

    for audit in audits.warnings() {
        let row = audit.row_key.as_deref().map_or_else(
            || audit.row_number.to_string(),
            |key| format!("{} ({key})", audit.row_number),
        );
        eprintln!(
            "warning: RON transform needs review: {} row {} column {} -> {}: {}",
            audit.table_name, row, audit.column_name, audit.field_name, audit.reason
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn ron_transform_audit(
    table: &GameSystemTable,
    schema: &GameSystemTableSchema,
    ron_path: &str,
    source_path: Option<&str>,
    row_index: usize,
    row_key: Option<&str>,
    column: &GameSystemColumnSchema,
    field: &RustField,
    source: &OwnedCellValue,
    emitted: &NativeDevCellValue,
) -> Result<Option<RonTransformAudit>> {
    let Some((severity, reason)) = ron_transform_reason(column, source, emitted) else {
        return Ok(None);
    };

    if matches!(severity, RonTransformAuditSeverity::Info) {
        return Ok(Some(RonTransformAudit::Info));
    }

    let source_value = render_source_cell_value(source)?;
    let emitted_value = render_native_dev_cell_value(emitted)?;

    Ok(Some(RonTransformAudit::Warning(RonTransformAuditEntry {
        severity,
        table_name: table.name().to_owned(),
        row_type_name: schema.row_type_name.clone(),
        source_path: source_path.map(ToOwned::to_owned),
        ron_path: ron_path.to_owned(),
        row_index,
        row_number: row_index + 1,
        row_key: row_key.map(ToOwned::to_owned),
        column_name: column.name.clone(),
        field_name: field.rust_name.clone(),
        source_value,
        emitted_value: Some(emitted_value),
        reason,
    })))
}

fn render_source_cell_value(value: &OwnedCellValue) -> Result<String> {
    match value {
        OwnedCellValue::String(value) => ron::ser::to_string(value)
            .with_context(|| format!("serialize source RON string value {value:?}")),
        OwnedCellValue::Number(value) => {
            ron::ser::to_string(value).context("serialize source RON number value")
        }
        OwnedCellValue::Boolean(value) => {
            ron::ser::to_string(value).context("serialize source RON boolean value")
        }
    }
}

fn ron_transform_reason(
    column: &GameSystemColumnSchema,
    source: &OwnedCellValue,
    emitted: &NativeDevCellValue,
) -> Option<(RonTransformAuditSeverity, String)> {
    match (&column.value_shape, source, emitted) {
        (
            GameSystemColumnValueShape::Boolean,
            OwnedCellValue::String(value),
            NativeDevCellValue::Scalar(NativeDevScalarValue::Boolean(_)),
        ) => Some(boolean_text_audit(value)),
        (
            GameSystemColumnValueShape::Crc32,
            OwnedCellValue::String(_),
            NativeDevCellValue::Scalar(NativeDevScalarValue::String(_)),
        ) => Some((
            RonTransformAuditSeverity::Info,
            "designer CRC32 token is kept as text; the asset compiler hashes it into az_core::crc::Crc32".to_owned(),
        )),
        (
            GameSystemColumnValueShape::Color { .. },
            OwnedCellValue::String(_),
            NativeDevCellValue::Scalar(NativeDevScalarValue::String(_)),
        ) => Some((
            RonTransformAuditSeverity::Info,
            "designer color token is kept as text; the asset compiler decodes it into bevy_color::LinearRgba".to_owned(),
        )),
        (
            GameSystemColumnValueShape::Enum { enum_shape },
            OwnedCellValue::String(_),
            NativeDevCellValue::Scalar(NativeDevScalarValue::String(_)),
        ) => Some((
            RonTransformAuditSeverity::Info,
            format!(
                "designer enum token is kept as text; the asset compiler maps it to {}",
                enum_shape.name
            ),
        )),
        (
            GameSystemColumnValueShape::Number { number_shape },
            OwnedCellValue::String(_),
            _,
        ) => Some((
            RonTransformAuditSeverity::Info,
            format!(
                "native text cell is emitted as {:?} because schema affinity resolved the column as numeric",
                number_shape
            ),
        )),
        (
            GameSystemColumnValueShape::Number { number_shape },
            OwnedCellValue::Number(_),
            _,
        ) if !matches!(number_shape, GameSystemNumberShape::Float) => Some((
            RonTransformAuditSeverity::Info,
            format!("source number is emitted as narrowed {:?} runtime RON", number_shape),
        )),
        (
            GameSystemColumnValueShape::Range {
                bounds,
                number_shape,
            },
            OwnedCellValue::String(value),
            _,
        ) => Some(range_text_audit(*bounds, *number_shape, value)),
        (
            GameSystemColumnValueShape::Range {
                bounds,
                number_shape,
            },
            OwnedCellValue::Number(_),
            _,
        ) => Some((
            RonTransformAuditSeverity::Info,
            format!(
                "source number is emitted as {:?} {:?} range RON",
                bounds, number_shape
            ),
        )),
        (
            GameSystemColumnValueShape::String {
                list: Some(list), ..
            },
            OwnedCellValue::String(value),
            NativeDevCellValue::List(values),
        ) if matches!(
            list.element_shape.as_ref(),
            Some(GameSystemListElementShape::Pair { .. })
        ) && values.is_empty()
            && authored_numeric_scalar(value) =>
        {
            Some((
                RonTransformAuditSeverity::Warn,
                "native text pair list contains a numeric scalar without a key; emitted RON is empty and source data should be corrected".to_owned(),
            ))
        }
        (
            GameSystemColumnValueShape::String {
                list: Some(list), ..
            },
            OwnedCellValue::String(value),
            NativeDevCellValue::List(_),
        ) => Some(list_text_audit(list, value)),
        _ => None,
    }
}

fn authored_numeric_scalar(value: &str) -> bool {
    value
        .trim()
        .parse::<f32>()
        .is_ok_and(|value| value.is_finite())
}

fn boolean_text_audit(value: &str) -> (RonTransformAuditSeverity, String) {
    match value.trim().to_ascii_lowercase().as_str() {
        "" | "true" | "1" | "yes" | "false" | "0" | "no" => (
            RonTransformAuditSeverity::Info,
            "native text bool is emitted as bool RON".to_owned(),
        ),
        _ => (
            RonTransformAuditSeverity::Warn,
            "native bool parser treats this unrecognized non-empty text as false; source data should be corrected or the column affinity should be repaired".to_owned(),
        ),
    }
}

fn range_text_audit(
    bounds: GameSystemRangeBounds,
    number_shape: GameSystemNumberShape,
    value: &str,
) -> (RonTransformAuditSeverity, String) {
    if matches!(number_shape, GameSystemNumberShape::Float) {
        let parts = value.trim().split('-').map(str::trim).collect::<Vec<_>>();
        let warning = match parts.as_slice() {
            [] | [""] => Some("empty f32 range text becomes 0.0"),
            [single] if single.parse::<f32>().is_ok_and(|value| value.is_finite()) => None,
            [single] if single.parse::<f32>().is_err() => {
                Some("invalid f32 range text becomes 0.0")
            }
            [start, last] => match (start.parse::<f32>(), last.parse::<f32>()) {
                (Ok(start), Ok(last)) if start.is_finite() && last.is_finite() && start > last => {
                    Some("reversed f32 range endpoints are normalized")
                }
                (Ok(start), Ok(last)) if start.is_finite() && last.is_finite() => None,
                _ => Some("invalid f32 range text becomes 0.0"),
            },
            _ => Some("invalid f32 range text becomes 0.0"),
        };
        if let Some(warning) = warning {
            return (
                RonTransformAuditSeverity::Warn,
                format!("{warning}; source data should be corrected"),
            );
        }
    }

    (
        RonTransformAuditSeverity::Info,
        format!(
            "native text cell is emitted as {:?} {:?} range RON",
            bounds, number_shape
        ),
    )
}

fn list_text_audit(
    list: &GameSystemListShape,
    _value: &str,
) -> (RonTransformAuditSeverity, String) {
    let reason = match list.element_shape.as_ref() {
        Some(GameSystemListElementShape::Crc32) => {
            "native text list is split into designer CRC32 tokens; the asset compiler hashes each entry"
        }
        Some(GameSystemListElementShape::Color { .. }) => {
            "native text list is split into designer color tokens; the asset compiler decodes each entry"
        }
        Some(GameSystemListElementShape::Enum { .. }) => {
            "native text list is split into designer enum tokens; the asset compiler maps each entry"
        }
        Some(GameSystemListElementShape::Number { .. }) => {
            "native text list is split and emitted as numeric RON entries"
        }
        Some(GameSystemListElementShape::Boolean) => {
            "native text list is split and emitted as bool RON entries"
        }
        Some(GameSystemListElementShape::Range { .. }) => {
            "native text list is split and emitted as range RON entries"
        }
        Some(GameSystemListElementShape::Pair { .. }) => {
            "native text list is split and emitted as typed pair RON entries"
        }
        Some(GameSystemListElementShape::String) | None => {
            "native text list is split and emitted as string RON entries"
        }
    };
    (RonTransformAuditSeverity::Info, reason.to_owned())
}
