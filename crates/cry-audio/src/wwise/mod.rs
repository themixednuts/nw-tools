//! Wwise soundbank, encoded-media, and ATL map formats.

use std::fmt;

use serde::Serialize;
use thiserror::Error;

pub mod hirc;
pub mod ranseq;

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
        #[repr(transparent)]
        pub struct $name(pub u32);
    };
}

id_type!(WwiseBankId);
id_type!(WwiseMediaId);
id_type!(WwiseObjectId);
id_type!(WwiseNameId);
id_type!(WwiseSectionId);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize)]
#[repr(transparent)]
pub struct AudioControlId(pub u64);

impl AudioControlId {
    /// The Lumberyard ATL control id for a name.
    ///
    /// `AudioStringToId` in Lumberyard's AudioSystem hashes control/preload names
    /// with `AZ::Crc32` (the reflected CRC-32/ISO-HDLC over the lowercased name).
    /// Verified against the shipped `triggerbankmapatlbin.bin`, whose fields are
    /// these ids (e.g. `blend_ftsp_alligator` → `2229255591`).
    #[must_use]
    pub const fn from_name(name: &str) -> Self {
        Self(az_crc32(name.as_bytes()) as u64)
    }
}

/// Reflected CRC-32/ISO-HDLC over the ASCII-lowercased bytes, matching
/// Lumberyard's `AZ::Crc32`. Computed bitwise so it stays `const` and dependency
/// free (`cry-audio` shells nothing and pulls no hashing crate).
#[must_use]
pub const fn az_crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        let byte = if byte.is_ascii_uppercase() {
            byte + 0x20
        } else {
            byte
        };
        crc ^= byte as u32;
        let mut bit = 0;
        while bit < 8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
            bit += 1;
        }
        index += 1;
    }
    !crc
}

impl WwiseNameId {
    pub const INVALID: Self = Self(0);

    #[must_use]
    pub const fn from_name(name: &str) -> Self {
        Self(hash_wwise_name(name.as_bytes()))
    }
}

impl WwiseSectionId {
    pub const BKHD: Self = Self::from_tag(*b"BKHD");
    pub const DIDX: Self = Self::from_tag(*b"DIDX");
    pub const DATA: Self = Self::from_tag(*b"DATA");
    pub const HIRC: Self = Self::from_tag(*b"HIRC");
    pub const STID: Self = Self::from_tag(*b"STID");
    pub const STMG: Self = Self::from_tag(*b"STMG");
    pub const INIT: Self = Self::from_tag(*b"INIT");
    pub const ENVS: Self = Self::from_tag(*b"ENVS");
    pub const FXPR: Self = Self::from_tag(*b"FXPR");
    pub const PLAT: Self = Self::from_tag(*b"PLAT");

    #[must_use]
    pub const fn from_tag(tag: [u8; 4]) -> Self {
        Self(u32::from_le_bytes(tag))
    }

    #[must_use]
    pub const fn tag(self) -> [u8; 4] {
        self.0.to_le_bytes()
    }

    #[must_use]
    pub fn tag_string(self) -> String {
        tag_string(self.tag())
    }
}

