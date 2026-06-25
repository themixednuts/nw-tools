use anyhow::{Result, bail};

use std::path::Path;

/// Output encoding for ObjectStream conversions (shared between the CLI surface
/// and the conversion path).
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub(super) enum EncodingArg {
    Binary,
    Xml,
    Json,
}

impl From<EncodingArg> for nw_objectstream::ObjectStreamEncoding {
    fn from(value: EncodingArg) -> Self {
        match value {
            EncodingArg::Binary => Self::Binary,
            EncodingArg::Xml => Self::Xml,
            EncodingArg::Json => Self::Json,
        }
    }
}

pub(super) fn path_label(path: &Path) -> String {
    path.display().to_string()
}

pub(super) fn lowered(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .map(|value| value.to_ascii_lowercase())
        .collect()
}

pub(super) fn text_matches(value: &str, needles: &[String]) -> bool {
    let value = value.to_ascii_lowercase();
    needles.iter().any(|needle| value.contains(needle))
}

pub(super) fn csv_cell(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') || value.contains('\r') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

pub(super) fn trim_cell(value: impl AsRef<str>) -> String {
    const MAX: usize = 160;
    let value = value.as_ref().replace(['\r', '\n', '\t'], " ");
    if value.chars().count() <= MAX {
        value
    } else {
        format!("{}...", value.chars().take(MAX).collect::<String>())
    }
}

pub(super) fn strip_suffix_ignore_ascii_case<'a>(value: &'a str, suffix: &str) -> Option<&'a str> {
    let split = value.len().checked_sub(suffix.len())?;
    value
        .get(split..)
        .is_some_and(|tail| tail.eq_ignore_ascii_case(suffix))
        .then(|| &value[..split])
}

pub(super) fn finish_scan(
    cancelled: bool,
    skipped: usize,
    errors: &[anyhow::Error],
    label: &str,
) -> Result<()> {
    if cancelled {
        bail!("{label} scan cancelled ({skipped} queued file(s) skipped)");
    }
    if !errors.is_empty() {
        for error in errors.iter().take(12) {
            eprintln!("{error:#}");
        }
        bail!("{} {label} file(s) failed", errors.len());
    }
    Ok(())
}
