use crate::game_system_schema::{GameSystemColumnSchema, GameSystemEnumShape};
use anyhow::{Context, Result, bail};
pub(super) fn enum_discriminant_for_source_token(
    column: &GameSystemColumnSchema,
    enum_shape: &GameSystemEnumShape,
    value: &str,
) -> Result<i64> {
    let value = value.trim();
    enum_shape
        .variants
        .iter()
        .find(|variant| {
            variant.name.eq_ignore_ascii_case(value)
                || variant
                    .source_tokens
                    .iter()
                    .any(|token| token.eq_ignore_ascii_case(value))
        })
        .map(|variant| variant.discriminant)
        .with_context(|| {
            format!(
                "column {} expected {} enum token, found {value}",
                column.name, enum_shape.name
            )
        })
}

pub(super) fn native_dev_string_bool_cell_value(
    column: &GameSystemColumnSchema,
    value: &str,
) -> Result<Option<bool>> {
    let value = value.trim();
    if value.is_empty() && !column.required {
        return Ok(None);
    }
    match value.to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" => Ok(Some(true)),
        "false" | "0" | "no" => Ok(Some(false)),
        _ => bail!(
            "column {} expected boolean text, found {value}",
            column.name
        ),
    }
}

pub(super) fn is_empty_authored_string(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.is_empty()
        || trimmed.eq_ignore_ascii_case("none")
        || trimmed.eq_ignore_ascii_case("null")
}
