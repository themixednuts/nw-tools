use std::collections::{BTreeMap, BTreeSet};

use glam::Mat4;
use nw_objectstream::asset_reference::{read_asset_path_or_string_owned, read_asset_value};

use super::canonical::{
    particle_settings_fingerprint, stringify_component_id, stringify_entity_reference,
    stringify_particle_entity_ids,
};
use super::*;

pub(super) fn parse_scene_particles(
    source: &dyn AssetSource,
    model: &nw_model::Model,
    model_source_path: &str,
    scene_path: &str,
    bytes: &[u8],
) -> Result<Vec<SceneParticleEmitter>> {
    let stream = nw_objectstream::ObjectStream::from_bytes(bytes, Some(&OBJECTSTREAM_LOOKUP))
        .with_context(|| format!("parse ObjectStream {scene_path}"))?;
    let codec = nw_objectstream::schema::SchemaValueCodec::new(Some(&OBJECTSTREAM_LOOKUP));
    let entities = nw_objectstream::query::az_entity_elements_skipping_fields(
        stream.elements(),
        &["m_embeddedShapeEntity"],
    )
    .collect::<Vec<_>>();
    let mut scene_entities = BTreeMap::new();
    for entity in &entities {
        let Some((entity_id, scene_entity)) =
            decode_scene_entity(source, &codec, scene_path, entity)?
        else {
            continue;
        };
        scene_entities.insert(entity_id, scene_entity);
    }

    let mut emitters = Vec::new();
    for entity in entities {
        let context = scene_entity_context(scene_path, entity)?;
        let Some(entity_id) = context.entity_id.map(nw_model::CryEntityId::get) else {
            continue;
        };
        let Some(components) = nw_objectstream::query::child_by_field_ignore_case_or_crc(
            entity,
            "Components",
            ENTITY_COMPONENTS_FIELD_CRC,
        ) else {
            continue;
        };

        let mut attachment = None;
        let mut particles = Vec::new();
        for component in components.children() {
            if component.id() == nw_reflected_types::types::AttachmentComponent::TYPE_ID.as_inner()
            {
                attachment = Some(decode_attachment(component)?);
            } else if component.id()
                == nw_reflected_types::types::ParticleComponent::TYPE_ID.as_inner()
            {
                particles.push(decode_particle_component(component)?);
            }
        }
        if particles.is_empty() {
            continue;
        }

        let entity_parent_id = scene_entities
            .get(&entity_id)
            .map_or(0, |entity| entity.parent_id);

        for particle in particles {
            if particle.settings.selected_emitter.trim().is_empty() {
                continue;
            }
            let Some(owner_entity_id) = particle_owner_entity(
                entity_id,
                attachment.as_ref(),
                model_source_path,
                &scene_entities,
            ) else {
                continue;
            };
            let entity_transform =
                relative_entity_transform(entity_id, owner_entity_id, &scene_entities);
            let (placement, placement_issue, binding) = resolve_particle_placement(
                model,
                owner_entity_id,
                entity_transform,
                attachment.as_ref(),
                &scene_entities,
            );

            let mut particle_source = particle.source.clone();
            stringify_particle_entity_ids(&mut particle_source);
            let mut attachment_source = attachment
                .as_ref()
                .map(|attachment| attachment.source.clone());
            if let Some(source) = &mut attachment_source {
                stringify_entity_reference(source, "Target ID");
            }
            let mut attachment_component_source = attachment
                .as_ref()
                .map(|attachment| attachment.component_source.clone());
            if let Some(source) = &mut attachment_component_source {
                stringify_component_id(source);
                if let Some(configuration) = source.get_mut("Configuration") {
                    stringify_entity_reference(configuration, "Target ID");
                }
            }
            let authored_settings_fingerprint = particle_settings_fingerprint(
                &particle_source,
                attachment_source.as_ref(),
                &scene_entities,
            )?;
            let authored_payload = nw_model::CryParticleAuthoredPayload {
                component_version: particle.component_version,
                settings_version: particle.settings_version,
                particle_component: particle_source,
                attachment_component_version: attachment
                    .as_ref()
                    .and_then(|attachment| attachment.component_version),
                attachment_configuration_version: attachment
                    .as_ref()
                    .and_then(|attachment| attachment.configuration_version),
                attachment_component: attachment_component_source,
            };
            let model_attachment = attachment.as_ref().map(|decoded| {
                let configuration = &decoded.configuration;
                nw_model::CryParticleAttachment {
                    target_entity_id: configuration.target_id.into(),
                    target_bone_name: configuration.target_bone_name.clone(),
                    target_offset: particle_transform(
                        configuration.target_offset.translation.to_array(),
                        configuration.target_offset.rotation.to_array(),
                        configuration.target_offset.scale.to_array(),
                    ),
                    attached_initially: configuration.attached_initially,
                    scale_source: configuration.scale_source,
                    update_tolerance: configuration.update_tolerance,
                }
            });
            emitters.push(SceneParticleEmitter {
                emitter: nw_model::CryParticleEmitter {
                    selected_emitter: particle.settings.selected_emitter.clone(),
                    particle_library_asset_id: particle
                        .particle_library_asset_id
                        .map(|id| id.to_string()),
                    particle_library_path: None,
                    visible: particle.settings.visible,
                    enabled: particle.settings.enable,
                    attach_to_mesh: particle.settings.attach_to_mesh,
                    load_emitter_on_activate: particle.load_emitter_on_activate,
                    color: [
                        particle.settings.color.red,
                        particle.settings.color.green,
                        particle.settings.color.blue,
                        particle.settings.color.alpha,
                    ],
                    particle_target_entity_id: particle.settings.target_entity.into(),
                    gpu_edge_dissolve_target_entity_id: particle
                        .settings
                        .gpu_edge_dissolve_target_entity
                        .into(),
                    entity_transform: entity_transform.unwrap_or_default(),
                    entity_parent_id: entity_parent_id.into(),
                    placement,
                    placement_issue,
                    attachment: model_attachment,
                    authored_payload,
                    authored_settings_fingerprint,
                    context: context.clone(),
                },
                particle_library_asset_id: particle.particle_library_asset_id,
                binding,
            });
        }
    }
    Ok(emitters)
}

