//! CAF controller value decoding.
//!
//! This is intentionally part of `cry-chunk`: the raw CAF controller payloads
//! live here, and this module is the legacy-only value layer above those bytes.

use thiserror::Error;

use crate::{
    ChunkFile, ChunkFileError, ChunkPayload, ChunkPayloadError, ChunkType, ControllerChunk,
    ControllerCompressedChunk, ControllerDbChunk, ControllerTrack, GlobalAnimationHeaderCafChunk,
    MotionParametersChunk, QuatT, TimingChunk,
};

/// Decoded CAF animation with per-controller TRS tracks in CAF key seconds.
#[derive(Debug, Clone)]
pub struct CafAnimation {
    pub header: CafAnimationHeader,
    pub sample_rate: f32,
    pub controllers: Vec<CafController>,
    pub root_motion: Option<CafRootMotion>,
}

/// One CAF payload stored inside a Cry 0x0905 tracks database (`.dba`).
#[derive(Debug, Clone)]
pub struct DbaAnimation {
    pub file_path: String,
    pub animation: CafAnimation,
}

#[derive(Debug, Clone)]
pub struct DbaArchive {
    pub animations: Vec<DbaAnimation>,
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
    pub file_path: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CafRootMotion {
    pub start: QuatT,
    pub end: QuatT,
    pub velocity: [f32; 3],
    pub distance: f32,
    pub speed: f32,
    pub turn_speed: f32,
    pub turn_angle: f32,
    pub slope: f32,
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
    pub scales: Vec<ScaleKey>,
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScaleKey {
    pub time: f32,
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

#[derive(Debug, Error)]
pub enum DbaDecodeError {
    #[error(transparent)]
    ChunkFile(#[from] ChunkFileError),
    #[error(transparent)]
    ChunkPayload(#[from] ChunkPayloadError),
    #[error(transparent)]
    Caf(#[from] CafDecodeError),
    #[error("DBA has no ControllerDb 0x0905 chunk")]
    MissingControllerDatabase,
    #[error("DBA 0x0905 layout is invalid: {0}")]
    InvalidLayout(&'static str),
    #[error("DBA animation path is not UTF-8")]
    InvalidPath,
    #[error("DBA track index {index} is outside {kind} table length {count}")]
    TrackIndexOutOfRange {
        kind: &'static str,
        index: u32,
        count: usize,
    },
    #[error("DBA track {index} has no known {kind} compression format")]
    UnknownTrackFormat { kind: &'static str, index: usize },
}

impl DbaArchive {
    /// Decode a CryAnimation `.dba` using the exact ControllerDb 0x0905 layout
    /// consumed by Lumberyard `LoaderDBA.cpp::ReadController905`.
    pub fn parse(bytes: &[u8]) -> Result<Self, DbaDecodeError> {
        let file = ChunkFile::parse(bytes)?;
        for chunk in file.decoded_chunks() {
            if let ChunkPayload::Controller(ControllerChunk::ControllerDb(database)) =
                chunk?.payload
            {
                return decode_dba_0905(&database);
            }
        }
        Err(DbaDecodeError::MissingControllerDatabase)
    }
}

impl CafAnimation {
    /// Parse a CAF chunk file and decode compressed PQ and legacy PQLog controllers.
    ///
    /// The formulas mirror Lumberyard CryAnimation:
    /// `GlobalAnimationHeaderCAF::ReadMotionParameters` for sample rate,
    /// `ControllerPQ` key-time tracks, and `QuatQuantization` for quaternion
    /// unpacking. TCB/ControllerDb chunks fail the whole conversion rather than
    /// being silently discarded.
    pub fn parse(bytes: &[u8]) -> Result<Self, CafDecodeError> {
        let file = ChunkFile::parse(bytes)?;
        let mut global_header = None;
        let mut motion_parameters = None;
        let mut timing = None;
        let mut controller_chunks = Vec::new();

        for chunk in file.decoded_chunks() {
            let chunk = chunk?;
            match chunk.payload {
                ChunkPayload::GlobalAnimationHeaderCaf(chunk) => global_header = Some(chunk),
                ChunkPayload::MotionParameters(chunk) => motion_parameters = Some(chunk),
                ChunkPayload::Timing(chunk) => timing = Some(chunk),
                ChunkPayload::Controller(ControllerChunk::Compressed(controller)) => {
                    controller_chunks.push(ControllerChunk::Compressed(controller));
                }
                ChunkPayload::Controller(controller @ ControllerChunk::Tcb(_))
                | ChunkPayload::Controller(controller @ ControllerChunk::Uncompressed(_))
                | ChunkPayload::Controller(controller @ ControllerChunk::ControllerDb(_)) => {
                    controller_chunks.push(controller);
                }
                ChunkPayload::Controller(ControllerChunk::Empty0828) => {}
                _ => {}
            }
        }

        let root_motion = build_root_motion(global_header.as_ref(), motion_parameters.as_ref());
        let (header, sample_rate) = build_header(
            global_header,
            motion_parameters,
            timing,
            controller_chunks.len() as u32,
        )?;
        if !sample_rate.is_finite() || sample_rate <= 0.0 {
            return Err(CafDecodeError::InvalidSampleRate { sample_rate });
        }
        if !header.total_duration.is_finite() || header.total_duration < 0.0 {
            return Err(CafDecodeError::InvalidDuration {
                duration: header.total_duration,
            });
        }

        let mut controllers = Vec::with_capacity(controller_chunks.len());
        for controller in controller_chunks {
            match controller {
                ControllerChunk::Compressed(controller) => controllers.push(
                    decode_compressed_controller(&controller, &header, sample_rate)?,
                ),
                ControllerChunk::Uncompressed(controller) => controllers.push(
                    decode_uncompressed_controller(&controller, &header, sample_rate)?,
                ),
                ControllerChunk::Tcb(controller) => {
                    match controller.controller_type {
                        0 => controllers.push(CafController {
                            controller_id: controller.controller_id,
                            flags: controller.flags,
                            rotations: Vec::new(),
                            positions: Vec::new(),
                            scales: Vec::new(),
                        }),
                        // Lumberyard's CTRL_CRYBONE is the same 28-byte
                        // CryKeyPQLog layout as 0x0827/0x0830 controllers.
                        1 => controllers.push(decode_pq_log_controller(
                            controller.controller_id,
                            controller.flags,
                            controller.key_data,
                            controller.key_count,
                            &header,
                            sample_rate,
                        )?),
                        _ => {
                            return Err(CafDecodeError::UnsupportedControllerForm {
                                controller_id: controller.controller_id,
                                form: "non-skeletal TCB curve",
                            });
                        }
                    }
                }
                ControllerChunk::ControllerDb(_) => {
                    return Err(CafDecodeError::UnsupportedControllerForm {
                        controller_id: 0,
                        form: "ControllerDb",
                    });
                }
                ControllerChunk::Empty0828 => {}
            }
        }

        Ok(Self {
            header,
            sample_rate,
            controllers,
            root_motion,
        })
    }
}

#[derive(Debug, Clone)]
struct DbaMotionParameters {
    asset_flags: u32,
    ticks_per_frame: i32,
    seconds_per_tick: f32,
    start: i32,
    end: i32,
    move_speed: f32,
    turn_speed: f32,
    asset_turn: f32,
    distance: f32,
    slope: f32,
    start_location: QuatT,
    end_location: QuatT,
}

#[derive(Debug, Clone)]
struct DbaAnimationRecord {
    file_path: String,
    motion: DbaMotionParameters,
    controllers: Vec<DbaControllerInfo>,
}

#[derive(Debug, Clone, Copy)]
struct DbaControllerInfo {
    controller_id: u32,
    position_time: u32,
    position: u32,
    rotation_time: u32,
    rotation: u32,
}

fn decode_dba_0905(database: &ControllerDbChunk<'_>) -> Result<DbaArchive, DbaDecodeError> {
    const TIME_FORMATS: usize = 7;
    const VALUE_FORMATS: usize = 9;
    const MOTION_PARAMS_SIZE: usize = 132;
    const CONTROLLER_INFO_SIZE: usize = 20;

    let data = database.data;
    let mut cursor = 0usize;
    let time_sizes = dba_u16_array(data, &mut cursor, database.time_key_count)?;
    let time_formats = dba_u32_array(data, &mut cursor, TIME_FORMATS)?;
    let position_sizes = dba_u16_array(data, &mut cursor, database.position_key_count)?;
    let position_formats = dba_u32_array(data, &mut cursor, VALUE_FORMATS)?;
    let rotation_sizes = dba_u16_array(data, &mut cursor, database.rotation_key_count)?;
    let rotation_formats = dba_u32_array(data, &mut cursor, VALUE_FORMATS)?;
    let time_offsets = dba_i32_array(data, &mut cursor, database.time_key_count)?;
    let position_offsets = dba_i32_array(data, &mut cursor, database.position_key_count)?;
    let rotation_offsets = dba_i32_array(
        data,
        &mut cursor,
        database
            .rotation_key_count
            .checked_add(1)
            .ok_or(DbaDecodeError::InvalidLayout("rotation count overflow"))?,
    )?;
    let storage_length_marker = *rotation_offsets
        .last()
        .ok_or(DbaDecodeError::InvalidLayout(
            "missing rotation storage sentinel",
        ))?;
    let in_place = storage_length_marker < 0;
    let padding = if in_place {
        dba_u32(data, &mut cursor)? as usize
    } else {
        0
    };
    cursor = align_usize(cursor, 4)?;
    let track_length = usize::try_from(storage_length_marker.unsigned_abs())
        .map_err(|_| DbaDecodeError::InvalidLayout("track storage length overflow"))?;
    let track_length = align_usize(track_length, 4)?;
    let storage_start = if in_place { None } else { Some(cursor) };
    if !in_place {
        cursor = cursor
            .checked_add(track_length)
            .filter(|end| *end <= data.len())
            .ok_or(DbaDecodeError::InvalidLayout("track storage is truncated"))?;
    }
    cursor = cursor
        .checked_add(padding)
        .filter(|end| *end <= data.len())
        .ok_or(DbaDecodeError::InvalidLayout(
            "in-place padding is truncated",
        ))?;
    let animation_block_start = cursor;

    let mut records = Vec::with_capacity(database.animation_count);
    for _ in 0..database.animation_count {
        let path_length = dba_u16(data, &mut cursor)? as usize;
        let path_bytes = dba_bytes(data, &mut cursor, path_length)?;
        let file_path = str::from_utf8(path_bytes)
            .map_err(|_| DbaDecodeError::InvalidPath)?
            .to_owned();
        let motion_bytes = dba_bytes(data, &mut cursor, MOTION_PARAMS_SIZE)?;
        let motion = parse_dba_motion_parameters(motion_bytes)?;
        let foot_plant_count = dba_u16(data, &mut cursor)? as usize;
        let _foot_plants = dba_bytes(data, &mut cursor, foot_plant_count)?;
        let controller_count = dba_u16(data, &mut cursor)? as usize;
        let controller_offset = if in_place {
            let offset = dba_i32(data, &mut cursor)?;
            usize::try_from(offset)
                .ok()
                .and_then(|offset| animation_block_start.checked_add(offset))
                .ok_or(DbaDecodeError::InvalidLayout(
                    "negative in-place controller-info offset",
                ))?
        } else {
            cursor
        };
        let controller_bytes = controller_count.checked_mul(CONTROLLER_INFO_SIZE).ok_or(
            DbaDecodeError::InvalidLayout("controller-info size overflow"),
        )?;
        let mut info_cursor = controller_offset;
        let mut controllers = Vec::with_capacity(controller_count);
        for _ in 0..controller_count {
            controllers.push(DbaControllerInfo {
                controller_id: dba_u32(data, &mut info_cursor)?,
                position_time: dba_u32(data, &mut info_cursor)?,
                position: dba_u32(data, &mut info_cursor)?,
                rotation_time: dba_u32(data, &mut info_cursor)?,
                rotation: dba_u32(data, &mut info_cursor)?,
            });
        }
        if !in_place {
            cursor = cursor
                .checked_add(controller_bytes)
                .ok_or(DbaDecodeError::InvalidLayout(
                    "controller-info cursor overflow",
                ))?;
        }
        records.push(DbaAnimationRecord {
            file_path,
            motion,
            controllers,
        });
    }

    let mut starts = Vec::new();
    for offset in time_offsets
        .iter()
        .chain(position_offsets.iter())
        .chain(rotation_offsets[..database.rotation_key_count].iter())
    {
        starts.push(resolve_dba_track_offset(
            *offset,
            in_place,
            storage_start,
            data.len(),
        )?);
    }
    starts.sort_unstable();
    starts.dedup();
    let storage_end = if in_place {
        data.len()
    } else {
        storage_start
            .and_then(|start| start.checked_add(track_length))
            .ok_or(DbaDecodeError::InvalidLayout("track storage end overflow"))?
    };
    starts.push(storage_end);

    let track = |kind: &'static str,
                 index: u32,
                 sizes: &[u16],
                 formats: &[u32],
                 offsets: &[i32]|
     -> Result<Option<ControllerTrack<'_>>, DbaDecodeError> {
        if index == u32::MAX {
            return Ok(None);
        }
        let index = usize::try_from(index).map_err(|_| DbaDecodeError::TrackIndexOutOfRange {
            kind,
            index,
            count: sizes.len(),
        })?;
        let key_count = usize::from(*sizes.get(index).ok_or(
            DbaDecodeError::TrackIndexOutOfRange {
                kind,
                index: index as u32,
                count: sizes.len(),
            },
        )?);
        let format = dba_format_for_index(index, formats)
            .ok_or(DbaDecodeError::UnknownTrackFormat { kind, index })?;
        let start = resolve_dba_track_offset(offsets[index], in_place, storage_start, data.len())?;
        let end = starts
            .iter()
            .copied()
            .find(|candidate| *candidate > start)
            .ok_or(DbaDecodeError::InvalidLayout("track has no end boundary"))?;
        let bytes = data
            .get(start..end)
            .ok_or(DbaDecodeError::InvalidLayout("track points outside DBA"))?;
        Ok(Some(ControllerTrack {
            format: u8::try_from(format)
                .map_err(|_| DbaDecodeError::InvalidLayout("track format exceeds u8"))?,
            key_count,
            data: bytes,
        }))
    };

    let mut animations = Vec::with_capacity(records.len());
    for record in records {
        let sample_rate = compute_sample_rate(
            record.motion.seconds_per_tick,
            record.motion.ticks_per_frame,
        );
        if !sample_rate.is_finite() || sample_rate <= 0.0 {
            return Err(CafDecodeError::InvalidSampleRate { sample_rate }.into());
        }
        let mut start_key = record.motion.start;
        if record.motion.asset_flags & 0x001 != 0 {
            start_key += 1;
        }
        let start_sec = start_key as f32 / sample_rate;
        let end_sec = record.motion.end as f32 / sample_rate;
        let total_duration = (end_sec - start_sec).max(0.0);
        let header = CafAnimationHeader {
            flags: record.motion.asset_flags,
            start_sec,
            end_sec,
            total_duration,
            controller_count: record.controllers.len() as u32,
            source: CafAnimationHeaderSource::MotionParameters,
            file_path: Some(record.file_path.clone()),
        };
        let root_motion = Some(CafRootMotion {
            start: record.motion.start_location,
            end: record.motion.end_location,
            velocity: [0.0; 3],
            distance: record.motion.distance,
            speed: record.motion.move_speed,
            turn_speed: record.motion.turn_speed,
            turn_angle: record.motion.asset_turn,
            slope: record.motion.slope,
        });
        let mut controllers = Vec::with_capacity(record.controllers.len());
        for info in record.controllers {
            let rotation = track(
                "rotation",
                info.rotation,
                &rotation_sizes,
                &rotation_formats,
                &rotation_offsets,
            )?;
            let rotation_times = track(
                "time",
                info.rotation_time,
                &time_sizes,
                &time_formats,
                &time_offsets,
            )?;
            let position = track(
                "position",
                info.position,
                &position_sizes,
                &position_formats,
                &position_offsets,
            )?;
            let position_times = track(
                "time",
                info.position_time,
                &time_sizes,
                &time_formats,
                &time_offsets,
            )?;
            controllers.push(decode_compressed_controller(
                &ControllerCompressedChunk {
                    controller_id: info.controller_id,
                    flags: 0,
                    rotation,
                    rotation_times,
                    position,
                    position_times,
                    position_keys_info: 1,
                    tracks_aligned: true,
                },
                &header,
                sample_rate,
            )?);
        }
        animations.push(DbaAnimation {
            file_path: record.file_path,
            animation: CafAnimation {
                header,
                sample_rate,
                controllers,
                root_motion,
            },
        });
    }
    Ok(DbaArchive { animations })
}

fn parse_dba_motion_parameters(bytes: &[u8]) -> Result<DbaMotionParameters, DbaDecodeError> {
    let mut cursor = 0usize;
    let asset_flags = dba_u32(bytes, &mut cursor)?;
    let _compression = dba_u32(bytes, &mut cursor)?;
    let ticks_per_frame = dba_i32(bytes, &mut cursor)?;
    let seconds_per_tick = dba_f32(bytes, &mut cursor)?;
    let start = dba_i32(bytes, &mut cursor)?;
    let end = dba_i32(bytes, &mut cursor)?;
    let move_speed = dba_f32(bytes, &mut cursor)?;
    let turn_speed = dba_f32(bytes, &mut cursor)?;
    let asset_turn = dba_f32(bytes, &mut cursor)?;
    let distance = dba_f32(bytes, &mut cursor)?;
    let slope = dba_f32(bytes, &mut cursor)?;
    let start_location = dba_quat_t(bytes, &mut cursor)?;
    let end_location = dba_quat_t(bytes, &mut cursor)?;
    for _ in 0..8 {
        let _ = dba_f32(bytes, &mut cursor)?;
    }
    Ok(DbaMotionParameters {
        asset_flags,
        ticks_per_frame,
        seconds_per_tick,
        start,
        end,
        move_speed,
        turn_speed,
        asset_turn,
        distance,
        slope,
        start_location,
        end_location,
    })
}

fn dba_quat_t(bytes: &[u8], cursor: &mut usize) -> Result<QuatT, DbaDecodeError> {
    Ok(QuatT {
        rotation: [
            dba_f32(bytes, cursor)?,
            dba_f32(bytes, cursor)?,
            dba_f32(bytes, cursor)?,
            dba_f32(bytes, cursor)?,
        ],
        translation: [
            dba_f32(bytes, cursor)?,
            dba_f32(bytes, cursor)?,
            dba_f32(bytes, cursor)?,
        ],
    })
}

fn dba_format_for_index(index: usize, format_counts: &[u32]) -> Option<usize> {
    let mut end = 0usize;
    for (format, count) in format_counts.iter().copied().enumerate() {
        end = end.checked_add(count as usize)?;
        if index < end {
            return Some(format);
        }
    }
    None
}

fn resolve_dba_track_offset(
    offset: i32,
    in_place: bool,
    storage_start: Option<usize>,
    data_len: usize,
) -> Result<usize, DbaDecodeError> {
    let resolved = if in_place {
        data_len.checked_add_signed(offset as isize)
    } else {
        usize::try_from(offset)
            .ok()
            .and_then(|offset| storage_start?.checked_add(offset))
    };
    resolved
        .filter(|offset| *offset <= data_len)
        .ok_or(DbaDecodeError::InvalidLayout("track offset is outside DBA"))
}

fn align_usize(value: usize, alignment: usize) -> Result<usize, DbaDecodeError> {
    value
        .checked_add(alignment - 1)
        .map(|value| value / alignment * alignment)
        .ok_or(DbaDecodeError::InvalidLayout("alignment overflow"))
}

fn dba_bytes<'a>(
    bytes: &'a [u8],
    cursor: &mut usize,
    count: usize,
) -> Result<&'a [u8], DbaDecodeError> {
    let end = cursor
        .checked_add(count)
        .ok_or(DbaDecodeError::InvalidLayout("cursor overflow"))?;
    let value = bytes
        .get(*cursor..end)
        .ok_or(DbaDecodeError::InvalidLayout("truncated field"))?;
    *cursor = end;
    Ok(value)
}

fn dba_u16(bytes: &[u8], cursor: &mut usize) -> Result<u16, DbaDecodeError> {
    Ok(u16::from_le_bytes(
        dba_bytes(bytes, cursor, 2)?
            .try_into()
            .expect("two-byte field"),
    ))
}

fn dba_u32(bytes: &[u8], cursor: &mut usize) -> Result<u32, DbaDecodeError> {
    Ok(u32::from_le_bytes(
        dba_bytes(bytes, cursor, 4)?
            .try_into()
            .expect("four-byte field"),
    ))
}

fn dba_i32(bytes: &[u8], cursor: &mut usize) -> Result<i32, DbaDecodeError> {
    Ok(i32::from_le_bytes(
        dba_bytes(bytes, cursor, 4)?
            .try_into()
            .expect("four-byte field"),
    ))
}

fn dba_f32(bytes: &[u8], cursor: &mut usize) -> Result<f32, DbaDecodeError> {
    Ok(f32_from_le_bytes(
        dba_bytes(bytes, cursor, 4)?
            .try_into()
            .expect("four-byte field"),
    ))
}

fn dba_u16_array(
    bytes: &[u8],
    cursor: &mut usize,
    count: usize,
) -> Result<Vec<u16>, DbaDecodeError> {
    (0..count).map(|_| dba_u16(bytes, cursor)).collect()
}

fn dba_u32_array(
    bytes: &[u8],
    cursor: &mut usize,
    count: usize,
) -> Result<Vec<u32>, DbaDecodeError> {
    (0..count).map(|_| dba_u32(bytes, cursor)).collect()
}

fn dba_i32_array(
    bytes: &[u8],
    cursor: &mut usize,
    count: usize,
) -> Result<Vec<i32>, DbaDecodeError> {
    (0..count).map(|_| dba_i32(bytes, cursor)).collect()
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
                file_path: Some(global.file_path.to_string()),
            },
            None => CafAnimationHeader {
                flags: motion.asset_flags,
                start_sec,
                end_sec,
                total_duration,
                controller_count,
                source: CafAnimationHeaderSource::MotionParameters,
                file_path: None,
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
                file_path: Some(global.file_path.to_string()),
            },
            None => CafAnimationHeader {
                flags: 0,
                start_sec,
                end_sec,
                total_duration: end_sec - start_sec,
                controller_count,
                source: CafAnimationHeaderSource::Timing,
                file_path: None,
            },
        };
        return Ok((header, sample_rate));
    }

    Err(CafDecodeError::MissingHeader)
}

fn build_root_motion(
    global: Option<&GlobalAnimationHeaderCafChunk>,
    motion: Option<&MotionParametersChunk>,
) -> Option<CafRootMotion> {
    if let Some(global) = global {
        return Some(CafRootMotion {
            start: global.start_location,
            end: global.last_locator_key,
            velocity: global.velocity,
            distance: global.distance,
            speed: global.speed,
            turn_speed: motion.map_or(0.0, |value| value.turn_speed),
            turn_angle: motion.map_or(0.0, |value| value.asset_turn),
            slope: motion.map_or(0.0, |value| value.slope),
        });
    }
    motion.map(|motion| CafRootMotion {
        start: motion.start_location,
        end: motion.end_location,
        velocity: [0.0; 3],
        distance: motion.distance,
        speed: motion.move_speed,
        turn_speed: motion.turn_speed,
        turn_angle: motion.asset_turn,
        slope: motion.slope,
    })
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
        scales: Vec::new(),
    })
}

