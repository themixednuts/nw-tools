//! Pak archive wrapper around `zip::ZipArchive`.
//!
//! One generic [`PakArchive<R>`] over any seekable reader, with a
//! [`PakFile`] type alias for the file-backed common case.
//!
//! # Performance
//!
//! - [`PakFileMmap::open_mmap`] memory-maps the archive — best
//!   for repeated / interactive scans where the OS file cache
//!   stays warm. ~10× faster than buffered I/O on warm cache,
//!   ~3× slower cold (Windows mmap fault-in is more expensive
//!   per page than batched `read()`).
//! - [`PakFile::open`] uses a 16 KiB-tuned [`BufReader`]. See
//!   [`PAK_READ_BUFFER_BYTES`]
//!   for why bigger isn't better.
//! - Entry metadata (name, sizes, compression) is collected once at
//!   open time into a `Vec<EntryInfo>` so [`PakArchive::entries`]
//!   yields fully-populated metadata without taking a mutable
//!   reference back to the underlying zip state.
//! - [`PakFile::extract_parallel`] decompresses N entries in
//!   parallel via rayon — each worker opens its own `PakFile`
//!   (fresh `File` + `ZipArchive`) so the slow oodle/zlib step
//!   runs concurrently with no contention. **Don't** wrap a
//!   single `PakArchive` in a `Mutex` and call from multiple
//!   threads: the mutex serializes the decompress step and you
//!   get zero actual parallelism.

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufReader, Cursor, Read, Seek};
use std::path::Path;
use std::str;

use rayon::prelude::*;
use thiserror::Error;
use zip::ZipArchive;
use zip::result::ZipError;

use crate::decompress::{
    Compression, DecompressError, decompress_bytes_into, decompress_bytes_raw_into,
};
use nw_filesystem::normalize_archive_path;

/// LFH (Local File Header) signature: `PK\x03\x04`.
const LFH_SIGNATURE: [u8; 4] = [0x50, 0x4B, 0x03, 0x04];
/// LFH fixed-size prefix length, before the variable-length name +
/// extra fields.
const LFH_FIXED_LEN: usize = 30;
const CENTRAL_DIRECTORY_SIGNATURE: u32 = 0x0201_4b50;
const CENTRAL_DIRECTORY_FIXED_LEN: usize = 46;
const EOCD_SIGNATURE: u32 = 0x0605_4b50;
const EOCD_FIXED_LEN: usize = 22;
const EOCD_MAX_SEARCH: usize = EOCD_FIXED_LEN + u16::MAX as usize;

/// `BufReader` capacity used by [`PakFile::open`].
///
/// `zip::ZipArchive` does many seek+small-read cycles while parsing
/// the central directory. Every seek invalidates `BufReader`'s buffer;
/// every refill reads up to `capacity` bytes regardless of the request
/// size. Large buffers can amplify reads heavily; 16 KiB is large
/// enough to amortize most central-directory reads without that penalty.
pub const PAK_READ_BUFFER_BYTES: usize = 16 * 1024;

