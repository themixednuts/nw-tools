//! `format model` — convert Cry meshes (`.cgf`/`.skin`) into glTF source assets.
//!
//! Both the filesystem and the install's paks present the same [`AssetSource`]
//! interface (read an asset by path; resolve a material GUID), so one converter
//! drives single-file, directory-batch, and whole-install exports. The install
//! source loads New World's asset catalog from `Engine.pak` to resolve each mesh's
//! material by its MtlName GUID rather than guessing by file name.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use clap::{Args, ValueEnum};
use humansize::{DECIMAL, format_size};
use image::ImageEncoder;
use nw_asset::{AssetCatalog, AssetId, Raoc, Rasc};
use nw_pak::PakMmapReader;
use uuid::Uuid;

use crate::jobs::{JobArgs, RunCtx};
use crate::support::{PakSet, ScanIssues, collect_matching, path_ext};
use crate::ui::Report;

/// Output container.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Container {
    Glb,
    Gltf,
}

impl Container {
    const fn extension(self) -> &'static str {
        match self {
            Self::Glb => "glb",
            Self::Gltf => "gltf",
        }
    }
}

#[derive(Debug, Args)]
pub struct Model {
    /// A `.cgf`/`.skin` file, or a directory to batch-convert. Omit to convert
    /// from the located install's paks (narrow with `--filter`).
    path: Option<PathBuf>,

    /// Output file (single) or directory (batch). Defaults beside the input, or
    /// `./models` for install mode.
    #[arg(long)]
    out: Option<PathBuf>,

    /// Output container.
    #[arg(long, value_enum, default_value_t = Container::Glb)]
    format: Container,

    /// Material file override (single-file mode); otherwise resolved automatically.
    #[arg(long)]
    mtl: Option<PathBuf>,

    /// Case-insensitive path substring filter (install mode).
    #[arg(long)]
    filter: Option<String>,

    /// Geometry only — skip materials and textures.
    #[arg(long)]
    no_materials: bool,

    /// Replace existing output files.
    #[arg(long)]
    overwrite: bool,

    #[command(flatten)]
    jobs: JobArgs,
}

impl Model {
    pub fn run(self) -> Result<()> {
        let ctx = self.jobs.ctx()?;
        match self.path.clone() {
            None => self.export_install(&ctx),
            Some(path) if path.is_dir() => self.export_tree(&ctx, &path),
            Some(path) => self.export_file(&path),
        }
    }

    /// Convert a single mesh file on disk.
    fn export_file(&self, path: &Path) -> Result<()> {
        let source = Tree::around(path);
        let out = self
            .out
            .clone()
            .unwrap_or_else(|| path.with_extension(self.format.extension()));
        self.guard_existing(&out)?;
        let cgf = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
        let heap = std::fs::read(heap_sibling(path)).unwrap_or_default();
        let mtl_override = self
            .mtl
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok());
        let stats = self.convert(&source, &cgf, &heap, &MeshRef::for_file(path), mtl_override, &out)?;

