use std::collections::{BTreeMap, BTreeSet};
use std::mem;

use super::*;

pub(super) fn particle_settings_fingerprint(
    component_source: &serde_json::Value,
    attachment_source: Option<&serde_json::Value>,
    entities: &BTreeMap<u64, SceneEntity>,
) -> Result<String> {
    let mut component = component_source.clone();
    if let Some(object) = component.as_object_mut() {
        // The component base carries only its per-instance component ID. All
        // behavior fields, including both particle-settings entity IDs,
        // MeshParticle layers, and load-on-activate remain lossless below.
        object.remove("BaseClass1");
        if let Some(settings) = object
            .get_mut("Particle")
            .and_then(serde_json::Value::as_object_mut)
        {
            normalize_entity_reference(settings, "Target Entity", entities);
            normalize_entity_reference(settings, "GPU Edge Dissolve Target Entity", entities);
        }
    }
    let mut attachment = attachment_source.cloned();
    if let Some(object) = attachment
        .as_mut()
        .and_then(serde_json::Value::as_object_mut)
    {
        if object
            .get("Target Bone Name")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|bone| !bone.trim().is_empty())
        {
            object.insert(
                "Target ID".to_owned(),
                serde_json::Value::String("resolvedBoneOwner".to_owned()),
            );
        } else {
            normalize_entity_reference(object, "Target ID", entities);
        }
    }
    let canonical = serde_json::json!({
        "particleComponent": component,
        "attachmentConfiguration": attachment,
    });
    let bytes = serde_json::to_vec(&canonical).context("serialize canonical particle settings")?;
    Ok(nw_reflected_types::az::uuid::Uuid::create_data(&bytes).to_string())
}

pub(super) fn stringify_particle_entity_ids(component: &mut serde_json::Value) {
    stringify_component_id(component);
    let Some(settings) = component
        .get_mut("Particle")
        .and_then(serde_json::Value::as_object_mut)
    else {
        return;
    };
    stringify_entity_reference_in_object(settings, "Target Entity");
    stringify_entity_reference_in_object(settings, "GPU Edge Dissolve Target Entity");
}

pub(super) fn stringify_component_id(component: &mut serde_json::Value) {
    let Some(base) = component
        .get_mut("BaseClass1")
        .and_then(serde_json::Value::as_object_mut)
    else {
        return;
    };
    stringify_entity_reference_in_object(base, "Id");
}

pub(super) fn stringify_entity_reference(value: &mut serde_json::Value, field: &str) {
    if let Some(object) = value.as_object_mut() {
        stringify_entity_reference_in_object(object, field);
    }
}

fn stringify_entity_reference_in_object(
    object: &mut serde_json::Map<String, serde_json::Value>,
    field: &str,
) {
    let Some(value) = object.get_mut(field) else {
        return;
    };
    if let Some(entity_id) = value.as_u64() {
        *value = serde_json::Value::String(entity_id.to_string());
    }
}

fn normalize_entity_reference(
    object: &mut serde_json::Map<String, serde_json::Value>,
    field: &str,
    entities: &BTreeMap<u64, SceneEntity>,
) {
    let Some(value) = object.get_mut(field) else {
        return;
    };
    let Some(entity_id) = value
        .as_u64()
        .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
    else {
        return;
    };
    *value = stable_entity_reference(entity_id, entities);
}

fn stable_entity_reference(
    entity_id: u64,
    entities: &BTreeMap<u64, SceneEntity>,
) -> serde_json::Value {
    if !entities.contains_key(&entity_id) {
        return serde_json::json!({ "unresolved": entity_id.to_string() });
    }
    let mut current = entity_id;
    let mut visited = BTreeSet::new();
    let mut chain = Vec::new();
    while visited.insert(current) {
        let Some(entity) = entities.get(&current) else {
            break;
        };
        chain.push(serde_json::json!({
            "name": entity.name,
            "localTransform": entity.local_transform,
            "characterDefinitions": entity.character_definitions,
        }));
        if entity.parent_id == 0 {
            break;
        }
        current = entity.parent_id;
    }
    serde_json::json!({ "entityPath": chain })
}

pub(super) fn collapse_scene_particle_variants(emitters: &mut Vec<SceneParticleEmitter>) {
    if emitters.is_empty() {
        return;
    }
    let mut slices = BTreeMap::<String, SliceParticles>::new();
    for emitter in mem::take(emitters) {
        let source_path = &emitter.emitter.context.source_path;
        slice_for_source(&mut slices, source_path)
            .emitters
            .push(emitter);
    }
    let mut variants = BTreeMap::<Vec<ParticleSignature>, Vec<SliceParticles>>::new();
    for slice in slices.into_values() {
        variants.entry(slice.signature()).or_default().push(slice);
    }
    let mut kept = variants
        .into_values()
        .map(collapse_variant_slices)
        .collect::<Vec<_>>();
    kept.sort_by(|left, right| compare_source_paths(&left.source_path, &right.source_path));
    for slice in &mut kept {
        slice
            .emitters
            .sort_by_key(ParticleOrderKey::from_scene_emitter);
    }
    *emitters = kept.into_iter().flat_map(|slice| slice.emitters).collect();
}

