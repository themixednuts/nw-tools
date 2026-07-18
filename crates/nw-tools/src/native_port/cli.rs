//! `nw-tools port` — extract-time legacy asset port (ADR 0027 + ADR 0028).
//!
//! Two subcommands over an extracted/legacy content root:
//!
//! - `inventory` (pass 1) walks the root, classifies each file, applies the
//!   deterministic mapper, resolves identity, and prints the mapping report
//!   (counts per legacy tree, collisions, exceptions hit, uncataloged
//!   fallbacks) without writing any native tree.
//! - `publish` (pass 2) writes the Phase-1 vertical slice — the self-contained
//!   texture image family — through the mapper as native source files plus
//!   `.azmeta.json` sidecars under an output `assets/` tree.
//!
//! Classification is by legacy tree + extension/suffix. Decode-confirmed
//! classification (parsing DDS/ObjectStream to verify the decoded type) is
//! Phase 2; the texture slice needs only image detection.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Subcommand;
use walkdir::WalkDir;

use super::{
    AssetId, DecodedClass, Inventory, MappedSource, Mapper, SourceInput, normalize_legacy_path,
    writer::write_native_sources,
};

#[derive(Debug, Subcommand)]
pub enum Cmd {
    /// Pass 1: walk a legacy root and print the mapping report (no output).
    #[command(about = "Walk a legacy root and print the pass-1 mapping report")]
    Inventory {
        /// Extracted/legacy content root to walk.
        root: PathBuf,
        /// Optional RASC `assetcatalog.catalog` for identity preservation.
        #[arg(long)]
        catalog: Option<PathBuf>,
    },
    /// Pass 2: write the texture vertical slice as native sources + sidecars.
    #[command(about = "Write the texture vertical slice as native sources and sidecars")]
    Publish {
        /// Extracted/legacy content root to walk.
        root: PathBuf,
        /// Output directory for the native `assets/` tree (must not exist).
        out: PathBuf,
        /// Optional RASC `assetcatalog.catalog` for identity preservation.
        #[arg(long)]
        catalog: Option<PathBuf>,
    },
}

impl Cmd {
    /// Run the selected subcommand.
    ///
    /// # Errors
    ///
    /// Returns an error if the root cannot be walked, the catalog cannot be
    /// loaded, the inventory has blocking errors, or publication fails.
    pub fn run(self) -> Result<()> {
        match self {
            Self::Inventory { root, catalog } => run_inventory(&root, catalog.as_deref()),
            Self::Publish { root, out, catalog } => run_publish(&root, &out, catalog.as_deref()),
        }
    }
}

/// A walked legacy file: its normalized key, decoded class, and disk path.
struct WalkedSource {
    input: SourceInput,
    disk_path: PathBuf,
    normalized: String,
}

fn run_inventory(root: &Path, catalog: Option<&Path>) -> Result<()> {
    let catalog = catalog.map(load_catalog).transpose()?;
    let walked = walk_root(root, catalog.as_ref())?;
    let inputs = walked.iter().map(|w| w.input.clone());
    let inventory = Mapper::pinned()?.map_all(inputs);
    print_report(root, &inventory);
    if inventory.has_blocking_errors() {
        anyhow::bail!(
            "pass-1 inventory has blocking errors: {} collisions, {} blocked, {} unmapped, {} normalize failures",
            inventory.collisions.len(),
            inventory.blocked.len(),
            inventory.route_errors.len(),
            inventory.normalize_errors.len(),
        );
    }
    Ok(())
}

fn run_publish(root: &Path, out: &Path, catalog: Option<&Path>) -> Result<()> {
    let catalog = catalog.map(load_catalog).transpose()?;
    let walked = walk_root(root, catalog.as_ref())?;

    // Phase-1 vertical slice: the self-contained texture image family only.
    let slice: Vec<&WalkedSource> = walked
        .iter()
        .filter(|w| w.input.class == DecodedClass::Image)
        .collect();
    let inputs = slice.iter().map(|w| w.input.clone());
    let inventory = Mapper::pinned()?.map_all(inputs);
    print_report(root, &inventory);
    if inventory.has_blocking_errors() {
        anyhow::bail!("texture slice has blocking errors; refusing to write");
    }

    // Map normalized legacy path -> disk path for the payload reader.
    let disk: HashMap<String, PathBuf> = slice
        .iter()
        .map(|w| (w.normalized.clone(), w.disk_path.clone()))
        .collect();
    let reader = move |source: &MappedSource| {
        let path = disk.get(&source.normalized_legacy_path).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("no disk payload for `{}`", source.normalized_legacy_path),
            )
        })?;
        std::fs::read(path)
    };

    let summary = write_native_sources(&inventory.sources, &reader, out)?;
    println!(
        "wrote {} native texture sources ({} catalog-preserved) into {}",
        summary.source_count,
        summary.preserved_count,
        summary.output.display()
    );
    Ok(())
}

fn walk_root(root: &Path, catalog: Option<&nw_asset::AssetCatalog>) -> Result<Vec<WalkedSource>> {
    anyhow::ensure!(
        root.is_dir(),
        "root `{}` is not a directory",
        root.display()
    );
    let mut walked = Vec::new();
    for entry in WalkDir::new(root).into_iter() {
        let entry = entry.with_context(|| format!("walking `{}`", root.display()))?;
        if !entry.file_type().is_file() {
            continue;
        }
        let relative = entry
            .path()
            .strip_prefix(root)
            .unwrap_or(entry.path())
            .to_string_lossy()
            .into_owned();
        let Ok(normalized) = normalize_legacy_path(&relative) else {
            continue;
        };
        let Some(class) = classify(&normalized) else {
            continue;
        };
        let catalog_identity = catalog.and_then(|catalog| {
            catalog
                .entry_by_path(&normalized)
                .map(|entry| AssetId::new(entry.asset_id().guid, entry.asset_id().sub_id))
        });
        walked.push(WalkedSource {
            input: SourceInput::new(relative, class, catalog_identity),
            disk_path: entry.path().to_path_buf(),
            normalized,
        });
    }
    Ok(walked)
}

