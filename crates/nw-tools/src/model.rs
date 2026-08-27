//! `format model` — convert complete Cry model/character graphs into glTF.
//!
//! Both the filesystem and the install's paks present the same [`AssetSource`]
//! interface (read an asset by path; resolve a material GUID), so one converter
//! drives single-file, directory-batch, and whole-install exports. The install
//! source loads New World's asset catalog from `Engine.pak` to resolve each mesh's
//! material by its MtlName GUID rather than guessing by file name.

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::{Args, ValueEnum};
use humansize::{DECIMAL, format_size};
use image::ImageEncoder;
use nw_artifact::PackageWriter;

use crate::jobs::{JobArgs, RunCtx};
use crate::source::{self, Install};
use crate::support::{ScanIssues, collect_matching, ensure_parent, guard_existing, path_ext};
use crate::ui::Report;

/// Formats which can own or indirectly select a render model. Structured glTF
/// exports index these once, then reuse the reverse graph for every model in a
/// batch. Opaque leaf formats remain handled by the normal forward closure.
const MODEL_CONSUMER_EXTENSIONS: &[&str] = &[
    "cdf",
    "cloth",
    "slice",
    "dynamicslice",
    "entity",
    "entities",
    "entities_xml",
    "prefab",
    "slicedata",
    "meta",
    "metadata",
    "chunks",
    "distribution",
    "vegetation",
    "terrain.json",
    "tracts.json",
    "terrain",
    "worldmat",
    "regionmat",
];

/// Output container.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Container {
    /// Self-contained binary glTF file.
    Glb,
    /// JSON glTF manifest with external resources.
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
    /// A `.cgf`/`.skin`/`.chr`/`.cdf`/`.caf`/`.dba` file, or a directory to batch-convert. Omit to convert
    /// from the located install's paks (narrow with `--filter`).
    path: Option<PathBuf>,

    /// Output file for a single input. Defaults beside the input.
    #[arg(long, value_name = "FILE", conflicts_with = "out_dir")]
    out: Option<PathBuf>,

    /// Output directory for directory or installed-asset conversion.
    /// Defaults to the input directory, or `./models` for install mode.
    #[arg(long = "out-dir", value_name = "DIR", conflicts_with = "out")]
    out_dir: Option<PathBuf>,

    /// Output container.
    #[arg(long, value_enum, default_value_t = Container::Glb)]
    container: Container,

    /// Material file override (single-file mode); otherwise resolved automatically.
    #[arg(long)]
    mtl: Option<PathBuf>,

    /// Skeleton asset for standalone CAF or skinned-geometry export.
    #[arg(long)]
    skeleton: Option<String>,

    /// Additional CAF/i_caf assets to embed as glTF animations.
    #[arg(long = "animation")]
    animations: Vec<String>,

    /// Cry `.animevents` database to attach to exported CAF clips.
    #[arg(long)]
    animation_events: Option<String>,

    /// Mannequin ADB/tag/controller/blend-space source to retain in glTF extras.
    #[arg(long = "mannequin")]
    mannequin: Vec<String>,

    /// ATL controls, Wwise BNK/WEM, or trigger-bank maps to validate and retain in glTF extras.
    #[arg(long = "audio")]
    audio: Vec<String>,

    /// Skip Wwise → WAV decoding (default for glTF is to decode).
    #[arg(long = "no-decode-audio")]
    no_decode_audio: bool,

    /// Skip writing a playable Blender `.blend` next to each manifest (default for glTF is to write one).
    #[arg(long = "no-blend")]
    no_blend: bool,

    /// Explicit path to `vgmstream-cli` (otherwise PATH / WinGet install).
    #[arg(long = "vgmstream", value_name = "PATH")]
    vgmstream: Option<PathBuf>,

    /// Explicit path to `blender` (otherwise PATH / Program Files).
    #[arg(long = "blender", value_name = "PATH")]
    blender: Option<PathBuf>,

    /// Case-insensitive path substring filter (install mode). Repeat `--filter`
    /// to select several characters in one run (union match, one dependency-index
    /// build). Characters export one at a time; `--jobs` parallelizes work inside
    /// each character. Omit to convert the whole install.
    #[arg(long = "filter", conflicts_with = "path")]
    filters: Vec<String>,

    /// Geometry only — skip materials and textures.
    #[arg(long = "geometry-only")]
    geometry_only: bool,

    /// Replace existing output files.
    #[arg(long = "force")]
    overwrite: bool,

    /// Validate arguments and print the conversion plan without writing outputs.
    #[arg(long)]
    dry_run: bool,

    #[command(flatten)]
    jobs: JobArgs,
}

impl Model {
    fn decode_audio_enabled(&self) -> bool {
        self.container == Container::Gltf && !self.no_decode_audio
    }

    fn blend_enabled(&self) -> bool {
        self.container == Container::Gltf && !self.no_blend
    }

