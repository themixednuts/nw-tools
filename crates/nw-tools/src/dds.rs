use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::ValueEnum;
use image::{DynamicImage, RgbaImage};

use crate::support::{ensure_parent, guard_existing};

pub const DEFAULT_FRAMES_PER_SECOND: u32 = 30;

/// One frame of a texture: its DDS header path, split sidecars, and optional
/// attached-alpha surface. A standalone texture is a single frame; a numbered
/// texture sequence has several.
#[derive(Clone)]
pub struct DdsFrame {
    pub header: String,
    pub sidecars: Vec<(nw_dds::SplitPart, String)>,
    pub alpha: Option<AlphaSurface>,
}

/// The attached-alpha companion surface of a [`DdsFrame`].
#[derive(Clone)]
pub struct AlphaSurface {
    pub header: String,
    pub sidecars: Vec<(nw_dds::SplitPart, String)>,
}

/// A logical texture: one frame for an ordinary DDS, or several ordered frames
/// for a sequence.
#[derive(Clone)]
pub struct DdsItem {
    pub label: String,
    pub frames: Vec<DdsFrame>,
}

impl DdsItem {
    #[must_use]
    pub fn single(label: String, frame: DdsFrame) -> Self {
        Self {
            label,
            frames: vec![frame],
        }
    }

    #[must_use]
    pub fn frame(&self, index: usize) -> &DdsFrame {
        &self.frames[index % self.frames.len().max(1)]
    }

    #[must_use]
    pub fn is_sequence(&self) -> bool {
        self.frames.len() > 1
    }
}

/// Image formats supported by DDS image and sequence exports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ImageFormat {
    /// Portable Network Graphics image.
    Png,
    /// Tagged Image File Format image.
    #[value(alias = "tif")]
    Tiff,
    /// OpenEXR high-dynamic-range image.
    Exr,
    /// Graphics Interchange Format image or animation.
    Gif,
    /// Quite OK Image format.
    Qoi,
}

