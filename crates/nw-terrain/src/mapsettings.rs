//! Parser for terrain region `mapsettings.json` files.

use std::fmt;

use serde::Deserialize;

/// Per-region terrain settings.
///
/// Source path:
/// `sharedassets/coatlicue/<world>/regions/<region>/mapsettings.json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct MapSettings {
    #[serde(rename = "cellResolution")]
    pub cell_resolution: u32,
    #[serde(rename = "regionSize")]
    pub region_size: u32,
    #[serde(rename = "regionType")]
    pub region_type: u32,
}

impl MapSettings {
    /// Parse `mapsettings.json` from bytes.
    pub fn parse_json(bytes: &[u8]) -> Result<Self, ParseError> {
        let settings: Self = serde_json::from_slice(bytes).map_err(ParseError::Json)?;
        settings.validate()?;
        Ok(settings)
    }

    /// Validate fields needed by terrain import.
    pub fn validate(self) -> Result<Self, ParseError> {
        if self.cell_resolution == 0 {
            return Err(ParseError::ZeroCellResolution);
        }
        if self.region_size == 0 {
            return Err(ParseError::ZeroRegionSize);
        }
        Ok(self)
    }
}

/// Parse error returned by [`MapSettings::parse_json`].
#[derive(Debug)]
pub enum ParseError {
    /// JSON decoding failed.
    Json(serde_json::Error),
    /// `cellResolution` was zero.
    ZeroCellResolution,
    /// `regionSize` was zero.
    ZeroRegionSize,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(err) => write!(f, "parse mapsettings JSON: {err}"),
            Self::ZeroCellResolution => write!(f, "mapsettings cellResolution must be non-zero"),
            Self::ZeroRegionSize => write!(f, "mapsettings regionSize must be non-zero"),
        }
    }
}

impl std::error::Error for ParseError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_mapsettings_document() {
        let settings =
            MapSettings::parse_json(br#"{"cellResolution":2,"regionSize":2048,"regionType":1}"#)
                .unwrap();

        assert_eq!(settings.cell_resolution, 2);
        assert_eq!(settings.region_size, 2048);
        assert_eq!(settings.region_type, 1);
    }

    #[test]
    fn rejects_zero_region_size() {
        let err = MapSettings::parse_json(br#"{"cellResolution":1,"regionSize":0,"regionType":0}"#)
            .unwrap_err();

        assert!(matches!(err, ParseError::ZeroRegionSize));
    }
}