    /// Write one playable `.blend` per exported manifest, placed next to that
    /// manifest (`<manifest dir>/<stem>.blend`). Each manifest's directory is
    /// unique, so the blend names never collide. `package_root` is the shared
    /// package writer root (audio WAVs resolve relative to it).
    fn write_blends(&self, package_root: &Path, manifests: &[PathBuf]) -> Result<()> {
        if !self.blend_enabled() || manifests.is_empty() {
            return Ok(());
        }
        let blender = match self
            .blender
            .clone()
            .or_else(crate::audio_export::find_blender)
        {
            Some(path) => path,
            None => {
                eprintln!(
                    "note: skipping .blend write - blender not found (install Blender or pass --blender; use --no-blend to silence)"
                );
                return Ok(());
            }
        };
        let mut written = 0usize;
        for manifest in manifests {
            let stem = manifest
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("model");
            let blend_path = manifest.with_file_name(format!("{stem}.blend"));
            crate::audio_export::write_playable_blend(
                &blender,
                manifest,
                package_root,
                &blend_path,
            )
            .with_context(|| format!("write playable blend {}", blend_path.display()))?;
            eprintln!("blend {}", blend_path.display());
            written += 1;
        }
        eprintln!("{written} blend(s) written");
        Ok(())
    }

    pub fn run(self) -> Result<()> {
        let ctx = self.jobs.ctx()?;
        let single_file = self.path.as_ref().is_some_and(|path| !path.is_dir());
        if single_file && self.out_dir.is_some() {
            bail!("use --out <FILE> when converting one model file");
        }
        if !single_file && self.out.is_some() {
            bail!("use --out-dir <DIR> for directory or installed-asset conversion");
        }
        if self.dry_run {
            let input = self.path.as_ref().map_or_else(
                || "<installed assets>".to_owned(),
                |path| path.display().to_string(),
            );
            let output = self
                .out
                .as_ref()
                .or(self.out_dir.as_ref())
                .map_or_else(|| "<default>".to_owned(), |path| path.display().to_string());
            Report::new("model")
                .stat("dry-run", "yes")
                .stat("input", input)
                .stat("output", output)
                .stat("container", self.container.extension())
                .print();
            return Ok(());
        }
        match self.path.clone() {
            None => self.export_install(&ctx),
            Some(path) if path.is_dir() => self.export_tree(&ctx, &path),
            Some(path) => self.export_file(&ctx, &path),
        }
    }

    /// Convert a single mesh file on disk.
    fn export_file(&self, ctx: &RunCtx, path: &Path) -> Result<()> {
        let source = Tree::around(path);
        let index_source = Tree::rooted(path.parent().unwrap_or_else(|| Path::new(".")));
        let dependency_index = self.build_model_dependency_index(ctx, &index_source)?;
        let out = self
            .out
            .clone()
            .unwrap_or_else(|| path.with_extension(self.container.extension()));
        let package = if self.container == Container::Gltf {
            Some(PackageWriter::new(
                out.parent().unwrap_or_else(|| Path::new(".")),
            )?)
        } else {
            None
        };
        let artifact = out
            .file_name()
            .map(PathBuf::from)
            .context("model output has no file name")?;
        guard_existing(&out, self.overwrite.into())?;
        let cgf = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
        let heap = std::fs::read(heap_sibling(path)).unwrap_or_default();
        let mtl_override = self
            .mtl
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok());
        let source_path = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .context("model input has no file name")?;
        let stats = self.convert(
            &ctx.runner,
            &source,
            ConvertRequest {
                source_path: &source_path,
                cgf: &cgf,
                heap: &heap,
                mesh: MeshRef::for_file(path),
                mtl_override,
                out: &out,
                package: package.as_ref(),
                artifact: &artifact,
                dependency_index: dependency_index.as_ref(),
            },
        )?;

