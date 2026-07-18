//! Source-asset resolution: CDF/character-definition assembly, model-context
//! dependency projection, and mannequin/audio/dependency source embedding.
//!
//! Split out of `model_asset` as a pure move; shared helpers stay in the parent.

use super::animation::{load_character_parameters, push_animation_assets};
use super::dependencies::{add_dependency_resources, dependency_resource_kind};
use super::materials::{append_material_table, load_material, resolve_primary_materials};
use super::*;

pub(super) fn model_context_assets(
    source: &dyn AssetSource,
    source_path: &str,
    index: &nw_asset_graph::AssetDependencyIndex,
) -> Vec<String> {
    // Cross only character-definition ownership wrappers in reverse. Scene
    // instances are a frontier: walking through their consumers reaches every
    // world/region that places the character and turns one model package into a
    // near-global asset dump.
    let owners = index
        .transitive_consumers_where(source_path, |edge| source_extension(edge.source()) == "cdf");
    let mut owned_assets = Vec::with_capacity(owners.len() + 1);
    owned_assets.push(normalize_path(source_path));
    owned_assets.extend(owners.iter().cloned());

    let mut scene_contexts = owned_assets
        .iter()
        .flat_map(|path| index.consumers_of(path))
        .map(nw_asset_graph::AssetDependencyEdge::source)
        .filter(|path| is_legacy_scene_asset(path))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if is_legacy_scene_asset(source_path) {
        scene_contexts.push(normalize_path(source_path));
    }
    scene_contexts.sort_by_key(|path| normalize_path(path).to_ascii_lowercase());
    scene_contexts.dedup_by(|left, right| left.eq_ignore_ascii_case(right));

    // Placement records author the selected slice and exact variant metadata as
    // sibling dependencies. Project only their explicit `*.variant` sibling;
    // retaining the consumer itself would pull every world placement of this
    // character into the model package.
    let mut variants = scene_contexts
        .iter()
        .flat_map(|path| {
            index.associated_dependencies_from_consumers_where(path, |edge| {
                edge.relation().ends_with("variant")
            })
        })
        .collect::<Vec<_>>();
    if source_path.to_ascii_lowercase().ends_with(".slice.meta") {
        variants.push(normalize_path(source_path));
    }

    let mut context_roots = owners;
    context_roots.extend(scene_contexts);
    context_roots.extend(variants);
    let projected = index
        .transitive_dependencies_where(context_roots.iter().map(String::as_str), |edge| {
            is_model_context_asset(source, edge.target())
        });
    context_roots.extend(projected);
    context_roots.sort_by_key(|path| normalize_path(path).to_ascii_lowercase());
    context_roots.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
    context_roots
}

fn is_legacy_scene_asset(path: &str) -> bool {
    matches!(
        source_extension(path).as_str(),
        "slice" | "dynamicslice" | "entity" | "entities" | "entities_xml" | "prefab"
    )
}

fn is_model_context_asset(source: &dyn AssetSource, path: &str) -> bool {
    if dependency_resource_kind(path).is_some()
        || cry_mannequin::MannequinXmlKind::from_source_path(path).is_some()
        || cry_mannequin::BlendSpaceXmlKind::from_source_path(path).is_some()
    {
        return true;
    }
    match source_extension(path).as_str() {
        "chrparams" | "animevents" | "caf" | "i_caf" | "dba" | "fxl" => true,
        _ => is_audio_source(source, path),
    }
}

pub(super) fn apply_resolved_dependencies(
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
            &database.parsed.source,
        );
        add_resource(
            &mut resolved.extras,
            path,
            nw_model::CryEmbeddedResourceKind::AnimationEvents,
            database.bytes,
        );
        for animation in &mut resolved.animations {
            animation.clip.events.extend(
                database
                    .parsed
                    .events_for(&animation.clip.source_path)
                    .cloned(),
            );
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
    add_dependency_resources(runner, source, paths, resolved)?;
    Ok(())
}