        Report::new("model")
            .stat("source", path.display())
            .stat("meshes", stats.meshes)
            .stat("vertices", stats.vertices)
            .stat("triangles", stats.triangles)
            .stat("joints", stats.joints)
            .stat("materials", stats.materials)
            .stat("textures", stats.textures)
            .stat("output", out.display())
            .stat("bytes", format_size(stats.bytes, DECIMAL))
            .print();
        Ok(())
    }

    /// Batch-convert every mesh under a directory, in parallel.
    fn export_tree(&self, ctx: &RunCtx, dir: &Path) -> Result<()> {
        let out_dir = self.out.clone().unwrap_or_else(|| dir.to_path_buf());
        let meshes = collect_matching(dir, is_mesh_file)?;
        let batch = ctx.map_results_compact(
            "model",
            &meshes,
            |path| display_path(path),
            |path, progress| {
                progress.step(|| {
                    let source = Tree::around(path);
                    let relative = path.strip_prefix(dir).unwrap_or(path);
                    let out = out_dir.join(relative).with_extension(self.format.extension());
                    self.guard_existing(&out)?;
                    create_parent(&out)?;
                    let cgf = std::fs::read(path)?;
                    let heap = std::fs::read(heap_sibling(path)).unwrap_or_default();
                    self.convert(&source, &cgf, &heap, &MeshRef::for_file(path), None, &out)
                })
            },
        );
        report_batch(&batch.into_completed(), dir.display().to_string())
    }

    /// Convert meshes straight out of the install's paks (+ asset catalog), parallel.
    fn export_install(&self, ctx: &RunCtx) -> Result<()> {
        let install = nw_locator::Install::locate().context(
            "no New World install found; pass a path, or run `nw-tools locate` to check detection",
        )?;
        let source = Install::open(ctx, &install.assets())?;
        let meshes = source.meshes(self.filter.as_deref());
        if meshes.is_empty() {
            bail!("no matching meshes found in the install paks");
        }
        let out_dir = self.out.clone().unwrap_or_else(|| PathBuf::from("models"));

        let batch = ctx.map_results_compact(
            "model",
            &meshes,
            Clone::clone,
            |key, progress| {
                progress.step(|| {
                    let out = out_dir.join(key).with_extension(self.format.extension());
                    self.guard_existing(&out)?;
                    create_parent(&out)?;
                    let cgf = source.read(key).with_context(|| format!("read {key}"))?;
                    let heap = source.read(&format!("{key}heap")).unwrap_or_default();
                    self.convert(&source, &cgf, &heap, &MeshRef::for_key(key), None, &out)
                })
            },
        );
        report_batch(&batch.into_completed(), install.assets().display().to_string())
    }

    fn guard_existing(&self, out: &Path) -> Result<()> {
        if out.exists() && !self.overwrite {
            bail!("{} exists (pass --overwrite to replace it)", out.display());
        }
        Ok(())
    }

    /// The shared conversion: assemble the model, resolve materials/textures via the
    /// source, and write the chosen container.
    fn convert(
        &self,
        source: &dyn AssetSource,
        cgf: &[u8],
        heap: &[u8],
        mesh: &MeshRef,
        mtl_override: Option<String>,
        out: &Path,
    ) -> Result<ModelStats> {
        let model = nw_model::model_from_bytes(cgf, heap)
            .with_context(|| format!("assemble {}", mesh.stem))?;

        let materials = if self.no_materials {
            None
        } else {
            mtl_override
                .and_then(|xml| xml.parse::<nw_model::MaterialSet>().ok())
                .or_else(|| source.materials(cgf, mesh))
        };

        let mut textures = 0usize;
        let bytes = {
            let mut load = |file: &str| {
                let data = decode_texture(source, file);
                textures += usize::from(data.is_some());
                data
            };
            match (&materials, self.format) {
                (Some(set), Container::Glb) => {
                    let glb = nw_model::Gltf::new(&model).materials(set).to_glb(&mut load);
                    write_glb(out, &glb)?
                }
                (Some(set), Container::Gltf) => {
                    let (json, blob) =
                        nw_model::Gltf::new(&model).materials(set).to_gltf(&bin_uri(out), &mut load);
                    write_gltf(out, &json, &blob)?
                }
                (None, Container::Glb) => write_glb(out, &nw_model::Gltf::new(&model).to_glb())?,
                (None, Container::Gltf) => {
                    let (json, blob) = nw_model::Gltf::new(&model).to_gltf(&bin_uri(out));
                    write_gltf(out, &json, &blob)?
                }
            }
        };

        Ok(ModelStats {
            meshes: model.meshes.len(),
            vertices: model.vertex_count(),
            triangles: model.triangle_count(),
            joints: model.skeleton.as_ref().map_or(0, |s| s.bones.len()),
            materials: materials.as_ref().map_or(0, |m| m.sub_materials.len()),
            textures,
            bytes,
        })
    }
}

/// Reads assets and resolves a mesh's material, abstracting over the filesystem and
/// the install's paks.
trait AssetSource: Sync {
    /// Read an asset's bytes by virtual path (forward slashes, case-insensitive).
    fn read(&self, path: &str) -> Option<Vec<u8>>;

    /// Resolve the material set for a mesh, using whatever the source affords —
    /// the catalog (install) or a sibling-directory scan (filesystem).
    fn materials(&self, cgf: &[u8], mesh: &MeshRef) -> Option<nw_model::MaterialSet>;
}

/// Parse the first `.mtl` among `keys` that this source can read.
fn first_material(
    source: &dyn AssetSource,
    keys: impl IntoIterator<Item = String>,
) -> Option<nw_model::MaterialSet> {
    keys.into_iter().find_map(|key| {
        let xml = source.read(&key)?;
        String::from_utf8_lossy(&xml).parse::<nw_model::MaterialSet>().ok()
    })
}

/// A filesystem asset tree. `read` resolves a virtual path against the mesh's
/// directory and its ancestors, so sibling `.mtl`s and extract-rooted texture paths
/// both resolve.
struct Tree {
    roots: Vec<PathBuf>,
}