fn decode_uncompressed_controller(
    controller: &crate::ControllerUncompressedChunk<'_>,
    header: &CafAnimationHeader,
    sample_rate: f32,
) -> Result<CafController, CafDecodeError> {
    decode_pq_log_controller(
        controller.controller_id,
        controller.flags,
        controller.keys,
        controller.key_count,
        header,
        sample_rate,
    )
}

fn decode_pq_log_controller(
    controller_id: u32,
    flags: u32,
    keys: &[u8],
    key_count: usize,
    header: &CafAnimationHeader,
    sample_rate: f32,
) -> Result<CafController, CafDecodeError> {
    let mut rotations = Vec::with_capacity(key_count);
    let mut positions = Vec::with_capacity(key_count);
    for key in keys.chunks_exact(28) {
        let tick = i32::from_le_bytes(key[0..4].try_into().expect("PQLog key time"));
        let time = tick as f32 / sample_rate - header.start_sec;
        let position = [
            f32_from_le_bytes(key[4..8].try_into().expect("PQLog position x")),
            f32_from_le_bytes(key[8..12].try_into().expect("PQLog position y")),
            f32_from_le_bytes(key[12..16].try_into().expect("PQLog position z")),
        ];
        let log = [
            f32_from_le_bytes(key[16..20].try_into().expect("PQLog rotation x")),
            f32_from_le_bytes(key[20..24].try_into().expect("PQLog rotation y")),
            f32_from_le_bytes(key[24..28].try_into().expect("PQLog rotation z")),
        ];
        let rotation =
            inverse_quat_exp(log).ok_or(CafDecodeError::InvalidQuaternion { controller_id })?;
        positions.push(PositionKey {
            time,
            value: position,
        });
        rotations.push(RotationKey {
            time,
            value: rotation,
        });
    }
    validate_key_times("PQLog position", positions.iter().map(|key| key.time))?;
    validate_key_times("PQLog rotation", rotations.iter().map(|key| key.time))?;
    Ok(CafController {
        controller_id,
        flags,
        rotations,
        positions,
        scales: Vec::new(),
    })
}

