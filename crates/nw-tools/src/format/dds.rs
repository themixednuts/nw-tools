use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use clap::{Args, ValueEnum};
use humansize::{DECIMAL, format_size};
use nw_pak::PakMmapReader;

use crate::jobs::{JobArgs, RunCtx};
use crate::support::{PakSet, collect_matching, ensure_parent, guard_existing, write_guarded};
use crate::ui::{Cell, Report, Table};

use super::common::{finish_scan, path_label, strip_suffix_ignore_ascii_case};

#[derive(Debug, Args)]
pub struct Dds {
    /// DDS file or directory. Omit to browse textures under the current directory.
    path: Option<PathBuf>,

    #[arg(long, default_value_t = 40)]
    show: usize,

    /// Convert the texture(s) to this format, written under --out.
    #[arg(long, value_enum, requires = "out")]
    to: Option<DdsFormat>,

    /// Output path/dir for --to.
    #[arg(long, value_name = "PATH", requires = "to")]
    out: Option<PathBuf>,

    /// Decode the texture and show it inline (kitty graphics protocol).
    #[arg(long)]
    view: bool,

    /// Replace existing outputs.
    #[arg(long)]
    overwrite: bool,

    #[command(flatten)]
    jobs: JobArgs,
}

/// Output format for `dds --to`.
#[derive(Debug, Clone, Copy, ValueEnum)]
enum DdsFormat {
    Ktx2,
    Png,
}

#[derive(Debug, Clone)]
struct DdsScan {
    source: String,
    kind: String,
    dimensions: String,
    mipmaps: String,
    format: String,
    dx10: String,
    cry: String,
    bytes: String,
}

#[derive(Debug, Clone)]
struct DdsGroup {
    header: PathBuf,
    sidecars: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
struct DdsConvert {
    source: String,
    output: String,
    bytes: String,
}

impl Dds {
    pub(super) fn run(self) -> Result<()> {
        let ctx = self.jobs.ctx()?;
        if let Some(format) = self.to {
            let path = self
                .path
                .as_deref()
                .context("--to needs a DDS file or directory path")?;
            let out = self.out.as_deref().expect("--out is required with --to");
            return match format {
                DdsFormat::Ktx2 => convert_dds_to_ktx2(&ctx, path, out, self.overwrite),
                DdsFormat::Png => write_dds_png(path, out, self.overwrite),
            };
        }
        if self.view {
            let path = self
                .path
                .as_deref()
                .context("--view needs a DDS file path")?;
            return view_dds(path);
        }

        // No path: default to the located install and browse its textures
        // straight out of the pak catalog (the install has no loose DDS files).
        if self.path.is_none() {
            return self.browse_install(&ctx);
        }

        let root = self.path.clone().unwrap_or_else(|| PathBuf::from("."));
        // A directory on a TTY: open the interactive filesystem texture browser.
        if crate::tui::interactive() && root.is_dir() {
            let items = dds_browser_items_fs(&ctx, &root)?;
            if items.is_empty() {
                Report::new("dds")
                    .stat("dir", root.display())
                    .note("no DDS textures found")
                    .print();
                return Ok(());
            }
            let store = Arc::new(crate::tui::TextureStore::Fs);
            let source = root.display().to_string();
            let catalog = Arc::new(crate::tui::DdsCatalog::ready(items));
            return Ok(crate::tui::dds_browser(catalog, store, source, ctx.runner.clone())?);
        }

        let paths = collect_matching(&root, |path| nw_dds::is_dds_path(path))?;
        let batch = ctx.map_results_compact(
            "dds",
            &paths,
            |path| path_label(path),
            |path, progress| progress.step(|| scan_dds(path)),
        );
        let skipped = batch.skipped();
        let cancelled = batch.was_cancelled();
        let mut scans = Vec::new();
        let mut errors = Vec::new();

        for result in batch.into_completed() {
            match result {
                Ok(scan) => scans.push(scan),
                Err(error) => errors.push(error),
            }
        }
        scans.sort_by(|left, right| left.source.cmp(&right.source));

        let mut report = Report::new("dds")
            .stat("files", scans.len())
            .stat("shown", scans.len().min(self.show));
        let mut table = Table::new([
            "Source",
            "Kind",
            "Dimensions",
            "Mips",
            "Format",
            "DX10",
            "Cry",
            "Bytes",
        ])
        .right([7]);
        for scan in scans.iter().take(self.show) {
            table.push([
                Cell::path(scan.source.clone()),
                Cell::text(scan.kind.clone()),
                Cell::text(scan.dimensions.clone()),
                Cell::text(scan.mipmaps.clone()),
                Cell::text(scan.format.clone()),
                Cell::text(scan.dx10.clone()),
                Cell::dim(scan.cry.clone()),
                Cell::size(scan.bytes.clone()),
            ]);
        }
        report.table_or(table, "no DDS files to show");
        if scans.len() > self.show {
            report.more(scans.len() - self.show, "file(s)");
        }
        report.print();

        finish_scan(cancelled, skipped, &errors, "dds")
    }

