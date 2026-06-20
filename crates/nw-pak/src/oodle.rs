//! Small Oodle LZ FFI surface used by pak read/write paths.

use std::ffi::c_void;
use std::fmt;
use std::os::raw::{c_int, c_uint};
use std::ptr;

use thiserror::Error;

const OODLELZ_FAILED: isize = 0;

type OoSinta = isize;
type OodleCompressorRaw = c_int;
type OodleCompressionLevelRaw = c_int;
type OodleFuzzSafe = c_uint;
type OodleCheckCrc = c_uint;
type OodleVerbosity = c_uint;
type OodleDecodeThreadPhase = c_uint;
type OodleDecompressCallbackRet = c_uint;
type OodleDecompressCallback = Option<
    unsafe extern "C" fn(
        userdata: *mut c_void,
        raw_buf: *const u8,
        raw_len: OoSinta,
        comp_buf: *const u8,
        comp_buffer_size: OoSinta,
        raw_done: OoSinta,
        comp_used: OoSinta,
    ) -> OodleDecompressCallbackRet,
>;

const COMPRESSOR_KRAKEN: OodleCompressorRaw = 8;
const COMPRESSOR_MERMAID: OodleCompressorRaw = 9;
const COMPRESSOR_SELKIE: OodleCompressorRaw = 11;
const COMPRESSOR_HYDRA: OodleCompressorRaw = 12;
const COMPRESSOR_LEVIATHAN: OodleCompressorRaw = 13;

const LEVEL_SUPER_FAST: OodleCompressionLevelRaw = 1;
const LEVEL_FAST: OodleCompressionLevelRaw = 3;
const LEVEL_NORMAL: OodleCompressionLevelRaw = 4;
const LEVEL_OPTIMAL1: OodleCompressionLevelRaw = 5;
const LEVEL_OPTIMAL2: OodleCompressionLevelRaw = 6;
const LEVEL_OPTIMAL5: OodleCompressionLevelRaw = 9;

const FUZZ_SAFE_YES: OodleFuzzSafe = 1;
const CHECK_CRC_NO: OodleCheckCrc = 0;
const VERBOSITY_NONE: OodleVerbosity = 0;
const DECODE_THREAD_PHASE_ALL: OodleDecodeThreadPhase = 3;

