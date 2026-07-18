//! Shared legacy-key normalization.
//!
//! Per ADR 0028 the same normalization is applied by pass-1 inventory and by
//! reference lookup: convert separators to `/`, collapse separators and `.`
//! segments, reject absolute/drive/UNC/parent/NUL/invalid components, and
//! lowercase. Original spelling survives only in the offline port report.
//!
//! Unicode NFC composition is a documented Phase-1 residual: the entire
//! checked-in New World legacy path space is ASCII (2026-07-16 inventory), for
//! which NFC is the identity. Non-ASCII inputs are lowercased case-foldingly but
//! not recomposed; that refinement lands with a dedicated NFC dependency.

use thiserror::Error;

/// A legacy key could not be normalized into a valid relative source path.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum NormalizeError {
    /// The key was empty after trimming and separator collapse.
    #[error("legacy key `{0}` normalizes to an empty path")]
    Empty(String),

    /// The key is absolute (leading separator) rather than relative.
    #[error("legacy key `{0}` is an absolute path")]
    Absolute(String),

    /// The key names a Windows drive or UNC prefix.
    #[error("legacy key `{0}` has a drive or UNC prefix")]
    DriveOrUnc(String),

    /// The key contains a `..` parent-directory component.
    #[error("legacy key `{0}` contains a `..` parent component")]
    ParentComponent(String),

    /// The key contains a NUL byte.
    #[error("legacy key `{0}` contains a NUL byte")]
    NulByte(String),

    /// A component ends in a reserved trailing dot or space.
    #[error("legacy key `{0}` has a component with a trailing dot or space")]
    TrailingDotOrSpace(String),
}

/// Normalize a legacy key into a lowercase, forward-slash, relative source path.
///
/// # Errors
///
/// Returns [`NormalizeError`] when the key is absolute, has a drive/UNC prefix,
/// contains a `..` parent component or a NUL byte, has a component with a
/// trailing dot or space, or collapses to an empty path.
pub fn normalize_legacy_path(raw: &str) -> Result<String, NormalizeError> {
    if raw.contains('\0') {
        return Err(NormalizeError::NulByte(raw.to_owned()));
    }

    let unified = raw.replace('\\', "/");

    // Reject Windows drive prefixes (`c:/...`) before splitting.
    if let Some((head, _)) = unified.split_once('/') {
        if head.len() == 2 && head.as_bytes()[1] == b':' {
            return Err(NormalizeError::DriveOrUnc(raw.to_owned()));
        }
    } else if unified.len() == 2 && unified.as_bytes()[1] == b':' {
        return Err(NormalizeError::DriveOrUnc(raw.to_owned()));
    }

    // A leading separator is absolute; a leading `//` is a UNC prefix.
    if unified.starts_with('/') {
        if unified.starts_with("//") {
            return Err(NormalizeError::DriveOrUnc(raw.to_owned()));
        }
        return Err(NormalizeError::Absolute(raw.to_owned()));
    }

    let mut components: Vec<String> = Vec::new();
    for component in unified.split('/') {
        let trimmed = component.trim();
        if trimmed.is_empty() || trimmed == "." {
            // Collapse empty segments and single-dot segments.
            continue;
        }
        if trimmed == ".." {
            return Err(NormalizeError::ParentComponent(raw.to_owned()));
        }
        if trimmed.ends_with('.') || trimmed.ends_with(' ') {
            return Err(NormalizeError::TrailingDotOrSpace(raw.to_owned()));
        }
        components.push(trimmed.to_lowercase());
    }

    if components.is_empty() {
        return Err(NormalizeError::Empty(raw.to_owned()));
    }

    Ok(components.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lowercases_and_unifies_separators() {
        assert_eq!(
            normalize_legacy_path(r"Textures\Items\Weapons\Sword_Diff.PNG").unwrap(),
            "textures/items/weapons/sword_diff.png"
        );
    }

    #[test]
    fn collapses_redundant_separators_and_dot_segments() {
        assert_eq!(
            normalize_legacy_path("materials//./decals/foo.material.ron").unwrap(),
            "materials/decals/foo.material.ron"
        );
    }

    #[test]
    fn rejects_absolute_drive_unc_and_parent() {
        assert!(matches!(
            normalize_legacy_path("/textures/x.png"),
            Err(NormalizeError::Absolute(_))
        ));
        assert!(matches!(
            normalize_legacy_path(r"C:\textures\x.png"),
            Err(NormalizeError::DriveOrUnc(_))
        ));
        assert!(matches!(
            normalize_legacy_path(r"\\host\share\x.png"),
            Err(NormalizeError::DriveOrUnc(_))
        ));
        assert!(matches!(
            normalize_legacy_path("textures/../secrets.png"),
            Err(NormalizeError::ParentComponent(_))
        ));
    }

    #[test]
    fn rejects_nul_and_trailing_dot_or_space() {
        assert!(matches!(
            normalize_legacy_path("textures/x\0.png"),
            Err(NormalizeError::NulByte(_))
        ));
        assert!(matches!(
            normalize_legacy_path("textures/trailing. /x.png"),
            Err(NormalizeError::TrailingDotOrSpace(_))
        ));
    }

    #[test]
    fn case_and_separator_variants_normalize_to_one_key() {
        let a = normalize_legacy_path("LyShineUI/Globals.ron").unwrap();
        let b = normalize_legacy_path(r"lyshineui\globals.ron").unwrap();
        assert_eq!(a, b);
    }
}