        Report::new("model")
            .stat("source", path.display())
            .stat("meshes", stats.meshes)
            .stat("vertices", stats.vertices)
            .stat("triangles", stats.triangles)
            .stat("joints", stats.joints)
            .stat("materials", stats.materials)
            .stat("textures", stats.textures)
            .stat("animations", stats.animations)
            .stat("output", out.display())
            .stat("bytes", format_size(stats.bytes, DECIMAL))
            .print();
        if package.is_some() {
            let package_root = out.parent().unwrap_or_else(|| Path::new(".")).to_path_buf();
            self.write_blends(&package_root, std::slice::from_ref(&out))?;
        }
        Ok(())
    }

    /// Batch-convert every mesh under a directory.
    ///
    /// Characters/meshes run one at a time so a full resolved package (all clips,
    /// decoded audio, glTF buffers) is never multiplied by host parallelism.
    /// `--jobs` still parallelizes work *inside* each conversion.
    fn export_tree(&self, ctx: &RunCtx, dir: &Path) -> Result<()> {
        let out_dir = self.out_dir.clone().unwrap_or_else(|| dir.to_path_buf());
        let package = if self.container == Container::Gltf {
            Some(PackageWriter::new(&out_dir)?)
        } else {
            None
        };
        let meshes = collect_matching(dir, is_mesh_file)?;
        let index_source = Tree::rooted(dir);
        let dependency_index = self.build_model_dependency_index(ctx, &index_source)?;
        let results = export_roots_sequentially(
            ctx,
            meshes.len(),
            |index| display_path(&meshes[index]),
            |index, job| {
                let path = &meshes[index];
                job.step(|| {
                    let source = Tree::around(path);
                    let relative = path.strip_prefix(dir).unwrap_or(path);
                    let artifact = relative.with_extension(self.container.extension());
                    let out = out_dir.join(&artifact);
                    guard_existing(&out, self.overwrite.into())?;
                    ensure_parent(&out)?;
                    let cgf = std::fs::read(path)?;
                    let heap = std::fs::read(heap_sibling(path)).unwrap_or_default();
                    let source_path = relative.to_string_lossy().replace('\\', "/");
                    let stats = self.convert(
                        &ctx.runner,
                        &source,
                        ConvertRequest {
                            source_path: &source_path,
                            cgf: &cgf,
                            heap: &heap,
                            mesh: MeshRef::for_file(path),
                            mtl_override: None,
                            out: &out,
                            package: package.as_ref(),
                            artifact: &artifact,
                            dependency_index: dependency_index.as_ref(),
                        },
                    )?;
                    let manifest = package.is_some().then(|| out.clone());
                    Ok(Exported { stats, manifest })
                })
            },
        );
        report_batch(&results, dir.display().to_string())?;
        self.write_blends(&out_dir, &exported_manifests(&results))?;
        Ok(())
    }

    /// Convert meshes straight out of the install's paks (+ asset catalog).
    ///
    /// Selected characters export sequentially (one resolved package resident at
    /// a time). Shared dependency-index build and in-character `--jobs`
    /// parallelism are unchanged.
    fn export_install(&self, ctx: &RunCtx) -> Result<()> {
        let install = source::locate()?;
        let source = Install::open(ctx, &install.assets())?;
        let mut meshes = source.paths_with_extensions(
            &["cgf", "skin", "chr", "cga", "cdf", "caf", "i_caf", "dba"],
            &self.filters,
        );
        if meshes.is_empty() {
            bail!("no matching meshes found in the install paks");
        }
        let out_dir = self
            .out_dir
            .clone()
            .unwrap_or_else(|| PathBuf::from("models"));
        let package = if self.container == Container::Gltf {
            Some(PackageWriter::new(&out_dir)?)
        } else {
            None
        };
        let dependency_index = self.install_dependency_index(ctx, &source, &install.assets())?;
        if !self.filters.is_empty()
            && let Some(index) = dependency_index.as_ref()
        {
            let roots = character_roots(&meshes);
            // A character filter such as `isabella_t2` also matches its `.skin`
            // and `.chr` leaves. Those are dependency-owned parts whose material
            // and skeleton context lives in the CDF, not separate export roots.
            // Prefer complete CDF roots whenever any match; standalone mesh/CAF
            // queries retain the original all-matches behavior.
            if !roots.is_empty() {
                meshes.clone_from(&roots);
            }
            for root in roots {
                meshes.extend(
                    crate::model_asset::context_variant_cdfs(&source, &root, index)
                        .with_context(|| format!("discover context variants for {root}"))?,
                );
            }
            meshes.sort_by_key(|path| path.to_ascii_lowercase());
            meshes.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
        }

        let results = export_roots_sequentially(
            ctx,
            meshes.len(),
            |index| meshes[index].clone(),
            |index, job| {
                let key = &meshes[index];
                job.step(|| {
                    let artifact = Path::new(key).with_extension(self.container.extension());
                    let out = out_dir.join(&artifact);
                    guard_existing(&out, self.overwrite.into())?;
                    ensure_parent(&out)?;
                    let cgf = source.read(key).with_context(|| format!("read {key}"))?;
                    let heap = source.read(&format!("{key}heap")).unwrap_or_default();
                    let stats = self.convert(
                        &ctx.runner,
                        &source,
                        ConvertRequest {
                            source_path: key,
                            cgf: &cgf,
                            heap: &heap,
                            mesh: MeshRef::for_key(key),
                            mtl_override: None,
                            out: &out,
                            package: package.as_ref(),
                            artifact: &artifact,
                            dependency_index: dependency_index.as_ref(),
                        },
                    )?;
                    let manifest = package.is_some().then(|| out.clone());
                    Ok(Exported { stats, manifest })
                })
            },
        );
        report_batch(&results, install.assets().display().to_string())?;
        self.write_blends(&out_dir, &exported_manifests(&results))?;
        Ok(())
    }

    /// The shared conversion: assemble the model, resolve materials/textures via the
    /// source, and write the chosen container.
    fn convert(
        &self,
        runner: &nw_jobs::JobRunner,
        source: &dyn AssetSource,
        request: ConvertRequest<'_>,
    ) -> Result<ModelStats> {
        let ConvertRequest {
            source_path,
            cgf,
            heap,
            mesh,
            mtl_override,
            out,
            package,
            artifact,
            dependency_index,
        } = request;
        let resolved = crate::model_asset::resolve(
            source,
            source_path,
            cgf,
            heap,
            &mesh,
            mtl_override.as_deref(),
            crate::model_asset::ResolveOptions {
                runner,
                no_materials: self.geometry_only,
                skeleton: self.skeleton.as_deref(),
                animations: &self.animations,
                animation_events: self.animation_events.as_deref(),
                mannequin: &self.mannequin,
                audio: &self.audio,
                decode_audio: self.decode_audio_enabled(),
                vgmstream: self.vgmstream.as_deref(),
                dependency_index,
            },
        )?;
        let model = resolved.model;
        let materials = resolved.materials;
        let animations = resolved.animations;
        let extras = resolved.extras;
        let physics = resolved.physics;

        let texture_cache = materials
            .as_ref()
            .map(|set| {
                let mut files = set
                    .sub_materials
                    .iter()
                    .flat_map(|material| &material.textures)
                    .filter(|texture| texture.source_kind() == nw_model::TextureSourceKind::Asset)
                    .map(|texture| texture.file.clone())
                    .collect::<Vec<_>>();
                files.sort_unstable();
                files.dedup();
                // Files fed to a normal-map slot get ddna processing (Z rebuild +
                // gloss→roughness split); every other slot decodes verbatim.
                let normal_files = set
                    .sub_materials
                    .iter()
                    .flat_map(|material| &material.textures)
                    .filter(|texture| texture.slot == nw_model::MapSlot::Normals)
                    .map(|texture| texture.file.clone())
                    .collect::<std::collections::HashSet<_>>();
                runner
                    .try_map(&files, |file| {
                        decode_texture(source, file, normal_files.contains(file))
                            .with_context(|| format!("decode material texture {file}"))
                            .map(|texture| (file.clone(), texture))
                    })
                    .map(|textures| textures.into_iter().collect::<HashMap<_, _>>())
            })
            .transpose()?
            .unwrap_or_default();
        let textures = texture_cache.len();
        let bytes = {
            let mut load = |file: &str| texture_cache.get(file).cloned();
            let gltf = nw_model::Gltf::new(&model)
                .extras(&extras)
                .physics(&physics)?
                .animations(&animations)?;
            match (&materials, self.container) {
                (Some(set), Container::Glb) => {
                    let glb = gltf.materials(set).to_glb_with_runner(runner, &mut load);
                    write_glb(out, &glb)?
                }
                (Some(set), Container::Gltf) => {
                    let package = package.context("structured glTF package writer missing")?;
                    let gltf = gltf
                        .materials(set)
                        .to_gltf_package_with_runner(runner, &mut load);
                    write_gltf_package(runner, package, artifact, gltf)?
                }
                (None, Container::Glb) => write_glb(out, &gltf.to_glb_with_runner(runner))?,
                (None, Container::Gltf) => {
                    let package = package.context("structured glTF package writer missing")?;
                    let gltf = gltf.to_gltf_package_with_runner(runner);
                    write_gltf_package(runner, package, artifact, gltf)?
                }
            }
        };

        Ok(ModelStats {
            meshes: model.meshes.len(),
            vertices: model.vertex_count(),
            triangles: model.triangle_count(),
            joints: model
                .skeletons
                .iter()
                .map(|skeleton| skeleton.bones.len())
                .sum(),
            materials: materials.as_ref().map_or(0, |m| m.sub_materials.len()),
            textures,
            animations: animations.len(),
            bytes,
        })
    }

    /// Build the shared authored-asset dependency index for a filesystem tree.
    fn build_model_dependency_index(
        &self,
        ctx: &RunCtx,
        source: &dyn nw_asset_graph::AssetSource,
    ) -> Result<Option<nw_asset_graph::AssetDependencyIndex>> {
        if self.container != Container::Gltf {
            return Ok(None);
        }
        let paths = source.paths_with_extensions(MODEL_CONSUMER_EXTENSIONS)?;
        nw_asset_graph::AssetDependencyIndex::build_with_runner(source, &paths, &ctx.runner)
            .map(Some)
            .context("build shared authored-asset dependency index")
    }

    /// Build the shared dependency index for the install, reusing the on-disk
    /// cache so repeated single-character exports don't each rebuild the whole
    /// install's reverse graph.
    fn install_dependency_index(
        &self,
        ctx: &RunCtx,
        source: &Install,
        assets: &Path,
    ) -> Result<Option<nw_asset_graph::AssetDependencyIndex>> {
        if self.container != Container::Gltf {
            return Ok(None);
        }
        let paths =
            nw_asset_graph::AssetSource::paths_with_extensions(source, MODEL_CONSUMER_EXTENSIONS)?;
        source::load_or_build_dependency_index(assets, source, &paths, &ctx.runner).map(Some)
    }
}

