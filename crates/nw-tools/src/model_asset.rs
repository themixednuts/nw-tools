//! Complete Cry character/model graph resolution for glTF export.

use std::collections::HashSet;
use std::str;

use anyhow::{Context, Result, bail};
use cry_character::{
    AttachmentKind, CharacterAnimationEntryKind, CharacterDefinition, CharacterParameters,
};
use glam::{Mat4, Quat, Vec3};

use crate::model::{AssetSource, MeshRef};

pub(crate) struct ResolveOptions<'a> {
    pub runner: &'a nw_jobs::JobRunner,
    pub no_materials: bool,
    pub skeleton: Option<&'a str>,
    pub animations: &'a [String],
    pub animation_events: Option<&'a str>,
    pub mannequin: &'a [String],
    pub audio: &'a [String],
}

pub(crate) struct ResolvedAsset {
    pub model: nw_model::Model,
    pub materials: Option<nw_model::MaterialSet>,
    pub animations: Vec<nw_model::ModelAnimation>,
    pub extras: nw_model::CryAssetExtras,
    parsed_animation_assets: HashSet<(usize, String)>,
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
            add_dependency(&mut extras, source_path);
            add_dependency(&mut extras, &skeleton_path);
            let mut animations = Vec::new();
            for clip in clips {
                if clip_targets_skeleton(&clip, &skeleton) {
                    animations.push(nw_model::ModelAnimation { skeleton: 0, clip });
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
                parsed_animation_assets: [(0, normalize_path(source_path))].into_iter().collect(),
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
                !model.is_empty(),
            )?;
            ResolvedAsset {
                model,
                materials,
                animations: Vec::new(),
                extras: nw_model::CryAssetExtras {
                    source_path: normalize_path(source_path),
                    ..Default::default()
                },
                parsed_animation_assets: HashSet::new(),
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
        add_xml_source(
            &mut resolved.extras,
            path,
            nw_model::CrySourceAssetKind::AnimationEvents,
            &event_database
                .as_ref()
                .expect("loaded event database")
                .source,
        );
    }
    push_animation_assets(
        runner,
        source,
        options.animations,
        event_database.as_ref(),
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
    apply_resolved_dependencies(runner, source, dependency_graph.assets(), &mut resolved)?;
    Ok(resolved)
}

fn apply_resolved_dependencies(
    runner: &nw_jobs::JobRunner,
    source: &dyn AssetSource,
    paths: &[String],
    resolved: &mut ResolvedAsset,
) -> Result<()> {
    for path in paths {
        add_dependency(&mut resolved.extras, path);
    }

    if !resolved.model.skeletons.is_empty() {
        for path in paths
            .iter()
            .filter(|path| source_extension(path) == "chrparams")
        {
            if !has_source_asset(&resolved.extras, path) {
                load_character_parameters(runner, source, path, false, 0, resolved)?;
            }
        }
        let animation_paths = paths
            .iter()
            .filter(|path| matches!(source_extension(path).as_str(), "caf" | "i_caf" | "dba"))
            .cloned()
            .collect::<Vec<_>>();
        push_animation_assets(runner, source, &animation_paths, None, 0, false, resolved)?;
    }

    for path in paths
        .iter()
        .filter(|path| source_extension(path) == "animevents")
    {
        if has_source_asset(&resolved.extras, path) {
            continue;
        }
        let database = load_event_database(source, path)?;
        add_xml_source(
            &mut resolved.extras,
            path,
            nw_model::CrySourceAssetKind::AnimationEvents,
            &database.source,
        );
        for animation in &mut resolved.animations {
            animation
                .clip
                .events
                .extend(database.events_for(&animation.clip.source_path).cloned());
        }
    }

    for path in paths.iter().filter(|path| {
        cry_mannequin::MannequinXmlKind::from_source_path(path).is_some()
            || cry_mannequin::BlendSpaceXmlKind::from_source_path(path).is_some()
    }) {
        if !has_source_asset(&resolved.extras, path) {
            add_mannequin_source(source, path, &mut resolved.extras)?;
        }
    }

    for path in paths {
        if !has_source_asset(&resolved.extras, path) && is_audio_source(source, path) {
            add_audio_source(source, path, &mut resolved.extras)?;
        }
    }
    Ok(())
}

fn is_audio_source(source: &dyn AssetSource, path: &str) -> bool {
    if path
        .to_ascii_lowercase()
        .ends_with(cry_audio::WWISE_TRIGGER_BANK_MAP_FILE)
        || matches!(source_extension(path).as_str(), "bnk" | "wem")
    {
        return true;
    }
    if source_extension(path) != "xml" {
        return false;
    }
    source
        .read(path)
        .and_then(|bytes| {
            str::from_utf8(&bytes)
                .ok()
                .map(|xml| cry_audio::AudioControlsSource::from_xml(path, xml).is_ok())
        })
        .unwrap_or(false)
}

fn resolve_cdf(
    runner: &nw_jobs::JobRunner,
    source: &dyn AssetSource,
    source_path: &str,
    bytes: &[u8],
    no_materials: bool,
) -> Result<ResolvedAsset> {
    let mut resolving = HashSet::new();
    resolve_cdf_inner(
        runner,
        source,
        source_path,
        bytes,
        no_materials,
        &mut resolving,
    )
}

fn resolve_cdf_inner(
    runner: &nw_jobs::JobRunner,
    source: &dyn AssetSource,
    source_path: &str,
    bytes: &[u8],
    no_materials: bool,
    resolving: &mut HashSet<String>,
) -> Result<ResolvedAsset> {
    let cycle_key = normalize_path(source_path).to_ascii_lowercase();
    if !resolving.insert(cycle_key.clone()) {
        bail!("cyclic nested CDF reference at {source_path}");
    }
    let xml = str::from_utf8(bytes).with_context(|| format!("decode UTF-8 CDF {source_path}"))?;
    let definition = CharacterDefinition::from_xml(xml)
        .with_context(|| format!("parse character definition {source_path}"))?;
    if definition.model.skeleton.trim().is_empty() {
        bail!("CDF {source_path} has an empty Model File");
    }

    let skeleton_path = normalize_path(&definition.model.skeleton);
    let skeleton_bytes = read_required(source, &skeleton_path)?;
    let skeleton_heap = source
        .read(&format!("{skeleton_path}heap"))
        .unwrap_or_default();
    let skeleton_file = cry_chunk::CgfFile::parse(&skeleton_bytes)
        .with_context(|| format!("parse CDF skeleton {skeleton_path}"))?;
    let mut model = nw_model::Model::try_from_cgf(&skeleton_file, &skeleton_heap)
        .with_context(|| format!("assemble CDF skeleton {skeleton_path}"))?;
    if model.skeletons.is_empty() {
        bail!("CDF skeleton {skeleton_path} has no CompiledBones chunk");
    }

    let mut materials = None;
    if !no_materials && !model.is_empty() {
        let set = if let Some(path) = definition.model.material.as_deref() {
            load_material(source, path)?
        } else {
            resolve_primary_materials(
                source,
                &skeleton_bytes,
                &MeshRef::for_key(&skeleton_path),
                None,
                false,
                true,
            )?
            .with_context(|| format!("resolve material for CDF skeleton {skeleton_path}"))?
        };
        append_material_table(&mut model, &mut materials, set)?;
    }

    let mut extras = nw_model::CryAssetExtras {
        source_path: normalize_path(source_path),
        ..Default::default()
    };
    add_xml_source(
        &mut extras,
        source_path,
        nw_model::CrySourceAssetKind::CharacterDefinition,
        &definition.source,
    );
    add_dependency(&mut extras, &skeleton_path);

    let mut animations = Vec::new();
    let mut parsed_animation_assets = HashSet::new();
    for attachment in &definition.attachments {
        let Some(binding) = attachment.binding.as_deref() else {
            continue;
        };
        let binding = normalize_path(binding);
        let binding_bytes = read_required(source, &binding)?;
        if source_extension(&binding) == "cdf" {
            let mut child = resolve_cdf_inner(
                runner,
                source,
                &binding,
                &binding_bytes,
                no_materials,
                resolving,
            )
            .with_context(|| format!("resolve nested CDF attachment {binding}"))?;
            if let Some(set) = child.materials.take() {
                append_material_table(&mut child.model, &mut materials, set)?;
            }
            let skeleton_offset = match attachment.kind {
                AttachmentKind::Bone => {
                    let bone_name = attachment.bone_name.as_deref().with_context(|| {
                        format!("CA_BONE nested CDF attachment {binding} has no BoneName")
                    })?;
                    let local = attachment_bone_local(&model, bone_name, attachment)?;
                    model.append_character_attachment(child.model, 0, bone_name, local)?
                }
                AttachmentKind::Face
                | AttachmentKind::Skin
                | AttachmentKind::VertexCloth
                | AttachmentKind::Proxy
                | AttachmentKind::PendulumRow
                | AttachmentKind::Unknown(_) => model.append_character_root(
                    child.model,
                    attachment_character_transform(attachment),
                )?,
            };
            for mut animation in child.animations {
                animation.skeleton += skeleton_offset;
                animations.push(animation);
            }
            for (skeleton, path) in child.parsed_animation_assets {
                parsed_animation_assets.insert((skeleton + skeleton_offset, path));
            }
            for animation in &mut child.extras.unbound_animations {
                animation.skeleton += skeleton_offset;
            }
            merge_extras(&mut extras, child.extras);
            add_dependency(&mut extras, &binding);
            if let Some(simulation) = attachment.simulation_binding.as_deref() {
                add_dependency(&mut extras, simulation);
            }
            continue;
        }
        let binding_heap = source.read(&format!("{binding}heap")).unwrap_or_default();
        let binding_file = cry_chunk::CgfFile::parse(&binding_bytes)
            .with_context(|| format!("parse CDF attachment {binding}"))?;
        let mut part = nw_model::Model::try_from_cgf(&binding_file, &binding_heap)
            .with_context(|| format!("assemble CDF attachment {binding}"))?;
        if part.is_empty() && part.auxiliary_nodes.is_empty() {
            bail!("CDF attachment {binding} contains no drawable geometry");
        }

        if !no_materials && !part.is_empty() {
            let explicit = attachment
                .material
                .as_deref()
                .or_else(|| attachment.material_lods.get(&0).map(String::as_str));
            let set = if let Some(path) = explicit {
                load_material(source, path)?
            } else {
                resolve_primary_materials(
                    source,
                    &binding_bytes,
                    &MeshRef::for_key(&binding),
                    None,
                    false,
                    true,
                )?
                .with_context(|| format!("resolve material for CDF attachment {binding}"))?
            };
            append_material_table(&mut part, &mut materials, set)?;
        }

        match attachment.kind {
            AttachmentKind::Bone => {
                let bone_name = attachment
                    .bone_name
                    .as_deref()
                    .with_context(|| format!("CA_BONE attachment {binding} has no BoneName"))?;
                let local = attachment_bone_local(&model, bone_name, attachment)?;
                model.append_attached_geometry(part, 0, bone_name, local)?;
            }
            AttachmentKind::Face => {
                model.append_root_geometry(part, attachment_character_transform(attachment));
            }
            // Skin/cloth bindings carry bone mappings and belong on the character skin.
            AttachmentKind::Skin | AttachmentKind::VertexCloth => {
                model.append_skinned_geometry(part, 0)?;
            }
            AttachmentKind::Proxy | AttachmentKind::PendulumRow | AttachmentKind::Unknown(_) => {
                // If these extension types carry drawable binding geometry, retain it
                // in character space; the exact simulation attributes remain in extras.
                model.append_root_geometry(part, attachment_character_transform(attachment));
            }
        }
        add_dependency(&mut extras, &binding);
        if let Some(simulation) = attachment.simulation_binding.as_deref() {
            add_dependency(&mut extras, simulation);
        }
    }

    let mut resolved = ResolvedAsset {
        model,
        materials,
        animations,
        extras,
        parsed_animation_assets,
    };
    let parameters_path = definition
        .model
        .params_override
        .as_deref()
        .map(normalize_path)
        .unwrap_or_else(|| replace_extension(&skeleton_path, "chrparams"));
    load_character_parameters(
        runner,
        source,
        &parameters_path,
        definition.model.params_override.is_some(),
        0,
        &mut resolved,
    )?;
    resolving.remove(&cycle_key);
    Ok(resolved)
}

fn load_character_parameters(
    runner: &nw_jobs::JobRunner,
    source: &dyn AssetSource,
    path: &str,
    required: bool,
    skeleton: usize,
    resolved: &mut ResolvedAsset,
) -> Result<()> {
    let path = normalize_path(path);
    if has_source_asset(&resolved.extras, &path) {
        return Ok(());
    }
    let Some(bytes) = source.read(&path) else {
        if required {
            bail!("CDF ParamsOverride asset not found: {path}");
        }
        return Ok(());
    };
    let xml = str::from_utf8(&bytes).with_context(|| format!("decode UTF-8 {path}"))?;
    let parameters = CharacterParameters::from_xml(xml)
        .with_context(|| format!("parse character parameters {path}"))?;
    add_xml_source(
        &mut resolved.extras,
        &path,
        nw_model::CrySourceAssetKind::CharacterParameters,
        &parameters.source,
    );

    let mut animation_directory = String::new();
    let mut event_database = None;
    let mut visited = HashSet::new();
    load_animation_list(
        runner,
        source,
        &path,
        &parameters,
        &mut animation_directory,
        &mut event_database,
        &mut visited,
        skeleton,
        resolved,
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn load_animation_list(
    runner: &nw_jobs::JobRunner,
    source: &dyn AssetSource,
    source_path: &str,
    parameters: &CharacterParameters,
    animation_directory: &mut String,
    event_database: &mut Option<cry_animation::AnimationEventDatabase>,
    visited: &mut HashSet<String>,
    skeleton: usize,
    resolved: &mut ResolvedAsset,
) -> Result<()> {
    if !visited.insert(normalize_path(source_path).to_ascii_lowercase()) {
        return Ok(());
    }
    for entry in &parameters.animations {
        let value = normalize_path(entry.path.trim_start_matches(['/', '\\']));
        match entry.kind {
            CharacterAnimationEntryKind::FilePath => {
                *animation_directory = value.trim_end_matches('/').to_owned();
            }
            CharacterAnimationEntryKind::ParseSubfolders => {}
            CharacterAnimationEntryKind::AnimationEventDatabase => {
                if event_database.is_none() {
                    let database = load_event_database(source, &value)?;
                    add_xml_source(
                        &mut resolved.extras,
                        &value,
                        nw_model::CrySourceAssetKind::AnimationEvents,
                        &database.source,
                    );
                    for animation in resolved
                        .animations
                        .iter_mut()
                        .filter(|animation| animation.skeleton == skeleton)
                    {
                        animation.clip.events = database
                            .events_for(&animation.clip.source_path)
                            .cloned()
                            .collect();
                    }
                    *event_database = Some(database);
                }
            }
            CharacterAnimationEntryKind::Include => {
                let bytes = read_required(source, &value)?;
                let xml = str::from_utf8(&bytes)
                    .with_context(|| format!("decode included CHRPARAMS {value}"))?;
                let included = CharacterParameters::from_xml(xml)
                    .with_context(|| format!("parse included CHRPARAMS {value}"))?;
                add_xml_source(
                    &mut resolved.extras,
                    &value,
                    nw_model::CrySourceAssetKind::CharacterParameters,
                    &included.source,
                );
                load_animation_list(
                    runner,
                    source,
                    &value,
                    &included,
                    animation_directory,
                    event_database,
                    visited,
                    skeleton,
                    resolved,
                )?;
            }
            CharacterAnimationEntryKind::TracksDatabase => {
                let wildcard = value.contains(['*', '?', '[']);
                let paths = if wildcard {
                    source.matching_paths(&value)?
                } else {
                    vec![value.clone()]
                };
                push_animation_assets(
                    runner,
                    source,
                    &paths,
                    event_database.as_ref(),
                    skeleton,
                    false,
                    resolved,
                )?;
            }
            CharacterAnimationEntryKind::FaceLibrary => {
                add_dependency(&mut resolved.extras, &value);
            }
            CharacterAnimationEntryKind::WildcardAsset => {
                let pattern = animation_path(animation_directory, &value);
                let paths = source.matching_paths(&pattern)?;
                let (animations, mannequin): (Vec<_>, Vec<_>) =
                    paths.into_iter().partition(|path| {
                        matches!(source_extension(path).as_str(), "caf" | "i_caf" | "dba")
                    });
                push_animation_assets(
                    runner,
                    source,
                    &animations,
                    event_database.as_ref(),
                    skeleton,
                    false,
                    resolved,
                )?;
                for path in mannequin {
                    if matches!(source_extension(&path).as_str(), "bspace" | "comb") {
                        add_mannequin_source(source, &path, &mut resolved.extras)?;
                    }
                }
            }
            CharacterAnimationEntryKind::Asset => {
                let path = animation_path(animation_directory, &value);
                match source_extension(&path).as_str() {
                    "caf" | "i_caf" | "dba" => {
                        push_animation_assets(
                            runner,
                            source,
                            std::slice::from_ref(&path),
                            event_database.as_ref(),
                            skeleton,
                            false,
                            resolved,
                        )?;
                    }
                    "bspace" | "comb" => {
                        add_mannequin_source(source, &path, &mut resolved.extras)?;
                    }
                    _ => add_dependency(&mut resolved.extras, &path),
                }
            }
            CharacterAnimationEntryKind::UnknownDirective => {
                bail!(
                    "unsupported CHRPARAMS directive `{}` in {source_path}",
                    entry.name
                );
            }
        }
    }
    Ok(())
}

struct ParsedAnimationAsset {
    path: String,
    is_dba: bool,
    clips: Vec<cry_animation::AnimationClip>,
}

fn push_animation_assets(
    runner: &nw_jobs::JobRunner,
    source: &dyn AssetSource,
    paths: &[String],
    events: Option<&cry_animation::AnimationEventDatabase>,
    skeleton: usize,
    require_mapping: bool,
    resolved: &mut ResolvedAsset,
) -> Result<()> {
    let mut paths = paths
        .iter()
        .map(|path| normalize_path(path))
        .filter(|path| {
            !resolved
                .parsed_animation_assets
                .contains(&(skeleton, path.clone()))
        })
        .collect::<Vec<_>>();
    paths.sort_unstable();
    paths.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
    if paths.is_empty() {
        return Ok(());
    }
    let parsed = runner.try_map(&paths, |path| {
        let bytes = read_required(source, path)?;
        let is_dba = source_extension(path) == "dba";
        let mut clips = if is_dba {
            cry_animation::AnimationClip::parse_dba(&bytes)
                .with_context(|| format!("decode tracks database {path}"))?
        } else {
            vec![
                cry_animation::AnimationClip::parse(path.clone(), &bytes)
                    .with_context(|| format!("decode animation {path}"))?,
            ]
        };
        if let Some(events) = events {
            for clip in &mut clips {
                clip.events = events.events_for(&clip.source_path).cloned().collect();
            }
        }
        Ok::<_, anyhow::Error>(ParsedAnimationAsset {
            path: path.clone(),
            is_dba,
            clips,
        })
    })?;
    let target = resolved
        .model
        .skeletons
        .get(skeleton)
        .with_context(|| format!("animation targets missing skeleton {skeleton}"))?;
    for asset in parsed {
        let mut mapped = 0usize;
        let mut duplicate = false;
        for clip in asset.clips {
            if resolved.animations.iter().any(|existing| {
                existing.skeleton == skeleton
                    && existing
                        .clip
                        .source_path
                        .eq_ignore_ascii_case(&clip.source_path)
            }) {
                duplicate = true;
                continue;
            }
            if !clip_targets_skeleton(&clip, target) {
                record_unbound_animation(&mut resolved.extras, &clip.source_path, skeleton);
                continue;
            }
            mapped += 1;
            resolved
                .animations
                .push(nw_model::ModelAnimation { skeleton, clip });
        }
        resolved
            .parsed_animation_assets
            .insert((skeleton, asset.path.clone()));
        add_dependency(&mut resolved.extras, &asset.path);
        if require_mapping && mapped == 0 && (asset.is_dba || !duplicate) {
            if asset.is_dba {
                bail!(
                    "tracks database {} has no controllers targeting model skeleton {skeleton}",
                    asset.path
                );
            }
            bail!(
                "CAF {} has no controllers targeting model skeleton {skeleton}",
                asset.path
            );
        }
    }
    Ok(())
}

fn clip_targets_skeleton(
    clip: &cry_animation::AnimationClip,
    skeleton: &nw_model::Skeleton,
) -> bool {
    clip.caf.controllers.iter().any(|controller| {
        skeleton
            .bones
            .iter()
            .any(|bone| bone.controller_id == controller.controller_id)
    })
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
    add_dependency(extras, &path);
    Ok(())
}

fn add_audio_source(
    source: &dyn AssetSource,
    path: &str,
    extras: &mut nw_model::CryAssetExtras,
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
                "xml" => {
                    let xml = str::from_utf8(&bytes)
                        .with_context(|| format!("decode UTF-8 ATL controls {path}"))?;
                    let controls = cry_audio::AudioControlsSource::from_xml(&path, xml)
                        .with_context(|| format!("parse ATL controls {path}"))?;
                    let banks = audio_preload_banks(&controls);
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

fn resolve_primary_materials(
    source: &dyn AssetSource,
    cgf: &[u8],
    mesh: &MeshRef,
    material_override: Option<&str>,
    no_materials: bool,
    has_geometry: bool,
) -> Result<Option<nw_model::MaterialSet>> {
    if no_materials || !has_geometry {
        return Ok(None);
    }
    if let Some(xml) = material_override {
        return Ok(Some(xml.parse().context("parse --mtl material XML")?));
    }
    if let Some(materials) = source.materials(cgf, mesh) {
        return Ok(Some(materials));
    }
    resolve_material_name(source, cgf)?.map(Some).context(
        "mesh material could not be resolved; use --no-materials for an explicit geometry-only export",
    )
}

/// Resolve Cry's legacy basename-only MtlName references without guessing when
/// multiple shipped paths share that basename. All candidates must project to
/// the same lossless material document.
fn resolve_material_name(
    source: &dyn AssetSource,
    cgf: &[u8],
) -> Result<Option<nw_model::MaterialSet>> {
    let file = cry_chunk::CgfFile::parse(cgf)?;
    let Some(material) = file.materials().values().next() else {
        return Ok(None);
    };
    let mut name = normalize_path(material.name.as_str());
    if name.is_empty() {
        return Ok(None);
    }
    if !name.to_ascii_lowercase().ends_with(".mtl") {
        name.push_str(".mtl");
    }
    let basename = name.rsplit('/').next().unwrap_or(&name);
    let mut paths = if name.contains('/') {
        vec![name.clone()]
    } else {
        source.matching_paths(&format!("**/{basename}"))?
    };
    paths.sort_by_key(|path| path.to_ascii_lowercase());
    paths.dedup_by(|left, right| left.eq_ignore_ascii_case(right));

    let mut resolved: Option<nw_model::MaterialSet> = None;
    for path in paths {
        let Some(bytes) = source.read(&path) else {
            continue;
        };
        let xml =
            str::from_utf8(&bytes).with_context(|| format!("decode UTF-8 material {path}"))?;
        let candidate = xml
            .parse::<nw_model::MaterialSet>()
            .with_context(|| format!("parse material {path}"))?;
        if resolved
            .as_ref()
            .is_some_and(|existing| existing != &candidate)
        {
            bail!("ambiguous basename-only Cry material reference {name}");
        }
        resolved = Some(candidate);
    }
    Ok(resolved)
}

fn append_material_table(
    model: &mut nw_model::Model,
    combined: &mut Option<nw_model::MaterialSet>,
    set: nw_model::MaterialSet,
) -> Result<()> {
    let table = combined.get_or_insert_with(nw_model::MaterialSet::default);
    let offset = table.append(set);
    model.rebase_material_ids(offset)?;
    Ok(())
}

fn load_material(source: &dyn AssetSource, path: &str) -> Result<nw_model::MaterialSet> {
    let path = if source_extension(path) == "mtl" {
        normalize_path(path)
    } else {
        format!("{}.mtl", normalize_path(path))
    };
    let bytes = read_required(source, &path)?;
    let xml = str::from_utf8(&bytes).with_context(|| format!("decode UTF-8 material {path}"))?;
    xml.parse()
        .with_context(|| format!("parse material {path}"))
}

fn load_event_database(
    source: &dyn AssetSource,
    path: &str,
) -> Result<cry_animation::AnimationEventDatabase> {
    let bytes = read_required(source, path)?;
    let xml =
        str::from_utf8(&bytes).with_context(|| format!("decode UTF-8 animation events {path}"))?;
    cry_animation::AnimationEventDatabase::from_xml(xml)
        .with_context(|| format!("parse animation events {path}"))
}

fn attachment_bone_local(
    model: &nw_model::Model,
    bone_name: &str,
    attachment: &cry_character::CharacterAttachment,
) -> Result<Mat4> {
    if attachment.relative_rotation.is_some() || attachment.relative_position.is_some() {
        return Ok(cry_transform(
            attachment.relative_rotation,
            attachment.relative_position,
        ));
    }
    let skeleton = model
        .primary_skeleton()
        .context("CDF model has no skeleton")?;
    let index = skeleton
        .bone_index(bone_name)
        .with_context(|| format!("CDF attachment targets missing bone {bone_name}"))?;
    let bone_world = skeleton
        .bone_world(index)
        .with_context(|| format!("CDF skeleton hierarchy for {bone_name} is cyclic"))?;
    Ok(bone_world.inverse() * attachment_character_transform(attachment))
}

fn attachment_character_transform(attachment: &cry_character::CharacterAttachment) -> Mat4 {
    cry_transform(attachment.character_rotation, attachment.character_position)
}

fn cry_transform(rotation: Option<[f32; 4]>, translation: Option<[f32; 3]>) -> Mat4 {
    let rotation = rotation.map_or(Quat::IDENTITY, |value| {
        // Cry XML serializes quaternion as w,x,y,z.
        Quat::from_xyzw(value[1], value[2], value[3], value[0]).normalize()
    });
    let translation = translation.map_or(Vec3::ZERO, Vec3::from_array);
    nw_model::math::cry_to_gltf_mat(Mat4::from_rotation_translation(rotation, translation))
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

fn merge_extras(target: &mut nw_model::CryAssetExtras, source: nw_model::CryAssetExtras) {
    for asset in source.source_assets {
        if !target
            .source_assets
            .iter()
            .any(|existing| existing.path.eq_ignore_ascii_case(&asset.path))
        {
            target.source_assets.push(asset);
        }
    }
    for dependency in source.dependencies {
        add_dependency(target, &dependency);
    }
    for animation in source.unbound_animations {
        record_unbound_animation(target, &animation.source_path, animation.skeleton);
    }
    target.non_render_nodes.extend(source.non_render_nodes);
}

fn read_required(source: &dyn AssetSource, path: &str) -> Result<Vec<u8>> {
    source
        .read(path)
        .with_context(|| format!("referenced Cry asset not found: {path}"))
}

fn animation_path(directory: &str, value: &str) -> String {
    if directory.is_empty() {
        normalize_path(value)
    } else {
        normalize_path(&format!("{directory}/{value}"))
    }
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

fn replace_extension(path: &str, extension: &str) -> String {
    path.rsplit_once('.').map_or_else(
        || format!("{path}.{extension}"),
        |(stem, _)| format!("{stem}.{extension}"),
    )
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    struct EmptySource;

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

    #[test]
    fn empty_animation_work_does_not_require_a_skeleton() {
        let mut resolved = ResolvedAsset {
            model: nw_model::Model {
                meshes: Vec::new(),
                skeletons: Vec::new(),
                auxiliary_nodes: Vec::new(),
            },
            materials: None,
            animations: Vec::new(),
            extras: nw_model::CryAssetExtras::default(),
            parsed_animation_assets: HashSet::new(),
        };

        push_animation_assets(
            &nw_jobs::JobRunner::inline(),
            &EmptySource,
            &[],
            None,
            0,
            false,
            &mut resolved,
        )
        .unwrap();
    }
}