impl fmt::Display for WwiseSectionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.tag_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct WwiseBankHeader {
    pub version: u32,
    pub bank_id: WwiseBankId,
    pub language_id: Option<u32>,
    pub feedback_in_bank: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct WwiseBankSection {
    pub id: WwiseSectionId,
    pub offset: u32,
    pub size: u32,
}

impl WwiseBankSection {
    pub fn payload(self, bytes: &[u8]) -> Option<&[u8]> {
        let start = self.offset as usize;
        let end = start.checked_add(self.size as usize)?;
        bytes.get(start..end)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct WwiseMediaEntry {
    pub id: WwiseMediaId,
    pub offset: u32,
    pub size: u32,
}

impl WwiseMediaEntry {
    #[must_use]
    pub const fn end_offset(self) -> Option<u32> {
        self.offset.checked_add(self.size)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize)]
#[repr(transparent)]
pub struct WwiseHierarchyObjectKind(pub u8);

impl WwiseHierarchyObjectKind {
    pub const EVENT: Self = Self(4);

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self.0 {
            1 => "State",
            2 => "Sound",
            3 => "Action",
            4 => "Event",
            5 => "RandomSequenceContainer",
            6 => "SwitchContainer",
            7 => "ActorMixer",
            8 => "AudioBus",
            9 => "BlendContainer",
            10 => "MusicSegment",
            11 => "MusicTrack",
            12 => "MusicSwitchContainer",
            13 => "MusicPlaylistContainer",
            14 => "Attenuation",
            15 => "DialogueEvent",
            16 => "MotionBus",
            17 => "MotionFx",
            18 => "Effect",
            19 => "AuxiliaryBus",
            20 => "LfoModulator",
            21 => "EnvelopeModulator",
            22 => "AudioDevice",
            _ => "Unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WwiseHierarchyObject {
    pub kind: WwiseHierarchyObjectKind,
    pub kind_name: &'static str,
    pub object_id: WwiseObjectId,
    pub data_offset: u32,
    pub data_size: u32,
    pub event_action_ids: Vec<WwiseObjectId>,
}

impl WwiseHierarchyObject {
    pub fn data<'a>(&self, bytes: &'a [u8]) -> Option<&'a [u8]> {
        let start = self.data_offset as usize;
        let end = start.checked_add(self.data_size as usize)?;
        bytes.get(start..end)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct WwiseSoundBank {
    pub header: Option<WwiseBankHeader>,
    pub sections: Vec<WwiseBankSection>,
    pub media: Vec<WwiseMediaEntry>,
    pub hierarchy: Vec<WwiseHierarchyObject>,
}

impl WwiseSoundBank {
    pub fn parse(bytes: &[u8]) -> Result<Self, WwiseSoundBankParseError> {
        let mut bank = Self::default();
        let mut cursor = 0usize;
        while cursor < bytes.len() {
            let id = WwiseSectionId::from_tag(read_4(bytes, cursor, "section id")?);
            let size = read_u32(bytes, cursor + 4, "section size")?;
            let payload_offset = cursor
                .checked_add(8)
                .ok_or(WwiseSoundBankParseError::SectionOutOfBounds { section: id })?;
            let payload_end = payload_offset
                .checked_add(size as usize)
                .filter(|end| *end <= bytes.len())
                .ok_or(WwiseSoundBankParseError::SectionOutOfBounds { section: id })?;
            let section = WwiseBankSection {
                id,
                offset: u32::try_from(payload_offset)
                    .map_err(|_| WwiseSoundBankParseError::OffsetTooLarge { section: id })?,
                size,
            };
            let payload = &bytes[payload_offset..payload_end];
            if id == WwiseSectionId::BKHD {
                bank.header = Some(parse_bank_header(payload)?);
            } else if id == WwiseSectionId::DIDX {
                bank.media = parse_media_index(payload)?;
            } else if id == WwiseSectionId::HIRC {
                bank.hierarchy = parse_hierarchy(payload, section.offset)?;
            }
            bank.sections.push(section);
            cursor = payload_end;
        }
        if bank.header.is_none() {
            return Err(WwiseSoundBankParseError::MissingBankHeader);
        }
        if !bank.media.is_empty() {
            let data = bank
                .section(WwiseSectionId::DATA)
                .ok_or(WwiseSoundBankParseError::MissingDataSection)?;
            for entry in &bank.media {
                if !matches!(entry.end_offset(), Some(end) if end <= data.size) {
                    return Err(WwiseSoundBankParseError::InvalidMediaRange { media_id: entry.id });
                }
            }
        }
        Ok(bank)
    }

    #[must_use]
    pub fn section(&self, id: WwiseSectionId) -> Option<WwiseBankSection> {
        self.sections
            .iter()
            .copied()
            .find(|section| section.id == id)
    }

    pub fn embedded_media<'a>(
        &self,
        bytes: &'a [u8],
        entry: WwiseMediaEntry,
    ) -> Result<&'a [u8], WwiseSoundBankParseError> {
        let data = self
            .section(WwiseSectionId::DATA)
            .ok_or(WwiseSoundBankParseError::MissingDataSection)?;
        let start = (data.offset as usize)
            .checked_add(entry.offset as usize)
            .ok_or(WwiseSoundBankParseError::InvalidMediaRange { media_id: entry.id })?;
        let end = start
            .checked_add(entry.size as usize)
            .ok_or(WwiseSoundBankParseError::InvalidMediaRange { media_id: entry.id })?;
        bytes
            .get(start..end)
            .ok_or(WwiseSoundBankParseError::InvalidMediaRange { media_id: entry.id })
    }

    /// Extract every embedded media payload from a bank in DIDX order.
    ///
    /// Pure parse helper — no decoding. Callers that want PCM/WAV shell out to
    /// a Wwise decoder (e.g. vgmstream) with the returned WEM bytes.
    pub fn embedded_media_all<'a>(
        &self,
        bytes: &'a [u8],
    ) -> Result<Vec<(WwiseMediaId, &'a [u8])>, WwiseSoundBankParseError> {
        self.media
            .iter()
            .copied()
            .map(|entry| {
                let payload = self.embedded_media(bytes, entry)?;
                Ok((entry.id, payload))
            })
            .collect()
    }
}

fn parse_bank_header(payload: &[u8]) -> Result<WwiseBankHeader, WwiseSoundBankParseError> {
    if payload.len() < 8 {
        return Err(WwiseSoundBankParseError::InvalidBankHeaderSize {
            size: payload.len(),
        });
    }
    Ok(WwiseBankHeader {
        version: read_u32(payload, 0, "BKHD version")?,
        bank_id: WwiseBankId(read_u32(payload, 4, "BKHD bank id")?),
        language_id: read_optional_u32(payload, 8, "BKHD language id")?,
        feedback_in_bank: read_optional_u32(payload, 12, "BKHD feedback flag")?,
    })
}

fn parse_media_index(payload: &[u8]) -> Result<Vec<WwiseMediaEntry>, WwiseSoundBankParseError> {
    if !payload.len().is_multiple_of(12) {
        return Err(WwiseSoundBankParseError::InvalidDidxSize {
            size: payload.len(),
        });
    }
    payload
        .chunks_exact(12)
        .map(|chunk| {
            Ok(WwiseMediaEntry {
                id: WwiseMediaId(read_u32(chunk, 0, "DIDX media id")?),
                offset: read_u32(chunk, 4, "DIDX media offset")?,
                size: read_u32(chunk, 8, "DIDX media size")?,
            })
        })
        .collect()
}

fn parse_hierarchy(
    payload: &[u8],
    payload_offset: u32,
) -> Result<Vec<WwiseHierarchyObject>, WwiseSoundBankParseError> {
    let count = read_u32(payload, 0, "HIRC object count")?;
    let mut cursor = 4usize;
    let mut objects = Vec::with_capacity(count as usize);
    for index in 0..count {
        let kind = WwiseHierarchyObjectKind(*payload.get(cursor).ok_or(
            WwiseSoundBankParseError::UnexpectedEof {
                context: "HIRC object type",
            },
        )?);
        cursor += 1;
        let size = read_u32(payload, cursor, "HIRC object size")?;
        cursor += 4;
        if size < 4 {
            return Err(WwiseSoundBankParseError::InvalidHircObjectSize { index, size });
        }
        let end = cursor
            .checked_add(size as usize)
            .filter(|end| *end <= payload.len())
            .ok_or(WwiseSoundBankParseError::HircObjectOutOfBounds { index })?;
        let object_id = WwiseObjectId(read_u32(payload, cursor, "HIRC object id")?);
        let body = &payload[cursor + 4..end];
        let event_action_ids = if kind == WwiseHierarchyObjectKind::EVENT {
            parse_event_actions(object_id, body)?
        } else {
            Vec::new()
        };
        let absolute = (payload_offset as usize)
            .checked_add(cursor)
            .and_then(|offset| u32::try_from(offset).ok())
            .ok_or(WwiseSoundBankParseError::OffsetTooLarge {
                section: WwiseSectionId::HIRC,
            })?;
        objects.push(WwiseHierarchyObject {
            kind,
            kind_name: kind.name(),
            object_id,
            data_offset: absolute,
            data_size: size,
            event_action_ids,
        });
        cursor = end;
    }
    if cursor != payload.len() {
        return Err(WwiseSoundBankParseError::TrailingHircBytes {
            count: payload.len() - cursor,
        });
    }
    Ok(objects)
}

fn parse_event_actions(
    object_id: WwiseObjectId,
    body: &[u8],
) -> Result<Vec<WwiseObjectId>, WwiseSoundBankParseError> {
    let (count, cursor) = read_packed_u32(body, "HIRC event action count")?;
    let end = cursor
        .checked_add(count as usize * 4)
        .filter(|end| *end <= body.len())
        .ok_or(WwiseSoundBankParseError::HircEventActionsOutOfBounds { object_id })?;
    body[cursor..end]
        .chunks_exact(4)
        .map(|bytes| {
            Ok(WwiseObjectId(u32::from_le_bytes(
                bytes.try_into().expect("four bytes"),
            )))
        })
        .collect()
}

fn read_packed_u32(
    bytes: &[u8],
    context: &'static str,
) -> Result<(u32, usize), WwiseSoundBankParseError> {
    let mut cursor = 0usize;
    let mut byte = *bytes
        .get(cursor)
        .ok_or(WwiseSoundBankParseError::UnexpectedEof { context })?;
    cursor += 1;
    let mut value = u32::from(byte & 0x7f);
    while byte & 0x80 != 0 {
        byte = *bytes
            .get(cursor)
            .ok_or(WwiseSoundBankParseError::UnexpectedEof { context })?;
        cursor += 1;
        if value > u32::MAX >> 7 {
            return Err(WwiseSoundBankParseError::PackedIntegerOverflow { context });
        }
        value = (value << 7) | u32::from(byte & 0x7f);
    }
    Ok((value, cursor))
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize)]
#[repr(transparent)]
pub struct WwiseMediaChunkId(pub u32);