#[derive(Debug, Error)]
pub enum PakError {
    #[error("io error: {0}")]
    Io(#[from] io::Error),

    #[error("zip error: {0}")]
    Zip(#[from] ZipError),

    #[error("decompress error: {0}")]
    Decompress(#[from] DecompressError),

    #[error("pak entry not found: {0}")]
    NotFound(String),

    #[error("pak entry index out of bounds: {0}")]
    IndexOutOfBounds(usize),

    #[error(
        "corrupt local-file-header for entry {entry:?} at offset {offset:#x}: bad PK signature"
    )]
    CorruptHeader { entry: String, offset: u64 },

    #[error("corrupt central directory: {field}")]
    CorruptCentralDirectory { field: &'static str },

    #[error("{field} is too large for this process: {value} bytes")]
    SizeTooLarge { field: &'static str, value: u64 },
}

/// File-backed pak archive — the common case.
pub type PakFile = PakArchive<BufReader<File>>;

/// Mmap-backed pak archive (open via [`PakFile::open_mmap`]).
pub type PakFileMmap = PakArchive<Cursor<memmap2::Mmap>>;

/// Shared mmap-backed pak reader for repeated runtime lookups.
///
/// Unlike [`PakArchive`], reads take `&self`: the central directory is
/// parsed once at open time, then each lookup indexes directly into the
/// mmap and decompresses the entry bytes from a slice. That makes the
/// reader cheap to share behind an `Arc` without serialising unrelated
/// asset loads on a mutex.
pub struct PakMmapReader {
    mmap: memmap2::Mmap,
    metadata: Vec<EntryMetadata>,
    by_name: HashMap<String, usize>,
}

impl PakMmapReader {
    /// Open a pak from a filesystem path via memory-mapped I/O.
    ///
    /// # Safety
    ///
    /// `memmap2::Mmap` is internally `unsafe`; if the file is
    /// concurrently truncated or modified, reads may observe torn data.
    /// Pak files should be treated as immutable while mapped.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, PakError> {
        let file = File::open(path.as_ref())?;
        // SAFETY: callers use pak files as immutable inputs. If another
        // process truncates or mutates the file concurrently, mmap reads
        // could observe torn data.
        let mmap = unsafe { memmap2::Mmap::map(&file)? };
        let bytes: &[u8] = &mmap;
        let mut archive = ZipArchive::new(Cursor::new(bytes))?;
        let mut metadata = collect_metadata(&mut archive)?;
        drop(archive);
        patch_metadata_from_central_directory(bytes, &mut metadata)?;

        let mut by_name = HashMap::with_capacity(metadata.len());
        for (index, entry) in metadata.iter().enumerate() {
            by_name.entry(entry.lookup_name.clone()).or_insert(index);
        }

        Ok(Self {
            mmap,
            metadata,
            by_name,
        })
    }

    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.metadata.len()
    }

    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.metadata.is_empty()
    }

    #[inline]
    #[must_use]
    pub fn entries(&self) -> EntryIter<'_> {
        EntryIter {
            inner: self.metadata.iter(),
        }
    }

    #[inline]
    #[must_use]
    pub fn entry(&self, name: &str) -> Option<EntryInfo<'_>> {
        let lookup_name = lookup_name(name);
        self.by_name
            .get(&lookup_name)
            .and_then(|index| self.metadata.get(*index))
            .map(EntryMetadata::view)
    }

    #[inline]
    #[must_use]
    pub fn entry_by_index(&self, idx: usize) -> Option<EntryInfo<'_>> {
        self.metadata.get(idx).map(EntryMetadata::view)
    }

    pub fn read(&self, name: &str) -> Result<Vec<u8>, PakError> {
        let lookup_name = lookup_name(name);
        let index = self
            .by_name
            .get(&lookup_name)
            .copied()
            .ok_or_else(|| PakError::NotFound(name.to_string()))?;
        self.read_by_index(index)
    }

    pub fn read_by_index(&self, idx: usize) -> Result<Vec<u8>, PakError> {
        let entry = self
            .metadata
            .get(idx)
            .ok_or(PakError::IndexOutOfBounds(idx))?;
        decompress_entry_from_slice(&self.mmap, entry)
    }

    /// Read + decompress a named entry without peeling an inner AZCS
    /// wrapper.
    pub fn read_wrapped(&self, name: &str) -> Result<Vec<u8>, PakError> {
        let lookup_name = lookup_name(name);
        let index = self
            .by_name
            .get(&lookup_name)
            .copied()
            .ok_or_else(|| PakError::NotFound(name.to_string()))?;
        self.read_wrapped_by_index(index)
    }

    /// Read + decompress an entry by index without peeling an inner
    /// AZCS wrapper.
    pub fn read_wrapped_by_index(&self, idx: usize) -> Result<Vec<u8>, PakError> {
        let entry = self
            .metadata
            .get(idx)
            .ok_or(PakError::IndexOutOfBounds(idx))?;
        decompress_entry_from_slice_raw(&self.mmap, entry)
    }
}