impl Tree {
    fn around(mesh: &Path) -> Self {
        let mut roots = Vec::new();
        let mut dir = mesh.parent();
        while let Some(current) = dir {
            roots.push(current.to_path_buf());
            dir = current.parent();
        }
        Self { roots }
    }
}

impl AssetSource for Tree {
    fn read(&self, path: &str) -> Option<Vec<u8>> {
        self.roots
            .iter()
            .map(|root| root.join(path))
            .find(|candidate| candidate.is_file())
            .and_then(|candidate| std::fs::read(candidate).ok())
    }

    /// Pick the sibling `.mtl` whose sub-materials best match the mesh's MtlName
    /// chunk (and the `foo_mesh` → `foo_mat` naming convention), then fall back to
    /// convention-named candidates.
    fn materials(&self, cgf: &[u8], mesh: &MeshRef) -> Option<nw_model::MaterialSet> {
        let dir = self.roots.first()?;
        let wanted = cgf_submaterial_names(cgf);
        let best = std::fs::read_dir(dir)
            .ok()?
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path_ext(path).as_deref() == Some("mtl"))
            .max_by_key(|path| mtl_match_score(path, &mesh.stem, &wanted));
        best.and_then(|path| std::fs::read_to_string(path).ok())
            .and_then(|xml| xml.parse::<nw_model::MaterialSet>().ok())
            .or_else(|| first_material(self, mesh.mtl_candidates()))
    }
}

/// Score how well a candidate `.mtl` matches a mesh: naming-convention hits weigh
/// heavily, then the count of shared sub-material names.
fn mtl_match_score(path: &Path, stem: &str, wanted: &[String]) -> usize {
    let candidate = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let name_bonus = usize::from(
        candidate == format!("{stem}_mat")
            || candidate == stem
            || candidate == format!("{stem}_material"),
    );
    let overlap = std::fs::read_to_string(path)
        .ok()
        .and_then(|xml| xml.parse::<nw_model::MaterialSet>().ok())
        .map_or(0, |set| {
            set.sub_materials
                .iter()
                .filter(|sub| wanted.iter().any(|w| w.eq_ignore_ascii_case(&sub.name)))
                .count()
        });
    name_bonus * 100 + overlap
}

/// The sub-material names from a mesh's MtlName chunk (for `.mtl` matching).
fn cgf_submaterial_names(cgf: &[u8]) -> Vec<String> {
    cry_chunk::CgfFile::parse(cgf)
        .ok()
        .and_then(|file| {
            file.materials()
                .values()
                .next()
                .map(|mtl| mtl.sub_material_names.iter().map(|s| s.to_string()).collect())
        })
        .unwrap_or_default()
}

/// The install's paks plus its asset catalog. `read` resolves a virtual path through
/// the pak table-of-contents; `material_path` resolves a GUID through the catalog.
struct Install {
    toc: HashMap<String, (Arc<PakMmapReader>, usize)>,
    catalog: AssetCatalog,
}

impl Install {
    fn open(ctx: &RunCtx, assets: &Path) -> Result<Self> {
        let paks = PakSet::collect(assets.to_path_buf(), Vec::new())?;
        if paks.paths().is_empty() {
            bail!("no pak archives found in {}", assets.display());
        }
        let toc = build_toc(ctx, paks.paths());
        let catalog = load_catalog(ctx, &toc).context("load asset catalog from Engine.pak")?;
        Ok(Self { toc, catalog })
    }

    /// Mesh asset paths (`.cgf`/`.skin`), optionally filtered by substring.
    fn meshes(&self, filter: Option<&str>) -> Vec<String> {
        let filter = filter.map(str::to_ascii_lowercase);
        let mut meshes = self
            .toc
            .keys()
            .filter(|key| key.ends_with(".cgf") || key.ends_with(".skin"))
            .filter(|key| filter.as_ref().is_none_or(|f| key.contains(f.as_str())))
            .cloned()
            .collect::<Vec<_>>();
        meshes.sort();
        meshes
    }
}

impl Install {
    /// Resolve a MtlName-chunk GUID to its `.mtl` path via the asset catalog.
    fn material_path(&self, guid: &str) -> Option<String> {
        let id = guid_to_asset_id(guid)?;
        self.catalog.entry_by_id(id).map(|entry| entry.path().to_string())
    }
}

impl AssetSource for Install {
    fn read(&self, path: &str) -> Option<Vec<u8>> {
        let (reader, entry) = self.toc.get(&path.to_ascii_lowercase())?;
        reader.read_wrapped_by_index(*entry).ok()
    }