impl ImageFormat {
    #[must_use]
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Tiff => "tiff",
            Self::Exr => "exr",
            Self::Gif => "gif",
            Self::Qoi => "qoi",
        }
    }

    pub fn from_path(path: &Path) -> Result<Self> {
        let extension = path
            .extension()
            .and_then(|extension| extension.to_str())
            .with_context(|| format!("export path has no image extension: {}", path.display()))?;
        match extension.to_ascii_lowercase().as_str() {
            "png" => Ok(Self::Png),
            "tif" | "tiff" => Ok(Self::Tiff),
            "exr" => Ok(Self::Exr),
            "gif" => Ok(Self::Gif),
            "qoi" => Ok(Self::Qoi),
            _ => bail!(
                "unsupported DDS export extension .{extension}; use png, tif/tiff, exr, gif, or qoi"
            ),
        }
    }

    const fn image_format(self) -> image::ImageFormat {
        match self {
            Self::Png => image::ImageFormat::Png,
            Self::Tiff => image::ImageFormat::Tiff,
            Self::Exr => image::ImageFormat::OpenExr,
            Self::Gif => image::ImageFormat::Gif,
            Self::Qoi => image::ImageFormat::Qoi,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExportedImage {
    pub output: PathBuf,
    pub width: u32,
    pub height: u32,
    pub frames: usize,
    pub bytes: u64,
}

/// Decode one DDS frame's largest mip, including split mip files and Cry's
/// attached-alpha surface when present.
pub fn decode_frame(frame: &DdsFrame, read: impl Fn(&str) -> Result<Vec<u8>>) -> Result<RgbaImage> {
    let header = read(&frame.header).with_context(|| format!("read {}", frame.header))?;
    let sidecar_bytes = read_sidecars(&frame.sidecars, &read)?;
    let sidecars = borrowed_sidecars(&sidecar_bytes);

    let alpha_bytes = frame
        .alpha
        .as_ref()
        .map(|alpha| {
            Ok::<_, anyhow::Error>((
                read(&alpha.header).with_context(|| format!("read {}", alpha.header))?,
                read_sidecars(&alpha.sidecars, &read)?,
            ))
        })
        .transpose()?;
    let alpha_sidecars = alpha_bytes
        .as_ref()
        .map(|(_, sidecars)| borrowed_sidecars(sidecars));
    let attached_alpha = alpha_bytes.as_ref().map(|(header, _)| {
        (
            header.as_slice(),
            alpha_sidecars.as_deref().unwrap_or_default(),
        )
    });

    let decoded = nw_dds::decode_top_mip_with_attached_alpha(&header, &sidecars, attached_alpha)
        .with_context(|| format!("decode {}", frame.header))?;
    RgbaImage::from_raw(decoded.width, decoded.height, decoded.rgba)
        .context("decoded texture had an unexpected size")
}

fn read_sidecars(
    sidecars: &[(nw_dds::SplitPart, String)],
    read: &impl Fn(&str) -> Result<Vec<u8>>,
) -> Result<Vec<(nw_dds::SplitPart, Vec<u8>)>> {
    sidecars
        .iter()
        .map(|(part, key)| {
            read(key)
                .with_context(|| format!("read {key}"))
                .map(|bytes| (*part, bytes))
        })
        .collect()
}

fn borrowed_sidecars(sidecars: &[(nw_dds::SplitPart, Vec<u8>)]) -> Vec<nw_dds::Sidecar<'_>> {
    sidecars
        .iter()
        .map(|(part, bytes)| nw_dds::Sidecar::new(*part, bytes))
        .collect()
}

/// Decode every frame and export an ordinary image or an automatically tiled
/// sequence sheet. Frame order is the order carried by [`DdsItem::frames`].
pub fn export_item(
    item: &DdsItem,
    read: impl Fn(&str) -> Result<Vec<u8>>,
    output: &Path,
    format: ImageFormat,
    overwrite: bool,
    frames_per_second: u32,
) -> Result<ExportedImage> {
    if item.frames.is_empty() {
        bail!("DDS item {} has no frames", item.label);
    }
    if frames_per_second == 0 {
        bail!("GIF frame rate must be greater than zero");
    }
    guard_existing(output, overwrite.into())?;
    ensure_parent(output)?;

    let frames = item
        .frames
        .iter()
        .map(|frame| decode_frame(frame, &read))
        .collect::<Result<Vec<_>>>()?;
    let frame_count = frames.len();
    let (width, height) = match format {
        ImageFormat::Gif => write_gif(frames, output, frames_per_second)?,
        _ => {
            let frame_refs = frames.iter().collect::<Vec<_>>();
            let image = sequence_sheet(&frame_refs).context("DDS sequence has no frames")?;
            let dimensions = image.dimensions();
            let image = match format {
                ImageFormat::Exr => {
                    DynamicImage::ImageRgba32F(DynamicImage::ImageRgba8(image).to_rgba32f())
                }
                _ => DynamicImage::ImageRgba8(image),
            };
            image
                .save_with_format(output, format.image_format())
                .with_context(|| format!("write {}", output.display()))?;
            dimensions
        }
    };
    let bytes = std::fs::metadata(output)
        .with_context(|| format!("stat {}", output.display()))?
        .len();

    Ok(ExportedImage {
        output: output.to_path_buf(),
        width,
        height,
        frames: frame_count,
        bytes,
    })
}

fn write_gif(frames: Vec<RgbaImage>, output: &Path, frames_per_second: u32) -> Result<(u32, u32)> {
    let width = frames.iter().map(RgbaImage::width).max().unwrap_or(1);
    let height = frames.iter().map(RgbaImage::height).max().unwrap_or(1);
    let delay = image::Delay::from_numer_denom_ms(1_000, frames_per_second);
    let frames = frames.into_iter().map(|image| {
        let image = if image.dimensions() == (width, height) {
            image
        } else {
            let mut canvas = RgbaImage::new(width, height);
            let x = (width - image.width()) / 2;
            let y = (height - image.height()) / 2;
            image::imageops::overlay(&mut canvas, &image, i64::from(x), i64::from(y));
            canvas
        };
        image::Frame::from_parts(image, 0, 0, delay)
    });

    let file = File::create(output).with_context(|| format!("create {}", output.display()))?;
    let mut encoder = image::codecs::gif::GifEncoder::new(BufWriter::new(file));
    encoder
        .set_repeat(image::codecs::gif::Repeat::Infinite)
        .with_context(|| format!("configure {}", output.display()))?;
    encoder
        .encode_frames(frames)
        .with_context(|| format!("write {}", output.display()))?;
    Ok((width, height))
}

/// Tile images into a compact square-ish sheet, centering differently sized
/// frames in transparent cells. A single image passes through unchanged.
pub fn sequence_sheet(images: &[&RgbaImage]) -> Option<RgbaImage> {
    if images.is_empty() {
        return None;
    }
    let count = images.len();
    let columns = (count as f64).sqrt().ceil() as u32;
    let rows = u32::try_from(count).ok()?.div_ceil(columns);
    let cell_width = images.iter().map(|image| image.width()).max()?;
    let cell_height = images.iter().map(|image| image.height()).max()?;
    let width = cell_width.checked_mul(columns)?;
    let height = cell_height.checked_mul(rows)?;
    let mut sheet = RgbaImage::new(width, height);
    for (index, image) in images.iter().enumerate() {
        let index = u32::try_from(index).ok()?;
        let x = (index % columns) * cell_width + (cell_width - image.width()) / 2;
        let y = (index / columns) * cell_height + (cell_height - image.height()) / 2;
        image::imageops::overlay(&mut sheet, *image, i64::from(x), i64::from(y));
    }
    Some(sheet)
}

/// Return the numbered-sequence base and frame number for a DDS label.
/// Underscore-separated suffixes accept any digit count; bare suffixes require
/// at least two digits unless the path is within a `sequence` directory.
#[must_use]
pub fn sequence_base(label: &str) -> Option<(String, u32)> {
    let suffix = label.get(label.len().checked_sub(4)?..)?;
    if !suffix.eq_ignore_ascii_case(".dds") {
        return None;
    }
    let stem = &label[..label.len() - 4];
    let digits = stem.len() - stem.bytes().rev().take_while(u8::is_ascii_digit).count();
    let (head, digits) = stem.split_at(digits);
    if digits.is_empty() {
        return None;
    }
    let number = digits.parse().ok()?;
    let (base, separated) = match head.strip_suffix('_') {
        Some(base) => (base, true),
        None => (head, false),
    };
    if base.is_empty() {
        return None;
    }
    if let [.., b'_', axis] = base.as_bytes()
        && axis.is_ascii_alphabetic()
    {
        return None;
    }
    let directory = label.rsplit_once(['/', '\\']).map_or("", |(dir, _)| dir);
    let in_sequence_dir = directory.to_ascii_lowercase().contains("sequence");
    if !separated && !in_sequence_dir && digits.len() < 2 {
        return None;
    }
    Some((base.to_string(), number))
}

/// A safe default export name for a browser label.
#[must_use]
pub fn default_export_path(label: &str, format: ImageFormat) -> PathBuf {
    let name = label.rsplit(['/', '\\']).next().unwrap_or(label);
    let stem = name
        .get(..name.len().saturating_sub(4))
        .filter(|_| {
            name.get(name.len().saturating_sub(4)..)
                .is_some_and(|s| s.eq_ignore_ascii_case(".dds"))
        })
        .unwrap_or(name)
        .trim_end_matches('*')
        .trim_end_matches('_');
    let stem = if stem.is_empty() { "texture" } else { stem };
    PathBuf::from(format!("{stem}.{}", format.extension()))
}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::io::BufReader;
    use std::path::{Path, PathBuf};

    use clap::ValueEnum;
    use image::{AnimationDecoder, Rgba, RgbaImage};

    use super::{
        AlphaSurface, DEFAULT_FRAMES_PER_SECOND, DdsFrame, DdsItem, ImageFormat, decode_frame,
        default_export_path, export_item, sequence_base, sequence_sheet,
    };

    #[test]
    fn sequence_base_matches_numbered_frames_without_matching_variants() {
        assert_eq!(
            sequence_base("fx/spark_0.dds"),
            Some(("fx/spark".into(), 0))
        );
        assert_eq!(sequence_base("ui/coin01.DDS"), Some(("ui/coin".into(), 1)));
        assert_eq!(sequence_base("objects/tree_lod0.dds"), None);
        assert_eq!(sequence_base("map_l1_y001_x017.dds"), None);
        assert_eq!(
            sequence_base("ui/tensionimagesequence/tension0.dds"),
            Some(("ui/tensionimagesequence/tension".into(), 0))
        );
    }

    #[test]
    fn sequence_sheet_uses_frame_order_and_transparent_padding() {
        let red = RgbaImage::from_pixel(2, 1, Rgba([255, 0, 0, 255]));
        let green = RgbaImage::from_pixel(1, 2, Rgba([0, 255, 0, 255]));
        let blue = RgbaImage::from_pixel(1, 1, Rgba([0, 0, 255, 255]));

        let sheet = sequence_sheet(&[&red, &green, &blue]).unwrap();

        assert_eq!(sheet.dimensions(), (4, 4));
        assert_eq!(sheet.get_pixel(0, 0), red.get_pixel(0, 0));
        assert_eq!(*sheet.get_pixel(3, 0), Rgba([0, 0, 0, 0]));
        assert_eq!(sheet.get_pixel(2, 1), green.get_pixel(0, 0));
        assert_eq!(sheet.get_pixel(0, 2), blue.get_pixel(0, 0));
        assert_eq!(*sheet.get_pixel(1, 2), Rgba([0, 0, 0, 0]));
    }

    #[test]
    fn default_export_path_removes_browser_wildcard() {
        assert_eq!(
            default_export_path("fx/spark_*.dds", ImageFormat::Png),
            PathBuf::from("spark.png")
        );
    }

    #[test]
    fn exr_is_the_only_openexr_name() {
        assert_eq!(
            ImageFormat::from_path(Path::new("image.exr")).unwrap(),
            ImageFormat::Exr
        );
        assert!(ImageFormat::from_path(Path::new("image.exf")).is_err());
        assert!(<ImageFormat as ValueEnum>::from_str("exf", true).is_err());
    }

    #[test]
    fn decode_frame_merges_attached_alpha() {
        let color = rgba8_dds(2, 1, &[10, 20, 30, 255, 40, 50, 60, 255]);
        let alpha = alpha8_dds(2, 1, &[64, 128]);
        let frame = DdsFrame {
            header: "color".to_string(),
            sidecars: Vec::new(),
            alpha: Some(AlphaSurface {
                header: "alpha".to_string(),
                sidecars: Vec::new(),
            }),
        };

        let image = decode_frame(&frame, |key| match key {
            "color" => Ok(color.clone()),
            "alpha" => Ok(alpha.clone()),
            _ => unreachable!(),
        })
        .unwrap();

        assert_eq!(image.get_pixel(0, 0)[3], 64);
        assert_eq!(image.get_pixel(1, 0)[3], 128);
    }

    #[test]
    fn export_item_writes_every_supported_format() {
        let dds = rgba8_dds(1, 1, &[10, 20, 30, 255]);
        let item = DdsItem::single(
            "pixel.dds".to_string(),
            DdsFrame {
                header: "pixel".to_string(),
                sidecars: Vec::new(),
                alpha: None,
            },
        );
        let temp = tempfile::tempdir().unwrap();

        for format in [
            ImageFormat::Png,
            ImageFormat::Tiff,
            ImageFormat::Exr,
            ImageFormat::Gif,
            ImageFormat::Qoi,
        ] {
            let output = temp.path().join(format!("pixel.{}", format.extension()));
            let exported = export_item(
                &item,
                |_| Ok(dds.clone()),
                &output,
                format,
                false,
                DEFAULT_FRAMES_PER_SECOND,
            )
            .unwrap();

            assert_eq!(
                (exported.width, exported.height, exported.frames),
                (1, 1, 1)
            );
            assert!(exported.bytes > 0);
            let image = image::open(output).unwrap();
            assert_eq!((image.width(), image.height()), (1, 1));
        }
    }

    #[test]
    fn gif_export_animates_sequence_frames_at_requested_rate() {
        let red = rgba8_dds(1, 1, &[255, 0, 0, 255]);
        let blue = rgba8_dds(1, 1, &[0, 0, 255, 255]);
        let item = DdsItem {
            label: "pixel_*.dds".to_string(),
            frames: ["red", "blue"]
                .map(|header| DdsFrame {
                    header: header.to_string(),
                    sidecars: Vec::new(),
                    alpha: None,
                })
                .into(),
        };
        let temp = tempfile::tempdir().unwrap();
        let output = temp.path().join("pixels.gif");

        let exported = export_item(
            &item,
            |key| match key {
                "red" => Ok(red.clone()),
                "blue" => Ok(blue.clone()),
                _ => unreachable!(),
            },
            &output,
            ImageFormat::Gif,
            false,
            20,
        )
        .unwrap();
        let decoder =
            image::codecs::gif::GifDecoder::new(BufReader::new(File::open(output).unwrap()))
                .unwrap();
        let frames = decoder.into_frames().collect_frames().unwrap();

        assert_eq!(
            (exported.width, exported.height, exported.frames),
            (1, 1, 2)
        );
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].delay().numer_denom_ms(), (50, 1));
        assert_eq!(frames[0].buffer().get_pixel(0, 0)[0], 255);
        assert_eq!(frames[1].buffer().get_pixel(0, 0)[2], 255);
    }

    fn rgba8_dds(width: u32, height: u32, payload: &[u8]) -> Vec<u8> {
        let mut bytes = dds_header(width, height);
        put_u32(&mut bytes, 80, 0x40 | 0x1);
        put_u32(&mut bytes, 88, 32);
        put_u32(&mut bytes, 92, 0x0000_00ff);
        put_u32(&mut bytes, 96, 0x0000_ff00);
        put_u32(&mut bytes, 100, 0x00ff_0000);
        put_u32(&mut bytes, 104, 0xff00_0000);
        bytes.extend_from_slice(payload);
        bytes
    }

    fn alpha8_dds(width: u32, height: u32, payload: &[u8]) -> Vec<u8> {
        let mut bytes = dds_header(width, height);
        put_u32(&mut bytes, 80, 0x2);
        put_u32(&mut bytes, 88, 8);
        bytes.extend_from_slice(payload);
        bytes
    }

    fn dds_header(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = vec![0; 128];
        bytes[0..4].copy_from_slice(b"DDS ");
        put_u32(&mut bytes, 4, 124);
        put_u32(&mut bytes, 8, 0x1 | 0x2 | 0x4 | 0x1000);
        put_u32(&mut bytes, 12, height);
        put_u32(&mut bytes, 16, width);
        put_u32(&mut bytes, 28, 1);
        put_u32(&mut bytes, 76, 32);
        put_u32(&mut bytes, 108, 0x1000);
        bytes
    }

    fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
}