/// Pak archive over any seekable reader.
///
/// Entry metadata is pre-collected at open time so [`PakArchive::entries`]
/// (and any other metadata-only operation) takes `&self` and is
/// cheap to share. Reading entry *bytes* requires `&mut self`
/// because zip's underlying reader needs a mutable position.
///
/// For parallel decompression, **don't** wrap a single `PakArchive`
/// in a `Mutex` — that serializes the slow decompress step. Instead
/// open one [`PakFile`] per rayon worker via
/// [`PakFile::extract_parallel`] (or use [`map_init`][1] to roll
/// your own).
///
/// [1]: https://docs.rs/rayon/latest/rayon/iter/trait.IndexedParallelIterator.html#method.map_init
pub struct PakArchive<R: Read + Seek> {
    inner: ZipArchive<R>,
    metadata: Vec<EntryMetadata>,
}

impl PakFile {
    /// Open a pak from a filesystem path, buffered.
    ///
    /// Uses a [`PAK_READ_BUFFER_BYTES`]-sized (16 KiB) [`BufReader`].
    /// See the constant's docs for the bisect table that picked
    /// this size — bigger is dramatically worse here because
    /// `zip::ZipArchive` does small seek+read cycles that pay
    /// the full BufReader capacity in I/O per refill.
    ///
    /// For interactive workflows that re-scan paks repeatedly,
    /// prefer [`PakFileMmap::open_mmap`] — it's ~10× faster on
    /// warm cache, at the cost of being ~3× slower on the first
    /// cold run.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, PakError> {
        let file = File::open(path.as_ref())?;
        Self::from_reader(BufReader::with_capacity(PAK_READ_BUFFER_BYTES, file))
    }
}

impl PakFileMmap {
    /// Open a pak from a filesystem path via memory-mapped I/O.
    ///
    /// # Safety
    ///
    /// `memmap2::Mmap` is internally `unsafe`; if the file is
    /// concurrently truncated or modified, reads may observe torn
    /// data. Pak files should be treated as immutable while mapped.
    pub fn open_mmap(path: impl AsRef<Path>) -> Result<Self, PakError> {
        let file = File::open(path.as_ref())?;
        // SAFETY: callers use pak files as immutable inputs. If another
        // process truncates or mutates the file concurrently, mmap reads
        // could observe torn data.
        let mmap = unsafe { memmap2::Mmap::map(&file)? };
        Self::from_reader(Cursor::new(mmap))
    }
}