    /// Resolve the material by the mesh's MtlName GUID through the catalog, falling
    /// back to convention-named siblings.
    fn materials(&self, cgf: &[u8], mesh: &MeshRef) -> Option<nw_model::MaterialSet> {
        let by_guid = mtlname_guid(cgf).and_then(|guid| self.material_path(&guid));
        first_material(self, by_guid.into_iter().chain(mesh.mtl_candidates()))
    }
}

/// Identifies a mesh for material resolution: its sub-material GUID hint plus the
/// virtual directory and base name used for naming-convention fallback.
struct MeshRef {
    dir: String,
    stem: String,
}

impl MeshRef {
    /// For a pak virtual path like `a/b/foo_mesh.cgf`.
    fn for_key(key: &str) -> Self {
        let (dir, file) = key.rsplit_once('/').unwrap_or(("", key));
        Self {
            dir: dir.to_string(),
            stem: mesh_stem(file).to_string(),
        }
    }

    /// For a filesystem path: the `.mtl` is a sibling, so the directory is empty
    /// (the [`Tree`] source resolves siblings via its roots).
    fn for_file(path: &Path) -> Self {
        let file = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        Self {
            dir: String::new(),
            stem: mesh_stem(file).to_string(),
        }
    }

    /// Candidate `.mtl` keys by naming convention.
    fn mtl_candidates(&self) -> Vec<String> {
        let names = [
            format!("{}_mat.mtl", self.stem),
            format!("{}.mtl", self.stem),
            format!("{}_material.mtl", self.stem),
        ];
        names
            .into_iter()
            .map(|name| {
                if self.dir.is_empty() {
                    name
                } else {
                    format!("{}/{name}", self.dir)
                }
            })
            .collect()
    }
}

/// The sub-material GUID recorded in a mesh's MtlName chunk.
fn mtlname_guid(cgf: &[u8]) -> Option<String> {
    let file = cry_chunk::CgfFile::parse(cgf).ok()?;
    file.materials()
        .values()
        .next()
        .map(|mtl| mtl.name.to_string())
}

/// Map a Cry MtlName GUID to a Lumberyard catalog [`AssetId`].
///
/// AzCore stores UUIDs as straight big-endian bytes (`Uuid::CreateName` →
/// `from_be_bytes`), but the MtlName chunk records the GUID in Microsoft display
/// form, where Data1/Data2/Data3 are little-endian. Reinterpreting those three
/// fields (`swap_bytes`) yields the AZ-canonical UUID. A `.mtl` is a single-product
/// source asset, so its product sub-id is 0.
fn guid_to_asset_id(guid: &str) -> Option<AssetId> {
    let uuid = Uuid::parse_str(guid.trim()).ok()?;
    let (d1, d2, d3, d4) = uuid.as_fields();
    let canonical = Uuid::from_fields(d1.swap_bytes(), d2.swap_bytes(), d3.swap_bytes(), d4);
    Some(AssetId::new(canonical, 0))
}

/// Decode a referenced texture (`.tif` → `.dds`, split mips assembled) to PNG bytes.
fn decode_texture(source: &dyn AssetSource, file: &str) -> Option<nw_model::TextureData> {
    let dds = tif_to_dds(file);
    let header = source.read(&dds)?;
    let mut sidecars = Vec::new();
    let mut mip = 1u32;
    while let Some(bytes) = source.read(&format!("{dds}.{mip}")) {
        sidecars.push((
            nw_dds::SplitPart::Mip {
                index: mip,
                alpha: false,
            },
            bytes,
        ));
        mip += 1;
    }
    let parts = sidecars
        .iter()
        .map(|(part, bytes)| nw_dds::Sidecar::new(*part, bytes.as_slice()))
        .collect::<Vec<_>>();
    let decoded = nw_dds::decode_top_mip(&header, &parts).ok()?;
    let image = image::RgbaImage::from_raw(decoded.width, decoded.height, decoded.rgba)?;

    let mut bytes = Vec::new();
    image::codecs::png::PngEncoder::new(&mut bytes)
        .write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            image::ExtendedColorType::Rgba8,
        )
        .ok()?;
    Some(nw_model::TextureData {
        bytes,
        mime: "image/png".to_string(),
    })
}

/// The `.bin` sidecar URI for a `.gltf` output (its file name).
fn bin_uri(out: &Path) -> String {
    out.with_extension("bin")
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("model.bin")
        .to_string()
}