impl WwiseMediaChunkId {
    pub const FMT: Self = Self::from_tag(*b"fmt ");
    pub const DATA: Self = Self::from_tag(*b"data");
    pub const CUE: Self = Self::from_tag(*b"cue ");
    pub const LIST: Self = Self::from_tag(*b"LIST");
    pub const SMPL: Self = Self::from_tag(*b"smpl");
    pub const VORB: Self = Self::from_tag(*b"vorb");

    #[must_use]
    pub const fn from_tag(tag: [u8; 4]) -> Self {
        Self(u32::from_le_bytes(tag))
    }

    #[must_use]
    pub const fn tag(self) -> [u8; 4] {
        self.0.to_le_bytes()
    }

    #[must_use]
    pub fn tag_string(self) -> String {
        tag_string(self.tag())
    }
}

impl fmt::Display for WwiseMediaChunkId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.tag_string())
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct WwiseMediaChunk {
    pub id: WwiseMediaChunkId,
    pub offset: u32,
    pub size: u32,
}

impl WwiseMediaChunk {
    pub fn payload(self, bytes: &[u8]) -> Option<&[u8]> {
        let start = self.offset as usize;
        let end = start.checked_add(self.size as usize)?;
        bytes.get(start..end)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WwiseWaveFormat {
    pub format_tag: u16,
    pub channels: u16,
    pub sample_rate: u32,
    pub average_bytes_per_second: u32,
    pub block_align: u16,
    pub bits_per_sample: u16,
    pub extra_size: Option<u16>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct WwiseMediaInfo {
    pub riff_size: u32,
    pub wave_format: Option<WwiseWaveFormat>,
    pub chunks: Vec<WwiseMediaChunk>,
    pub trailing_bytes: usize,
}

impl WwiseMediaInfo {
    pub fn parse(bytes: &[u8]) -> Result<Self, WwiseMediaParseError> {
        if bytes.len() < 12 {
            return Err(WwiseMediaParseError::TooShort { size: bytes.len() });
        }
        if &bytes[0..4] != b"RIFF" {
            return Err(WwiseMediaParseError::InvalidRiffMagic {
                actual: bytes[0..4].try_into().expect("four bytes"),
            });
        }
        if &bytes[8..12] != b"WAVE" {
            return Err(WwiseMediaParseError::InvalidWaveFormat {
                actual: bytes[8..12].try_into().expect("four bytes"),
            });
        }
        let riff_size = u32::from_le_bytes(bytes[4..8].try_into().expect("four bytes"));
        let declared_end = (riff_size as usize)
            .checked_add(8)
            .filter(|end| *end <= bytes.len())
            .ok_or(WwiseMediaParseError::Truncated {
                declared: riff_size as usize + 8,
                actual: bytes.len(),
            })?;
        let mut info = Self {
            riff_size,
            wave_format: None,
            chunks: Vec::new(),
            trailing_bytes: bytes.len() - declared_end,
        };
        let mut cursor = 12usize;
        while cursor < declared_end {
            if cursor + 8 > declared_end {
                return Err(WwiseMediaParseError::ChunkOutOfBounds {
                    offset: cursor,
                    size: 0,
                });
            }
            let id = WwiseMediaChunkId::from_tag(
                bytes[cursor..cursor + 4].try_into().expect("four bytes"),
            );
            let size = u32::from_le_bytes(
                bytes[cursor + 4..cursor + 8]
                    .try_into()
                    .expect("four bytes"),
            );
            let offset = cursor + 8;
            let end = offset
                .checked_add(size as usize)
                .filter(|end| *end <= declared_end)
                .ok_or(WwiseMediaParseError::ChunkOutOfBounds {
                    offset: cursor,
                    size,
                })?;
            let chunk = WwiseMediaChunk {
                id,
                offset: u32::try_from(offset)
                    .map_err(|_| WwiseMediaParseError::ChunkOffsetTooLarge { id })?,
                size,
            };
            if id == WwiseMediaChunkId::FMT {
                if info.wave_format.is_some() {
                    return Err(WwiseMediaParseError::DuplicateFormatChunk);
                }
                info.wave_format = Some(parse_wave_format(&bytes[offset..end])?);
            }
            info.chunks.push(chunk);
            // RIFF chunks are word-aligned when another chunk follows. Wwise
            // omits the otherwise-unused pad byte after a terminal odd-sized
            // DATA chunk in shipped New World banks.
            let padded = end + usize::from(size & 1 != 0 && end < declared_end);
            if padded > declared_end {
                return Err(WwiseMediaParseError::ChunkOutOfBounds {
                    offset: cursor,
                    size,
                });
            }
            cursor = padded;
        }
        if info.wave_format.is_none() {
            return Err(WwiseMediaParseError::MissingFormatChunk);
        }
        if !info
            .chunks
            .iter()
            .any(|chunk| chunk.id == WwiseMediaChunkId::DATA)
        {
            return Err(WwiseMediaParseError::MissingDataChunk);
        }
        Ok(info)
    }
}

fn parse_wave_format(bytes: &[u8]) -> Result<WwiseWaveFormat, WwiseMediaParseError> {
    if bytes.len() < 16 {
        return Err(WwiseMediaParseError::InvalidFormatChunk { size: bytes.len() });
    }
    Ok(WwiseWaveFormat {
        format_tag: u16::from_le_bytes(bytes[0..2].try_into().expect("two bytes")),
        channels: u16::from_le_bytes(bytes[2..4].try_into().expect("two bytes")),
        sample_rate: u32::from_le_bytes(bytes[4..8].try_into().expect("four bytes")),
        average_bytes_per_second: u32::from_le_bytes(bytes[8..12].try_into().expect("four bytes")),
        block_align: u16::from_le_bytes(bytes[12..14].try_into().expect("two bytes")),
        bits_per_sample: u16::from_le_bytes(bytes[14..16].try_into().expect("two bytes")),
        extra_size: (bytes.len() >= 18)
            .then(|| u16::from_le_bytes(bytes[16..18].try_into().expect("two bytes"))),
    })
}

pub const WWISE_TRIGGER_BANK_MAP_FILE: &str = "libs/gameaudio/wwise/triggerbankmapatlbin.bin";
const TRIGGER_BANK_RECORD_SIZE: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WwiseTriggerBankMap<'a> {
    bytes: &'a [u8],
}

impl<'a> WwiseTriggerBankMap<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self, WwiseTriggerBankMapError> {
        if !bytes.len().is_multiple_of(TRIGGER_BANK_RECORD_SIZE) {
            return Err(WwiseTriggerBankMapError::InvalidSize { size: bytes.len() });
        }
        Ok(Self { bytes })
    }