fn decode_scene_entity(
    source: &dyn AssetSource,
    codec: &nw_objectstream::schema::SchemaValueCodec<'_>,
    scene_path: &str,
    entity: &nw_objectstream::Element,
) -> Result<Option<(u64, SceneEntity)>> {
    let context = scene_entity_context(scene_path, entity)?;
    let Some(entity_id) = context.entity_id.map(nw_model::CryEntityId::get) else {
        return Ok(None);
    };
    let Some(components) = nw_objectstream::query::child_by_field_ignore_case_or_crc(
        entity,
        "Components",
        ENTITY_COMPONENTS_FIELD_CRC,
    ) else {
        return Ok(Some((
            entity_id,
            SceneEntity {
                name: context.entity_name,
                parent_id: 0,
                local_transform: Default::default(),
                character_definitions: Vec::new(),
            },
        )));
    };

    let mut placement = None;
    for component in components.children() {
        if component.id() == nw_reflected_types::types::TransformComponent::TYPE_ID.as_inner() {
            let fields = codec.field_map(component)?;
            let value: nw_reflected_types::types::TransformComponent =
                serde_json::from_value(fields.to_serde_value()?)
                    .context("deserialize generated TransformComponent")?;
            placement = Some((
                value.parent,
                particle_transform(
                    value.local_transform.translation.to_array(),
                    value.local_transform.rotation.to_array(),
                    value.local_transform.scale.to_array(),
                ),
            ));
            break;
        }
        if component.id() == nw_reflected_types::types::GameTransformComponent::TYPE_ID.as_inner()
            && placement.is_none()
        {
            let fields = codec.field_map(component)?;
            let value: nw_reflected_types::types::GameTransformComponent =
                serde_json::from_value(fields.to_serde_value()?)
                    .context("deserialize generated GameTransformComponent")?;
            placement = Some((
                value.parent_id,
                particle_transform(
                    value.local_tm.translation.to_array(),
                    value.local_tm.rotation.to_array(),
                    value.local_tm.scale.to_array(),
                ),
            ));
        }
    }
    let (parent_id, local_transform) = placement.unwrap_or_default();
    Ok(Some((
        entity_id,
        SceneEntity {
            name: context.entity_name,
            parent_id,
            local_transform,
            character_definitions: character_definition_paths(source, components)?,
        },
    )))
}

fn decode_attachment(component: &nw_objectstream::Element) -> Result<DecodedAttachment> {
    let source = nw_scene::read_attachment_component(component)
        .context("decode generated AttachmentComponent")?;
    let component_source = serde_json::to_value(&source.component)
        .context("serialize generated AttachmentComponent")?;
    let configuration = source.component.configuration;
    let authored = serde_json::to_value(&configuration)
        .context("serialize generated AttachmentConfiguration")?;
    Ok(DecodedAttachment {
        configuration,
        component_version: source.component_version,
        configuration_version: source.configuration_version,
        component_source,
        source: authored,
    })
}

