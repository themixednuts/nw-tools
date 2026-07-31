//! Complete Cry character/model graph resolution for glTF export.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::str;
use std::sync::{Arc, LazyLock};

use anyhow::{Context, Result, bail};
use cry_character::{
    AttachmentKind, CharacterAnimationEntryKind, CharacterAnimationPathResolver,
    CharacterDefinition, CharacterParameters,
};
use glam::{Mat4, Quat, Vec3};
use nw_reflected_types::az::rtti::AzRtti;

use crate::model::{AssetSource, MeshRef};

mod audio;
mod character_event;

mod animation;
mod cloth;
mod dependencies;
mod mannequin;
mod materials;
mod particles;
mod physics;
mod sources;
mod variants;

pub(crate) use variants::context_variant_cdfs;

use animation::{
    AnimationAssetEvaluation, AnimationBindingPolicy, clip_targets_skeleton, push_animation_assets,
    scope_blend_space_dependencies,
};
use materials::resolve_primary_materials;
use physics::scope_scene_physics;
use sources::{apply_resolved_dependencies, model_context_assets, resolve_cdf};

static OBJECTSTREAM_LOOKUP: LazyLock<nw_objectstream::lookup::NameLookup> = LazyLock::new(|| {
    nw_objectstream::lookup::NameLookup::from_serialize_json(nw_resources::SERIALIZE_JSON)
        .expect("bundled serialize.json must build the ObjectStream name lookup")
});

pub(crate) struct ResolveOptions<'a> {
    pub runner: &'a nw_jobs::JobRunner,
    pub no_materials: bool,
    pub skeleton: Option<&'a str>,
    pub animations: &'a [String],
    pub animation_events: Option<&'a str>,
    pub mannequin: &'a [String],
    pub audio: &'a [String],
    /// Decode Wwise bank media to PCM WAV and ship at catalog paths.
    pub decode_audio: bool,
    /// Optional explicit path to `vgmstream-cli`.
    pub vgmstream: Option<&'a Path>,
    pub dependency_index: Option<&'a nw_asset_graph::AssetDependencyIndex>,
}

pub(crate) struct ResolvedAsset {
    pub model: nw_model::Model,
    pub materials: Option<nw_model::MaterialSet>,
    pub animations: Vec<nw_model::ModelAnimation>,
    pub extras: nw_model::CryAssetExtras,
    pub physics: nw_model::PhysicsScene,
    animation_asset_evaluations: HashMap<AnimationAssetEvaluation, bool>,
}

struct LoadedEventDatabase {
    parsed: cry_animation::AnimationEventDatabase,
    bytes: Vec<u8>,
}