#[cfg(windows)]
unsafe extern "C" {
    fn OodleLZ_Compress(
        compressor: OodleCompressorRaw,
        raw_buf: *const c_void,
        raw_len: OoSinta,
        comp_buf: *mut c_void,
        level: OodleCompressionLevelRaw,
        options: *const c_void,
        dictionary_base: *const c_void,
        long_range_matcher: *const c_void,
        scratch_mem: *mut c_void,
        scratch_size: OoSinta,
    ) -> OoSinta;

    fn OodleLZ_Decompress(
        comp_buf: *const c_void,
        comp_buf_size: OoSinta,
        raw_buf: *mut c_void,
        raw_len: OoSinta,
        fuzz_safe: OodleFuzzSafe,
        check_crc: OodleCheckCrc,
        verbosity: OodleVerbosity,
        dec_buf_base: *mut c_void,
        dec_buf_size: OoSinta,
        callback: OodleDecompressCallback,
        callback_user_data: *mut c_void,
        decoder_memory: *mut c_void,
        decoder_memory_size: OoSinta,
        thread_phase: OodleDecodeThreadPhase,
    ) -> OoSinta;

    fn OodleLZ_GetCompressedBufferSizeNeeded(
        compressor: OodleCompressorRaw,
        raw_size: OoSinta,
    ) -> OoSinta;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Compressor {
    #[default]
    Kraken,
    Mermaid,
    Selkie,
    Hydra,
    Leviathan,
}

impl Compressor {
    const fn raw(self) -> OodleCompressorRaw {
        match self {
            Self::Kraken => COMPRESSOR_KRAKEN,
            Self::Mermaid => COMPRESSOR_MERMAID,
            Self::Selkie => COMPRESSOR_SELKIE,
            Self::Hydra => COMPRESSOR_HYDRA,
            Self::Leviathan => COMPRESSOR_LEVIATHAN,
        }
    }
}

impl fmt::Display for Compressor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Kraken => f.write_str("kraken"),
            Self::Mermaid => f.write_str("mermaid"),
            Self::Selkie => f.write_str("selkie"),
            Self::Hydra => f.write_str("hydra"),
            Self::Leviathan => f.write_str("leviathan"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Level {
    SuperFast,
    Fast,
    #[default]
    Normal,
    Optimal1,
    Optimal2,
    Optimal5,
}

impl Level {
    const fn raw(self) -> OodleCompressionLevelRaw {
        match self {
            Self::SuperFast => LEVEL_SUPER_FAST,
            Self::Fast => LEVEL_FAST,
            Self::Normal => LEVEL_NORMAL,
            Self::Optimal1 => LEVEL_OPTIMAL1,
            Self::Optimal2 => LEVEL_OPTIMAL2,
            Self::Optimal5 => LEVEL_OPTIMAL5,
        }
    }
}

impl fmt::Display for Level {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SuperFast => f.write_str("superfast"),
            Self::Fast => f.write_str("fast"),
            Self::Normal => f.write_str("normal"),
            Self::Optimal1 => f.write_str("optimal1"),
            Self::Optimal2 => f.write_str("optimal2"),
            Self::Optimal5 => f.write_str("optimal5"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Options {
    pub compressor: Compressor,
    pub level: Level,
}

impl Options {
    #[must_use]
    pub const fn new(compressor: Compressor, level: Level) -> Self {
        Self { compressor, level }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Codec {
    options: Options,
}

impl Codec {
    #[must_use]
    pub const fn new(options: Options) -> Self {
        Self { options }
    }

    #[must_use]
    pub const fn options(&self) -> Options {
        self.options
    }

    /// Compress `input` with the configured Oodle compressor and level.
    ///
    /// # Errors
    ///
    /// Returns an error if Oodle is unavailable, a buffer length does not fit
    /// the Oodle ABI, or the Oodle call reports failure.
    pub fn compress_to_vec(&self, input: &[u8]) -> Result<Vec<u8>, Error> {
        if input.is_empty() {
            return Ok(Vec::new());
        }

        compress_to_vec_impl(input, self.options)
    }

    /// Decompress `compressed` into `output`.
    ///
    /// # Errors
    ///
    /// Returns an error if Oodle is unavailable, a buffer length does not fit
    /// the Oodle ABI, or the Oodle call reports failure.
    pub fn decompress_into(&self, compressed: &[u8], output: &mut [u8]) -> Result<usize, Error> {
        if compressed.is_empty() && output.is_empty() {
            return Ok(0);
        }

        decompress_into_impl(compressed, output)
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("oodle is only available on Windows builds")]
    Unavailable,

    #[error("buffer length does not fit in the Oodle API")]
    BufferTooLarge,

    #[error("oodle reported an invalid compression bound")]
    InvalidCompressionBound,

    #[error("oodle compression failed")]
    CompressFailed,

    #[error("oodle decompression failed (expected {expected_size} bytes)")]
    DecompressFailed { expected_size: usize },
}

#[cfg(windows)]
fn compress_to_vec_impl(input: &[u8], options: Options) -> Result<Vec<u8>, Error> {
    let raw_len = checked_len(input.len())?;
    // SAFETY: Oodle reads no input buffer for this query. `raw_len` was
    // checked to fit the ABI integer type and the compressor id is one of
    // the supported constants exposed by `Compressor`.
    let bound = unsafe { OodleLZ_GetCompressedBufferSizeNeeded(options.compressor.raw(), raw_len) };
    if bound <= 0 {
        return Err(Error::InvalidCompressionBound);
    }

    let mut output = vec![0; usize::try_from(bound).map_err(|_| Error::BufferTooLarge)?];
    // SAFETY: `input` and `output` are valid for the byte counts passed to
    // Oodle, lengths were checked to fit `OoSinta`, and all optional Oodle
    // extension pointers are null as allowed by the API.
    let written = unsafe {
        OodleLZ_Compress(
            options.compressor.raw(),
            input.as_ptr().cast(),
            raw_len,
            output.as_mut_ptr().cast(),
            options.level.raw(),
            ptr::null(),
            ptr::null(),
            ptr::null(),
            ptr::null_mut(),
            0,
        )
    };

    if written == OODLELZ_FAILED {
        return Err(Error::CompressFailed);
    }

    output.truncate(usize::try_from(written).map_err(|_| Error::BufferTooLarge)?);
    Ok(output)
}

#[cfg(not(windows))]
fn compress_to_vec_impl(_input: &[u8], _options: Options) -> Result<Vec<u8>, Error> {
    Err(Error::Unavailable)
}

#[cfg(windows)]
fn decompress_into_impl(compressed: &[u8], output: &mut [u8]) -> Result<usize, Error> {
    let compressed_len = checked_len(compressed.len())?;
    let output_len = checked_len(output.len())?;
    // SAFETY: `compressed` and `output` are valid for the byte counts passed
    // to Oodle, lengths were checked to fit `OoSinta`, and optional buffers /
    // callbacks are null because this wrapper uses the simple single-call
    // decode path.
    let written = unsafe {
        OodleLZ_Decompress(
            compressed.as_ptr().cast(),
            compressed_len,
            output.as_mut_ptr().cast(),
            output_len,
            FUZZ_SAFE_YES,
            CHECK_CRC_NO,
            VERBOSITY_NONE,
            ptr::null_mut(),
            0,
            None,
            ptr::null_mut(),
            ptr::null_mut(),
            0,
            DECODE_THREAD_PHASE_ALL,
        )
    };

    if written == OODLELZ_FAILED {
        return Err(Error::DecompressFailed {
            expected_size: output.len(),
        });
    }

    usize::try_from(written).map_err(|_| Error::BufferTooLarge)
}

#[cfg(not(windows))]
fn decompress_into_impl(_compressed: &[u8], _output: &mut [u8]) -> Result<usize, Error> {
    Err(Error::Unavailable)
}

fn checked_len(len: usize) -> Result<OoSinta, Error> {
    OoSinta::try_from(len).map_err(|_| Error::BufferTooLarge)
}
