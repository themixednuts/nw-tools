//! New World Wwise mapping CSV formats.
//!
//! These path-qualified schemas are legacy extraction inputs. They live with
//! the other CryAudio source formats and do not leak into a runtime crate.

use std::{num::ParseIntError, str};

use csv::{ByteRecord, ReaderBuilder, Trim};
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AudioMappingKind {
    EventIds,
    TractSoundBanks,
    Tags,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "document", rename_all = "camelCase")]
pub enum AudioMappingDocument {
    EventIds(AudioEventIds),
    TractSoundBanks(AudioTractSoundBankMap),
    Tags(AudioTagTable),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioEventIds {
    pub source_path: String,
    pub events: Vec<AudioEventIdEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioEventIdEntry {
    pub name: String,
    pub id: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioTractSoundBankMap {
    pub source_path: String,
    pub entries: Vec<AudioTractSoundBankEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioTractSoundBankEntry {
    pub tract: String,
    pub sound_bank: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioTagTable {
    pub source_path: String,
    pub entries: Vec<AudioTagEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioTagEntry {
    pub valid_tag: String,
    pub combat_music_start: String,
    pub combat_music_stop: String,
}

#[must_use]
pub fn audio_mapping_kind(source_path: &str) -> Option<AudioMappingKind> {
    let path = normalize_path(source_path);
    if path.ends_with("sounds/wwise/soundbank_tract_mapping.csv") {
        Some(AudioMappingKind::TractSoundBanks)
    } else if path.starts_with("sounds/wwise/") && path.ends_with("_events.csv") {
        Some(AudioMappingKind::EventIds)
    } else if path.ends_with("libs/gameaudio/wwise/audio_tag_data.csv") {
        Some(AudioMappingKind::Tags)
    } else {
        None
    }
}

#[must_use]
pub fn is_audio_mapping_source(source_path: &str) -> bool {
    audio_mapping_kind(source_path).is_some()
}

pub fn parse_audio_mapping(
    source_path: &str,
    bytes: &[u8],
) -> Result<AudioMappingDocument, AudioMappingParseError> {
    let source_path = normalize_path(source_path);
    let kind = audio_mapping_kind(&source_path).ok_or_else(|| {
        AudioMappingParseError::UnsupportedPath {
            path: source_path.clone(),
        }
    })?;
    match kind {
        AudioMappingKind::EventIds => {
            let mut events = Vec::new();
            visit_records(bytes, &["Name", "Id"], |row, record| {
                events.push(AudioEventIdEntry {
                    name: field_str(record, row, 0, "Name")?.to_owned(),
                    id: field_u32(record, row, 1, "Id")?,
                });
                Ok(())
            })?;
            Ok(AudioMappingDocument::EventIds(AudioEventIds {
                source_path,
                events,
            }))
        }
        AudioMappingKind::TractSoundBanks => {
            let mut entries = Vec::new();
            visit_records(bytes, &["tract", "soundbank"], |row, record| {
                entries.push(AudioTractSoundBankEntry {
                    tract: field_str(record, row, 0, "tract")?.to_owned(),
                    sound_bank: field_str(record, row, 1, "soundbank")?.to_owned(),
                });
                Ok(())
            })?;
            Ok(AudioMappingDocument::TractSoundBanks(
                AudioTractSoundBankMap {
                    source_path,
                    entries,
                },
            ))
        }
        AudioMappingKind::Tags => {
            let mut entries = Vec::new();
            visit_records(
                bytes,
                &["ValidTag", "CombatMusicStart", "CombatMusicStop"],
                |row, record| {
                    entries.push(AudioTagEntry {
                        valid_tag: field_str(record, row, 0, "ValidTag")?.to_owned(),
                        combat_music_start: field_str(record, row, 1, "CombatMusicStart")?
                            .to_owned(),
                        combat_music_stop: field_str(record, row, 2, "CombatMusicStop")?.to_owned(),
                    });
                    Ok(())
                },
            )?;
            Ok(AudioMappingDocument::Tags(AudioTagTable {
                source_path,
                entries,
            }))
        }
    }
}

fn visit_records(
    bytes: &[u8],
    expected_header: &'static [&'static str],
    mut visit: impl FnMut(usize, &ByteRecord) -> Result<(), AudioMappingParseError>,
) -> Result<(), AudioMappingParseError> {
    let mut reader = ReaderBuilder::new()
        .has_headers(false)
        .flexible(false)
        .trim(Trim::None)
        .from_reader(bytes);
    let mut record = ByteRecord::new();
    if !reader.read_byte_record(&mut record)? {
        return Err(AudioMappingParseError::MissingHeader);
    }
    validate_header(&record, expected_header)?;
    let mut row = 1;
    while reader.read_byte_record(&mut record)? {
        row += 1;
        visit(row, &record)?;
    }
    Ok(())
}

fn validate_header(
    record: &ByteRecord,
    expected: &'static [&'static str],
) -> Result<(), AudioMappingParseError> {
    let found = record
        .iter()
        .map(trim_utf8)
        .collect::<Result<Vec<_>, _>>()?;
    if found.as_slice() == expected {
        Ok(())
    } else {
        Err(AudioMappingParseError::UnexpectedHeader {
            expected,
            found: found.into_iter().map(ToOwned::to_owned).collect(),
        })
    }
}

fn field_str<'a>(
    record: &'a ByteRecord,
    row: usize,
    index: usize,
    column: &'static str,
) -> Result<&'a str, AudioMappingParseError> {
    trim_utf8(
        record
            .get(index)
            .ok_or(AudioMappingParseError::MissingField { row, column })?,
    )
}

fn field_u32(
    record: &ByteRecord,
    row: usize,
    index: usize,
    column: &'static str,
) -> Result<u32, AudioMappingParseError> {
    let value = field_str(record, row, index, column)?;
    value
        .parse()
        .map_err(|source| AudioMappingParseError::InvalidInteger {
            row,
            column,
            value: value.to_owned(),
            source,
        })
}

fn trim_utf8(bytes: &[u8]) -> Result<&str, AudioMappingParseError> {
    Ok(str::from_utf8(bytes)?.trim())
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
        .trim_start_matches('/')
        .to_ascii_lowercase()
}

#[derive(Debug, Error)]
pub enum AudioMappingParseError {
    #[error("unsupported audio mapping CSV path {path}")]
    UnsupportedPath { path: String },
    #[error("audio mapping CSV is not UTF-8")]
    InvalidUtf8(#[from] str::Utf8Error),
    #[error("CSV parser error: {0}")]
    Csv(#[from] csv::Error),
    #[error("audio mapping CSV is missing a header row")]
    MissingHeader,
    #[error("unexpected audio mapping header {found:?}; expected {expected:?}")]
    UnexpectedHeader {
        expected: &'static [&'static str],
        found: Vec<String>,
    },
    #[error("row {row} is missing {column}")]
    MissingField { row: usize, column: &'static str },
    #[error("row {row} has invalid integer in {column}: {value:?}")]
    InvalidInteger {
        row: usize,
        column: &'static str,
        value: String,
        source: ParseIntError,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_three_shipped_mapping_schemas() {
        let events = parse_audio_mapping(
            "Sounds/Wwise/npc_alligator_events.csv",
            b"Name,Id\nPlay_A,1\nPlay_B,2\n",
        )
        .unwrap();
        assert!(matches!(
            events,
            AudioMappingDocument::EventIds(AudioEventIds { ref events, .. })
                if events[1].id == 2
        ));

        let tracts = parse_audio_mapping(
            "sounds/wwise/soundbank_tract_mapping.csv",
            b"tract,soundbank\ncoast,Amb_Wildlife_Coast_\n",
        )
        .unwrap();
        assert!(matches!(
            tracts,
            AudioMappingDocument::TractSoundBanks(AudioTractSoundBankMap { ref entries, .. })
                if entries[0].tract == "coast"
        ));

        let tags = parse_audio_mapping(
            "libs/gameaudio/wwise/audio_tag_data.csv",
            b"ValidTag,CombatMusicStart,CombatMusicStop\nAlligator,Start,Stop\n",
        )
        .unwrap();
        assert!(matches!(
            tags,
            AudioMappingDocument::Tags(AudioTagTable { ref entries, .. })
                if entries[0].valid_tag == "Alligator"
        ));
    }

    #[test]
    fn only_claims_path_qualified_audio_csvs() {
        assert!(is_audio_mapping_source(
            "sounds/wwise/npc_alligator_events.csv"
        ));
        assert!(!is_audio_mapping_source("dictionary/en-us/profanity.csv"));
    }
}