fn write_glb(out: &Path, glb: &[u8]) -> Result<usize> {
    std::fs::write(out, glb).with_context(|| format!("write {}", out.display()))?;
    Ok(glb.len())
}

fn write_gltf(out: &Path, json: &str, blob: &[u8]) -> Result<usize> {
    let bin = out.with_extension("bin");
    std::fs::write(out, json.as_bytes()).with_context(|| format!("write {}", out.display()))?;
    std::fs::write(&bin, blob).with_context(|| format!("write {}", bin.display()))?;
    Ok(json.len() + blob.len())
}

/// Build the pak table of contents: every entry's virtual path → (reader, index),
/// enumerated across all archives in parallel.
fn build_toc(ctx: &RunCtx, pak_paths: &[PathBuf]) -> HashMap<String, (Arc<PakMmapReader>, usize)> {
    let per_pak = ctx.runner.map(pak_paths, |path| {
        let mut found = Vec::new();
        if let Ok(reader) = PakMmapReader::open(path) {
            let reader = Arc::new(reader);
            for entry in reader.entries() {
                found.push((entry.name().to_ascii_lowercase(), entry.index(), reader.clone()));
            }
        }
        found
    });
    let mut toc = HashMap::new();
    for list in per_pak {
        for (name, entry, reader) in list {
            toc.insert(name, (reader, entry));
        }
    }
    toc
}

/// Load and parse the asset catalog from the pak TOC (rasc + raoc parsed in parallel).
fn load_catalog(
    ctx: &RunCtx,
    toc: &HashMap<String, (Arc<PakMmapReader>, usize)>,
) -> Result<AssetCatalog> {
    let read = |key: &str| -> Option<Vec<u8>> {
        let (reader, entry) = toc.get(key)?;
        reader.read_wrapped_by_index(*entry).ok()
    };
    let rasc_bytes = read(nw_asset::ASSET_CATALOG_PATH)
        .context("assetcatalog.catalog not found in paks")?;
    let raoc_bytes = read(nw_asset::ASSET_CATALOG_OPTIMIZED_PATH);

    let (rasc, raoc) = ctx.runner.join(
        || Rasc::parse(&rasc_bytes),
        || raoc_bytes.as_deref().map(Raoc::parse).transpose(),
    );
    Ok(AssetCatalog::new(rasc?, raoc?))
}

#[derive(Debug, Default, Clone, Copy)]
struct ModelStats {
    meshes: usize,
    vertices: usize,
    triangles: usize,
    joints: usize,
    materials: usize,
    textures: usize,
    bytes: usize,
}

fn report_batch(results: &[Result<ModelStats>], source: String) -> Result<()> {
    let mut converted = 0usize;
    let mut vertices = 0usize;
    let mut errors = Vec::new();
    for result in results {
        match result {
            Ok(stats) => {
                converted += 1;
                vertices += stats.vertices;
            }
            Err(error) => errors.push(anyhow::anyhow!("{error:#}")),
        }
    }
    Report::new("model")
        .stat("source", source)
        .stat("converted", converted)
        .stat("vertices", vertices)
        .print();
    ScanIssues::new("model", 0, false, errors).finish()
}

/// The geometry-heap sidecar (`foo.cgf` → `foo.cgfheap`, `foo.skin` → `foo.skinheap`).
fn heap_sibling(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push("heap");
    PathBuf::from(name)
}

fn is_mesh_file(path: &Path) -> bool {
    matches!(path_ext(path).as_deref(), Some("cgf" | "skin"))
}

/// A mesh base name with the `_mesh`/`_lod0` suffix stripped, for `.mtl` matching.
fn mesh_stem(file: &str) -> &str {
    let stem = file
        .rsplit_once('.')
        .map_or(file, |(stem, _)| stem);
    stem.strip_suffix("_mesh")
        .or_else(|| stem.strip_suffix("_lod0"))
        .unwrap_or(stem)
}

/// Map an authored texture path (usually `.tif`) to its shipped `.dds`.
fn tif_to_dds(file: &str) -> String {
    match file.rsplit_once('.') {
        Some((stem, ext)) if ext.eq_ignore_ascii_case("tif") || ext.eq_ignore_ascii_case("tiff") => {
            format!("{stem}.dds")
        }
        _ => file.to_string(),
    }
}

fn create_parent(out: &Path) -> Result<()> {
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create {}", parent.display()))?;
    }
    Ok(())
}

fn display_path(path: &Path) -> String {
    path.display().to_string()
}