impl<R: Read + Seek> PakArchive<R> {
    /// Wrap an arbitrary seekable reader (in-memory pak, mmap, etc.).
    pub fn from_reader(reader: R) -> Result<Self, PakError> {
        let mut zip = ZipArchive::new(reader)?;
        let metadata = collect_metadata(&mut zip)?;
        Ok(Self {
            inner: zip,
            metadata,
        })
    }

    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.metadata.len()
    }

    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.metadata.is_empty()
    }

    /// Cheap, borrow-free metadata for every entry in the pak.
    #[inline]
    #[must_use]
    pub fn entries(&self) -> EntryIter<'_> {
        EntryIter {
            inner: self.metadata.iter(),
        }
    }

    /// Look up entry metadata by name.
    #[inline]
    #[must_use]
    pub fn entry(&self, name: &str) -> Option<EntryInfo<'_>> {
        let lookup_name = lookup_name(name);
        self.metadata
            .iter()
            .find(|m| m.lookup_name == lookup_name)
            .map(EntryMetadata::view)
    }

    /// Look up entry metadata by index.
    #[inline]
    #[must_use]
    pub fn entry_by_index(&self, idx: usize) -> Option<EntryInfo<'_>> {
        self.metadata.get(idx).map(EntryMetadata::view)
    }

    /// Read + decompress a named entry into a fresh `Vec<u8>`.
    ///
    /// Takes `&mut self` because the zip crate's underlying reader
    /// position mutates per call. For parallel reads across many
    /// entries, see [`PakFile::extract_parallel`].
    pub fn read(&mut self, name: &str) -> Result<Vec<u8>, PakError> {
        let lookup_name = lookup_name(name);
        let idx = self
            .metadata
            .iter()
            .position(|m| m.lookup_name == lookup_name)
            .ok_or_else(|| PakError::NotFound(name.to_string()))?;
        self.read_by_index(idx)
    }

    /// Read + decompress a named entry into a caller-supplied
    /// buffer (cleared first). Returns the resulting length.
    pub fn read_into(&mut self, name: &str, buf: &mut Vec<u8>) -> Result<usize, PakError> {
        let lookup_name = lookup_name(name);
        let idx = self
            .metadata
            .iter()
            .position(|m| m.lookup_name == lookup_name)
            .ok_or_else(|| PakError::NotFound(name.to_string()))?;
        self.read_by_index_into(idx, buf)
    }

    /// Read + decompress an entry by index.
    ///
    /// Uses the same raw-read + `decompress_bytes_into` path as
    /// [`PakFile::extract_parallel`] — `by_index_raw` gives us the
    /// compressed bytes without zip's compression-method
    /// validation gate (which rejects the Oodle method id).
    pub fn read_by_index(&mut self, idx: usize) -> Result<Vec<u8>, PakError> {
        let meta = self
            .metadata
            .get(idx)
            .ok_or(PakError::IndexOutOfBounds(idx))?
            .clone();
        let mut out = Vec::with_capacity(usize_from_u64(
            "uncompressed entry size",
            meta.uncompressed_size,
        )?);
        self.read_by_index_into_inner(&meta, &mut out)?;
        Ok(out)
    }

    /// Read + decompress an entry by index into a caller-supplied
    /// buffer (cleared first). Returns the resulting length.
    pub fn read_by_index_into(&mut self, idx: usize, buf: &mut Vec<u8>) -> Result<usize, PakError> {
        let meta = self
            .metadata
            .get(idx)
            .ok_or(PakError::IndexOutOfBounds(idx))?
            .clone();
        self.read_by_index_into_inner(&meta, buf)?;
        Ok(buf.len())
    }

    /// Internal: do the raw read + decompress for a metadata entry.
    fn read_by_index_into_inner(
        &mut self,
        meta: &EntryMetadata,
        buf: &mut Vec<u8>,
    ) -> Result<(), PakError> {
        // `by_index_raw` returns a ZipFile that yields the
        // **compressed** bytes verbatim, with no decompression
        // attempted — so zip's "method not supported" check
        // doesn't fire for our Oodle entries.
        let mut entry = self.inner.by_index_raw(meta.index)?;
        let mut compressed = Vec::with_capacity(usize_from_u64(
            "compressed entry size",
            meta.compressed_size,
        )?);
        entry.read_to_end(&mut compressed)?;
        decompress_bytes_into(
            meta.compression,
            &compressed,
            usize_from_u64("uncompressed entry size", meta.uncompressed_size)?,
            buf,
        )?;
        Ok(())
    }
}

impl PakFile {
    /// Read + decompress many named entries in parallel.
    ///
    /// **Architecture (zero per-worker syscalls):**
    /// 1. mmap the whole pak file once on the calling thread.
    /// 2. Parse the central directory once (single `ZipArchive::new`
    ///    over the mmap'd bytes) to capture each entry's
    ///    `(header_offset, compressed_size, compression)`.
    /// 3. Each rayon worker receives a `&[u8]` slice into the
    ///    mmap — *no* file handle, *no* seek state, *no* syscalls.
    ///    The worker:
    ///    - reads the 30-byte LFH directly from the slice +
    ///      validates the `PK\x03\x04` signature
    ///    - slices out `compressed_size` bytes at `data_start`
    ///    - hands them to [`decompress_bytes_into`] (same vetted
    ///      `flate2` / Oodle / AZCS path as the single-
    ///      thread reads)
    ///
    /// vs. the per-worker `File::open` approach this also saves:
    /// - per-worker file-descriptor allocation
    /// - per-entry seek+read syscalls (replaced by direct slice
    ///   indexing into the kernel-paged mmap)
    ///
    /// On warm cache the inner read+decompress loop is essentially
    /// memcpy-bound rather than syscall-bound.
    ///
    /// **Safety:** offsets and sizes come from the zip crate's own
    /// vetted central-directory parse done in step 2. The mmap
    /// stays alive for the entire `par_iter` so worker slices
    /// remain valid. We validate the LFH signature on every read
    /// before trusting the variable-length fields.
    pub fn extract_parallel(
        path: impl AsRef<Path>,
        names: &[&str],
    ) -> Vec<Result<Vec<u8>, PakError>> {
        match extract_parallel_inner(path.as_ref(), names) {
            Ok(results) => results,
            Err(e) => names.iter().map(|_| Err(clone_error(&e))).collect(),
        }
    }
}

