use std::io::{self, Write};

use thiserror::Error;

const LOCAL_FILE_HEADER: u32 = 0x0403_4b50;
const CENTRAL_DIRECTORY_HEADER: u32 = 0x0201_4b50;
const END_OF_CENTRAL_DIRECTORY: u32 = 0x0605_4b50;
const ZIP2_VERSION: u16 = 20;
const ZIP1_VERSION: u16 = 10;
const OODLE_VERSION: u16 = 9;
const FLAGS: u16 = 0;
pub(crate) const MARKER_EXTRA_ID: u16 = 0x0020;

#[derive(Debug, Clone, Copy)]
pub(crate) struct Entry<'a> {
    pub name: &'a str,
    pub method: u16,
    pub modified: DosTime,
    pub crc32: u32,
    pub uncompressed_size: u32,
    pub compressed_size: u32,
    pub central_extra: &'a [u8],
    pub compressed_data: &'a [u8],
}

#[derive(Debug, Clone)]
struct DirectoryEntry {
    name: String,
    method: u16,
    modified: DosTime,
    crc32: u32,
    uncompressed_size: u32,
    compressed_size: u32,
    local_header_offset: u32,
    central_extra: Vec<u8>,
}

#[derive(Debug, Error)]
pub(crate) enum Error {
    #[error("io error: {0}")]
    Io(#[from] io::Error),

    #[error("CryPak format limit exceeded for {field}: {value}")]
    Limit { field: &'static str, value: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DosTime {
    time: u16,
    date: u16,
}

impl DosTime {
    pub(crate) fn now() -> Self {
        let now =
            time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
        Self::from_ymdhms(
            now.year(),
            u8::from(now.month()),
            now.day(),
            now.hour(),
            now.minute(),
            now.second(),
        )
    }

    pub(crate) fn from_ymdhms(
        year: i32,
        month: u8,
        day: u8,
        hour: u8,
        minute: u8,
        second: u8,
    ) -> Self {
        let year = u16::try_from(year.clamp(1980, 2107)).expect("clamped DOS year fits u16");
        let month = u16::from(month.clamp(1, 12));
        let day = u16::from(day.clamp(1, 31));
        let hour = u16::from(hour.min(23));
        let minute = u16::from(minute.min(59));
        let second = u16::from(second.min(59));

        Self {
            time: (hour << 11) | (minute << 5) | (second >> 1),
            date: ((year - 1980) << 9) | (month << 5) | day,
        }
    }

    pub(crate) const fn from_raw(time: u16, date: u16) -> Self {
        Self { time, date }
    }
}

#[derive(Debug)]
pub(crate) struct Writer<W> {
    inner: W,
    written: u64,
    directory: Vec<DirectoryEntry>,
}

impl<W: Write> Writer<W> {
    pub(crate) fn new(inner: W) -> Self {
        Self {
            inner,
            written: 0,
            directory: Vec::new(),
        }
    }

    pub(crate) fn push(&mut self, entry: Entry<'_>) -> Result<(), Error> {
        let local_header_offset = checked_u32("local header offset", self.written)?;
        self.write_local_file_header(entry)?;
        self.written += local_file_header_len(entry)?;
        self.inner.write_all(entry.compressed_data)?;
        self.written += u64::from(entry.compressed_size);

        self.directory.push(DirectoryEntry {
            name: entry.name.to_string(),
            method: entry.method,
            modified: entry.modified,
            crc32: entry.crc32,
            uncompressed_size: entry.uncompressed_size,
            compressed_size: entry.compressed_size,
            local_header_offset,
            central_extra: entry.central_extra.to_vec(),
        });
        Ok(())
    }

    pub(crate) fn finish(mut self) -> Result<(W, u64), Error> {
        let central_directory_offset = checked_u32("central directory offset", self.written)?;
        let central_directory_start = self.written;
        let directory = std::mem::take(&mut self.directory);
        for entry in &directory {
            self.write_central_directory_header(entry)?;
            self.written += central_directory_header_len(entry)?;
        }
        let central_directory_size = self.written - central_directory_start;
        self.write_end_of_central_directory(
            checked_u16("directory entry count", directory.len() as u64)?,
            checked_u32("central directory size", central_directory_size)?,
            central_directory_offset,
        )?;
        self.written += 22;

        Ok((self.inner, self.written))
    }

    fn write_local_file_header(&mut self, entry: Entry<'_>) -> Result<(), Error> {
        write_u32(&mut self.inner, LOCAL_FILE_HEADER)?;
        write_u16(&mut self.inner, local_version_needed(entry.method))?;
        write_u16(&mut self.inner, FLAGS)?;
        write_u16(&mut self.inner, entry.method)?;
        write_u16(&mut self.inner, entry.modified.time)?;
        write_u16(&mut self.inner, entry.modified.date)?;
        write_u32(&mut self.inner, entry.crc32)?;
        write_u32(&mut self.inner, entry.compressed_size)?;
        write_u32(&mut self.inner, entry.uncompressed_size)?;
        write_name_len(&mut self.inner, entry.name)?;
        write_u16(&mut self.inner, 0)?;
        self.inner.write_all(entry.name.as_bytes())?;
        Ok(())
    }

    fn write_central_directory_header(&mut self, entry: &DirectoryEntry) -> Result<(), Error> {
        write_u32(&mut self.inner, CENTRAL_DIRECTORY_HEADER)?;
        let version = central_directory_version(entry.method);
        write_u16(&mut self.inner, version)?;
        write_u16(&mut self.inner, version)?;
        write_u16(&mut self.inner, FLAGS)?;
        write_u16(&mut self.inner, entry.method)?;
        write_u16(&mut self.inner, entry.modified.time)?;
        write_u16(&mut self.inner, entry.modified.date)?;
        write_u32(&mut self.inner, entry.crc32)?;
        write_u32(&mut self.inner, entry.compressed_size)?;
        write_u32(&mut self.inner, entry.uncompressed_size)?;
        write_name_len(&mut self.inner, &entry.name)?;
        write_u16(
            &mut self.inner,
            checked_u16(
                "central directory extra length",
                entry.central_extra.len() as u64,
            )?,
        )?;
        write_u16(&mut self.inner, 0)?;
        write_u16(&mut self.inner, 0)?;
        write_u16(&mut self.inner, 0)?;
        write_u32(&mut self.inner, 0)?;
        write_u32(&mut self.inner, entry.local_header_offset)?;
        self.inner.write_all(entry.name.as_bytes())?;
        self.inner.write_all(&entry.central_extra)?;
        Ok(())
    }

    fn write_end_of_central_directory(
        &mut self,
        entries: u16,
        central_directory_size: u32,
        central_directory_offset: u32,
    ) -> Result<(), Error> {
        write_u32(&mut self.inner, END_OF_CENTRAL_DIRECTORY)?;
        write_u16(&mut self.inner, 0)?;
        write_u16(&mut self.inner, 0)?;
        write_u16(&mut self.inner, entries)?;
        write_u16(&mut self.inner, entries)?;
        write_u32(&mut self.inner, central_directory_size)?;
        write_u32(&mut self.inner, central_directory_offset)?;
        write_u16(&mut self.inner, 0)?;
        Ok(())
    }
}

pub(crate) fn checked_u16(field: &'static str, value: u64) -> Result<u16, Error> {
    u16::try_from(value).map_err(|_| Error::Limit { field, value })
}

pub(crate) fn checked_u32(field: &'static str, value: u64) -> Result<u32, Error> {
    u32::try_from(value).map_err(|_| Error::Limit { field, value })
}

fn local_file_header_len(entry: Entry<'_>) -> Result<u64, Error> {
    checked_u16("file name length", entry.name.len() as u64)?;
    Ok(30 + entry.name.len() as u64)
}

fn central_directory_header_len(entry: &DirectoryEntry) -> Result<u64, Error> {
    checked_u16("file name length", entry.name.len() as u64)?;
    checked_u16(
        "central directory extra length",
        entry.central_extra.len() as u64,
    )?;
    Ok(46 + entry.name.len() as u64 + entry.central_extra.len() as u64)
}

fn write_name_len<W: Write>(writer: &mut W, name: &str) -> Result<(), Error> {
    write_u16(writer, checked_u16("file name length", name.len() as u64)?)
}

fn write_u16<W: Write>(writer: &mut W, value: u16) -> Result<(), Error> {
    writer.write_all(&value.to_le_bytes())?;
    Ok(())
}

fn write_u32<W: Write>(writer: &mut W, value: u32) -> Result<(), Error> {
    writer.write_all(&value.to_le_bytes())?;
    Ok(())
}

fn local_version_needed(method: u16) -> u16 {
    if method == 15 {
        OODLE_VERSION
    } else {
        ZIP1_VERSION
    }
}

fn central_directory_version(method: u16) -> u16 {
    if method == 15 {
        OODLE_VERSION
    } else {
        ZIP2_VERSION
    }
}

pub(crate) fn marker_extra(marker: u8) -> [u8; 5] {
    let id = MARKER_EXTRA_ID.to_le_bytes();
    [id[0], id[1], 1, 0, marker]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_current_game_versions_by_method() {
        let mut bytes = Vec::new();
        let mut writer = Writer::new(&mut bytes);
        let time = DosTime::from_ymdhms(2026, 6, 19, 12, 0, 0);
        let marker = marker_extra(0x15);
        writer
            .push(Entry {
                name: "a.bin",
                method: 0,
                modified: time,
                crc32: 0,
                uncompressed_size: 0,
                compressed_size: 0,
                central_extra: &marker,
                compressed_data: &[],
            })
            .unwrap();
        writer
            .push(Entry {
                name: "b.bin",
                method: 8,
                modified: time,
                crc32: 0,
                uncompressed_size: 0,
                compressed_size: 0,
                central_extra: &[],
                compressed_data: &[],
            })
            .unwrap();
        writer
            .push(Entry {
                name: "c.bin",
                method: 15,
                modified: time,
                crc32: 0,
                uncompressed_size: 0,
                compressed_size: 0,
                central_extra: &marker,
                compressed_data: &[],
            })
            .unwrap();
        writer.finish().unwrap();

        let mut pos = 0usize;
        assert_eq!(u32_at(&bytes, pos), LOCAL_FILE_HEADER);
        assert_eq!(u16_at(&bytes, pos + 4), 10);
        assert_eq!(u16_at(&bytes, pos + 28), 0);
        pos += 30 + "a.bin".len();
        assert_eq!(u32_at(&bytes, pos), LOCAL_FILE_HEADER);
        assert_eq!(u16_at(&bytes, pos + 4), 10);
        assert_eq!(u16_at(&bytes, pos + 28), 0);
        pos += 30 + "b.bin".len();
        assert_eq!(u32_at(&bytes, pos), LOCAL_FILE_HEADER);
        assert_eq!(u16_at(&bytes, pos + 4), 9);
        assert_eq!(u16_at(&bytes, pos + 28), 0);
        pos += 30 + "c.bin".len();

        assert_eq!(u32_at(&bytes, pos), CENTRAL_DIRECTORY_HEADER);
        assert_eq!(u16_at(&bytes, pos + 4), 20);
        assert_eq!(u16_at(&bytes, pos + 6), 20);
        assert_eq!(
            u16_at(&bytes, pos + 30),
            u16::try_from(marker.len()).expect("marker length fits u16")
        );
        assert_eq!(
            &bytes[pos + 46 + "a.bin".len()..pos + 46 + "a.bin".len() + marker.len()],
            marker
        );
        pos += 46 + "a.bin".len() + marker.len();
        assert_eq!(u32_at(&bytes, pos), CENTRAL_DIRECTORY_HEADER);
        assert_eq!(u16_at(&bytes, pos + 4), 20);
        assert_eq!(u16_at(&bytes, pos + 6), 20);
        assert_eq!(u16_at(&bytes, pos + 30), 0);
        pos += 46 + "b.bin".len();
        assert_eq!(u32_at(&bytes, pos), CENTRAL_DIRECTORY_HEADER);
        assert_eq!(u16_at(&bytes, pos + 4), 9);
        assert_eq!(u16_at(&bytes, pos + 6), 9);
        assert_eq!(
            u16_at(&bytes, pos + 30),
            u16::try_from(marker.len()).expect("marker length fits u16")
        );
        assert_eq!(
            &bytes[pos + 46 + "c.bin".len()..pos + 46 + "c.bin".len() + marker.len()],
            marker
        );
    }

    fn u16_at(bytes: &[u8], offset: usize) -> u16 {
        u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
    }

    fn u32_at(bytes: &[u8], offset: usize) -> u32 {
        u32::from_le_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ])
    }
}