fn load_catalog(path: &Path) -> Result<nw_asset::AssetCatalog> {
    let bytes =
        std::fs::read(path).with_context(|| format!("reading catalog `{}`", path.display()))?;
    let rasc = nw_asset::Rasc::parse(&bytes)
        .with_context(|| format!("parsing RASC catalog `{}`", path.display()))?;
    Ok(nw_asset::AssetCatalog::new(rasc, None))
}

/// Classify a normalized legacy path by tree + extension/suffix (Phase 1).
fn classify(path: &str) -> Option<DecodedClass> {
    let tree = path.split('/').next()?;
    let ends = |suffix: &str| path.ends_with(suffix);
    match tree {
        "textures" => {
            if ends(".material.ron") {
                Some(DecodedClass::Material)
            } else if ends(".png") || ends(".dds") {
                Some(DecodedClass::Image)
            } else if ends(".ron") {
                Some(DecodedClass::TextureSettings)
            } else {
                None
            }
        }
        "materials" | "objects" | "slices" | "coatgen" | "engineassets" => {
            ends(".ron").then_some(DecodedClass::Material)
        }
        "libs" => {
            (path.starts_with("libs/particles/") && ends(".ron")).then_some(DecodedClass::Material)
        }
        "meshes" | "characters" => ends(".glb").then_some(DecodedClass::Model),
        "animations" => {
            if path.starts_with("animations/mannequin/adb/") {
                Some(DecodedClass::MannequinDatabase)
            } else if ends(".glb") || ends(".ron") {
                Some(DecodedClass::Motion)
            } else {
                None
            }
        }
        "gamedata" => ends(".ron").then_some(DecodedClass::GameDataTable),
        "prefabs" => {
            if path.starts_with("prefabs/lyshineui/") {
                Some(DecodedClass::UiPrefab)
            } else {
                ends(".ron").then_some(DecodedClass::Prefab)
            }
        }
        "sharedassets" => {
            if path.starts_with("sharedassets/coatlicue/") && ends(".material.ron") {
                Some(DecodedClass::Material)
            } else if ends(".actionlist.ron") {
                Some(DecodedClass::CageActionList)
            } else if ends(".grid.ron") {
                Some(DecodedClass::CageGrid)
            } else {
                None
            }
        }
        "sounds" => {
            if ends(".audiocontrols.ron") {
                Some(DecodedClass::AudioControls)
            } else if ends(".audioevents.ron") {
                Some(DecodedClass::AudioEvents)
            } else if ends(".audiotags.ron") {
                Some(DecodedClass::AudioTags)
            } else if ends(".audiotracts.ron") {
                Some(DecodedClass::AudioTracts)
            } else if ends(".bnk") {
                Some(DecodedClass::AudioBank)
            } else if ends(".wem") {
                Some(DecodedClass::AudioMedia)
            } else {
                None
            }
        }
        "terrain" => {
            if path.starts_with("terrain/worlds/") {
                Some(DecodedClass::TerrainWorld)
            } else if path.starts_with("terrain/regions/") {
                Some(DecodedClass::TerrainRegion)
            } else if path.starts_with("terrain/layers/") {
                Some(DecodedClass::TerrainLayers)
            } else if path.starts_with("terrain/payloads/") {
                Some(DecodedClass::TerrainPayload)
            } else {
                None
            }
        }
        "ui" => path
            .starts_with("ui/lyshineui/images/textureatlas/")
            .then_some(DecodedClass::Atlas),
        "world" => {
            if path.starts_with("world/levels/") {
                Some(DecodedClass::LevelPrefab)
            } else if path.starts_with("world/sharedassets/coatlicue/") && ends("region.prefab.ron")
            {
                Some(DecodedClass::RegionPrefab)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn print_report(root: &Path, inventory: &Inventory) {
    println!("pass-1 inventory for {}", root.display());
    println!("  mapped sources:        {}", inventory.sources.len());
    println!(
        "  uncataloged fallbacks: {}",
        inventory.uncataloged_fallbacks
    );
    println!(
        "  exceptions hit:        {}",
        inventory.exceptions_hit.len()
    );
    println!("  counts per legacy tree:");
    for (tree, count) in &inventory.tree_counts {
        println!("    {tree:<14} {count}");
    }
    if !inventory.blocked.is_empty() {
        println!("  BLOCKED (publication hard-fail):");
        for blocked in &inventory.blocked {
            println!(
                "    {} -> {} ({})",
                blocked.normalized_legacy_path, blocked.canonical_path, blocked.reason
            );
        }
    }
    if !inventory.collisions.is_empty() {
        println!("  COLLISIONS:");
        for collision in &inventory.collisions {
            println!(
                "    {:?} on `{}`: `{}` vs `{}`",
                collision.kind, collision.key, collision.first_input, collision.second_input
            );
        }
    }
    if !inventory.route_errors.is_empty() {
        println!("  UNMAPPED:");
        for failure in &inventory.route_errors {
            println!("    {}: {}", failure.original_spelling, failure.detail);
        }
    }
    if !inventory.normalize_errors.is_empty() {
        println!("  NORMALIZE ERRORS:");
        for failure in &inventory.normalize_errors {
            println!("    {}: {}", failure.original_spelling, failure.detail);
        }
    }
}
