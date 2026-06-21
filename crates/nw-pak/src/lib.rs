//! NW pak archive reader (zip + zlib + Oodle + AZCS).
//!
//! Each `.pak` is read as a zip archive. Entries may be stored,
//! deflated, or Oodle-compressed with zip method `15`; payload bytes may
//! also be wrapped in an AZCS compressed-stream envelope. This crate
//! returns the final decompressed bytes and leaves format-specific
//! decoding to sibling crates.
//!
//! # Quick start
//!
//! ```no_run
//! use nw_pak::PakFile;
//!
//! let mut pak = PakFile::open("assets/game.pak")?;
//! let entry_count = pak.len();
//! let bytes = pak.read("textures/foo.dds")?;
//! assert!(entry_count > 0 || bytes.is_empty());
//! # Ok::<(), nw_pak::PakError>(())
//! ```
//!
pub mod archive;
pub mod azcs;
pub mod crypak;
mod crypak_format;
pub mod decompress;
pub mod extract;
pub mod oodle;
pub mod shape;

pub use archive::{
    EntryInfo, EntryIter, PakArchive, PakError, PakFile, PakFileMmap, PakMmapReader,
};
pub use azcs::{AZCS_SIGNATURE, AzcsError, AzcsHeader, AzcsId, is_azcs};
pub use decompress::{
    Compression, DecompressError, decompress_bytes_into, decompress_bytes_raw_into,
    decompress_zip_entry, decompress_zip_entry_into,
};
pub use extract::{
    PakExtractEntryFailure, PakExtractError, PakExtractFailures, PakExtractOptions,
    PakExtractReport,
};
pub use shape::{Report as PakShapeReport, Scanner as PakShapeScanner};
