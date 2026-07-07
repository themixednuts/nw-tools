//! CAF controller value decoding.
//!
//! This is intentionally part of `cry-chunk`: the raw CAF controller payloads
//! live here, and this module is the legacy-only value layer above those bytes.

use thiserror::Error;

use crate::{
    ChunkFile, ChunkFileError, ChunkPayload, ChunkPayloadError, ChunkType, ControllerChunk,
    ControllerCompressedChunk, ControllerTrack, GlobalAnimationHeaderCafChunk,
    MotionParametersChunk, TimingChunk,
};

/// Decoded CAF animation with per-controller TRS tracks in CAF key seconds.
#[derive(Debug, Clone)]
pub struct CafAnimation {
    pub header: CafAnimationHeader,
    pub sample_rate: f32,
    pub controllers: Vec<CafController>,
}

/// Timing/header values used by Lumberyard to sample a CAF.
#[derive(Debug, Clone)]
pub struct CafAnimationHeader {
    pub flags: u32,
    pub start_sec: f32,
    pub end_sec: f32,
    pub total_duration: f32,
    pub controller_count: u32,
    pub source: CafAnimationHeaderSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CafAnimationHeaderSource {
    GlobalAnimationHeaderCaf,
    MotionParameters,
    Timing,
}

/// Raw controller chunk coverage for one CAF file.
#[derive(Debug, Clone)]
pub struct CafControllerScan {
    pub controllers: Vec<CafControllerChunkScan>,
}

/// Raw controller chunk form and header fields as stored in a CAF.
#[derive(Debug, Clone)]
pub enum CafControllerChunkScan {
    Tcb {
        controller_id: u32,
        controller_type: i32,
        key_count: usize,
    },
    PqLog {
        version: u16,
        controller_id: u32,
        key_count: usize,
        flags: u32,
    },
    Empty0828,
    Compressed(CafCompressedControllerScan),
    ControllerDb {
        position_key_count: usize,
        rotation_key_count: usize,
        time_key_count: usize,
        animation_count: usize,
    },
    Unknown {
        version: u16,
        byte_len: usize,
    },
}

impl CafControllerChunkScan {
    #[must_use]
    pub const fn form(&self) -> CafControllerForm {
        match self {
            Self::Tcb { .. } => CafControllerForm::Tcb0826,
            Self::PqLog {
                version: 0x0827, ..
            } => CafControllerForm::PqLog0827,
            Self::PqLog {
                version: 0x0830, ..
            } => CafControllerForm::PqLog0830,
            Self::PqLog { version, .. } => CafControllerForm::Unknown(*version),
            Self::Empty0828 => CafControllerForm::Empty0828,
            Self::Compressed(controller) => {
                if controller.version == 0x0831 {
                    CafControllerForm::Compressed0831
                } else {
                    CafControllerForm::Compressed0829
                }
            }
            Self::ControllerDb { .. } => CafControllerForm::ControllerDb0905,
            Self::Unknown { version, .. } => CafControllerForm::Unknown(*version),
        }
    }
}

/// Controller chunk form stored in the Cry chunk table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CafControllerForm {
    Tcb0826,
    PqLog0827,
    Empty0828,
    Compressed0829,
    PqLog0830,
    Compressed0831,
    ControllerDb0905,
    Unknown(u16),
}

impl CafControllerForm {
    #[must_use]
    pub fn label(self) -> String {
        match self {
            Self::Tcb0826 => "TCB 0x0826".to_string(),
            Self::PqLog0827 => "PQLog 0x0827".to_string(),
            Self::Empty0828 => "Empty 0x0828".to_string(),
            Self::Compressed0829 => "Compressed 0x0829".to_string(),
            Self::PqLog0830 => "PQLog 0x0830".to_string(),
            Self::Compressed0831 => "Compressed 0x0831".to_string(),
            Self::ControllerDb0905 => "ControllerDb 0x0905".to_string(),
            Self::Unknown(version) => format!("Unknown {version:#06x}"),
        }
    }
}