fn extract_parallel_inner(
    path: &Path,
    names: &[&str],
) -> Result<Vec<Result<Vec<u8>, PakError>>, PakError> {
    // Step 1: mmap the file. SAFETY: same caveats as
    // `PakFileMmap::open_mmap`; callers should treat pak files as immutable.
    let file = File::open(path)?;
    // SAFETY: callers use pak files as immutable inputs. If another process
    // truncates or mutates the file concurrently, mmap reads could observe
    // torn data.
    let mmap = unsafe { memmap2::Mmap::map(&file)? };
    let bytes: &[u8] = &mmap;

    // Step 2: parse CD once.
    let mut archive = ZipArchive::new(Cursor::new(bytes))?;
    let metadata = collect_metadata(&mut archive)?;
    drop(archive);

    // Resolve user-supplied names -> metadata snapshot.
    let resolved: Vec<Result<EntryMetadata, PakError>> = names
        .iter()
        .map(|&name| {
            let lookup_name = lookup_name(name);
            metadata
                .iter()
                .find(|m| m.lookup_name == lookup_name)
                .cloned()
                .ok_or_else(|| PakError::NotFound(name.to_string()))
        })
        .collect();

    // Step 3: parallel decompress, workers index directly into
    // the mmap. The closure captures `bytes` (a `&[u8]` borrowed
    // from `mmap`); the mmap stays alive on this stack frame for
    // the duration of the par_iter.
    let results: Vec<Result<Vec<u8>, PakError>> = resolved
        .into_par_iter()
        .map(|entry_result| {
            let entry = entry_result?;
            decompress_entry_from_slice(bytes, &entry)
        })
        .collect();

    // Explicit drop to make the lifetime relationship loud.
    drop(mmap);
    Ok(results)
}

/// Locate + decompress one entry directly from a mmap'd byte
/// slice. Pure memory access — no syscalls in this function.
fn decompress_entry_from_slice(bytes: &[u8], entry: &EntryMetadata) -> Result<Vec<u8>, PakError> {
    let mut out = decompress_entry_from_slice_raw(bytes, entry)?;
    if crate::azcs::is_azcs(&out) {
        let outer = std::mem::take(&mut out);
        let mut cursor = Cursor::new(outer.as_slice());
        let mut reader = crate::azcs::decompress(&mut cursor).map_err(DecompressError::from)?;
        reader.read_to_end(&mut out)?;
    }
    Ok(out)
}

fn decompress_entry_from_slice_raw(
    bytes: &[u8],
    entry: &EntryMetadata,
) -> Result<Vec<u8>, PakError> {
    let header_off = usize_from_u64("local header offset", entry.header_offset)?;
    let lfh = bytes
        .get(header_off..header_off + LFH_FIXED_LEN)
        .ok_or_else(|| PakError::CorruptHeader {
            entry: entry.name.clone(),
            offset: entry.header_offset,
        })?;

    if lfh[0..4] != LFH_SIGNATURE {
        return Err(PakError::CorruptHeader {
            entry: entry.name.clone(),
            offset: entry.header_offset,
        });
    }

    let name_len = u16::from_le_bytes([lfh[26], lfh[27]]) as usize;
    let extra_len = u16::from_le_bytes([lfh[28], lfh[29]]) as usize;
    let data_start = header_off + LFH_FIXED_LEN + name_len + extra_len;
    let data_end = data_start
        .checked_add(usize_from_u64(
            "compressed entry size",
            entry.compressed_size,
        )?)
        .ok_or_else(|| PakError::CorruptHeader {
            entry: entry.name.clone(),
            offset: entry.header_offset,
        })?;

    let compressed = bytes
        .get(data_start..data_end)
        .ok_or_else(|| PakError::CorruptHeader {
            entry: entry.name.clone(),
            offset: entry.header_offset,
        })?;

    let expected_size = usize_from_u64("uncompressed entry size", entry.uncompressed_size)?;
    let mut out = Vec::with_capacity(expected_size);
    decompress_bytes_raw_into(entry.compression, compressed, expected_size, &mut out)?;
    Ok(out)
}