    /// Browse textures straight out of the located install's pak catalog. On a
    /// TTY this opens the interactive browser; piped, it prints a texture listing
    /// so the install default is still useful in scripts.
    fn browse_install(&self, ctx: &RunCtx) -> Result<()> {
        let install = crate::source::locate()?;
        let paks = PakSet::collect(install.assets(), Vec::new())?;
        if paks.paths().is_empty() {
            Report::new("dds")
                .stat("install", install.assets().display())
                .note("no pak archives found in the install")
                .print();
            return Ok(());
        }
        let source = install.assets().display().to_string();

        // Interactive: open the browser immediately and discover textures on a
        // background thread, so the UI is live from the first frame and the scan
        // never blocks it.
        if crate::tui::interactive() {
            let (catalog, index) = spawn_dds_discovery(ctx, paks.paths());
            let store = Arc::new(crate::tui::TextureStore::Pak(index));
            return Ok(crate::tui::dds_browser(catalog, store, source, ctx.runner.clone())?);
        }

        // Piped: produce the full listing up front.
        let (items, _) = dds_browser_items_pak(ctx, paks.paths())?;
        if items.is_empty() {
            Report::new("dds")
                .stat("install", &source)
                .note("no DDS textures found in the install paks")
                .print();
            return Ok(());
        }
        let mut report = Report::new("dds")
            .stat("install", &source)
            .stat("textures", items.len())
            .stat("shown", items.len().min(self.show));
        let mut table = Table::new(["Texture", "Frames", "Mips"]).right([1, 2]);
        for item in items.iter().take(self.show) {
            table.push([
                Cell::path(item.label.clone()),
                Cell::text(item.frames.len().to_string()),
                Cell::text(item.frame(0).sidecars.len().to_string()),
            ]);
        }
        report.table_or(table, "no DDS textures to show");
        if items.len() > self.show {
            report.more(items.len() - self.show, "texture(s)");
        }
        report.print();
        Ok(())
    }
}

/// Spawn background discovery of the install's DDS textures: each pak is scanned
/// on a worker thread, its DDS entries indexed and grouped into logical textures,
/// and appended to the returned [`crate::tui::DdsCatalog`] as it goes. Returns
/// immediately so the browser can open before any pak is read.
fn spawn_dds_discovery(
    ctx: &RunCtx,
    pak_paths: &[PathBuf],
) -> (Arc<crate::tui::DdsCatalog>, crate::tui::SharedIndex) {
    let catalog = Arc::new(crate::tui::DdsCatalog::new(pak_paths.len()));
    let index = crate::tui::shared_index();
    let runner = ctx.runner.clone();
    let paths = pak_paths.to_vec();
    let (catalog_bg, index_bg) = (catalog.clone(), index.clone());
    std::thread::spawn(move || {
        runner.map(&paths, |path| discover_pak(path, &catalog_bg, &index_bg));
    });
    (catalog, index)
}

/// Scan one pak for DDS entries: index them (so the browser can read them) and
/// group them into logical textures appended to `catalog`. Runs on a worker.
fn discover_pak(path: &Path, catalog: &crate::tui::DdsCatalog, index: &crate::tui::SharedIndex) {
    if let Ok(reader) = PakMmapReader::open(path) {
        let reader = Arc::new(reader);
        let mut classified = Vec::new();
        let mut entries = Vec::new();
        for entry in reader.entries() {
            let name = entry.name().to_ascii_lowercase();
            if nw_dds::is_dds_path(&name) {
                if let Some(part) = nw_dds::SplitPart::from_path(&name) {
                    classified.push((part, name.clone()));
                }
                entries.push((name, entry.index()));
            }
        }
        // Index the entries before publishing the items, so every grouped texture
        // is immediately readable.
        if !entries.is_empty() {
            let mut index = index.write().unwrap_or_else(|error| error.into_inner());
            for (name, entry) in entries {
                index.insert(name, (reader.clone(), entry));
            }
        }
        // Group within the pak — a texture's header and split mips ship together,
        // and X_NN.dds frames into one sprite entry.
        if let Ok(items) = group_dds_items(classified, |header| header.to_string()) {
            let items = group_sprites(items);
            if !items.is_empty() {
                catalog.extend(items);
            }
        }
    }
    catalog.mark_pak_done();
}

/// Decode a single DDS texture (assembling its split sidecars) to an RGBA image,
/// returning it alongside the header path it came from.
fn decode_dds(path: &Path) -> Result<(PathBuf, image::RgbaImage)> {
    if path.is_dir() {
        bail!("--view and --to expect a single DDS file, not a directory");
    }
    let group = collect_dds_groups(path)?
        .into_iter()
        .next()
        .context("no DDS texture found")?;
    let header_bytes =
        std::fs::read(&group.header).with_context(|| format!("read {}", group.header.display()))?;
    let mut sidecar_bytes = Vec::with_capacity(group.sidecars.len());
    for sidecar in &group.sidecars {
        let part = split_part_for_path(sidecar)?;
        let bytes =
            std::fs::read(sidecar).with_context(|| format!("read {}", sidecar.display()))?;
        sidecar_bytes.push((part, bytes));
    }
    let sidecars = sidecar_bytes
        .iter()
        .map(|(part, bytes)| nw_dds::Sidecar::new(*part, bytes.as_slice()))
        .collect::<Vec<_>>();
    let decoded = nw_dds::decode_top_mip(&header_bytes, &sidecars)
        .with_context(|| format!("decode {}", group.header.display()))?;
    let image = image::RgbaImage::from_raw(decoded.width, decoded.height, decoded.rgba)
        .context("decoded texture had an unexpected size")?;
    Ok((group.header, image))
}

/// Show a DDS texture inline via the kitty graphics protocol.
fn view_dds(path: &Path) -> Result<()> {
    let (header, image) = decode_dds(path)?;
    if crate::ui::theme::caps().graphics {
        let shown = fit_image(&image, 720);
        crate::ui::image::print_kitty_rgba(shown.as_raw(), shown.width(), shown.height());
    } else {
        Report::new("dds view")
            .stat("source", path_label(&header))
            .stat("size", format!("{}x{}", image.width(), image.height()))
            .note("this terminal has no inline-image support (kitty graphics protocol)")
            .note("write a file with --to png --out <path> instead")
            .print();
    }
    Ok(())
}

/// Decode a DDS texture and write it as a PNG.
fn write_dds_png(path: &Path, out: &Path, overwrite: bool) -> Result<()> {
    let (header, image) = decode_dds(path)?;
    guard_existing(out, overwrite.into())?;
    ensure_parent(out)?;
    image
        .save(out)
        .with_context(|| format!("write {}", out.display()))?;
    Report::new("dds png")
        .stat("source", path_label(&header))
        .stat("size", format!("{}x{}", image.width(), image.height()))
        .stat("png", out.display())
        .print();
    Ok(())
}

/// Downscale an image so its largest side is at most `max`, preserving aspect.
fn fit_image(image: &image::RgbaImage, max: u32) -> image::RgbaImage {
    let (width, height) = (image.width(), image.height());
    if width <= max && height <= max {
        return image.clone();
    }
    let scale = f64::from(max) / f64::from(width.max(height));
    let target_w = ((f64::from(width) * scale) as u32).max(1);
    let target_h = ((f64::from(height) * scale) as u32).max(1);
    image::imageops::resize(image, target_w, target_h, image::imageops::FilterType::Triangle)
}

fn convert_dds_to_ktx2(ctx: &RunCtx, path: &Path, out: &Path, overwrite: bool) -> Result<()> {
    let groups = collect_dds_groups(path)?;
    let batch = ctx.map_results_compact(
        "dds conversion",
        &groups,
        |group| path_label(&group.header),
        |group, progress| progress.step(|| convert_dds_group(group, path, out, overwrite)),
    );
    let skipped = batch.skipped();
    let cancelled = batch.was_cancelled();
    let mut converted = Vec::new();
    let mut errors = Vec::new();

    for result in batch.into_completed() {
        match result {
            Ok(row) => converted.push(row),
            Err(error) => errors.push(error),
        }
    }
    converted.sort_by(|left, right| left.source.cmp(&right.source));

    let mut report = Report::new("dds conversion")
        .stat("textures", groups.len())
        .stat("converted", converted.len());
    let mut table = Table::new(["Source", "Output", "Bytes"]).right([2]);
    for row in converted {
        table.push([
            Cell::path(row.source),
            Cell::path(row.output),
            Cell::size(row.bytes),
        ]);
    }
    report.table_or(table, "no DDS textures converted");
    report.print();

    finish_scan(cancelled, skipped, &errors, "dds conversion")
}

/// Discover logical textures under a filesystem directory for the browser: every
/// DDS header with its ordered split-mip sidecars. Path classification fans out
/// across the job pool; the directory walk and grouping stay on the caller.
fn dds_browser_items_fs(ctx: &RunCtx, root: &Path) -> Result<Vec<crate::tui::DdsItem>> {
    let paths = collect_matching(root, |path| nw_dds::is_dds_path(path))?;
    let classified = ctx
        .runner
        .try_map(&paths, |path| -> Result<(nw_dds::SplitPart, String)> {
            Ok((split_part_for_path(path)?, path.to_string_lossy().into_owned()))
        })?;
    let mut items = group_sprites(group_dds_items(classified, |header| relative_label(root, header))?);
    items.sort_by(|left, right| left.label.cmp(&right.label));
    Ok(items)
}

/// Discover logical textures straight out of the install's pak archives via the
/// shared [`crate::source::Toc`], classify the DDS entries into logical textures,
/// and hand back the same path → (reader, entry) index so the browser's lazy reads
/// are O(1) (no re-parsing, no catalog — DDS browsing doesn't need it).
fn dds_browser_items_pak(
    ctx: &RunCtx,
    pak_paths: &[PathBuf],
) -> Result<(Vec<crate::tui::DdsItem>, crate::tui::PakIndex)> {
    let toc = crate::source::Toc::build(ctx, pak_paths, |name| nw_dds::is_dds_path(name));
    let classified = toc
        .names()
        .filter_map(|name| nw_dds::SplitPart::from_path(name).map(|part| (part, name.to_string())))
        .collect::<Vec<_>>();

    let mut items = group_sprites(group_dds_items(classified, |header| header.to_string())?);
    items.sort_by(|left, right| left.label.cmp(&right.label));
    Ok((items, toc.into_entries()))
}

/// Group classified `(part, key)` pairs into one [`crate::tui::DdsItem`] per
/// header, attaching its mip sidecars in load order. `label` derives the display
/// label from the resolved header key.
#[derive(Default)]
struct DdsGroupBuild {
    base_header: Option<String>,
    base_mips: Vec<(nw_dds::SplitPart, String)>,
    alpha_header: Option<String>,
    alpha_mips: Vec<(nw_dds::SplitPart, String)>,
}

fn group_dds_items(
    classified: Vec<(nw_dds::SplitPart, String)>,
    label: impl Fn(&str) -> String,
) -> Result<Vec<crate::tui::DdsItem>> {
    // Group every part — base header/mips and the attached-alpha header/mips —
    // under the one base key (`foo.dds`), so the alpha surface stays with its base.
    let mut groups = BTreeMap::<String, DdsGroupBuild>::new();
    for (part, key) in classified {
        let base = dds_base_key(&key, part)?;
        let group = groups.entry(base).or_default();
        match part {
            nw_dds::SplitPart::Header => group.base_header = Some(key),
            nw_dds::SplitPart::AlphaHeader => group.alpha_header = Some(key),
            nw_dds::SplitPart::Mip { alpha: false, .. } => group.base_mips.push((part, key)),
            nw_dds::SplitPart::Mip { alpha: true, .. } => group.alpha_mips.push((part, key)),
        }
    }

    let by_index =
        |(part, _): &(nw_dds::SplitPart, String)| part.mip_index().unwrap_or(0);
    Ok(groups
        .into_iter()
        .map(|(base, mut group)| {
            group.base_mips.sort_by_key(by_index);
            group.alpha_mips.sort_by_key(by_index);
            let alpha = (!group.alpha_mips.is_empty() || group.alpha_header.is_some()).then(|| {
                crate::tui::AlphaSurface {
                    header: group.alpha_header.unwrap_or_else(|| format!("{base}.a")),
                    sidecars: group.alpha_mips,
                }
            });
            let frame = crate::tui::DdsFrame {
                header: group.base_header.unwrap_or_else(|| base.clone()),
                sidecars: group.base_mips,
                alpha,
            };
            crate::tui::DdsItem::single(label(&base), frame)
        })
        .collect())
}

/// Collapse `X_NN.dds` frame sequences (≥2 frames) into one animatable sprite
/// entry; everything else passes through unchanged. Frames are co-located in a
/// pak, so this runs per-pak after [`group_dds_items`].
fn group_sprites(items: Vec<crate::tui::DdsItem>) -> Vec<crate::tui::DdsItem> {
    use std::collections::BTreeMap;
    // base label -> (frame number, item)
    let mut sprites: BTreeMap<String, Vec<(u32, crate::tui::DdsItem)>> = BTreeMap::new();
    let mut passthrough = Vec::new();
    for item in items {
        match sprite_base(&item.label) {
            Some((base, number)) => sprites.entry(base).or_default().push((number, item)),
            None => passthrough.push(item),
        }
    }
    let mut out = passthrough;
    for (base, mut group) in sprites {
        if group.len() < 2 {
            // A lone `_NN` is just a regular texture.
            out.extend(group.into_iter().map(|(_, item)| item));
            continue;
        }
        group.sort_by_key(|(number, _)| *number);
        let frames = group
            .into_iter()
            .flat_map(|(_, item)| item.frames)
            .collect::<Vec<_>>();
        out.push(crate::tui::DdsItem { label: format!("{base}_*.dds"), frames });
    }
    out
}

/// Split a `.dds` label into its sprite base and frame number. The frame number is
/// the trailing run of digits; an optional single `_` separator is absorbed into
/// the base. The underscore form accepts any digit count (`fx/spark_0.dds` →
/// (`fx/spark`, 0)); the bare form requires ≥2 digits (`ui/coin01.dds` →
/// (`ui/coin`, 1)) so ordinary names like `wood1.dds` aren't mistaken for frames.
fn sprite_base(label: &str) -> Option<(String, u32)> {
    let stem = label.strip_suffix(".dds")?;
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
    if base.is_empty() || (!separated && digits.len() < 2) {
        return None;
    }
    Some((base.to_string(), number))
}

/// Resolve the base texture key (`foo.dds`) for any DDS part — base or attached
/// alpha. Works on both filesystem and pak virtual paths (string, separator-safe).
fn dds_base_key(key: &str, part: nw_dds::SplitPart) -> Result<String> {
    match part {
        nw_dds::SplitPart::Header => Ok(key.to_string()),
        nw_dds::SplitPart::AlphaHeader => Ok(key
            .strip_suffix(".a")
            .or_else(|| key.strip_suffix(".A"))
            .unwrap_or(key)
            .to_string()),
        nw_dds::SplitPart::Mip { .. } => {
            let lower = key.to_ascii_lowercase();
            let pos = lower
                .rfind(".dds.")
                .with_context(|| format!("invalid DDS sidecar path: {key}"))?;
            Ok(key[..pos + ".dds".len()].to_string())
        }
    }
}

/// A texture's path relative to the browse root, with forward slashes.
fn relative_label(root: &Path, path: &str) -> String {
    Path::new(path)
        .strip_prefix(root)
        .map(|rel| rel.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_string())
        .replace('\\', "/")
}

fn collect_dds_groups(path: &Path) -> Result<Vec<DdsGroup>> {
    let paths = collect_dds_inputs(path)?;
    let mut groups = BTreeMap::<PathBuf, DdsGroup>::new();
    for dds_path in paths {
        let part = split_part_for_path(&dds_path)?;
        let header = dds_header_path(&dds_path, part)?;
        let group = groups.entry(header.clone()).or_insert_with(|| DdsGroup {
            header,
            sidecars: Vec::new(),
        });
        if matches!(part, nw_dds::SplitPart::Mip { .. }) {
            group.sidecars.push(dds_path);
        }
    }

    let mut groups = groups.into_values().collect::<Vec<_>>();
    for group in &mut groups {
        group.sidecars.sort();
    }
    groups.sort_by(|left, right| left.header.cmp(&right.header));
    Ok(groups)
}

fn collect_dds_inputs(path: &Path) -> Result<Vec<PathBuf>> {
    if !path.is_file() {
        return collect_matching(path, |path| nw_dds::is_dds_path(path));
    }

    let part = split_part_for_path(path)?;
    let header = dds_header_path(path, part)?;
    let parent = header.parent().filter(|path| !path.as_os_str().is_empty());
    let mut paths = Vec::new();
    for entry in std::fs::read_dir(parent.unwrap_or_else(|| Path::new(".")))
        .with_context(|| format!("scan DDS sidecars for {}", header.display()))?
    {
        let entry = entry?;
        if !entry.file_type()?.is_file() || !nw_dds::is_dds_path(entry.path()) {
            continue;
        }
        let candidate = entry.path();
        let candidate_part = split_part_for_path(&candidate)?;
        if dds_header_path(&candidate, candidate_part)? == header {
            paths.push(candidate);
        }
    }
    paths.sort();
    Ok(paths)
}

fn convert_dds_group(
    group: &DdsGroup,
    input: &Path,
    out: &Path,
    overwrite: bool,
) -> Result<DdsConvert> {
    let header_bytes =
        std::fs::read(&group.header).with_context(|| format!("read {}", group.header.display()))?;
    let mut sidecar_bytes = Vec::with_capacity(group.sidecars.len());
    for path in &group.sidecars {
        let part = split_part_for_path(path)?;
        let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
        sidecar_bytes.push((part, bytes));
    }
    sidecar_bytes.sort_by_key(|(part, _)| match part {
        nw_dds::SplitPart::Header | nw_dds::SplitPart::AlphaHeader => (0u8, 0u32),
        nw_dds::SplitPart::Mip { index, alpha } => (u8::from(*alpha), *index),
    });
    let sidecars = sidecar_bytes
        .iter()
        .map(|(part, bytes)| nw_dds::Sidecar::new(*part, bytes.as_slice()))
        .collect::<Vec<_>>();
    let ktx = nw_dds::Ktx2::from_dds(&header_bytes, &sidecars)
        .with_context(|| format!("convert {}", group.header.display()))?;
    let output = dds_ktx2_output_path(input, &group.header, out)?;
    write_guarded(&output, ktx.bytes(), overwrite.into())?;

    Ok(DdsConvert {
        source: group.header.display().to_string(),
        output: output.display().to_string(),
        bytes: format_size(ktx.bytes().len(), DECIMAL),
    })
}

fn split_part_for_path(path: &Path) -> Result<nw_dds::SplitPart> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .with_context(|| format!("DDS path is not UTF-8: {}", path.display()))?;
    nw_dds::SplitPart::from_path(name)
        .with_context(|| format!("not a DDS texture path: {}", path.display()))
}

