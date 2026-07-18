//! Legacy New World scene and region parsing boundaries.

pub mod region_manifest;

pub use region_manifest::{
    RegionManifestError, parse_region_chunks, parse_region_metadata, read_region_chunks,
    read_region_metadata,
};
