//! Scene-slice `ParticleComponent` attachments retained as structured glTF metadata.

use super::*;

mod canonical;
mod resources;
mod scene;

use canonical::*;
use resources::*;
use scene::*;

const ENTITY_ID_FIELD_CRC: u32 = 0xbf39_6750;
const ENTITY_NAME_FIELD_CRC: u32 = 0x5e23_7e06;
const ENTITY_COMPONENTS_FIELD_CRC: u32 = 0xee48_f5fd;
const INVALID_ENTITY_ID: u64 = u32::MAX as u64;

struct DecodedParticleComponent {
    settings: nw_reflected_types::types::ParticleEmitterSettings,
    particle_library_asset_id: Option<nw_asset::AssetId>,
    load_emitter_on_activate: bool,
    component_version: Option<u8>,
    settings_version: Option<u8>,
    source: serde_json::Value,
}

struct DecodedAttachment {
    configuration: nw_reflected_types::types::AttachmentConfiguration,
    component_version: Option<u8>,
    configuration_version: Option<u8>,
    component_source: serde_json::Value,
    source: serde_json::Value,
}

struct SceneEntity {
    name: Option<String>,
    parent_id: u64,
    local_transform: nw_model::CryParticleTransform,
    character_definitions: Vec<String>,
}

struct SceneParticleEmitter {
    emitter: nw_model::CryParticleEmitter,
    particle_library_asset_id: Option<nw_asset::AssetId>,
    binding: ParticleBinding,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ParticleBinding {
    Bound,
    MissingTargetBone,
    MissingEntityTransform,
}

/// Decode, scope, and attach particle emitters from the same scene-slice resources
/// already retained by the dependency pass. Libraries resolve only through the
/// authored `ParticleLibraryAssetId`; `SelectedEmitter` naming is never guessed.
pub(super) fn attach_scene_particles(
    runner: &nw_jobs::JobRunner,
    source: &dyn AssetSource,
    model: &nw_model::Model,
    extras: &mut nw_model::CryAssetExtras,
    dependency_graph: &nw_asset_graph::AssetDependencyGraph,
) -> Result<()> {
    let model_source_path = normalize_path(&extras.source_path);
    let mut emitters = Vec::new();
    for resource in &extras.resource_payloads {
        if resource.kind != nw_model::CryEmbeddedResourceKind::LegacyObjectStreamScene {
            continue;
        }
        emitters.extend(parse_scene_particles(
            source,
            model,
            &model_source_path,
            &resource.source_path,
            &resource.bytes,
        )?);
    }
    collapse_scene_particle_variants(&mut emitters);
    resolve_particle_libraries(runner, source, extras, &mut emitters, dependency_graph)?;

    let mut bound = Vec::new();
    let mut unbound = Vec::new();
    for emitter in emitters {
        match emitter.binding {
            ParticleBinding::Bound => bound.push(emitter),
            ParticleBinding::MissingTargetBone => {
                unbound.push(nw_model::CryUnboundParticleEmitter {
                    emitter: emitter.emitter,
                    reason: nw_model::CryParticleUnboundReason::MissingTargetBone,
                });
            }
            ParticleBinding::MissingEntityTransform => {
                unbound.push(nw_model::CryUnboundParticleEmitter {
                    emitter: emitter.emitter,
                    reason: nw_model::CryParticleUnboundReason::MissingEntityTransform,
                });
            }
        }
    }
    extras.particle_emitters = bound.into_iter().map(|emitter| emitter.emitter).collect();
    extras.unbound_particle_emitters = unbound;
    Ok(())
}

#[cfg(test)]
#[path = "particles_tests.rs"]
mod tests;