fn inverse_quat_exp(log: [f32; 3]) -> Option<[f32; 4]> {
    let length_sq = log.iter().map(|value| value * value).sum::<f32>();
    if !length_sq.is_finite() {
        return None;
    }
    if length_sq <= f32::EPSILON {
        return Some([0.0, 0.0, 0.0, 1.0]);
    }
    let length = length_sq.sqrt();
    let scale = -length.sin() / length;
    normalize_quat([log[0] * scale, log[1] * scale, log[2] * scale, length.cos()])
}

fn validate_key_times(
    track: &'static str,
    times: impl IntoIterator<Item = f32>,
) -> Result<(), CafDecodeError> {
    let times = times.into_iter().collect::<Vec<_>>();
    if !times.windows(2).all(|pair| pair[0] < pair[1]) {
        return Err(CafDecodeError::NonIncreasingTimes { track });
    }
    Ok(())
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

    #[test]
    fn decodes_crybone_tcb_payload_as_pq_log() {
        let mut keys = Vec::new();
        for (tick, position) in [(30_i32, [1.0_f32, 2.0, 3.0]), (31, [4.0, 5.0, 6.0])] {
            keys.extend_from_slice(&tick.to_le_bytes());
            for component in position {
                keys.extend_from_slice(&component.to_le_bytes());
            }
            for component in [0.0_f32; 3] {
                keys.extend_from_slice(&component.to_le_bytes());
            }
        }
        let header = CafAnimationHeader {
            flags: 0,
            start_sec: 1.0,
            end_sec: 2.0,
            total_duration: 1.0,
            controller_count: 1,
            source: CafAnimationHeaderSource::Timing,
            file_path: None,
        };

        let controller = decode_pq_log_controller(7, 3, &keys, 2, &header, 30.0).unwrap();

        assert_eq!(controller.controller_id, 7);
        assert_eq!(controller.flags, 3);
        assert_eq!(controller.positions[0].value, [1.0, 2.0, 3.0]);
        assert!((controller.positions[0].time - 0.0).abs() < 0.000_001);
        assert_quat_close(
            controller.rotations[0].value,
            [0.0, 0.0, 0.0, 1.0],
            0.000_001,
        );
    }

    #[test]
    fn decodes_controller_db_0905_shared_tracks() {
        let mut data = Vec::new();

        push_u16(&mut data, 2); // one two-key time track
        push_format_counts(&mut data, 7, 0);
        push_u16(&mut data, 2); // one two-key position track
        push_format_counts(&mut data, 9, 0);
        push_u16(&mut data, 2); // one two-key rotation track
        push_format_counts(&mut data, 9, 0);

        push_i32(&mut data, 0); // time storage offset
        push_i32(&mut data, 8); // position storage offset
        push_i32(&mut data, 32); // rotation storage offset
        push_i32(&mut data, 64); // aligned storage length sentinel
        while !data.len().is_multiple_of(4) {
            data.push(0);
        }

        for time in [0.0_f32, 1.0] {
            push_f32(&mut data, time);
        }
        for position in [[1.0_f32, 2.0, 3.0], [4.0, 5.0, 6.0]] {
            for component in position {
                push_f32(&mut data, component);
            }
        }
        for rotation in [[0.0_f32, 0.0, 0.0, 1.0]; 2] {
            for component in rotation {
                push_f32(&mut data, component);
            }
        }

        let path = b"animations/test.caf";
        push_u16(&mut data, path.len() as u16);
        data.extend_from_slice(path);
        push_u32(&mut data, 0); // asset flags
        push_u32(&mut data, u32::MAX); // compression setting
        push_i32(&mut data, 1); // ticks per frame
        push_f32(&mut data, 1.0 / 30.0); // seconds per tick
        push_i32(&mut data, 0); // start key
        push_i32(&mut data, 1); // end key
        for value in [1.0_f32, 2.0, 3.0, 4.0, 5.0] {
            push_f32(&mut data, value);
        }
        for _ in 0..2 {
            for component in [0.0_f32, 0.0, 0.0, 1.0] {
                push_f32(&mut data, component);
            }
            for component in [0.0_f32; 3] {
                push_f32(&mut data, component);
            }
        }
        for _ in 0..8 {
            push_f32(&mut data, -1.0);
        }
        push_u16(&mut data, 0); // foot-plant bytes
        push_u16(&mut data, 1); // controller count
        push_u32(&mut data, 0x1234_5678);
        push_u32(&mut data, 0); // position time track
        push_u32(&mut data, 0); // position track
        push_u32(&mut data, 0); // rotation time track
        push_u32(&mut data, 0); // rotation track

        let archive = decode_dba_0905(&ControllerDbChunk {
            position_key_count: 1,
            rotation_key_count: 1,
            time_key_count: 1,
            animation_count: 1,
            data: &data,
        })
        .unwrap();

        let clip = &archive.animations[0];
        assert_eq!(clip.file_path, "animations/test.caf");
        assert_eq!(clip.animation.controllers.len(), 1);
        assert_eq!(clip.animation.controllers[0].controller_id, 0x1234_5678);
        assert_eq!(
            clip.animation.controllers[0].positions[1].value,
            [4.0, 5.0, 6.0]
        );
        assert_quat_close(
            clip.animation.controllers[0].rotations[1].value,
            [0.0, 0.0, 0.0, 1.0],
            0.000_001,
        );
    }

    fn push_format_counts(bytes: &mut Vec<u8>, count: usize, populated_format: usize) {
        for format in 0..count {
            push_u32(bytes, u32::from(format == populated_format));
        }
    }

    fn push_u16(bytes: &mut Vec<u8>, value: u16) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn push_u32(bytes: &mut Vec<u8>, value: u32) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn push_i32(bytes: &mut Vec<u8>, value: i32) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn push_f32(bytes: &mut Vec<u8>, value: f32) {
        bytes.extend_from_slice(&value.to_le_bytes());
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