fn source_key(path: &str) -> String {
    normalize_path(path).to_ascii_lowercase()
}

fn compare_source_paths(left: &str, right: &str) -> std::cmp::Ordering {
    source_key(left)
        .cmp(&source_key(right))
        .then_with(|| normalize_path(left).cmp(&normalize_path(right)))
}

#[derive(Default)]
struct SliceParticles {
    source_path: String,
    emitters: Vec<SceneParticleEmitter>,
}

impl SliceParticles {
    fn signature(&self) -> Vec<ParticleSignature> {
        let mut signature = self
            .emitters
            .iter()
            .map(ParticleSignature::from_scene_emitter)
            .collect::<Vec<_>>();
        signature.sort();
        signature
    }
}

fn slice_for_source<'a>(
    slices: &'a mut BTreeMap<String, SliceParticles>,
    source_path: &str,
) -> &'a mut SliceParticles {
    let source_path = normalize_path(source_path);
    let key = source_key(&source_path);
    let slice = slices.entry(key).or_insert_with(|| SliceParticles {
        source_path: source_path.clone(),
        ..Default::default()
    });
    if compare_source_paths(&source_path, &slice.source_path).is_lt() {
        slice.source_path = source_path;
    }
    slice
}

fn collapse_variant_slices(mut variants: Vec<SliceParticles>) -> SliceParticles {
    variants.sort_by(|left, right| compare_source_paths(&left.source_path, &right.source_path));
    let mut kept = variants.remove(0);
    let alternate_source_paths = variants
        .into_iter()
        .map(|variant| variant.source_path)
        .collect::<Vec<_>>();
    for emitter in &mut kept.emitters {
        emitter
            .emitter
            .context
            .source_path
            .clone_from(&kept.source_path);
        emitter
            .emitter
            .context
            .alternate_source_paths
            .clone_from(&alternate_source_paths);
    }
    kept
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct ParticleSignature {
    authored_settings_fingerprint: String,
    placement: ParticlePlacementSignature,
    binding: ParticleBinding,
}

impl ParticleSignature {
    fn from_scene_emitter(emitter: &SceneParticleEmitter) -> Self {
        Self {
            authored_settings_fingerprint: emitter.emitter.authored_settings_fingerprint.clone(),
            placement: (&emitter.emitter.placement).into(),
            binding: emitter.binding,
        }
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum ParticlePlacementSignature {
    Entity(ParticleTransformSignature),
    TargetEntity(ParticleTransformSignature),
    Bone {
        skeleton_index: usize,
        bone_name: String,
        transform: ParticleTransformSignature,
    },
}

impl From<&nw_model::CryParticlePlacement> for ParticlePlacementSignature {
    fn from(placement: &nw_model::CryParticlePlacement) -> Self {
        match placement {
            nw_model::CryParticlePlacement::Entity { transform } => {
                Self::Entity((*transform).into())
            }
            nw_model::CryParticlePlacement::TargetEntity { transform, .. } => {
                Self::TargetEntity((*transform).into())
            }
            nw_model::CryParticlePlacement::Bone {
                skeleton_index,
                bone_name,
                transform,
                ..
            } => Self::Bone {
                skeleton_index: *skeleton_index,
                bone_name: bone_name.to_ascii_lowercase(),
                transform: (*transform).into(),
            },
        }
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct ParticleTransformSignature {
    translation: [u32; 3],
    rotation: [u32; 4],
    scale: [u32; 3],
}

impl From<nw_model::CryParticleTransform> for ParticleTransformSignature {
    fn from(transform: nw_model::CryParticleTransform) -> Self {
        Self {
            translation: transform.translation.map(f32::to_bits),
            rotation: transform.rotation.map(f32::to_bits),
            scale: transform.scale.map(f32::to_bits),
        }
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct ParticleOrderKey {
    signature: ParticleSignature,
    entity_id: Option<nw_model::CryEntityId>,
    entity_name: Option<String>,
}

impl ParticleOrderKey {
    fn from_scene_emitter(emitter: &SceneParticleEmitter) -> Self {
        Self {
            signature: ParticleSignature::from_scene_emitter(emitter),
            entity_id: emitter.emitter.context.entity_id,
            entity_name: emitter.emitter.context.entity_name.clone(),
        }
    }
}