pub(crate) fn resolve(
    source: &dyn AssetSource,
    source_path: &str,
    bytes: &[u8],
    heap: &[u8],
    mesh: &MeshRef,
    material_override: Option<&str>,
    options: ResolveOptions<'_>,
) -> Result<ResolvedAsset> {
    let runner = options.runner;
    let extension = source_extension(source_path);
    let mut additional_roots = options.animations.to_vec();
    additional_roots.extend(options.mannequin.iter().cloned());
    additional_roots.extend(options.audio.iter().cloned());
    additional_roots.extend(options.animation_events.map(str::to_owned));
    additional_roots.extend(options.skeleton.map(str::to_owned));
    let mut root_keys = HashSet::new();
    additional_roots.retain(|path| root_keys.insert(normalize_path(path).to_ascii_lowercase()));
    let dependency_graph = nw_asset_graph::resolve_with_runner(
        source,
        source_path,
        &nw_asset_graph::ResolveOptions { additional_roots },
        runner,
    )?;
    if !dependency_graph.is_complete() {
        let unresolved = dependency_graph
            .unresolved()
            .iter()
            .filter(|dependency| dependency.is_required())
            .map(|dependency| {
                format!(
                    "{} --{}--> {} ({:?})",
                    dependency.source(),
                    dependency.relation(),
                    dependency.target(),
                    dependency.reason()
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        bail!("authored dependency closure is incomplete: {unresolved}");
    }
    let mut resolved = match extension.as_str() {
        "cdf" => resolve_cdf(runner, source, source_path, bytes, options.no_materials)?,
        "caf" | "i_caf" | "dba" => {
            let clips = if extension == "dba" {
                cry_animation::AnimationClip::parse_dba(bytes)?
            } else {
                vec![cry_animation::AnimationClip::parse(source_path, bytes)?]
            };
            let skeleton_path = options.skeleton.context(
                "CAF/DBA export requires --skeleton <character.chr|skin>; optimized catalogs do not contain the runtime product-dependency map",
            )?;
            let skeleton_path = normalize_path(skeleton_path);
            let skeleton_bytes = read_required(source, &skeleton_path)?;
            let skeleton = nw_model::skeleton_from_bytes(&skeleton_bytes)
                .with_context(|| format!("decode skeleton {skeleton_path}"))?;
            let mut extras = nw_model::CryAssetExtras {
                source_path: normalize_path(source_path),
                ..Default::default()
            };
            // The animation ships as its glTF channel buffer at this catalog
            // path (see `gltf::append_animation`); the raw CAF/DBA payload is
            // never embedded. Provenance survives via `cry_source_path` on each
            // clip and the dependency listing below.
            add_dependency(&mut extras, source_path);
            add_dependency(&mut extras, &skeleton_path);
            let mut animations = Vec::new();
            for clip in clips {
                if clip_targets_skeleton(&clip, &skeleton) {
                    animations.push(nw_model::ModelAnimation {
                        skeleton: 0,
                        clip,
                        controller_binding: None,
                    });
                } else {
                    record_unbound_animation(&mut extras, &clip.source_path, 0);
                }
            }
            if animations.is_empty() {
                bail!("{source_path} has no controllers targeting skeleton {skeleton_path}");
            }
            ResolvedAsset {
                model: nw_model::Model {
                    meshes: Vec::new(),
                    skeletons: vec![skeleton],
                    auxiliary_nodes: Vec::new(),
                },
                materials: None,
                animations,
                extras,
                physics: nw_model::PhysicsScene::default(),
                animation_asset_evaluations: [(
                    AnimationAssetEvaluation::new(
                        0,
                        source_path,
                        AnimationBindingPolicy::ExplicitPermissive,
                    ),
                    true,
                )]
                .into_iter()
                .collect(),
            }
        }
        _ => {
            let model = nw_model::model_from_bytes(bytes, heap)
                .with_context(|| format!("assemble {source_path}"))?;
            let materials = resolve_primary_materials(
                source,
                bytes,
                mesh,
                material_override,
                options.no_materials,
                model.has_render_geometry(),
            )?;
            ResolvedAsset {
                model,
                materials,
                animations: Vec::new(),
                extras: nw_model::CryAssetExtras {
                    source_path: normalize_path(source_path),
                    ..Default::default()
                },
                physics: nw_model::PhysicsScene::default(),
                animation_asset_evaluations: HashMap::new(),
            }
        }
    };

    if resolved.model.skeletons.is_empty() {
        let selected = if let Some(path) = options.skeleton {
            let path = normalize_path(path);
            let bytes = read_required(source, &path)?;
            Some((
                path.clone(),
                nw_model::skeleton_from_bytes(&bytes)
                    .with_context(|| format!("decode skeleton {path}"))?,
            ))
        } else {
            None
        };
        if let Some((skeleton_path, skeleton)) = selected {
            resolved.model.set_primary_skeleton(skeleton);
            add_dependency(&mut resolved.extras, &skeleton_path);
        }
    }

    let event_database = options
        .animation_events
        .map(|path| load_event_database(source, path))
        .transpose()?;
    if let Some(path) = options.animation_events {
        let event_database = event_database.as_ref().expect("loaded event database");
        add_xml_source(
            &mut resolved.extras,
            path,
            nw_model::CrySourceAssetKind::AnimationEvents,
            &event_database.parsed.source,
        );
        add_resource(
            &mut resolved.extras,
            path,
            nw_model::CryEmbeddedResourceKind::AnimationEvents,
            event_database.bytes.clone(),
        );
    }
    push_animation_assets(
        runner,
        source,
        options.animations,
        event_database.as_ref().map(|database| &database.parsed),
        0,
        true,
        &mut resolved,
    )?;
    for path in options.mannequin {
        add_mannequin_source(source, path, &mut resolved.extras)?;
    }
    for path in options.audio {
        add_audio_source(source, path, &mut resolved.extras)?;
    }
    let mut dependency_paths = dependency_graph.assets().to_vec();
    if let Some(index) = options.dependency_index {
        dependency_paths.extend(model_context_assets(source, source_path, index));
    }
    dependency_paths.sort_by_key(|path| normalize_path(path).to_ascii_lowercase());
    dependency_paths.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
    apply_resolved_dependencies(runner, source, &dependency_paths, &mut resolved)?;
    scope_blend_space_dependencies(source, &mut resolved)?;
    // Discover the creature's Mannequin databases from its scene slices and attach
    // fragment audio (bite/vocal/action sounds) before trigger resolution, so its
    // triggers flow through the same ATL → bank/media pipeline as footsteps.
    mannequin::attach_fragment_audio(source, &dependency_paths, &mut resolved)?;
    audio::resolve_animation_audio_triggers(source, &mut resolved)?;
    if options.decode_audio && !resolved.extras.audio_triggers.is_empty() {
        let owned = options
            .vgmstream
            .map(Path::to_path_buf)
            .or_else(crate::audio_export::find_vgmstream);
        let vgmstream = owned.with_context(|| {
            "glTF audio decode requires vgmstream-cli \
             (winget install vgmstream.vgmstream, or put it on PATH; pass --no-decode-audio to skip)"
        })?;
        crate::audio_export::materialize_decoded_waves(source, &mut resolved, &vgmstream)?;
    }
    scope_scene_physics(&resolved.model, &mut resolved.physics);
    particles::attach_scene_particles(
        runner,
        source,
        &resolved.model,
        &mut resolved.extras,
        &dependency_graph,
    )?;
    Ok(resolved)
}

fn record_unbound_animation(
    extras: &mut nw_model::CryAssetExtras,
    source_path: &str,
    skeleton: usize,
) {
    if !extras.unbound_animations.iter().any(|animation| {
        animation.skeleton == skeleton && animation.source_path.eq_ignore_ascii_case(source_path)
    }) {
        extras
            .unbound_animations
            .push(nw_model::CryUnboundAnimation {
                source_path: normalize_path(source_path),
                skeleton,
            });
    }
}

fn add_mannequin_source(
    source: &dyn AssetSource,
    path: &str,
    extras: &mut nw_model::CryAssetExtras,
) -> Result<()> {
    let path = normalize_path(path);
    if has_source_asset(extras, &path) {
        return Ok(());
    }
    let bytes = read_required(source, &path)?;
    let (kind, document) = if let Some(kind) =
        cry_mannequin::MannequinXmlKind::from_source_path(&path)
    {
        match kind {
            cry_mannequin::MannequinXmlKind::AnimationDatabase => (
                nw_model::CrySourceAssetKind::MannequinAnimationDatabase,
                serde_json::to_value(
                    cry_mannequin::MannequinAnimationDatabaseSource::from_legacy(&path, &bytes)?,
                )?,
            ),
            cry_mannequin::MannequinXmlKind::Actions | cry_mannequin::MannequinXmlKind::Tags => (
                nw_model::CrySourceAssetKind::MannequinTagDefinition,
                serde_json::to_value(cry_mannequin::MannequinTagDefinitionSource::from_legacy(
                    &path, &bytes,
                )?)?,
            ),
            cry_mannequin::MannequinXmlKind::ControllerDefinition => (
                nw_model::CrySourceAssetKind::MannequinControllerDefinition,
                serde_json::to_value(
                    cry_mannequin::MannequinControllerDefinitionSource::from_legacy(&path, &bytes)?,
                )?,
            ),
        }
    } else if cry_mannequin::BlendSpaceXmlKind::from_source_path(&path).is_some() {
        match cry_mannequin::BlendSpaceDocumentSource::from_legacy(&path, &bytes)? {
            cry_mannequin::BlendSpaceDocumentSource::BlendSpace(source) => (
                nw_model::CrySourceAssetKind::BlendSpace,
                serde_json::to_value(source)?,
            ),
            cry_mannequin::BlendSpaceDocumentSource::CombinedBlendSpace(source) => (
                nw_model::CrySourceAssetKind::CombinedBlendSpace,
                serde_json::to_value(source)?,
            ),
        }
    } else {
        bail!("unsupported Mannequin source path {path}");
    };
    extras.source_assets.push(nw_model::CrySourceAsset {
        path: path.clone(),
        kind,
        document,
    });
    add_resource(extras, &path, embedded_kind_for_source(kind), bytes);
    add_dependency(extras, &path);
    Ok(())
}

fn add_audio_source(
    source: &dyn AssetSource,
    path: &str,
    extras: &mut nw_model::CryAssetExtras,
) -> Result<()> {
    add_audio_source_with_options(source, path, extras, true)
}

/// Load an audio asset into extras. When `follow_preloads` is false, ATL control
/// documents are retained for lookup without recursively shipping every bank
/// listed in `AudioPreloads` (the animation-trigger resolver selects only the
/// banks a clip actually needs).
fn add_audio_source_with_options(
    source: &dyn AssetSource,
    path: &str,
    extras: &mut nw_model::CryAssetExtras,
    follow_preloads: bool,
) -> Result<()> {
    let path = normalize_path(path);
    if extras
        .source_assets
        .iter()
        .any(|asset| asset.path.eq_ignore_ascii_case(&path))
    {
        return Ok(());
    }
    let bytes = read_required(source, &path)?;
    let lowercase = path.to_ascii_lowercase();
    let (kind, document, preload_banks) =
        if lowercase.ends_with(cry_audio::WWISE_TRIGGER_BANK_MAP_FILE) {
            let entries = cry_audio::WwiseTriggerBankMap::parse(&bytes)?
                .entries()
                .collect::<Vec<_>>();
            (
                nw_model::CrySourceAssetKind::WwiseTriggerBankMap,
                serde_json::to_value(entries)?,
                Vec::new(),
            )
        } else {
            match source_extension(&path).as_str() {
                "csv" if cry_audio::is_audio_mapping_source(&path) => (
                    nw_model::CrySourceAssetKind::AudioMapping,
                    serde_json::to_value(cry_audio::parse_audio_mapping(&path, &bytes)?)?,
                    Vec::new(),
                ),
                "xml" => {
                    let xml = str::from_utf8(&bytes)
                        .with_context(|| format!("decode UTF-8 ATL controls {path}"))?;
                    let controls = cry_audio::AudioControlsSource::from_xml(&path, xml)
                        .with_context(|| format!("parse ATL controls {path}"))?;
                    let banks = if follow_preloads {
                        audio_preload_banks(&controls)
                    } else {
                        Vec::new()
                    };
                    (
                        nw_model::CrySourceAssetKind::AudioControls,
                        serde_json::to_value(controls)?,
                        banks,
                    )
                }
                "bnk" => {
                    let bank = cry_audio::WwiseSoundBank::parse(&bytes)
                        .with_context(|| format!("parse Wwise soundbank {path}"))?;
                    let embedded_media = bank
                        .media
                        .iter()
                        .copied()
                        .map(|entry| {
                            let media = bank.embedded_media(&bytes, entry)?;
                            let info = cry_audio::WwiseMediaInfo::parse(media)?;
                            Ok::<_, anyhow::Error>(serde_json::json!({
                                "mediaId": entry.id,
                                "info": info,
                            }))
                        })
                        .collect::<Result<Vec<_>>>()?;
                    (
                        nw_model::CrySourceAssetKind::WwiseSoundBank,
                        serde_json::json!({
                            "bank": bank,
                            "embeddedMedia": embedded_media,
                        }),
                        Vec::new(),
                    )
                }
                "wem" => (
                    nw_model::CrySourceAssetKind::WwiseMedia,
                    serde_json::to_value(
                        cry_audio::WwiseMediaInfo::parse(&bytes)
                            .with_context(|| format!("parse Wwise media {path}"))?,
                    )?,
                    Vec::new(),
                ),
                _ => bail!("unsupported audio source path {path}"),
            }
        };
    extras.source_assets.push(nw_model::CrySourceAsset {
        path: path.clone(),
        kind,
        document,
    });
    add_resource(extras, &path, embedded_kind_for_source(kind), bytes);
    add_dependency(extras, &path);

    for (bank_path, localized) in preload_banks {
        if localized {
            let basename = bank_path.rsplit('/').next().unwrap_or(&bank_path);
            let pattern = format!("sounds/wwise/**/{basename}");
            let paths = source.matching_paths(&pattern)?;
            if paths.is_empty() {
                bail!("localized ATL preload {bank_path} matched no shipped language bank");
            }
            for path in paths {
                add_audio_source(source, &path, extras)?;
            }
        } else {
            add_dependency(extras, &bank_path);
            add_audio_source(source, &bank_path, extras)?;
        }
    }
    Ok(())
}

fn audio_preload_banks(controls: &cry_audio::AudioControlsSource) -> Vec<(String, bool)> {
    let mut banks: Vec<(String, bool)> = Vec::new();
    for file in controls.preloads.iter().flat_map(|preload| {
        preload
            .files
            .iter()
            .chain(preload.config_groups.iter().flat_map(|group| &group.files))
    }) {
        let name = normalize_path(&file.wwise_name);
        let path = if name.contains('/') {
            name
        } else {
            format!("sounds/wwise/{name}")
        };
        let localized = file
            .localized
            .as_deref()
            .is_some_and(|value| value.eq_ignore_ascii_case("true"));
        if !banks
            .iter()
            .any(|(existing, _)| existing.eq_ignore_ascii_case(&path))
        {
            banks.push((path, localized));
        }
    }
    banks
}

fn load_event_database(source: &dyn AssetSource, path: &str) -> Result<LoadedEventDatabase> {
    let bytes = read_required(source, path)?;
    let xml =
        str::from_utf8(&bytes).with_context(|| format!("decode UTF-8 animation events {path}"))?;
    let parsed = cry_animation::AnimationEventDatabase::from_xml(xml)
        .with_context(|| format!("parse animation events {path}"))?;
    Ok(LoadedEventDatabase { parsed, bytes })
}

fn add_xml_source(
    extras: &mut nw_model::CryAssetExtras,
    path: &str,
    kind: nw_model::CrySourceAssetKind,
    element: &cry_xml::XmlElement,
) {
    if has_source_asset(extras, path) {
        add_dependency(extras, path);
        return;
    }
    extras.source_assets.push(nw_model::CrySourceAsset {
        path: normalize_path(path),
        kind,
        document: xml_json(element),
    });
    add_dependency(extras, path);
}

fn has_source_asset(extras: &nw_model::CryAssetExtras, path: &str) -> bool {
    extras
        .source_assets
        .iter()
        .any(|asset| asset.path.eq_ignore_ascii_case(path))
}

fn xml_json(element: &cry_xml::XmlElement) -> serde_json::Value {
    serde_json::json!({
        "name": element.name,
        "attributes": element.attributes,
        "text": element.text,
        "children": element.children.iter().map(xml_json).collect::<Vec<_>>(),
    })
}

fn add_dependency(extras: &mut nw_model::CryAssetExtras, path: &str) {
    let path = normalize_path(path);
    if !extras
        .dependencies
        .iter()
        .any(|existing| existing.eq_ignore_ascii_case(&path))
    {
        extras.dependencies.push(path);
    }
}

fn add_resource(
    extras: &mut nw_model::CryAssetExtras,
    path: &str,
    kind: nw_model::CryEmbeddedResourceKind,
    bytes: impl Into<Arc<[u8]>>,
) {
    let path = normalize_path(path);
    if extras
        .resource_payloads
        .iter()
        .any(|resource| resource.kind == kind && resource.source_path.eq_ignore_ascii_case(&path))
    {
        return;
    }
    extras
        .resource_payloads
        .push(nw_model::CryResourcePayload::new(path, kind, bytes));
}

fn embedded_kind_for_source(
    kind: nw_model::CrySourceAssetKind,
) -> nw_model::CryEmbeddedResourceKind {
    match kind {
        nw_model::CrySourceAssetKind::CharacterDefinition => {
            nw_model::CryEmbeddedResourceKind::CharacterDefinition
        }
        nw_model::CrySourceAssetKind::CharacterParameters => {
            nw_model::CryEmbeddedResourceKind::CharacterParameters
        }
        nw_model::CrySourceAssetKind::AnimationEvents => {
            nw_model::CryEmbeddedResourceKind::AnimationEvents
        }
        nw_model::CrySourceAssetKind::MannequinAnimationDatabase => {
            nw_model::CryEmbeddedResourceKind::MannequinAnimationDatabase
        }
        nw_model::CrySourceAssetKind::MannequinTagDefinition => {
            nw_model::CryEmbeddedResourceKind::MannequinTagDefinition
        }
        nw_model::CrySourceAssetKind::MannequinControllerDefinition => {
            nw_model::CryEmbeddedResourceKind::MannequinControllerDefinition
        }
        nw_model::CrySourceAssetKind::BlendSpace => nw_model::CryEmbeddedResourceKind::BlendSpace,
        nw_model::CrySourceAssetKind::CombinedBlendSpace => {
            nw_model::CryEmbeddedResourceKind::CombinedBlendSpace
        }
        nw_model::CrySourceAssetKind::AudioControls => {
            nw_model::CryEmbeddedResourceKind::AudioControls
        }
        nw_model::CrySourceAssetKind::AudioMapping => {
            nw_model::CryEmbeddedResourceKind::AudioMapping
        }
        nw_model::CrySourceAssetKind::MaterialEffectsFxLibrary => {
            nw_model::CryEmbeddedResourceKind::MaterialEffectsFxLibrary
        }
        nw_model::CrySourceAssetKind::ParticleLibrary => {
            nw_model::CryEmbeddedResourceKind::ParticleLibrary
        }
        nw_model::CrySourceAssetKind::WwiseSoundBank => {
            nw_model::CryEmbeddedResourceKind::WwiseSoundBank
        }
        nw_model::CrySourceAssetKind::WwiseMedia => nw_model::CryEmbeddedResourceKind::WwiseMedia,
        nw_model::CrySourceAssetKind::WwiseDecodedWave => {
            nw_model::CryEmbeddedResourceKind::WwiseDecodedWave
        }
        nw_model::CrySourceAssetKind::WwiseTriggerBankMap => {
            nw_model::CryEmbeddedResourceKind::WwiseTriggerBankMap
        }
        nw_model::CrySourceAssetKind::NvClothFabric => {
            nw_model::CryEmbeddedResourceKind::NvClothFabric
        }
        nw_model::CrySourceAssetKind::NvClothMaterial => {
            nw_model::CryEmbeddedResourceKind::NvClothMaterial
        }
        nw_model::CrySourceAssetKind::VertexShape => nw_model::CryEmbeddedResourceKind::VertexShape,
        nw_model::CrySourceAssetKind::CollisionFilters => {
            nw_model::CryEmbeddedResourceKind::CollisionFilters
        }
        nw_model::CrySourceAssetKind::PhysicsMaterialSet => {
            nw_model::CryEmbeddedResourceKind::PhysicsMaterialSet
        }
        nw_model::CrySourceAssetKind::TerrainHeightmap => {
            nw_model::CryEmbeddedResourceKind::TerrainHeightmap
        }
        nw_model::CrySourceAssetKind::TerrainSurfaceMap => {
            nw_model::CryEmbeddedResourceKind::TerrainSurfaceMap
        }
        nw_model::CrySourceAssetKind::TerrainMapSettings => {
            nw_model::CryEmbeddedResourceKind::TerrainMapSettings
        }
        nw_model::CrySourceAssetKind::TerrainWaterQuadtree => {
            nw_model::CryEmbeddedResourceKind::TerrainWaterQuadtree
        }
        nw_model::CrySourceAssetKind::TerrainTractMap => {
            nw_model::CryEmbeddedResourceKind::TerrainTractMap
        }
        nw_model::CrySourceAssetKind::TerrainRegionMaterial => {
            nw_model::CryEmbeddedResourceKind::TerrainRegionMaterial
        }
        nw_model::CrySourceAssetKind::TerrainWorldMaterial => {
            nw_model::CryEmbeddedResourceKind::TerrainWorldMaterial
        }
        nw_model::CrySourceAssetKind::TerrainSettings => {
            nw_model::CryEmbeddedResourceKind::TerrainSettings
        }
        nw_model::CrySourceAssetKind::TerrainTracts => {
            nw_model::CryEmbeddedResourceKind::TerrainTracts
        }
        nw_model::CrySourceAssetKind::VegetationDistribution => {
            nw_model::CryEmbeddedResourceKind::VegetationDistribution
        }
        nw_model::CrySourceAssetKind::VegetationRegion => {
            nw_model::CryEmbeddedResourceKind::VegetationRegion
        }
        nw_model::CrySourceAssetKind::VegetationImage => {
            nw_model::CryEmbeddedResourceKind::VegetationImage
        }
        nw_model::CrySourceAssetKind::SliceMetadata => {
            nw_model::CryEmbeddedResourceKind::SliceMetadata
        }
        nw_model::CrySourceAssetKind::RegionSliceData => {
            nw_model::CryEmbeddedResourceKind::RegionSliceData
        }
        nw_model::CrySourceAssetKind::RegionMetadata => {
            nw_model::CryEmbeddedResourceKind::RegionMetadata
        }
        nw_model::CrySourceAssetKind::RegionChunks => {
            nw_model::CryEmbeddedResourceKind::RegionChunks
        }
    }
}

fn read_required(source: &dyn AssetSource, path: &str) -> Result<Vec<u8>> {
    source
        .read(path)
        .with_context(|| format!("referenced Cry asset not found: {path}"))
}

fn source_extension(path: &str) -> String {
    let lowercase = path.to_ascii_lowercase();
    if lowercase.ends_with(".i_caf") {
        return "i_caf".to_owned();
    }
    lowercase
        .rsplit_once('.')
        .map_or(String::new(), |(_, extension)| extension.to_owned())
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

pub(super) fn push_unique_path(list: &mut Vec<String>, value: &str) {
    if !list
        .iter()
        .any(|existing| existing.eq_ignore_ascii_case(value))
    {
        list.push(value.to_owned());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    pub(super) struct EmptySource;

    impl nw_asset_graph::AssetSource for EmptySource {
        fn read(&self, _path: &str) -> Option<Vec<u8>> {
            None
        }

        fn matching_paths(&self, _pattern: &str) -> Result<Vec<String>> {
            Ok(Vec::new())
        }
    }

    impl AssetSource for EmptySource {
        fn materials(&self, _cgf: &[u8], _mesh: &MeshRef) -> Option<nw_model::MaterialSet> {
            None
        }
    }

    #[derive(Default)]
    pub(super) struct ContextSource {
        assets: BTreeMap<String, Vec<u8>>,
    }

    impl ContextSource {
        pub(super) fn with(mut self, path: &str, bytes: impl Into<Vec<u8>>) -> Self {
            self.assets.insert(normalize_path(path), bytes.into());
            self
        }
    }

    impl nw_asset_graph::AssetSource for ContextSource {
        fn read(&self, path: &str) -> Option<Vec<u8>> {
            self.assets.get(&normalize_path(path)).cloned()
        }

        fn matching_paths(&self, _pattern: &str) -> Result<Vec<String>> {
            Ok(Vec::new())
        }
    }

    impl AssetSource for ContextSource {
        fn materials(&self, _cgf: &[u8], _mesh: &MeshRef) -> Option<nw_model::MaterialSet> {
            None
        }

        fn allows_asset_hint_fallback(&self) -> bool {
            true
        }
    }
}
