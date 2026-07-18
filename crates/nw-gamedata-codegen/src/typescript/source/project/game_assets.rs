use crate::typescript::source::{TypeScriptSourceEmitError, format_typescript_source};

const CATALOG_TS: &str =
    include_str!("../../../../resources/typescript/standalone/game-assets/catalog.ts");
const DATASHEET_TS: &str =
    include_str!("../../../../resources/typescript/standalone/game-assets/datasheet.ts");
const LOCALIZATION_TS: &str =
    include_str!("../../../../resources/typescript/standalone/game-assets/localization.ts");
const OBJECT_STREAM_TS: &str =
    include_str!("../../../../resources/typescript/standalone/game-assets/object-stream.ts");
const PAK_TS: &str = include_str!("../../../../resources/typescript/standalone/game-assets/pak.ts");
const VALUES_TS: &str = include_str!("../../../../resources/typescript/standalone/values.ts");

pub(super) fn catalog_ts_source() -> Result<String, TypeScriptSourceEmitError> {
    format_typescript_source(CATALOG_TS)
}

pub(super) fn datasheet_ts_source() -> Result<String, TypeScriptSourceEmitError> {
    format_typescript_source(DATASHEET_TS)
}

pub(super) fn localization_ts_source() -> Result<String, TypeScriptSourceEmitError> {
    format_typescript_source(LOCALIZATION_TS)
}

pub(super) fn object_stream_ts_source() -> Result<String, TypeScriptSourceEmitError> {
    format_typescript_source(OBJECT_STREAM_TS)
}

pub(super) fn pak_ts_source() -> Result<String, TypeScriptSourceEmitError> {
    format_typescript_source(PAK_TS)
}

pub(super) fn values_ts_source() -> Result<String, TypeScriptSourceEmitError> {
    format_typescript_source(VALUES_TS)
}