    pub fn entries(self) -> impl ExactSizeIterator<Item = WwiseTriggerBankMapEntry> + 'a {
        self.bytes
            .chunks_exact(TRIGGER_BANK_RECORD_SIZE)
            .map(|bytes| WwiseTriggerBankMapEntry {
                bank_id: WwiseBankId(u32::from_le_bytes(
                    bytes[0..4].try_into().expect("four bytes"),
                )),
                control_ids: [
                    AudioControlId(u64::from(u32::from_le_bytes(
                        bytes[4..8].try_into().expect("four bytes"),
                    ))),
                    AudioControlId(u64::from(u32::from_le_bytes(
                        bytes[8..12].try_into().expect("four bytes"),
                    ))),
                    AudioControlId(u64::from(u32::from_le_bytes(
                        bytes[12..16].try_into().expect("four bytes"),
                    ))),
                ],
            })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct WwiseTriggerBankMapEntry {
    pub bank_id: WwiseBankId,
    pub control_ids: [AudioControlId; 3],
}

fn read_optional_u32(
    bytes: &[u8],
    offset: usize,
    context: &'static str,
) -> Result<Option<u32>, WwiseSoundBankParseError> {
    if offset >= bytes.len() {
        Ok(None)
    } else {
        read_u32(bytes, offset, context).map(Some)
    }
}

fn read_u32(
    bytes: &[u8],
    offset: usize,
    context: &'static str,
) -> Result<u32, WwiseSoundBankParseError> {
    Ok(u32::from_le_bytes(read_4(bytes, offset, context)?))
}

fn read_4(
    bytes: &[u8],
    offset: usize,
    context: &'static str,
) -> Result<[u8; 4], WwiseSoundBankParseError> {
    bytes
        .get(offset..offset.saturating_add(4))
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or(WwiseSoundBankParseError::UnexpectedEof { context })
}

fn tag_string(tag: [u8; 4]) -> String {
    tag.into_iter()
        .map(|byte| {
            if byte.is_ascii_graphic() || byte == b' ' {
                char::from(byte)
            } else {
                '.'
            }
        })
        .collect()
}

const fn hash_wwise_name(bytes: &[u8]) -> u32 {
    let mut hash = 0x811c_9dc5u32;
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        let byte = if byte.is_ascii_uppercase() {
            byte + 0x20
        } else {
            byte
        };
        hash = hash.wrapping_mul(0x0100_0193) ^ byte as u32;
        index += 1;
    }
    hash
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WwiseSoundBankParseError {
    #[error("Wwise soundbank is missing BKHD")]
    MissingBankHeader,
    #[error("Wwise soundbank has DIDX media but no DATA section")]
    MissingDataSection,
    #[error("unexpected end of Wwise soundbank while reading {context}")]
    UnexpectedEof { context: &'static str },
    #[error("Wwise section {section} extends past the bank")]
    SectionOutOfBounds { section: WwiseSectionId },
    #[error("Wwise section {section} offset does not fit u32")]
    OffsetTooLarge { section: WwiseSectionId },
    #[error("BKHD size {size} is smaller than 8")]
    InvalidBankHeaderSize { size: usize },
    #[error("DIDX size {size} is not a multiple of 12")]
    InvalidDidxSize { size: usize },
    #[error("HIRC object {index} size {size} is smaller than 4")]
    InvalidHircObjectSize { index: u32, size: u32 },
    #[error("HIRC object {index} extends past the section")]
    HircObjectOutOfBounds { index: u32 },
    #[error("HIRC has {count} unclaimed trailing bytes")]
    TrailingHircBytes { count: usize },
    #[error("HIRC packed integer overflow while reading {context}")]
    PackedIntegerOverflow { context: &'static str },
    #[error("HIRC event {object_id:?} action list extends past the object")]
    HircEventActionsOutOfBounds { object_id: WwiseObjectId },
    #[error("DIDX media {media_id:?} extends past DATA")]
    InvalidMediaRange { media_id: WwiseMediaId },
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WwiseMediaParseError {
    #[error("Wwise media is too short: {size}")]
    TooShort { size: usize },
    #[error("invalid RIFF magic {actual:?}")]
    InvalidRiffMagic { actual: [u8; 4] },
    #[error("invalid WAVE tag {actual:?}")]
    InvalidWaveFormat { actual: [u8; 4] },
    #[error("Wwise media is truncated: declared {declared}, actual {actual}")]
    Truncated { declared: usize, actual: usize },
    #[error("Wwise media chunk at {offset} size {size} is out of bounds")]
    ChunkOutOfBounds { offset: usize, size: u32 },
    #[error("Wwise media chunk {id} offset does not fit u32")]
    ChunkOffsetTooLarge { id: WwiseMediaChunkId },
    #[error("Wwise media has no fmt chunk")]
    MissingFormatChunk,
    #[error("Wwise media has no data chunk")]
    MissingDataChunk,
    #[error("Wwise media has multiple fmt chunks")]
    DuplicateFormatChunk,
    #[error("Wwise fmt chunk size {size} is smaller than 16")]
    InvalidFormatChunk { size: usize },
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WwiseTriggerBankMapError {
    #[error("Wwise trigger-bank map size {size} is not a multiple of 16")]
    InvalidSize { size: usize },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bank_media_hierarchy_and_embedded_media() {
        let mut bytes = Vec::new();
        push_section(
            &mut bytes,
            *b"BKHD",
            &[123u32.to_le_bytes(), 456u32.to_le_bytes()].concat(),
        );
        push_section(
            &mut bytes,
            *b"DIDX",
            &[100u32.to_le_bytes(), 0u32.to_le_bytes(), 4u32.to_le_bytes()].concat(),
        );
        push_section(&mut bytes, *b"DATA", &[1, 2, 3, 4]);
        let mut hirc = Vec::new();
        hirc.extend_from_slice(&1u32.to_le_bytes());
        hirc.push(4);
        hirc.extend_from_slice(&13u32.to_le_bytes());
        hirc.extend_from_slice(&77u32.to_le_bytes());
        hirc.push(2);
        hirc.extend_from_slice(&10u32.to_le_bytes());
        hirc.extend_from_slice(&11u32.to_le_bytes());
        push_section(&mut bytes, *b"HIRC", &hirc);

        let bank = WwiseSoundBank::parse(&bytes).unwrap();
        assert_eq!(bank.header.unwrap().bank_id, WwiseBankId(456));
        assert_eq!(
            bank.hierarchy[0].event_action_ids,
            [WwiseObjectId(10), WwiseObjectId(11)]
        );
        assert_eq!(
            bank.embedded_media(&bytes, bank.media[0]).unwrap(),
            [1, 2, 3, 4]
        );
    }

    #[test]
    fn parses_wem_format_and_chunks() {
        let mut chunks = Vec::new();
        chunks.extend_from_slice(b"fmt ");
        chunks.extend_from_slice(&16u32.to_le_bytes());
        chunks.extend_from_slice(&0xffffu16.to_le_bytes());
        chunks.extend_from_slice(&2u16.to_le_bytes());
        chunks.extend_from_slice(&48_000u32.to_le_bytes());
        chunks.extend_from_slice(&12_000u32.to_le_bytes());
        chunks.extend_from_slice(&4u16.to_le_bytes());
        chunks.extend_from_slice(&0u16.to_le_bytes());
        chunks.extend_from_slice(b"data");
        chunks.extend_from_slice(&3u32.to_le_bytes());
        chunks.extend_from_slice(&[1, 2, 3, 0]);
        let mut bytes = b"RIFF".to_vec();
        bytes.extend_from_slice(&(chunks.len() as u32 + 4).to_le_bytes());
        bytes.extend_from_slice(b"WAVE");
        bytes.extend_from_slice(&chunks);

        let media = WwiseMediaInfo::parse(&bytes).unwrap();
        assert_eq!(media.wave_format.unwrap().sample_rate, 48_000);
        assert_eq!(media.chunks.len(), 2);
    }

    #[test]
    fn accepts_terminal_odd_data_without_padding() {
        let mut chunks = Vec::new();
        chunks.extend_from_slice(b"fmt ");
        chunks.extend_from_slice(&16u32.to_le_bytes());
        chunks.extend_from_slice(&0xffffu16.to_le_bytes());
        chunks.extend_from_slice(&1u16.to_le_bytes());
        chunks.extend_from_slice(&48_000u32.to_le_bytes());
        chunks.extend_from_slice(&6_000u32.to_le_bytes());
        chunks.extend_from_slice(&2u16.to_le_bytes());
        chunks.extend_from_slice(&0u16.to_le_bytes());
        chunks.extend_from_slice(b"data");
        chunks.extend_from_slice(&3u32.to_le_bytes());
        chunks.extend_from_slice(&[1, 2, 3]);
        let mut bytes = b"RIFF".to_vec();
        bytes.extend_from_slice(&(chunks.len() as u32 + 4).to_le_bytes());
        bytes.extend_from_slice(b"WAVE");
        bytes.extend_from_slice(&chunks);

        let media = WwiseMediaInfo::parse(&bytes).unwrap();
        assert_eq!(media.chunks.last().unwrap().size, 3);
        assert_eq!(media.trailing_bytes, 0);
    }

    #[test]
    fn wwise_name_hash_matches_engine_adapter() {
        assert_eq!(WwiseNameId::from_name("Play_Foo"), WwiseNameId(0xb255_9ee2));
        assert_eq!(
            WwiseNameId::from_name("Play_Foo"),
            WwiseNameId::from_name("play_foo")
        );
    }

    #[test]
    fn atl_control_id_matches_shipped_trigger_bank_map() {
        // Verified against `libs/gameaudio/wwise/triggerbankmapatlbin.bin`.
        assert_eq!(
            AudioControlId::from_name("blend_ftsp_alligator"),
            AudioControlId(2_229_255_591)
        );
        assert_eq!(
            AudioControlId::from_name("Blend_Ftsp_Alligator"),
            AudioControlId::from_name("blend_ftsp_alligator")
        );
    }

    fn push_section(bytes: &mut Vec<u8>, tag: [u8; 4], payload: &[u8]) {
        bytes.extend_from_slice(&tag);
        bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        bytes.extend_from_slice(payload);
    }
}