/// Best-effort clone of a `PakError` for fan-out reporting.
/// `io::Error` isn't `Clone`, so we project to a string-cloned
/// equivalent. Loses the original `kind()`, but only used when
/// the open itself failed — every entry in the result vec gets
/// the same surface error.
fn clone_error(err: &PakError) -> PakError {
    PakError::Io(io::Error::other(err.to_string()))
}

// === entry metadata ===

#[derive(Debug, Clone)]
struct EntryMetadata {
    name: String,
    lookup_name: String,
    index: usize,
    uncompressed_size: u64,
    compressed_size: u64,
    compression: Compression,
    method_id: u16,
    modified_time: u16,
    modified_date: u16,
    central_extra: Vec<u8>,
    /// Byte offset of this entry's Local File Header in the
    /// underlying file. `data_start = header_offset + 30 +
    /// LFH.name_len + LFH.extra_len`.
    header_offset: u64,
}

impl EntryMetadata {
    #[inline]
    fn view(&self) -> EntryInfo<'_> {
        EntryInfo {
            name: &self.name,
            index: self.index,
            uncompressed_size: self.uncompressed_size,
            compressed_size: self.compressed_size,
            compression: self.compression,
            method_id: self.method_id,
            modified_time: self.modified_time,
            modified_date: self.modified_date,
            central_extra: &self.central_extra,
        }
    }
}

fn collect_metadata<R: Read + Seek>(
    zip: &mut ZipArchive<R>,
) -> Result<Vec<EntryMetadata>, PakError> {
    let len = zip.len();
    let mut out = Vec::with_capacity(len);
    for i in 0..len {
        let entry = zip.by_index_raw(i)?;
        let compression = Compression::from_zip_method(entry.compression());
        out.push(EntryMetadata {
            name: entry.name().to_owned(),
            lookup_name: lookup_name(entry.name()),
            index: i,
            uncompressed_size: entry.size(),
            compressed_size: entry.compressed_size(),
            compression,
            method_id: compression.method_id(),
            modified_time: 0,
            modified_date: 0,
            central_extra: Vec::new(),
            // header_start() comes from the central-directory record
            // — no extra I/O required to read it.
            header_offset: entry.header_start(),
        });
    }
    Ok(out)
}

fn patch_metadata_from_central_directory(
    bytes: &[u8],
    metadata: &mut [EntryMetadata],
) -> Result<(), PakError> {
    let Some(eocd) = find_eocd(bytes) else {
        return Err(PakError::CorruptCentralDirectory {
            field: "end-of-central-directory not found",
        });
    };

    let entry_count = read_u16(bytes, eocd + 10, "EOCD entry count")? as usize;
    let cd_offset = read_u32(bytes, eocd + 16, "EOCD central-directory offset")? as usize;
    let mut pos = cd_offset;

    for index in 0..entry_count {
        if read_u32(bytes, pos, "CDR signature")? != CENTRAL_DIRECTORY_SIGNATURE {
            return Err(PakError::CorruptCentralDirectory {
                field: "bad central-directory signature",
            });
        }

        let method_id = read_u16(bytes, pos + 10, "CDR method")?;
        let modified_time = read_u16(bytes, pos + 12, "CDR modified time")?;
        let modified_date = read_u16(bytes, pos + 14, "CDR modified date")?;
        let name_len = read_u16(bytes, pos + 28, "CDR name length")? as usize;
        let extra_len = read_u16(bytes, pos + 30, "CDR extra length")? as usize;
        let comment_len = read_u16(bytes, pos + 32, "CDR comment length")? as usize;
        let local_offset = read_u32(bytes, pos + 42, "CDR local-header offset")?;

        let name_start = pos + CENTRAL_DIRECTORY_FIXED_LEN;
        let name_end = name_start
            .checked_add(name_len)
            .ok_or(PakError::CorruptCentralDirectory { field: "CDR name" })?;
        let extra_end = name_end
            .checked_add(extra_len)
            .ok_or(PakError::CorruptCentralDirectory { field: "CDR extra" })?;
        let next = extra_end
            .checked_add(comment_len)
            .ok_or(PakError::CorruptCentralDirectory {
                field: "CDR comment",
            })?;
        let name = bytes
            .get(name_start..name_end)
            .ok_or(PakError::CorruptCentralDirectory { field: "CDR name" })?;
        let extra = bytes
            .get(name_end..extra_end)
            .ok_or(PakError::CorruptCentralDirectory { field: "CDR extra" })?;

        let target = metadata
            .get_mut(index)
            .ok_or(PakError::CorruptCentralDirectory {
                field: "CDR entry count mismatch",
            })?;
        if let Ok(name) = str::from_utf8(name) {
            name.clone_into(&mut target.name);
            target.lookup_name = lookup_name(name);
        }
        target.method_id = method_id;
        target.compression = Compression::from_method_id(method_id);
        target.modified_time = modified_time;
        target.modified_date = modified_date;
        target.header_offset = u64::from(local_offset);
        target.central_extra = extra.to_vec();

        pos = next;
    }

    Ok(())
}

