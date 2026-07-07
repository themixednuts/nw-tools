use crate::go::source::{GoSourceEmitError, format_go_source};

const CATALOG_GO_SOURCE: &str =
    include_str!("../../../../resources/go/standalone/game-assets/catalog.go");
const DATASHEET_GO_SOURCE: &str =
    include_str!("../../../../resources/go/standalone/game-assets/datasheet.go");
const FILESYSTEM_GO_SOURCE: &str =
    include_str!("../../../../resources/go/standalone/game-assets/filesystem.go");
const LOCALIZATION_GO_SOURCE: &str =
    include_str!("../../../../resources/go/standalone/game-assets/localization.go");
const OBJECTSTREAM_GO_SOURCE: &str =
    include_str!("../../../../resources/go/standalone/game-assets/objectstream.go");
const OODLE_UNSUPPORTED_GO_SOURCE: &str =
    include_str!("../../../../resources/go/standalone/game-assets/oodle_unsupported.go");
const OODLE_WINDOWS_GO_SOURCE: &str =
    include_str!("../../../../resources/go/standalone/game-assets/oodle_windows.go");
const PAK_GO_SOURCE: &str = include_str!("../../../../resources/go/standalone/game-assets/pak.go");

pub(super) fn catalog_go_source() -> Result<String, GoSourceEmitError> {
    format_go_source(CATALOG_GO_SOURCE)
}

pub(super) fn datasheet_go_source() -> Result<String, GoSourceEmitError> {
    format_go_source(DATASHEET_GO_SOURCE)
}

pub(super) fn filesystem_go_source() -> Result<String, GoSourceEmitError> {
    format_go_source(FILESYSTEM_GO_SOURCE)
}

pub(super) fn localization_go_source() -> Result<String, GoSourceEmitError> {
    format_go_source(LOCALIZATION_GO_SOURCE)
}

pub(super) fn object_stream_go_source() -> Result<String, GoSourceEmitError> {
    format_go_source(OBJECTSTREAM_GO_SOURCE)
}

pub(super) fn oodle_unsupported_go_source() -> Result<String, GoSourceEmitError> {
    format_go_source(OODLE_UNSUPPORTED_GO_SOURCE)
}

pub(super) fn oodle_windows_go_source() -> Result<String, GoSourceEmitError> {
    Ok(OODLE_WINDOWS_GO_SOURCE.to_owned())
}

pub(super) fn pak_go_source() -> Result<String, GoSourceEmitError> {
    format_go_source(PAK_GO_SOURCE)
}