/// Raw header fields for a compressed 0x0829/0x0831 controller chunk.
#[derive(Debug, Clone, Copy)]
pub struct CafCompressedControllerScan {
    pub version: u16,
    pub controller_id: u32,
    pub flags: u32,
    pub rotation_key_count: usize,
    pub position_key_count: usize,
    pub rotation_format: u8,
    pub rotation_time_format: u8,
    pub position_format: u8,
    pub position_keys_info: u8,
    pub position_time_format: u8,
    pub tracks_aligned: bool,
}

/// Error while scanning raw CAF controller chunk headers.
#[derive(Debug, Error)]
pub enum CafControllerScanError {
    #[error(transparent)]
    ChunkFile(#[from] ChunkFileError),
    #[error("CAF controller chunk {version:#06x} is truncated: needs {needed} bytes, has {actual}")]
    TruncatedControllerHeader {
        version: u16,
        needed: usize,
        actual: usize,
    },
}

/// One CAF controller keyed by the skeleton bone controller ID.
#[derive(Debug, Clone)]
pub struct CafController {
    pub controller_id: u32,
    pub flags: u32,
    pub rotations: Vec<RotationKey>,
    pub positions: Vec<PositionKey>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RotationKey {
    pub time: f32,
    /// Quaternion in Cry component order `[x, y, z, w]`.
    pub value: [f32; 4],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PositionKey {
    pub time: f32,
    /// Position in Cry component order `[x, y, z]`.
    pub value: [f32; 3],
}

/// Error while decoding CAF controller values.
#[derive(Debug, Error)]
pub enum CafDecodeError {
    #[error(transparent)]
    ChunkFile(#[from] ChunkFileError),
    #[error(transparent)]
    ChunkPayload(#[from] ChunkPayloadError),
    #[error(
        "CAF has controller key data but no GlobalAnimationHeaderCaf/MotionParameters/Timing header"
    )]
    MissingHeader,
    #[error("CAF sample rate is invalid: {sample_rate}")]
    InvalidSampleRate { sample_rate: f32 },
    #[error("CAF duration is invalid: {duration}")]
    InvalidDuration { duration: f32 },
    #[error("unsupported CAF controller form {form} for controller {controller_id:#010x}")]
    UnsupportedControllerForm {
        controller_id: u32,
        form: &'static str,
    },
    #[error("compressed controller {controller_id:#010x} is missing {track} times")]
    MissingTrackTimes {
        controller_id: u32,
        track: &'static str,
    },
    #[error(
        "compressed controller {controller_id:#010x} {track} count {value_count} does not match time count {time_count}"
    )]
    TrackCountMismatch {
        controller_id: u32,
        track: &'static str,
        value_count: usize,
        time_count: usize,
    },
    #[error("unsupported CAF {track} format {format}")]
    UnsupportedTrackFormat { track: &'static str, format: u8 },
    #[error("CAF {track} data is truncated")]
    TruncatedTrack { track: &'static str },
    #[error("CAF {track} key times are not strictly increasing")]
    NonIncreasingTimes { track: &'static str },
    #[error("CAF {track} key times do not span header duration {duration}: {first}..{last}")]
    TimeSpanMismatch {
        track: &'static str,
        first: f32,
        last: f32,
        duration: f32,
    },
    #[error("CAF bitset key-time track has invalid header")]
    InvalidBitsetTimeHeader,
    #[error("CAF bitset key-time track decoded {decoded} keys but header declares {declared}")]
    BitsetKeyCountMismatch { declared: usize, decoded: usize },
    #[error("CAF quaternion is invalid for controller {controller_id:#010x}")]
    InvalidQuaternion { controller_id: u32 },
}

impl CafAnimation {
    /// Parse a CAF chunk file and decode all compressed PQ controllers.
    ///
    /// The formulas mirror Lumberyard CryAnimation:
    /// `GlobalAnimationHeaderCAF::ReadMotionParameters` for sample rate,
    /// `ControllerPQ` key-time tracks, and `QuatQuantization` for quaternion
    /// unpacking. Old PQLog/TCB forms remain explicit unsupported errors.
    pub fn parse(bytes: &[u8]) -> Result<Self, CafDecodeError> {
        let file = ChunkFile::parse(bytes)?;
        let mut global_header = None;
        let mut motion_parameters = None;
        let mut timing = None;
        let mut compressed = Vec::new();
        let mut unsupported = Vec::new();

        for chunk in file.decoded_chunks() {
            let chunk = chunk?;
            match chunk.payload {
                ChunkPayload::GlobalAnimationHeaderCaf(chunk) => global_header = Some(chunk),
                ChunkPayload::MotionParameters(chunk) => motion_parameters = Some(chunk),
                ChunkPayload::Timing(chunk) => timing = Some(chunk),
                ChunkPayload::Controller(ControllerChunk::Compressed(controller)) => {
                    compressed.push(controller);
                }
                ChunkPayload::Controller(ControllerChunk::Tcb(controller)) => {
                    unsupported.push((controller.controller_id, "TCB"));
                }
                ChunkPayload::Controller(ControllerChunk::Uncompressed(controller)) => {
                    unsupported.push((controller.controller_id, "PQLog"));
                }
                ChunkPayload::Controller(ControllerChunk::ControllerDb(_)) => {
                    unsupported.push((0, "ControllerDb"));
                }
                ChunkPayload::Controller(ControllerChunk::Empty0828) => {}
                _ => {}
            }
        }

        let (header, sample_rate) = build_header(
            global_header,
            motion_parameters,
            timing,
            compressed.len() as u32,
        )?;
        if !sample_rate.is_finite() || sample_rate <= 0.0 {
            return Err(CafDecodeError::InvalidSampleRate { sample_rate });
        }
        if !header.total_duration.is_finite() || header.total_duration < 0.0 {
            return Err(CafDecodeError::InvalidDuration {
                duration: header.total_duration,
            });
        }

        if compressed.is_empty()
            && let Some((controller_id, form)) = unsupported.into_iter().next()
        {
            return Err(CafDecodeError::UnsupportedControllerForm {
                controller_id,
                form,
            });
        }

        let mut controllers = Vec::with_capacity(compressed.len());
        for controller in compressed {
            controllers.push(decode_compressed_controller(
                &controller,
                &header,
                sample_rate,
            )?);
        }

        Ok(Self {
            header,
            sample_rate,
            controllers,
        })
    }
}

impl CafControllerScan {
    /// Scan raw controller chunk forms and compressed-track format bytes.
    ///
    /// This intentionally reads controller chunk headers directly from the CAF
    /// chunk table, so coverage diagnostics can report unsupported formats
    /// without first needing value-level decoder support.
    pub fn scan(bytes: &[u8]) -> Result<Self, CafControllerScanError> {
        let file = ChunkFile::parse(bytes)?;
        let mut controllers = Vec::new();
        for chunk in file.chunks() {
            let chunk = chunk?;
            if chunk.chunk_type() != Some(ChunkType::Controller) {
                continue;
            }
            let payload = chunk.payload_from(file.bytes())?;
            controllers.push(scan_controller_chunk(chunk.version(), payload)?);
        }
        Ok(Self { controllers })
    }
}

fn scan_controller_chunk(
    version: u16,
    bytes: &[u8],
) -> Result<CafControllerChunkScan, CafControllerScanError> {
    match version {
        0x0826 => {
            require_controller_header(version, bytes, 16)?;
            Ok(CafControllerChunkScan::Tcb {
                controller_type: read_i32_at(bytes, 0),
                key_count: read_i32_at(bytes, 4).max(0) as usize,
                controller_id: read_u32_at(bytes, 12),
            })
        }
        0x0827 => {
            require_controller_header(version, bytes, 8)?;
            Ok(CafControllerChunkScan::PqLog {
                version,
                key_count: read_u32_at(bytes, 0) as usize,
                controller_id: read_u32_at(bytes, 4),
                flags: 0,
            })
        }
        0x0828 => Ok(CafControllerChunkScan::Empty0828),
        0x0829 => scan_compressed_controller_chunk(version, bytes, false)
            .map(CafControllerChunkScan::Compressed),
        0x0830 => {
            require_controller_header(version, bytes, 12)?;
            Ok(CafControllerChunkScan::PqLog {
                version,
                key_count: read_u32_at(bytes, 0) as usize,
                controller_id: read_u32_at(bytes, 4),
                flags: read_u32_at(bytes, 8),
            })
        }
        0x0831 => scan_compressed_controller_chunk(version, bytes, true)
            .map(CafControllerChunkScan::Compressed),
        0x0905 => {
            require_controller_header(version, bytes, 16)?;
            Ok(CafControllerChunkScan::ControllerDb {
                position_key_count: read_u32_at(bytes, 0) as usize,
                rotation_key_count: read_u32_at(bytes, 4) as usize,
                time_key_count: read_u32_at(bytes, 8) as usize,
                animation_count: read_u32_at(bytes, 12) as usize,
            })
        }
        _ => Ok(CafControllerChunkScan::Unknown {
            version,
            byte_len: bytes.len(),
        }),
    }
}

fn scan_compressed_controller_chunk(
    version: u16,
    bytes: &[u8],
    has_flags: bool,
) -> Result<CafCompressedControllerScan, CafControllerScanError> {
    let header_len = if has_flags { 18 } else { 14 };
    require_controller_header(version, bytes, header_len)?;
    let flags_offset = if has_flags { 4 } else { 0 };
    Ok(CafCompressedControllerScan {
        version,
        controller_id: read_u32_at(bytes, 0),
        flags: if has_flags { read_u32_at(bytes, 4) } else { 0 },
        rotation_key_count: read_u16_at(bytes, 4 + flags_offset) as usize,
        position_key_count: read_u16_at(bytes, 6 + flags_offset) as usize,
        rotation_format: bytes[8 + flags_offset],
        rotation_time_format: bytes[9 + flags_offset],
        position_format: bytes[10 + flags_offset],
        position_keys_info: bytes[11 + flags_offset],
        position_time_format: bytes[12 + flags_offset],
        tracks_aligned: bytes[13 + flags_offset] != 0,
    })
}

fn require_controller_header(
    version: u16,
    bytes: &[u8],
    needed: usize,
) -> Result<(), CafControllerScanError> {
    if bytes.len() < needed {
        return Err(CafControllerScanError::TruncatedControllerHeader {
            version,
            needed,
            actual: bytes.len(),
        });
    }
    Ok(())
}

fn read_u16_at(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(
        bytes[offset..offset + 2]
            .try_into()
            .expect("u16 header field"),
    )
}

fn read_u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("u32 header field"),
    )
}

fn read_i32_at(bytes: &[u8], offset: usize) -> i32 {
    i32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("i32 header field"),
    )
}

fn build_header(
    global_header: Option<GlobalAnimationHeaderCafChunk>,
    motion_parameters: Option<MotionParametersChunk>,
    timing: Option<TimingChunk>,
    controller_count: u32,
) -> Result<(CafAnimationHeader, f32), CafDecodeError> {
    if let Some(motion) = motion_parameters {
        let sample_rate = compute_sample_rate(motion.seconds_per_tick, motion.ticks_per_frame);
        let mut start_key = motion.start;
        if motion.asset_flags & 0x001 != 0 {
            start_key += 1;
        }
        let start_sec = start_key as f32 / sample_rate;
        let end_sec = motion.end as f32 / sample_rate;
        let total_duration = (end_sec - start_sec).max(0.0);
        let header = match global_header {
            Some(global) => CafAnimationHeader {
                flags: global.flags,
                start_sec: global.start_sec,
                end_sec: global.end_sec,
                total_duration: global.total_duration,
                controller_count: global.controller_count,
                source: CafAnimationHeaderSource::GlobalAnimationHeaderCaf,
            },
            None => CafAnimationHeader {
                flags: motion.asset_flags,
                start_sec,
                end_sec,
                total_duration,
                controller_count,
                source: CafAnimationHeaderSource::MotionParameters,
            },
        };
        return Ok((header, sample_rate));
    }

    if let Some(timing) = timing {
        let sample_rate = compute_sample_rate(timing.seconds_per_tick, timing.ticks_per_frame);
        let start_sec = 0.0;
        let end_key = (timing.range_end - timing.range_start).max(0);
        let end_sec = end_key as f32 / sample_rate;
        let header = match global_header {
            Some(global) => CafAnimationHeader {
                flags: global.flags,
                start_sec: global.start_sec,
                end_sec: global.end_sec,
                total_duration: global.total_duration,
                controller_count: global.controller_count,
                source: CafAnimationHeaderSource::GlobalAnimationHeaderCaf,
            },
            None => CafAnimationHeader {
                flags: 0,
                start_sec,
                end_sec,
                total_duration: end_sec - start_sec,
                controller_count,
                source: CafAnimationHeaderSource::Timing,
            },
        };
        return Ok((header, sample_rate));
    }

    Err(CafDecodeError::MissingHeader)
}

fn compute_sample_rate(seconds_per_tick: f32, ticks_per_frame: i32) -> f32 {
    1.0 / (seconds_per_tick * ticks_per_frame as f32)
}

fn decode_compressed_controller(
    controller: &ControllerCompressedChunk<'_>,
    header: &CafAnimationHeader,
    sample_rate: f32,
) -> Result<CafController, CafDecodeError> {
    let rotations = match (controller.rotation, controller.rotation_times) {
        (Some(values), Some(times)) => {
            let times = decode_times(times, sample_rate, header.start_sec)?;
            let values = decode_rotations(values, controller.controller_id)?;
            keys_with_times("rotation", controller.controller_id, values, times)?
                .into_iter()
                .map(|(time, value)| RotationKey { time, value })
                .collect()
        }
        (Some(_), None) => {
            return Err(CafDecodeError::MissingTrackTimes {
                controller_id: controller.controller_id,
                track: "rotation",
            });
        }
        (None, _) => Vec::new(),
    };

    let positions = match controller.position {
        Some(values) => {
            let time_track = match controller.position_keys_info {
                0 => controller.rotation_times,
                1 | 2 => controller.position_times,
                _ => None,
            }
            .ok_or(CafDecodeError::MissingTrackTimes {
                controller_id: controller.controller_id,
                track: "position",
            })?;
            let times = decode_times(time_track, sample_rate, header.start_sec)?;
            let values = decode_positions(values)?;
            keys_with_times("position", controller.controller_id, values, times)?
                .into_iter()
                .map(|(time, value)| PositionKey { time, value })
                .collect()
        }
        None => Vec::new(),
    };

    Ok(CafController {
        controller_id: controller.controller_id,
        flags: controller.flags,
        rotations,
        positions,
    })
}

fn keys_with_times<T>(
    track: &'static str,
    controller_id: u32,
    values: Vec<T>,
    times: Vec<f32>,
) -> Result<Vec<(f32, T)>, CafDecodeError> {
    if values.len() != times.len() {
        return Err(CafDecodeError::TrackCountMismatch {
            controller_id,
            track,
            value_count: values.len(),
            time_count: times.len(),
        });
    }
    if !times.windows(2).all(|pair| pair[0] < pair[1]) {
        return Err(CafDecodeError::NonIncreasingTimes { track });
    }
    Ok(times.into_iter().zip(values).collect())
}

fn decode_times(
    track: ControllerTrack<'_>,
    sample_rate: f32,
    start_sec: f32,
) -> Result<Vec<f32>, CafDecodeError> {
    let keys = match track.format {
        0 => decode_scalar_times::<4>(track, f32_from_le_bytes)?,
        1 => decode_scalar_times::<2>(track, |bytes| u16::from_le_bytes(bytes) as f32)?,
        2 => decode_scalar_times::<1>(track, |bytes| bytes[0] as f32)?,
        3 => decode_start_stop_times::<4>(track, f32_from_le_bytes)?,
        4 => decode_start_stop_times::<2>(track, |bytes| u16::from_le_bytes(bytes) as f32)?,
        5 => decode_start_stop_times::<1>(track, |bytes| bytes[0] as f32)?,
        6 => decode_bitset_times(track)?,
        _ => {
            return Err(CafDecodeError::UnsupportedTrackFormat {
                track: "key time",
                format: track.format,
            });
        }
    };
    Ok(keys
        .into_iter()
        .map(|key| key / sample_rate - start_sec)
        .collect())
}

fn decode_scalar_times<const N: usize>(
    track: ControllerTrack<'_>,
    read: impl Fn([u8; N]) -> f32,
) -> Result<Vec<f32>, CafDecodeError> {
    let expected = track
        .key_count
        .checked_mul(N)
        .ok_or(CafDecodeError::TruncatedTrack { track: "key time" })?;
    if track.data.len() < expected {
        return Err(CafDecodeError::TruncatedTrack { track: "key time" });
    }
    let mut times = Vec::with_capacity(track.key_count);
    for chunk in track.data[..expected].chunks_exact(N) {
        times.push(read(chunk.try_into().expect("chunk size matches const")));
    }
    Ok(times)
}

fn decode_start_stop_times<const N: usize>(
    track: ControllerTrack<'_>,
    read: impl Fn([u8; N]) -> f32,
) -> Result<Vec<f32>, CafDecodeError> {
    let expected = 2usize
        .checked_mul(N)
        .ok_or(CafDecodeError::TruncatedTrack { track: "key time" })?;
    if track.data.len() < expected {
        return Err(CafDecodeError::TruncatedTrack { track: "key time" });
    }
    let start = read(track.data[0..N].try_into().expect("slice length is const"));
    Ok((0..track.key_count)
        .map(|index| start + index as f32)
        .collect())
}

fn decode_bitset_times(track: ControllerTrack<'_>) -> Result<Vec<f32>, CafDecodeError> {
    if track.data.len() < 6 || !track.data.len().is_multiple_of(2) {
        return Err(CafDecodeError::InvalidBitsetTimeHeader);
    }
    let words = track
        .data
        .chunks_exact(2)
        .map(|bytes| u16::from_le_bytes(bytes.try_into().expect("word length")))
        .collect::<Vec<_>>();
    let start = words[0] as u32;
    let end = words[1] as u32;
    let declared = words[2] as usize;
    if declared != track.key_count || end < start {
        return Err(CafDecodeError::InvalidBitsetTimeHeader);
    }

    let mut offsets = Vec::new();
    for (word_index, word) in words[3..].iter().copied().enumerate() {
        for bit in 0..16 {
            if word & (1 << bit) != 0 {
                offsets.push((word_index as u32 * 16) + bit);
            }
        }
    }

    let span = end - start;
    let mut times = Vec::with_capacity(declared);
    times.push(start as f32);
    for offset in offsets {
        if offset != 0 && offset != span {
            times.push((start + offset) as f32);
        }
    }
    times.push(end as f32);
    times.sort_by(f32::total_cmp);
    times.dedup_by(|a, b| (*a - *b).abs() <= f32::EPSILON);

    if times.len() != declared {
        return Err(CafDecodeError::BitsetKeyCountMismatch {
            declared,
            decoded: times.len(),
        });
    }
    Ok(times)
}

fn decode_positions(track: ControllerTrack<'_>) -> Result<Vec<[f32; 3]>, CafDecodeError> {
    match track.format {
        0 | 2 => {
            let expected = track
                .key_count
                .checked_mul(12)
                .ok_or(CafDecodeError::TruncatedTrack { track: "position" })?;
            if track.data.len() < expected {
                return Err(CafDecodeError::TruncatedTrack { track: "position" });
            }
            let mut positions = Vec::with_capacity(track.key_count);
            for bytes in track.data[..expected].chunks_exact(12) {
                positions.push([
                    f32_from_le_bytes(bytes[0..4].try_into().expect("x")),
                    f32_from_le_bytes(bytes[4..8].try_into().expect("y")),
                    f32_from_le_bytes(bytes[8..12].try_into().expect("z")),
                ]);
            }
            Ok(positions)
        }
        _ => Err(CafDecodeError::UnsupportedTrackFormat {
            track: "position",
            format: track.format,
        }),
    }
}

fn decode_rotations(
    track: ControllerTrack<'_>,
    controller_id: u32,
) -> Result<Vec<[f32; 4]>, CafDecodeError> {
    let stride = match track.format {
        0 | 1 => 16,
        5 => 6,
        6 | 8 => 8,
        _ => {
            return Err(CafDecodeError::UnsupportedTrackFormat {
                track: "rotation",
                format: track.format,
            });
        }
    };
    let expected = track
        .key_count
        .checked_mul(stride)
        .ok_or(CafDecodeError::TruncatedTrack { track: "rotation" })?;
    if track.data.len() < expected {
        return Err(CafDecodeError::TruncatedTrack { track: "rotation" });
    }

    let mut rotations = Vec::with_capacity(track.key_count);
    for bytes in track.data[..expected].chunks_exact(stride) {
        let q = match track.format {
            0 | 1 => [
                f32_from_le_bytes(bytes[0..4].try_into().expect("x")),
                f32_from_le_bytes(bytes[4..8].try_into().expect("y")),
                f32_from_le_bytes(bytes[8..12].try_into().expect("z")),
                f32_from_le_bytes(bytes[12..16].try_into().expect("w")),
            ],
            5 => decode_small_tree_48(bytes.try_into().expect("48-bit quat")),
            6 => decode_small_tree_64(bytes.try_into().expect("64-bit quat"), false),
            8 => decode_small_tree_64(bytes.try_into().expect("64-bit ext quat"), true),
            _ => unreachable!("format checked above"),
        };
        rotations
            .push(normalize_quat(q).ok_or(CafDecodeError::InvalidQuaternion { controller_id })?);
    }
    Ok(rotations)
}

fn decode_small_tree_48(bytes: [u8; 6]) -> [f32; 4] {
    let m1 = u16::from_le_bytes([bytes[0], bytes[1]]) as u32;
    let m2 = u16::from_le_bytes([bytes[2], bytes[3]]) as u32;
    let m3 = u16::from_le_bytes([bytes[4], bytes[5]]) as u32;
    let index = (m3 >> 14) as usize;
    let packed = [
        m1 & 0x7fff,
        ((m1 >> 15) + (m2 << 1)) & 0x7fff,
        ((m2 >> 14) + (m3 << 2)) & 0x7fff,
    ];
    expand_missing_component(index, packed, [23_170.0; 3], [0.707_106_77; 3])
}

fn decode_small_tree_64(bytes: [u8; 8], extended: bool) -> [f32; 4] {
    let m1 = u32::from_le_bytes(bytes[0..4].try_into().expect("m1"));
    let m2 = u32::from_le_bytes(bytes[4..8].try_into().expect("m2"));
    let index = ((m2 >> 30) & 3) as usize;
    if extended {
        expand_missing_component(
            index,
            [
                m1 & 0x1f_ffff,
                ((m1 >> 21) + (m2 << 11)) & 0x1f_ffff,
                (m2 >> 10) & 0x0f_ffff,
            ],
            [1_482_909.0, 1_482_909.0, 741_454.0],
            [0.707_106_77, 0.707_106_77, 0.707_106_77],
        )
    } else {
        expand_missing_component(
            index,
            [
                m1 & 0x0f_ffff,
                ((m1 >> 20) + (m2 << 12)) & 0x0f_ffff,
                (m2 >> 8) & 0x0f_ffff,
            ],
            [741_454.0; 3],
            [0.707_106_77; 3],
        )
    }
}

fn expand_missing_component(
    missing: usize,
    packed: [u32; 3],
    max: [f32; 3],
    range: [f32; 3],
) -> [f32; 4] {
    let offsets = match missing {
        0 => [1, 2, 3],
        1 => [0, 2, 3],
        2 => [0, 1, 3],
        _ => [0, 1, 2],
    };
    let mut q = [0.0; 4];
    let mut sum = 0.0;
    for (slot, component) in offsets.into_iter().enumerate() {
        let value = (packed[slot] as f32 / max[slot]) - range[slot];
        q[component] = value;
        sum += value * value;
    }
    q[missing] = (1.0 - sum).max(0.0).sqrt();
    q
}

fn normalize_quat(mut q: [f32; 4]) -> Option<[f32; 4]> {
    let len_sq = q.iter().map(|v| v * v).sum::<f32>();
    if !len_sq.is_finite() || len_sq <= 0.0 {
        return None;
    }
    let len = len_sq.sqrt();
    for value in &mut q {
        *value /= len;
    }
    Some(q)
}

fn f32_from_le_bytes(bytes: [u8; 4]) -> f32 {
    f32::from_le_bytes(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_small_tree_48_identity_quaternion() {
        let bytes = pack_small_tree_48([0.0, 0.0, 0.0, 1.0]);

        let q = normalize_quat(decode_small_tree_48(bytes)).unwrap();

        assert_quat_close(q, [0.0, 0.0, 0.0, 1.0], 0.0001);
    }

    #[test]
    fn decodes_small_tree_64_identity_quaternion() {
        let bytes = pack_small_tree_64([0.0, 0.0, 0.0, 1.0], false);

        let q = normalize_quat(decode_small_tree_64(bytes, false)).unwrap();

        assert_quat_close(q, [0.0, 0.0, 0.0, 1.0], 0.000001);
    }

    #[test]
    fn decodes_small_tree_64_ext_identity_quaternion() {
        let bytes = pack_small_tree_64([0.0, 0.0, 0.0, 1.0], true);

        let q = normalize_quat(decode_small_tree_64(bytes, true)).unwrap();

        assert_quat_close(q, [0.0, 0.0, 0.0, 1.0], 0.000001);
    }

    #[test]
    fn decodes_start_stop_times_to_seconds() {
        let data = [30u16.to_le_bytes(), 33u16.to_le_bytes()].concat();
        let track = ControllerTrack {
            format: 4,
            key_count: 3,
            data: &data,
        };

        let times = decode_times(track, 30.0, 1.0).unwrap();

        assert!((times[0] - 0.0).abs() < 0.000001);
        assert!((times[1] - (1.0 / 30.0)).abs() < 0.000001);
        assert!((times[2] - (2.0 / 30.0)).abs() < 0.000001);
    }

    #[test]
    fn decodes_bitset_times_to_seconds() {
        let data = [
            10u16.to_le_bytes(),
            26u16.to_le_bytes(),
            3u16.to_le_bytes(),
            0x0009u16.to_le_bytes(),
            0x0001u16.to_le_bytes(),
        ]
        .concat();
        let track = ControllerTrack {
            format: 6,
            key_count: 3,
            data: &data,
        };

        let times = decode_times(track, 1.0, 0.0).unwrap();

        assert_eq!(times, [10.0, 13.0, 26.0]);
    }

    fn assert_quat_close(actual: [f32; 4], expected: [f32; 4], epsilon: f32) {
        for (actual, expected) in actual.into_iter().zip(expected) {
            assert!(
                (actual - expected).abs() <= epsilon,
                "actual {actual} expected {expected}"
            );
        }
    }

    fn pack_small_tree_48(q: [f32; 4]) -> [u8; 6] {
        let index = largest_abs_index(q);
        let mut value = 0u64;
        let mut shift = 0;
        for component in components_except(index) {
            let packed = ((q[component] + 0.707_106_77) * 23_170.0 + 0.5).floor() as u64;
            value |= packed << shift;
            shift += 15;
        }
        value |= (index as u64) << 46;
        [
            value as u8,
            (value >> 8) as u8,
            (value >> 16) as u8,
            (value >> 24) as u8,
            (value >> 32) as u8,
            (value >> 40) as u8,
        ]
    }

    fn pack_small_tree_64(q: [f32; 4], extended: bool) -> [u8; 8] {
        let index = largest_abs_index(q);
        let mut value = 0u64;
        let mut shift = 0;
        for (slot, component) in components_except(index).into_iter().enumerate() {
            let (range, max, bits) = if extended && slot < 2 {
                (0.707_106_77, 1_482_909.0, 21)
            } else {
                (0.707_106_77, 741_454.0, 20)
            };
            let packed = ((q[component] + range) * max + 0.5).floor() as u64;
            value |= packed << shift;
            shift += bits;
        }
        value |= (index as u64) << 62;
        value.to_le_bytes()
    }

    fn largest_abs_index(q: [f32; 4]) -> usize {
        q.into_iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.abs().total_cmp(&b.abs()))
            .map(|(index, _)| index)
            .unwrap()
    }

    fn components_except(index: usize) -> [usize; 3] {
        match index {
            0 => [1, 2, 3],
            1 => [0, 2, 3],
            2 => [0, 1, 3],
            _ => [0, 1, 2],
        }
    }
}