fn decode_particle_component(
    component: &nw_objectstream::Element,
) -> Result<DecodedParticleComponent> {
    let source = nw_scene::read_particle_component(component)
        .context("decode generated ParticleComponent")?;
    let authored =
        serde_json::to_value(&source.component).context("serialize generated ParticleComponent")?;
    let asset_id = source.component.particle_library_asset_id;
    let particle_library_asset_id = (!asset_id.is_nil()).then(|| {
        nw_asset::AssetId::new(
            uuid::Uuid::from_bytes(*asset_id.guid.as_bytes()),
            asset_id.sub_id,
        )
    });
    Ok(DecodedParticleComponent {
        settings: source.component.particle,
        particle_library_asset_id,
        load_emitter_on_activate: source.component.load_emitter_on_activate,
        component_version: source.component_version,
        settings_version: source.settings_version,
        source: authored,
    })
}

fn scene_entity_context(
    path: &str,
    entity: &nw_objectstream::Element,
) -> Result<nw_model::CryParticleEmitterContext> {
    let entity_id = nw_objectstream::query::child_by_field_ignore_case_or_crc(
        entity,
        "id",
        ENTITY_ID_FIELD_CRC,
    )
    .map(nw_objectstream::value::read_entity_id)
    .transpose()
    .context("decode AZ::Entity id")?
    .map(Into::into);
    let entity_name = nw_objectstream::query::child_by_field_ignore_case_or_crc(
        entity,
        "name",
        ENTITY_NAME_FIELD_CRC,
    )
    .map(nw_objectstream::value::read_trimmed_string_owned)
    .transpose()
    .context("decode AZ::Entity name")?
    .flatten();
    Ok(nw_model::CryParticleEmitterContext {
        source_path: normalize_path(path),
        alternate_source_paths: Vec::new(),
        entity_id,
        entity_name,
    })
}

fn character_definition_paths(
    source: &dyn AssetSource,
    components: &nw_objectstream::Element,
) -> Result<Vec<String>> {
    let mut paths = Vec::new();
    for element in components.iter_recursive() {
        let path = if element.id() == &nw_objectstream::types::ASSET && element.data().is_some() {
            let asset = read_asset_value(element).context("decode entity CDF asset")?;
            let asset_id = nw_asset::AssetId::new(asset.guid(), asset.sub_id());
            source
                .path_by_id(asset_id)
                .map(|path| normalize_path(&path))
                .or_else(|| {
                    let hint = normalize_path(asset.hint().trim());
                    (source.allows_asset_hint_fallback()
                        && !hint.is_empty()
                        && source.contains(&hint))
                    .then_some(hint)
                })
        } else if element
            .field()
            .is_some_and(|field| field.eq_ignore_ascii_case("m_cdfPath"))
        {
            read_asset_path_or_string_owned(element)
                .context("read entity m_cdfPath")?
                .map(|path| normalize_path(&path))
        } else {
            None
        };
        let Some(path) = path else {
            continue;
        };
        if source_extension(&path) == "cdf" && source.contains(&path) {
            push_unique_path(&mut paths, path);
        }
    }
    paths.sort_by_key(|path| path.to_ascii_lowercase());
    Ok(paths)
}

fn particle_owner_entity(
    particle_entity_id: u64,
    attachment: Option<&DecodedAttachment>,
    model_source_path: &str,
    entities: &BTreeMap<u64, SceneEntity>,
) -> Option<u64> {
    let attachment_target = attachment
        .filter(|attachment| attachment.configuration.attached_initially)
        .map(|attachment| attachment.configuration.target_id)
        .filter(|target| *target != INVALID_ENTITY_ID)
        .filter(|target| entities.contains_key(target));
    attachment_target
        .and_then(|target| model_owner_entity(target, model_source_path, entities))
        .or_else(|| model_owner_entity(particle_entity_id, model_source_path, entities))
}