fn is_audio_source(source: &dyn AssetSource, path: &str) -> bool {
    if cry_audio::is_audio_mapping_source(path) {
        return true;
    }
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

pub(super) fn resolve_cdf(
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
    if !no_materials && model.has_render_geometry() {
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
    add_resource(
        &mut extras,
        source_path,
        nw_model::CryEmbeddedResourceKind::CharacterDefinition,
        Arc::<[u8]>::from(bytes),
    );
    add_dependency(&mut extras, &skeleton_path);

    let mut physics = nw_model::PhysicsScene::default();
    if let Some(path) = definition.model.physics.as_deref() {
        let path = normalize_path(path);
        let bytes = read_required(source, &path)?;
        match source_extension(&path).as_str() {
            "rnr" => physics
                .shape_assets
                .push(crate::rnr_asset::physics_shape_asset(&path, &bytes)?),
            "phys" => add_resource(
                &mut extras,
                &path,
                nw_model::CryEmbeddedResourceKind::CharacterPhysics,
                bytes,
            ),
            extension => bail!(
                "unsupported CDF Physics asset extension `{extension}` for {path}; expected .rnr or .phys"
            ),
        }
        add_dependency(&mut extras, &path);
    }
    if let Some(path) = definition.model.rig.as_deref() {
        let path = normalize_path(path);
        let bytes = read_required(source, &path)?;
        if source_extension(&path) != "rig" {
            bail!("unsupported CDF Rig asset extension for {path}; expected .rig");
        }
        add_resource(
            &mut extras,
            &path,
            nw_model::CryEmbeddedResourceKind::CharacterRig,
            bytes,
        );
        add_dependency(&mut extras, &path);
    }

    let mut animations = Vec::new();
    let mut parsed_animation_assets = HashSet::new();
    resolve_attachments(
        runner,
        source,
        source_path,
        no_materials,
        resolving,
        &definition.attachments,
        AttachmentSink {
            model: &mut model,
            materials: &mut materials,
            animations: &mut animations,
            parsed_animation_assets: &mut parsed_animation_assets,
            physics: &mut physics,
            extras: &mut extras,
        },
    );

    let mut resolved = ResolvedAsset {
        model,
        materials,
        animations,
        extras,
        physics,
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

/// Mutable accumulators threaded through per-attachment resolution.
struct AttachmentSink<'a> {
    model: &'a mut nw_model::Model,
    materials: &'a mut Option<nw_model::MaterialSet>,
    animations: &'a mut Vec<nw_model::ModelAnimation>,
    parsed_animation_assets: &'a mut HashSet<(usize, String)>,
    physics: &'a mut nw_model::PhysicsScene,
    extras: &'a mut nw_model::CryAssetExtras,
}

/// Resolve every CDF attachment, degrading a single failing attachment to a
/// warning (still recording its binding) instead of aborting the character.
fn resolve_attachments(
    runner: &nw_jobs::JobRunner,
    source: &dyn AssetSource,
    source_path: &str,
    no_materials: bool,
    resolving: &mut HashSet<String>,
    attachments: &[cry_character::CharacterAttachment],
    sink: AttachmentSink<'_>,
) {
    let AttachmentSink {
        model,
        materials,
        animations,
        parsed_animation_assets,
        physics,
        extras,
    } = sink;
    for attachment in attachments {
        let iteration = AttachmentSink {
            model: &mut *model,
            materials: &mut *materials,
            animations: &mut *animations,
            parsed_animation_assets: &mut *parsed_animation_assets,
            physics: &mut *physics,
            extras: &mut *extras,
        };
        let Err(error) = resolve_single_attachment(
            runner,
            source,
            source_path,
            no_materials,
            resolving,
            attachment,
            iteration,
        ) else {
            continue;
        };
        // One failing attachment degrades to a warning; the character still
        // exports with the remaining attachments. Character-level errors are
        // reserved for the primary skeleton/model itself.
        let binding = attachment.binding.as_deref().map(normalize_path);
        let label = attachment.name.as_deref().unwrap_or("<unnamed>");
        let binding_label = binding.as_deref().unwrap_or("<no binding>");
        eprintln!(
            "warning: skipping CDF attachment `{label}` ({binding_label}) in {source_path}: {error:#}"
        );
        // Preserve the reference and, where a resource kind exists, the raw
        // bytes at the catalog path so the export still records the binding.
        if let Some(binding) = binding.as_deref() {
            if let Some(bytes) = source.read(binding)
                && let Some(kind) = dependency_resource_kind(binding)
            {
                add_resource(extras, binding, kind, bytes);
            }
            add_dependency(extras, binding);
        }
        if let Some(simulation) = attachment.simulation_binding.as_deref() {
            add_dependency(extras, &normalize_path(simulation));
        }
    }
}

/// Resolve one CDF attachment into the shared model/extras. Fallible so a single
/// failing attachment can degrade to a warning without aborting the character.
fn resolve_single_attachment(
    runner: &nw_jobs::JobRunner,
    source: &dyn AssetSource,
    source_path: &str,
    no_materials: bool,
    resolving: &mut HashSet<String>,
    attachment: &cry_character::CharacterAttachment,
    sink: AttachmentSink<'_>,
) -> Result<()> {
    let AttachmentSink {
        model,
        materials,
        animations,
        parsed_animation_assets,
        physics,
        extras,
    } = sink;

    let binding = match attachment.binding.as_deref() {
        Some(binding) => binding,
        None if attachment.kind == AttachmentKind::VertexCloth => {
            bail!("CA_VCLOTH attachment in {source_path} has no render Binding")
        }
        None => return Ok(()),
    };
    let binding = normalize_path(binding);
    if attachment.kind == AttachmentKind::VertexCloth && source_extension(&binding) != "skin" {
        bail!("CA_VCLOTH render Binding must be a .skin asset: {binding}");
    }
    let simulation_binding = if attachment.kind == AttachmentKind::VertexCloth {
        let simulation = attachment
            .simulation_binding
            .as_deref()
            .with_context(|| format!("CA_VCLOTH attachment {binding} has no SimBinding"))?;
        let simulation = normalize_path(simulation);
        if source_extension(&simulation) != "skin" {
            bail!("CA_VCLOTH SimBinding must be a .skin asset: {simulation}");
        }
        Some(simulation)
    } else {
        attachment.simulation_binding.as_deref().map(normalize_path)
    };
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
            append_material_table(&mut child.model, materials, set)?;
        }
        let skeleton_offset = match attachment.kind {
            AttachmentKind::Bone => {
                let bone_name = attachment.bone_name.as_deref().with_context(|| {
                    format!("CA_BONE nested CDF attachment {binding} has no BoneName")
                })?;
                let local = attachment_bone_local(model, bone_name, attachment)?;
                model.append_character_attachment(child.model, 0, bone_name, local)?
            }
            AttachmentKind::Face
            | AttachmentKind::Skin
            | AttachmentKind::VertexCloth
            | AttachmentKind::Proxy
            | AttachmentKind::PendulumRow
            | AttachmentKind::Unknown(_) => {
                model.append_character_root(child.model, attachment_character_transform(attachment))?
            }
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
        physics.shape_assets.append(&mut child.physics.shape_assets);
        physics.hit_volumes.append(&mut child.physics.hit_volumes);
        physics.rigid_bodies.append(&mut child.physics.rigid_bodies);
        merge_extras(extras, child.extras);
        add_dependency(extras, &binding);
        if let Some(simulation) = simulation_binding.as_deref() {
            add_dependency(extras, simulation);
        }
        return Ok(());
    }
    // CA_CLOTH attachments bind an NvCloth `.cloth` asset, not a Cry chunk file.
    if source_extension(&binding) == "cloth" {
        cloth::resolve_cloth_attachment(
            source,
            &binding,
            &binding_bytes,
            attachment,
            no_materials,
            model,
            materials,
            extras,
        )?;
        return Ok(());
    }
    let binding_heap = source.read(&format!("{binding}heap")).unwrap_or_default();
    let binding_file = cry_chunk::CgfFile::parse(&binding_bytes)
        .with_context(|| format!("parse CDF attachment {binding}"))?;
    let mut part = nw_model::Model::try_from_cgf(&binding_file, &binding_heap)
        .with_context(|| format!("assemble CDF attachment {binding}"))?;
    if part.is_empty() && part.auxiliary_nodes.is_empty() {
        bail!("CDF attachment {binding} contains no drawable geometry");
    }

    if !no_materials && part.has_render_geometry() {
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
        append_material_table(&mut part, materials, set)?;
    }

    match attachment.kind {
        AttachmentKind::Bone => {
            let bone_name = attachment
                .bone_name
                .as_deref()
                .with_context(|| format!("CA_BONE attachment {binding} has no BoneName"))?;
            let local = attachment_bone_local(model, bone_name, attachment)?;
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
    add_dependency(extras, &binding);
    if attachment.kind == AttachmentKind::VertexCloth {
        let simulation = simulation_binding
            .as_deref()
            .expect("validated CA_VCLOTH SimBinding");
        let simulation_bytes = read_required(source, simulation)?;
        let simulation_heap = source
            .read(&format!("{simulation}heap"))
            .unwrap_or_default();
        let simulation_file = cry_chunk::CgfFile::parse(&simulation_bytes)
            .with_context(|| format!("parse CA_VCLOTH SimBinding {simulation}"))?;
        let simulation_model = nw_model::Model::try_from_cgf(&simulation_file, &simulation_heap)
            .with_context(|| format!("assemble CA_VCLOTH SimBinding {simulation}"))?;
        if simulation_model.is_empty() {
            bail!("CA_VCLOTH SimBinding {simulation} contains no simulation geometry");
        }
        model.append_cloth_simulation_geometry(simulation_model, 0)?;
    }
    if let Some(simulation) = simulation_binding.as_deref() {
        add_dependency(extras, simulation);
    }
    Ok(())
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
    for resource in source.resource_payloads {
        add_resource(target, &resource.source_path, resource.kind, resource.bytes);
    }
    for resource in source.embedded_resources {
        if !target.embedded_resources.iter().any(|existing| {
            existing.kind == resource.kind
                && existing
                    .source_path
                    .eq_ignore_ascii_case(&resource.source_path)
        }) {
            target.embedded_resources.push(resource);
        }
    }
    target.non_render_nodes.extend(source.non_render_nodes);
}

fn replace_extension(path: &str, extension: &str) -> String {
    path.rsplit_once('.').map_or_else(
        || format!("{path}.{extension}"),
        |(stem, _)| format!("{stem}.{extension}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_asset::tests::ContextSource;

    fn asset_reference_stream(hint: &str) -> Vec<u8> {
        format!(
            r#"<ObjectStream version="3"><Class name="Asset" type="{{77A19D40-8731-4D3C-9041-1B43047366A4}}" value="id={{7A1472D1-DF54-5362-BC71-9974D5F25572}}:0,type={{78802ABF-9595-463A-8D2B-D022F906F9B1}},hint={{{hint}}}"/></ObjectStream>"#
        )
        .into_bytes()
    }

    #[test]
    fn model_context_stops_at_nearest_scene_instance_frontier() {
        let source = ContextSource::default()
            .with(
                "objects/alligator.cdf",
                br#"<CharacterDefinition><Model File="objects/alligator.chr"/></CharacterDefinition>"#,
            )
            .with("objects/alligator.chr", b"model")
            .with(
                "slices/characters/alligator.dynamicslice",
                asset_reference_stream("objects/alligator.cdf"),
            )
            .with(
                "coatgen/world/alligator_spawn.dynamicslice",
                asset_reference_stream("slices/characters/alligator.dynamicslice"),
            );
        let paths = vec![
            "objects/alligator.cdf".to_owned(),
            "slices/characters/alligator.dynamicslice".to_owned(),
            "coatgen/world/alligator_spawn.dynamicslice".to_owned(),
        ];
        let index = nw_asset_graph::AssetDependencyIndex::build_with_runner(
            &source,
            &paths,
            &nw_jobs::JobRunner::inline(),
        )
        .unwrap();

        assert_eq!(
            index.transitive_consumers_of("objects/alligator.cdf"),
            vec![
                "slices/characters/alligator.dynamicslice",
                "coatgen/world/alligator_spawn.dynamicslice"
            ]
        );
        assert_eq!(
            model_context_assets(&source, "objects/alligator.cdf", &index),
            vec!["slices/characters/alligator.dynamicslice"]
        );
    }

    fn put_u32(bytes: &mut [u8], at: usize, value: u32) {
        bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn put_u64(bytes: &mut [u8], at: usize, value: u64) {
        bytes[at..at + 8].copy_from_slice(&value.to_le_bytes());
    }

    /// A minimal-but-valid 3-particle single-triangle NvCloth `.cloth` asset with
    /// a Direct render mapping, an all-zero (finite) material, and no render skin.
    fn synthetic_cloth() -> Vec<u8> {
        const HEADER: usize = 16;
        const MATERIAL: usize = 208;
        const LAYOUT: usize = 272;
        let data = HEADER + MATERIAL + LAYOUT;
        let verts = data;
        let indices = verts + 3 * 64;
        let render_map = indices + 3 * 4;
        let paint = render_map + 3 * 4;
        let render_model = paint + 3 * 4;
        let material = render_model + 1;
        let triangles = material + 1;
        let total = triangles + 3 * 4;

        let mut bytes = vec![0u8; total];
        put_u32(&mut bytes, 0, 1); // header version; flags stay zero.

        let positions = [[0.0f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        for (index, position) in positions.iter().enumerate() {
            let base = verts + index * 64;
            bytes[base..base + 4].copy_from_slice(&position[0].to_le_bytes());
            bytes[base + 4..base + 8].copy_from_slice(&position[1].to_le_bytes());
            bytes[base + 8..base + 12].copy_from_slice(&position[2].to_le_bytes());
            bytes[base + 28..base + 32].copy_from_slice(&1.0f32.to_le_bytes()); // tangent quat w.
            bytes[base + 48] = u8::MAX; // full weight on skeleton bone 0.
        }
        for offset in [indices, render_map, triangles] {
            for index in 0..3u32 {
                put_u32(&mut bytes, offset + index as usize * 4, index);
            }
        }

        let layout = HEADER + MATERIAL;
        put_u32(&mut bytes, layout, 3); // internal counts[0] = num_particles.
        put_u32(&mut bytes, layout + 9 * 4, 3); // internal counts[9] = triangles.
        put_u64(&mut bytes, layout + 40 + 9 * 8, triangles as u64); // internal offsets[9].
        let geometry = layout + 128;
        put_u32(&mut bytes, geometry, 3); // geometry counts: particles, indices, render map.
        put_u32(&mut bytes, geometry + 4, 3);
        put_u32(&mut bytes, geometry + 8, 3);
        let geo_off = geometry + 16;
        put_u64(&mut bytes, geo_off, verts as u64);
        put_u64(&mut bytes, geo_off + 8, indices as u64);
        put_u64(&mut bytes, geo_off + 2 * 8, render_map as u64);
        put_u64(&mut bytes, geo_off + 6 * 8, render_model as u64);
        put_u64(&mut bytes, geo_off + 7 * 8, material as u64);
        put_u64(&mut bytes, geo_off + 8 * 8, paint as u64);
        let flags = geo_off + 11 * 8;
        put_u32(&mut bytes, flags, 0); // no barycentric / backstop.
        put_u32(&mut bytes, flags + 4, 4); // four skin influences.
        bytes
    }

    fn single_bone_model() -> nw_model::Model {
        let mut model = nw_model::Model::default();
        model.set_primary_skeleton(nw_model::Skeleton {
            bones: vec![nw_model::Bone {
                name: "root".to_owned(),
                controller_id: 0,
                parent: None,
                local: Mat4::IDENTITY,
                inverse_bind: Mat4::IDENTITY,
            }],
            placement: None,
        });
        model
    }

    fn attachment(kind: AttachmentKind, binding: &str) -> cry_character::CharacterAttachment {
        cry_character::CharacterAttachment {
            kind,
            name: Some("attach".to_owned()),
            bone_name: None,
            binding: Some(binding.to_owned()),
            simulation_binding: None,
            material: None,
            material_lods: std::collections::BTreeMap::new(),
            flags: None,
            character_rotation: None,
            character_position: None,
            relative_rotation: None,
            relative_position: None,
            attributes: std::collections::BTreeMap::new(),
            children: Vec::new(),
        }
    }

    #[test]
    fn one_failing_attachment_does_not_abort_the_character() {
        let source = ContextSource::default()
            .with("test/good.cloth", synthetic_cloth())
            .with("test/bad.skin", b"not a cry chunk file".to_vec());
        let mut model = single_bone_model();
        let mut materials = None;
        let mut animations = Vec::new();
        let mut parsed = HashSet::new();
        let mut physics = nw_model::PhysicsScene::default();
        let mut extras = nw_model::CryAssetExtras::default();
        let attachments = vec![
            attachment(AttachmentKind::Unknown("CA_CLOTH".to_owned()), "test/good.cloth"),
            attachment(AttachmentKind::Skin, "test/bad.skin"),
        ];

        resolve_attachments(
            &nw_jobs::JobRunner::inline(),
            &source,
            "test/character.cdf",
            true,
            &mut HashSet::new(),
            &attachments,
            AttachmentSink {
                model: &mut model,
                materials: &mut materials,
                animations: &mut animations,
                parsed_animation_assets: &mut parsed,
                physics: &mut physics,
                extras: &mut extras,
            },
        );

        // The good cloth still contributes its skinned simulation mesh.
        let cloth_meshes = model
            .meshes
            .iter()
            .filter(|mesh| mesh.role == nw_model::MeshRole::ClothSimulation)
            .count();
        assert_eq!(cloth_meshes, 1, "cloth simulation mesh should be present");
        assert_eq!(model.vertex_count(), 3);
        assert!(
            extras.resource_payloads.iter().any(|resource| {
                resource.kind == nw_model::CryEmbeddedResourceKind::NvClothFabric
                    && resource.source_path == "test/good.cloth"
            }),
            "fabric should ship as an embedded resource"
        );
        // The corrupt attachment degrades to a recorded dependency, not a fatal error.
        assert!(
            extras
                .dependencies
                .iter()
                .any(|dependency| dependency == "test/bad.skin"),
            "corrupt binding should still be recorded as a dependency"
        );
    }
}
