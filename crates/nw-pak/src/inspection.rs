//! Deterministic pak metadata reports.

use std::fmt;
use std::io::{Read, Seek};
use std::path::{Path, PathBuf};

use humansize::{DECIMAL, format_size};
use thiserror::Error;

use crate::archive::{PakArchive, PakError, PakFile};

/// Borrowed view for printing a pak entry-table inspection.
#[derive(Clone, Copy)]
pub struct PakInspectionReport<'a, R: Read + Seek> {
    archive: &'a PakArchive<R>,
    source: &'a Path,
    filter: Option<&'a str>,
    limit: usize,
}

impl<R: Read + Seek> PakArchive<R> {
    /// Build a displayable entry-table inspection report.
    #[must_use]
    pub const fn inspection_report<'a>(
        &'a self,
        source: &'a Path,
        filter: Option<&'a str>,
        limit: usize,
    ) -> PakInspectionReport<'a, R> {
        PakInspectionReport {
            archive: self,
            source,
            filter,
            limit,
        }
    }
}

impl<R: Read + Seek> fmt::Display for PakInspectionReport<'_, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "{}: {} entries",
            self.source.display(),
            self.archive.len()
        )?;

        let mut shown = 0usize;
        let mut total_uncompressed: u64 = 0;
        let mut total_compressed: u64 = 0;
        for entry in self.archive.entries() {
            total_uncompressed += entry.uncompressed_size();
            total_compressed += entry.compressed_size();
            if let Some(filter) = self.filter
                && !entry.name().contains(filter)
            {
                continue;
            }
            if shown < self.limit {
                writeln!(
                    f,
                    "  {:>10} -> {:>10}  {}  {}",
                    format_size(entry.compressed_size(), DECIMAL),
                    format_size(entry.uncompressed_size(), DECIMAL),
                    entry.compression(),
                    entry.name(),
                )?;
                shown += 1;
            }
        }

        let ratio = ratio_string(total_uncompressed, total_compressed);

        writeln!(f)?;
        writeln!(
            f,
            "  totals: {} compressed -> {} uncompressed (ratio {})",
            format_size(total_compressed, DECIMAL),
            format_size(total_uncompressed, DECIMAL),
            ratio
        )
    }
}

fn ratio_string(numerator: u64, denominator: u64) -> String {
    if denominator == 0 {
        return "0.00".to_string();
    }
    let hundredths = u128::from(numerator) * 100 / u128::from(denominator);
    let whole = hundredths / 100;
    let fraction = hundredths % 100;
    format!("{whole}.{fraction:02}")
}

#[derive(Debug, Error)]
pub enum PakInspectionError {
    #[error("open pak {path:?}")]
    Open {
        path: PathBuf,
        #[source]
        source: PakError,
    },
}

pub fn inspect_pak_path(
    path: impl AsRef<Path>,
    filter: Option<&str>,
    limit: usize,
) -> Result<String, PakInspectionError> {
    let path = path.as_ref();
    let archive = PakFile::open(path).map_err(|source| PakInspectionError::Open {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(archive.inspection_report(path, filter, limit).to_string())
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write};
    use std::path::Path;

    use zip::CompressionMethod;

    use crate::PakArchive;

    #[test]
    fn inspection_report_is_deterministic() {
        let mut buf = Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut buf);
            let opts: zip::write::FileOptions<'_, ()> =
                zip::write::FileOptions::default().compression_method(CompressionMethod::Stored);
            zip.start_file("textures/a.dds", opts).unwrap();
            zip.write_all(&[1, 2, 3, 4]).unwrap();
            zip.start_file("levels/main.slice", opts).unwrap();
            zip.write_all(&[5, 6]).unwrap();
            zip.finish().unwrap();
        }

        let archive = PakArchive::from_reader(Cursor::new(buf.into_inner())).unwrap();
        let report = archive
            .inspection_report(Path::new("assets/test.pak"), Some("textures/"), 10)
            .to_string();

        assert!(report.contains("assets/test.pak: 2 entries"));
        assert!(report.contains("4 B ->        4 B  stored  textures/a.dds"));
        assert!(report.contains("totals: 6 B compressed -> 6 B uncompressed (ratio 1.00)"));
    }
}