fn character_roots(matches: &[String]) -> Vec<String> {
    matches
        .iter()
        .filter(|path| path_ext(Path::new(path)).as_deref() == Some("cdf"))
        .cloned()
        .collect()
}

/// Run one export root at a time so peak memory stays near a single character.
///
/// Progress still reports the full batch; cancellation stops scheduling further
/// roots. Inner conversion work keeps using [`RunCtx::runner`] parallelism.
fn export_roots_sequentially<N, F>(
    ctx: &RunCtx,
    count: usize,
    name: N,
    mut export_one: F,
) -> Vec<Result<Exported>>
where
    N: Fn(usize) -> String,
    F: FnMut(usize, crate::progress::Job) -> Result<Exported>,
{
    let progress = ctx.progress.batch_compact("model", count);
    let mut results = Vec::with_capacity(count);
    for index in 0..count {
        if ctx.cancel.is_cancelled() {
            break;
        }
        let job = progress.job(name(index));
        let result = export_one(index, job.clone());
        if result.is_ok() {
            job.finish("done");
        } else {
            job.finish("failed");
        }
        results.push(result);
    }
    progress.finish();
    results
}

struct ConvertRequest<'a> {
    source_path: &'a str,
    cgf: &'a [u8],
    heap: &'a [u8],
    mesh: MeshRef,
    mtl_override: Option<String>,
    out: &'a Path,
    package: Option<&'a PackageWriter>,
    artifact: &'a Path,
    dependency_index: Option<&'a nw_asset_graph::AssetDependencyIndex>,
}