fn find_eocd(bytes: &[u8]) -> Option<usize> {
    if bytes.len() < EOCD_FIXED_LEN {
        return None;
    }
    let search_start = bytes.len().saturating_sub(EOCD_MAX_SEARCH);
    let last = bytes.len() - EOCD_FIXED_LEN;
    (search_start..=last)
        .rev()
        .find(|&pos| read_u32_lossy(bytes, pos) == Some(EOCD_SIGNATURE))
}

fn read_u16(bytes: &[u8], offset: usize, field: &'static str) -> Result<u16, PakError> {
    let raw = bytes
        .get(offset..offset + 2)
        .ok_or(PakError::CorruptCentralDirectory { field })?;
    Ok(u16::from_le_bytes([raw[0], raw[1]]))
}

fn read_u32(bytes: &[u8], offset: usize, field: &'static str) -> Result<u32, PakError> {
    read_u32_lossy(bytes, offset).ok_or(PakError::CorruptCentralDirectory { field })
}

fn read_u32_lossy(bytes: &[u8], offset: usize) -> Option<u32> {
    let raw = bytes.get(offset..offset + 4)?;
    Some(u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]))
}

fn lookup_name(name: &str) -> String {
    normalize_archive_path(name)
}

fn usize_from_u64(field: &'static str, value: u64) -> Result<usize, PakError> {
    usize::try_from(value).map_err(|_| PakError::SizeTooLarge { field, value })
}

/// Borrowed view into a single pak entry's metadata.
///
/// Cheap to copy; doesn't hold any open zip handle.
#[derive(Debug, Clone, Copy)]
pub struct EntryInfo<'a> {
    name: &'a str,
    index: usize,
    uncompressed_size: u64,
    compressed_size: u64,
    compression: Compression,
    method_id: u16,
    modified_time: u16,
    modified_date: u16,
    central_extra: &'a [u8],
}

impl<'a> EntryInfo<'a> {
    #[inline]
    #[must_use]
    pub const fn name(&self) -> &'a str {
        self.name
    }

    #[inline]
    #[must_use]
    pub const fn index(&self) -> usize {
        self.index
    }

    #[inline]
    #[must_use]
    pub const fn uncompressed_size(&self) -> u64 {
        self.uncompressed_size
    }

    #[inline]
    #[must_use]
    pub const fn compressed_size(&self) -> u64 {
        self.compressed_size
    }

    #[inline]
    #[must_use]
    pub const fn compression(&self) -> Compression {
        self.compression
    }

    #[inline]
    #[must_use]
    pub const fn method_id(&self) -> u16 {
        self.method_id
    }

    #[inline]
    #[must_use]
    pub const fn modified_time(&self) -> u16 {
        self.modified_time
    }

    #[inline]
    #[must_use]
    pub const fn modified_date(&self) -> u16 {
        self.modified_date
    }

    #[inline]
    #[must_use]
    pub const fn central_extra(&self) -> &'a [u8] {
        self.central_extra
    }
}

/// Iterator yielded by [`PakArchive::entries`].
pub struct EntryIter<'a> {
    inner: std::slice::Iter<'a, EntryMetadata>,
}

impl<'a> Iterator for EntryIter<'a> {
    type Item = EntryInfo<'a>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(EntryMetadata::view)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl ExactSizeIterator for EntryIter<'_> {}
impl DoubleEndedIterator for EntryIter<'_> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.inner.next_back().map(EntryMetadata::view)
    }
}