fn dds_header_path(path: &Path, part: nw_dds::SplitPart) -> Result<PathBuf> {
    match part {
        nw_dds::SplitPart::Header | nw_dds::SplitPart::AlphaHeader => Ok(path.to_path_buf()),
        nw_dds::SplitPart::Mip { alpha, .. } => {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .with_context(|| format!("DDS path is not UTF-8: {}", path.display()))?;
            let lower = name.to_ascii_lowercase();
            let dds_pos = lower
                .rfind(".dds.")
                .with_context(|| format!("invalid DDS sidecar path: {}", path.display()))?;
            let mut header_name = name[..dds_pos + ".dds".len()].to_string();
            if alpha {
                header_name.push_str(".a");
            }
            Ok(path.with_file_name(header_name))
        }
    }
}

fn dds_ktx2_output_path(input: &Path, header: &Path, out: &Path) -> Result<PathBuf> {
    let exact_file = input.is_file()
        && out
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("ktx2"));
    if exact_file {
        return Ok(out.to_path_buf());
    }

    let relative = if input.is_dir() {
        header.strip_prefix(input).unwrap_or(header).to_path_buf()
    } else {
        PathBuf::from(
            header
                .file_name()
                .with_context(|| format!("DDS header has no file name: {}", header.display()))?,
        )
    };
    let mut output = out.join(relative);
    let file_name = output
        .file_name()
        .and_then(|name| name.to_str())
        .with_context(|| format!("DDS output path is not UTF-8: {}", output.display()))?;
    output.set_file_name(dds_ktx2_file_name(file_name));
    Ok(output)
}