/// Reads assets and resolves a mesh's material, abstracting over the filesystem and
/// the install's paks.
pub(crate) trait AssetSource: Sync + nw_asset_graph::AssetSource {
    /// Resolve the material set for a mesh, using whatever the source affords —
    /// the catalog (install) or a sibling-directory scan (filesystem).
    fn materials(&self, cgf: &[u8], mesh: &MeshRef) -> Option<nw_model::MaterialSet>;

    /// Whether serialized asset hints may identify products absent from a catalog.
    /// Filesystem and in-memory test sources opt in; install sources require the
    /// catalog identity to resolve.
    fn allows_asset_hint_fallback(&self) -> bool {
        false
    }
}

/// Parse the first `.mtl` among `keys` that this source can read.
fn first_material(
    source: &dyn AssetSource,
    keys: impl IntoIterator<Item = String>,
) -> Option<nw_model::MaterialSet> {
    keys.into_iter().find_map(|key| {
        let xml = source.read(&key)?;
        String::from_utf8_lossy(&xml)
            .parse::<nw_model::MaterialSet>()
            .ok()
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

    fn rooted(root: &Path) -> Self {
        Self {
            roots: vec![root.to_path_buf()],
        }
    }
}

impl nw_asset_graph::AssetSource for Tree {
    fn read(&self, path: &str) -> Option<Vec<u8>> {
        self.roots
            .iter()
            .map(|root| root.join(path))
            .find(|candidate| candidate.is_file())
            .and_then(|candidate| std::fs::read(candidate).ok())
    }

    fn contains(&self, path: &str) -> bool {
        self.roots.iter().any(|root| root.join(path).is_file())
    }

    fn matching_paths(&self, pattern: &str) -> Result<Vec<String>> {
        let pattern = pattern.replace('\\', "/");
        let matcher = globset::Glob::new(&pattern)
            .with_context(|| format!("invalid Cry asset wildcard {pattern}"))?
            .compile_matcher();
        let wildcard = pattern.find(['*', '?']).unwrap_or(pattern.len());
        let directory = pattern[..wildcard]
            .rsplit_once('/')
            .map_or("", |(directory, _)| directory);
        let mut found = std::collections::BTreeMap::new();
        for root in &self.roots {
            let scan_root = root.join(directory);
            if !scan_root.is_dir() {
                continue;
            }
            for path in collect_matching(&scan_root, |_| true)? {
                let Ok(relative) = path.strip_prefix(root) else {
                    continue;
                };
                let virtual_path = relative.to_string_lossy().replace('\\', "/");
                if matcher.is_match(&virtual_path) {
                    found
                        .entry(virtual_path.to_ascii_lowercase())
                        .or_insert(virtual_path);
                }
            }
        }
        Ok(found.into_values().collect())
    }
}

impl AssetSource for Tree {
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

    fn allows_asset_hint_fallback(&self) -> bool {
        true
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
            file.materials().values().next().map(|mtl| {
                mtl.sub_material_names
                    .iter()
                    .map(|s| s.to_string())
                    .collect()
            })
        })
        .unwrap_or_default()
}

impl nw_asset_graph::AssetSource for Install {
    fn read(&self, path: &str) -> Option<Vec<u8>> {
        Install::read(self, path)
    }

    fn contains(&self, path: &str) -> bool {
        Install::contains(self, path)
    }

    fn matching_paths(&self, pattern: &str) -> Result<Vec<String>> {
        let pattern = pattern.replace('\\', "/").to_ascii_lowercase();
        let matcher = globset::Glob::new(&pattern)
            .with_context(|| format!("invalid Cry asset wildcard {pattern}"))?
            .compile_matcher();
        let extension = pattern
            .rsplit_once('.')
            .map(|(_, extension)| extension)
            .filter(|extension| !extension.contains(['*', '?', '[']));
        let candidates = extension
            .and_then(|extension| self.indexed_paths(extension))
            .unwrap_or_default();
        Ok(candidates
            .iter()
            .filter(|path| matcher.is_match(path))
            .cloned()
            .collect())
    }

    fn paths_with_extensions(&self, extensions: &[&str]) -> Result<Vec<String>> {
        Ok(Install::paths_with_extensions(self, extensions, &[]))
    }

    fn path_by_id(&self, asset_id: nw_asset::AssetId) -> Option<String> {
        self.catalog()
            .entry_by_id(asset_id)
            .map(|entry| entry.path().to_owned())
    }

    fn legacy_material_path(&self, name: &str) -> Option<String> {
        self.material_path(name)
    }
}

impl AssetSource for Install {
    /// Resolve the material by the mesh's MtlName GUID through the catalog, falling
    /// back to convention-named siblings.
    fn materials(&self, cgf: &[u8], mesh: &MeshRef) -> Option<nw_model::MaterialSet> {
        let by_guid = mtlname_guid(cgf).and_then(|guid| self.material_path(&guid));
        first_material(self, by_guid.into_iter().chain(mesh.mtl_candidates()))
    }
}

/// Identifies a mesh for material resolution: its sub-material GUID hint plus the
/// virtual directory and base name used for naming-convention fallback.
pub(crate) struct MeshRef {
    dir: String,
    stem: String,
}

impl MeshRef {
    /// For a pak virtual path like `a/b/foo_mesh.cgf`.
    pub(crate) fn for_key(key: &str) -> Self {
        let (dir, file) = key.rsplit_once('/').unwrap_or(("", key));
        Self {
            dir: dir.to_string(),
            stem: mesh_stem(file).to_string(),
        }
    }

    /// For a filesystem path: the `.mtl` is a sibling, so the directory is empty
    /// (the [`Tree`] source resolves siblings via its roots).
    pub(crate) fn for_file(path: &Path) -> Self {
        let file = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
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

/// Decode a referenced texture (`.tif` → `.dds`, split mips assembled) to PNG
/// bytes. When `is_normal` is set, a two-channel CryEngine ddna map is split
/// into an RGB normal (Z reconstructed) plus a derived metallic-roughness image
/// (gloss alpha → roughness); a plain RGB normal passes through with its alpha
/// stripped.
fn decode_texture(
    source: &dyn AssetSource,
    file: &str,
    is_normal: bool,
) -> Result<nw_model::TextureData> {
    let dds = tif_to_dds(file);
    let header = source
        .read(&dds)
        .with_context(|| format!("texture asset not found: {dds}"))?;
    let dds_header = nw_dds::Dds::parse(&header)?;
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
    let alpha_header = if dds_header.has_attached_alpha() {
        Some(source.read(&format!("{dds}.a")).with_context(|| {
            format!("texture {dds} declares attached alpha but {dds}.a is missing")
        })?)
    } else {
        None
    };
    let mut alpha_sidecars = Vec::new();
    if alpha_header.is_some() {
        let mut mip = 1u32;
        while let Some(bytes) = source.read(&format!("{dds}.{mip}a")) {
            alpha_sidecars.push((
                nw_dds::SplitPart::Mip {
                    index: mip,
                    alpha: true,
                },
                bytes,
            ));
            mip += 1;
        }
    }
    let alpha_parts = alpha_sidecars
        .iter()
        .map(|(part, bytes)| nw_dds::Sidecar::new(*part, bytes.as_slice()))
        .collect::<Vec<_>>();
    let attached_alpha = alpha_header
        .as_deref()
        .map(|alpha| (alpha, alpha_parts.as_slice()));
    let decoded = nw_dds::decode_top_mip_with_attached_alpha(&header, &parts, attached_alpha)?;
    let width = decoded.width;
    let height = decoded.height;
    let expected = (width as usize)
        .saturating_mul(height as usize)
        .saturating_mul(4);
    if decoded.rgba.len() != expected {
        bail!("decoded texture RGBA dimensions do not match payload");
    }

    // Place the decoded image at the shipped `.dds` catalog path, swapping the
    // extension for `png`.
    let path = png_from_dds(&dds);
    let two_channel_normal = is_normal && blue_channel_is_empty(&decoded.rgba);
    let (rgba, derived_roughness) = if two_channel_normal {
        // The gloss (smoothness) rides the alpha; split it into a glTF
        // metallic-roughness sibling before rebuilding the normal's blue (Z).
        let roughness_bytes =
            encode_rgba_png(&ddna_gloss_to_roughness(&decoded.rgba), width, height)?;
        let stem = dds.rsplit_once('.').map_or(dds.as_str(), |(stem, _)| stem);
        let derived = nw_model::TextureData {
            bytes: roughness_bytes,
            mime: "image/png".to_string(),
            path: Some(format!("{stem}.rough.png")),
            derived_roughness: None,
        };
        (
            ddna_reconstruct_normal(&decoded.rgba),
            Some(Box::new(derived)),
        )
    } else if is_normal {
        // A true RGB normal keeps its Z in blue; drop the unused alpha.
        (strip_alpha(&decoded.rgba), None)
    } else {
        (decoded.rgba, None)
    };
    Ok(nw_model::TextureData {
        bytes: encode_rgba_png(&rgba, width, height)?,
        mime: "image/png".to_string(),
        path: Some(path),
        derived_roughness,
    })
}

/// Encode an RGBA8 buffer to PNG bytes.
fn encode_rgba_png(rgba: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    image::codecs::png::PngEncoder::new(&mut bytes)
        .write_image(rgba, width, height, image::ExtendedColorType::Rgba8)
        .context("encode texture PNG")?;
    Ok(bytes)
}

/// The CryEngine ddna signature: the blue channel is unused (normal X in red, Y
/// in green, gloss in alpha). A true RGB normal keeps a meaningful Z in blue and
/// fails this test.
fn blue_channel_is_empty(rgba: &[u8]) -> bool {
    const BLUE_EMPTY_MAX: u8 = 4;
    !rgba.is_empty() && rgba.chunks_exact(4).all(|pixel| pixel[2] <= BLUE_EMPTY_MAX)
}

/// Rebuild a full RGB normal from a two-channel ddna map: X/Y stay in red/green,
/// Z is reconstructed per pixel into blue, alpha is cleared to opaque.
fn ddna_reconstruct_normal(rgba: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8; rgba.len()];
    for (dst, src) in out.chunks_exact_mut(4).zip(rgba.chunks_exact(4)) {
        let x = f32::from(src[0]) / 255.0 * 2.0 - 1.0;
        let y = f32::from(src[1]) / 255.0 * 2.0 - 1.0;
        let z = (1.0 - x * x - y * y).max(0.0).sqrt();
        dst[0] = src[0];
        dst[1] = src[1];
        dst[2] = ((z * 0.5 + 0.5) * 255.0).round().clamp(0.0, 255.0) as u8;
        dst[3] = 255;
    }
    out
}

/// Split a ddna map's gloss (smoothness in alpha) into a glTF metallic-roughness
/// image: roughness = 1 - smoothness in green, metallic 0 in blue, red unused,
/// alpha opaque.
fn ddna_gloss_to_roughness(rgba: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8; rgba.len()];
    for (dst, src) in out.chunks_exact_mut(4).zip(rgba.chunks_exact(4)) {
        dst[0] = 255;
        dst[1] = 255 - src[3];
        dst[2] = 0;
        dst[3] = 255;
    }
    out
}

/// Drop a true RGB normal map's alpha, leaving an opaque RGB image.
fn strip_alpha(rgba: &[u8]) -> Vec<u8> {
    let mut out = rgba.to_vec();
    for pixel in out.chunks_exact_mut(4) {
        pixel[3] = 255;
    }
    out
}

/// Replace a texture's `.dds` extension with `png`, keeping the catalog path.
fn png_from_dds(dds: &str) -> String {
    match dds.rsplit_once('.') {
        Some((stem, _)) => format!("{stem}.png"),
        None => format!("{dds}.png"),
    }
}

fn write_glb(out: &Path, glb: &[u8]) -> Result<usize> {
    std::fs::write(out, glb).with_context(|| format!("write {}", out.display()))?;
    Ok(glb.len())
}

fn write_gltf_package(
    runner: &nw_jobs::JobRunner,
    package: &PackageWriter,
    artifact: &Path,
    gltf: nw_model::GltfPackage,
) -> Result<usize> {
    // Resources that mirror a catalog asset carry their own path; anonymous
    // derived payloads (the geometry buffer, pathless images) are named after
    // the manifest they belong to.
    let manifest = artifact.to_string_lossy().replace('\\', "/");
    let manifest_stem = manifest
        .rsplit_once('.')
        .map_or(manifest.as_str(), |(stem, _)| stem);
    let targets = gltf
        .resources()
        .iter()
        .enumerate()
        .map(|(index, resource)| match (resource.path(), index) {
            (Some(path), _) => path.to_owned(),
            (None, 0) => format!("{manifest_stem}.bin"),
            (None, index) => format!("{manifest_stem}.{index}.{}", resource.extension()),
        })
        .collect::<Vec<_>>();
    let jobs = targets.iter().zip(gltf.resources()).collect::<Vec<_>>();
    let stored = runner.try_map(&jobs, |(target, resource)| {
        package
            .store_at(target, resource.bytes())
            .map_err(anyhow::Error::from)
    })?;
    let uris = stored
        .iter()
        .map(|blob| {
            package
                .uri_from(artifact, blob)
                .map_err(anyhow::Error::from)
        })
        .collect::<Result<Vec<_>>>()?;
    let resource_bytes = gltf
        .resources()
        .iter()
        .map(|resource| resource.bytes().len())
        .sum::<usize>();
    let manifest_bytes = package.write_stream(artifact, |writer| {
        gltf.write_json(&uris, writer).map_err(io::Error::other)
    })?;
    Ok(manifest_bytes + resource_bytes)
}

#[derive(Debug, Default, Clone, Copy)]
struct ModelStats {
    meshes: usize,
    vertices: usize,
    triangles: usize,
    joints: usize,
    materials: usize,
    textures: usize,
    animations: usize,
    bytes: usize,
}

/// One successful batch conversion: its stats plus, for structured glTF runs, the
/// on-disk manifest path a `.blend` is written next to.
struct Exported {
    stats: ModelStats,
    manifest: Option<PathBuf>,
}

/// The manifests written by a batch run, in order, for blend generation.
fn exported_manifests(results: &[Result<Exported>]) -> Vec<PathBuf> {
    results
        .iter()
        .filter_map(|result| result.as_ref().ok())
        .filter_map(|exported| exported.manifest.clone())
        .collect()
}

fn report_batch(results: &[Result<Exported>], source: String) -> Result<()> {
    let mut converted = 0usize;
    let mut vertices = 0usize;
    let mut errors = Vec::new();
    for result in results {
        match result {
            Ok(exported) => {
                converted += 1;
                vertices += exported.stats.vertices;
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
    matches!(
        path_ext(path).as_deref(),
        Some("cgf" | "skin" | "chr" | "cga" | "cdf" | "caf" | "i_caf" | "dba")
    )
}

/// A mesh base name with the `_mesh`/`_lod0` suffix stripped, for `.mtl` matching.
fn mesh_stem(file: &str) -> &str {
    let stem = file.rsplit_once('.').map_or(file, |(stem, _)| stem);
    stem.strip_suffix("_mesh")
        .or_else(|| stem.strip_suffix("_lod0"))
        .unwrap_or(stem)
}

/// Map an authored texture path (usually `.tif`) to its shipped `.dds`.
fn tif_to_dds(file: &str) -> String {
    match file.rsplit_once('.') {
        Some((stem, ext))
            if ext.eq_ignore_ascii_case("tif") || ext.eq_ignore_ascii_case("tiff") =>
        {
            format!("{stem}.dds")
        }
        _ => file.to_string(),
    }
}

fn display_path(path: &Path) -> String {
    path.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn character_filters_prefer_complete_cdf_roots() {
        let matches = vec![
            "objects/isabella_t2/isabella_t2.cdf".to_owned(),
            "objects/isabella_t2/isabella_t2_skel.chr".to_owned(),
            "objects/isabella_t2/isabella_upperbody.skin".to_owned(),
        ];
        assert_eq!(
            character_roots(&matches),
            ["objects/isabella_t2/isabella_t2.cdf"]
        );
        assert!(character_roots(&["objects/standalone.cgf".to_owned()]).is_empty());
    }

    #[test]
    fn detects_two_channel_ddna_by_empty_blue() {
        // ddna: X in red, Y in green, blue unused, gloss in alpha.
        let ddna = [130, 120, 0, 220, 128, 128, 0, 200];
        assert!(blue_channel_is_empty(&ddna));
        // A true RGB normal keeps Z in blue.
        let rgb_normal = [130, 120, 200, 255, 128, 128, 255, 255];
        assert!(!blue_channel_is_empty(&rgb_normal));
        assert!(!blue_channel_is_empty(&[]));
    }

    #[test]
    fn reconstructs_ddna_normal_z_into_blue() {
        // A flat normal (x=y=0 → R=G=128) reconstructs Z≈1 → blue≈255; alpha opaque.
        let ddna = [128, 128, 0, 210];
        let normal = ddna_reconstruct_normal(&ddna);
        assert_eq!(normal[0], 128);
        assert_eq!(normal[1], 128);
        assert!(
            normal[2] >= 254,
            "flat normal Z should encode near 255, got {}",
            normal[2]
        );
        assert_eq!(normal[3], 255);
    }

    #[test]
    fn ddna_gloss_becomes_inverted_roughness() {
        // Smoothness 210/255 → roughness 45/255 in green; metallic 0; red unused.
        let ddna = [128, 128, 0, 210];
        let roughness = ddna_gloss_to_roughness(&ddna);
        assert_eq!(roughness, [255, 45, 0, 255]);
    }

    #[test]
    fn strips_true_rgb_normal_alpha() {
        let normal = [130, 120, 200, 17];
        assert_eq!(strip_alpha(&normal), [130, 120, 200, 255]);
    }
}