pub(super) fn resolve_particle_placement(
    model: &nw_model::Model,
    owner_entity_id: u64,
    entity_transform: Option<nw_model::CryParticleTransform>,
    attachment: Option<&DecodedAttachment>,
    entities: &BTreeMap<u64, SceneEntity>,
) -> (
    nw_model::CryParticlePlacement,
    Option<nw_model::CryParticlePlacementIssue>,
    ParticleBinding,
) {
    let Some(entity_transform) = entity_transform else {
        return (
            nw_model::CryParticlePlacement::Entity {
                transform: Default::default(),
            },
            None,
            ParticleBinding::MissingEntityTransform,
        );
    };
    let Some(attachment) =
        attachment.filter(|attachment| attachment.configuration.attached_initially)
    else {
        return (
            nw_model::CryParticlePlacement::Entity {
                transform: entity_transform,
            },
            None,
            ParticleBinding::Bound,
        );
    };

    let configuration = &attachment.configuration;
    let target_id = configuration.target_id;
    let target_transform = (target_id != INVALID_ENTITY_ID)
        .then(|| relative_entity_transform(target_id, owner_entity_id, entities))
        .flatten();
    let Some(target_transform) = target_transform else {
        return (
            nw_model::CryParticlePlacement::Entity {
                transform: entity_transform,
            },
            Some(nw_model::CryParticlePlacementIssue::UnresolvedAttachmentTarget),
            ParticleBinding::Bound,
        );
    };

    let target_bone = configuration.target_bone_name.trim();
    if !target_bone.is_empty() {
        let Some((skeleton_index, bone_index)) =
            model
                .skeletons
                .iter()
                .enumerate()
                .find_map(|(skeleton_index, skeleton)| {
                    skeleton
                        .bone_index(target_bone)
                        .map(|bone_index| (skeleton_index, bone_index))
                })
        else {
            return (
                nw_model::CryParticlePlacement::Entity {
                    transform: entity_transform,
                },
                None,
                ParticleBinding::MissingTargetBone,
            );
        };
        return (
            nw_model::CryParticlePlacement::Bone {
                target_entity_id: target_id.into(),
                skeleton_index,
                bone_name: model.skeletons[skeleton_index].bones[bone_index]
                    .name
                    .clone(),
                transform: particle_transform(
                    configuration.target_offset.translation.to_array(),
                    configuration.target_offset.rotation.to_array(),
                    configuration.target_offset.scale.to_array(),
                ),
            },
            None,
            ParticleBinding::Bound,
        );
    }

    let target_offset = particle_transform(
        configuration.target_offset.translation.to_array(),
        configuration.target_offset.rotation.to_array(),
        configuration.target_offset.scale.to_array(),
    );
    (
        nw_model::CryParticlePlacement::TargetEntity {
            target_entity_id: target_id.into(),
            transform: particle_transform_from_matrix(
                particle_transform_matrix(target_transform)
                    * particle_transform_matrix(target_offset),
            ),
        },
        None,
        ParticleBinding::Bound,
    )
}

pub(super) fn model_owner_entity(
    start_entity_id: u64,
    model_source_path: &str,
    entities: &BTreeMap<u64, SceneEntity>,
) -> Option<u64> {
    if start_entity_id == 0 {
        return None;
    }
    let mut current = start_entity_id;
    let mut visited = BTreeSet::new();
    while visited.insert(current) {
        let entity = entities.get(&current)?;
        if entity
            .character_definitions
            .iter()
            .any(|path| path.eq_ignore_ascii_case(model_source_path))
        {
            return Some(current);
        }
        if entity.parent_id == 0 {
            return None;
        }
        current = entity.parent_id;
    }
    None
}

pub(super) fn relative_entity_transform(
    entity_id: u64,
    owner_entity_id: u64,
    entities: &BTreeMap<u64, SceneEntity>,
) -> Option<nw_model::CryParticleTransform> {
    let mut current = entity_id;
    let mut transform = Mat4::IDENTITY;
    let mut visited = BTreeSet::new();
    while current != owner_entity_id && visited.insert(current) {
        let entity = entities.get(&current)?;
        transform = particle_transform_matrix(entity.local_transform) * transform;
        current = entity.parent_id;
    }
    (current == owner_entity_id).then(|| particle_transform_from_matrix(transform))
}

const fn particle_transform(
    translation: [f32; 3],
    rotation: [f32; 4],
    scale: [f32; 3],
) -> nw_model::CryParticleTransform {
    nw_model::CryParticleTransform {
        translation,
        rotation,
        scale,
    }
}

fn particle_transform_matrix(transform: nw_model::CryParticleTransform) -> Mat4 {
    Mat4::from_scale_rotation_translation(
        Vec3::from_array(transform.scale),
        Quat::from_array(transform.rotation),
        Vec3::from_array(transform.translation),
    )
}

fn particle_transform_from_matrix(transform: Mat4) -> nw_model::CryParticleTransform {
    let (scale, rotation, translation) = transform.to_scale_rotation_translation();
    nw_model::CryParticleTransform {
        translation: translation.to_array(),
        rotation: rotation.to_array(),
        scale: scale.to_array(),
    }
}

fn push_unique_path(paths: &mut Vec<String>, path: String) {
    if !paths
        .iter()
        .any(|existing| existing.eq_ignore_ascii_case(&path))
    {
        paths.push(path);
    }
}