fn dds_ktx2_file_name(name: &str) -> String {
    if let Some(stem) = strip_suffix_ignore_ascii_case(name, ".dds.a") {
        format!("{stem}.a.ktx2")
    } else if let Some(stem) = strip_suffix_ignore_ascii_case(name, ".dds") {
        format!("{stem}.ktx2")
    } else {
        format!("{name}.ktx2")
    }
}

fn scan_dds(path: &Path) -> Result<DdsScan> {
    let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let source = path.display().to_string();
    let asset = nw_dds::Asset::parse(&source, &bytes)
        .with_context(|| format!("parse {}", path.display()))?;
    Ok(match asset.kind() {
        nw_dds::AssetKind::Header(dds) => {
            let dx10 = dds.dx10().map_or_else(
                || "-".to_string(),
                |header| header.dxgi_format().to_string(),
            );
            DdsScan {
                source,
                kind: if dds.is_cry_extended() {
                    "DDS/Cry".to_string()
                } else {
                    "DDS".to_string()
                },
                dimensions: format!("{}x{}x{}", dds.width(), dds.height(), dds.depth().max(1)),
                mipmaps: format!(
                    "{} (persistent {})",
                    dds.mipmaps(),
                    dds.header().persistent_mips()
                ),
                format: dds.format_name(),
                dx10,
                cry: format!(
                    "flags=0x{:08x} split={} alpha={}",
                    dds.header().cry_flags().bits(),
                    dds.is_split(),
                    dds.has_attached_alpha()
                ),
                bytes: format_size(bytes.len(), DECIMAL),
            }
        }
        nw_dds::AssetKind::Split(payload) => {
            let part = payload.part();
            let mipmaps = part.mip_index().map_or_else(
                || "-".to_string(),
                |index| format!("{index} alpha={}", part.is_alpha()),
            );
            DdsScan {
                source,
                kind: part.to_string(),
                dimensions: "-".to_string(),
                mipmaps,
                format: "-".to_string(),
                dx10: "-".to_string(),
                cry: "-".to_string(),
                bytes: format_size(payload.bytes().len(), DECIMAL),
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::sprite_base;

    #[test]
    fn sprite_base_matches_underscore_and_bare_forms() {
        // Underscore form: any digit count.
        assert_eq!(sprite_base("fx/spark_0.dds"), Some(("fx/spark".into(), 0)));
        assert_eq!(sprite_base("fx/spark_07.dds"), Some(("fx/spark".into(), 7)));
        // Bare form: requires ≥2 digits.
        assert_eq!(sprite_base("ui/coin01.dds"), Some(("ui/coin".into(), 1)));
        assert_eq!(sprite_base("ui/coin12.dds"), Some(("ui/coin".into(), 12)));
    }

    #[test]
    fn sprite_base_rejects_non_frames() {
        // A single trailing digit without a separator is part of the name.
        assert_eq!(sprite_base("wood1.dds"), None);
        // No trailing digits at all.
        assert_eq!(sprite_base("stone.dds"), None);
        // All digits (no base) is not a frame.
        assert_eq!(sprite_base("00.dds"), None);
        // Not a DDS.
        assert_eq!(sprite_base("fx/spark_00.png"), None);
    }
}
