//! Authored Cry particle-emitter attachments retained as structured glTF metadata.

use std::fmt;

use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A Lumberyard entity identifier serialized as a decimal string so values above
/// JavaScript's 53-bit integer range remain lossless in glTF JSON.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CryEntityId(pub u64);

impl CryEntityId {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl From<u64> for CryEntityId {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl Serialize for CryEntityId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for CryEntityId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct EntityIdVisitor;

        impl Visitor<'_> for EntityIdVisitor {
            type Value = CryEntityId;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a decimal entity-id string or legacy unsigned integer")
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
                Ok(CryEntityId(value))
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                u64::try_from(value)
                    .map(CryEntityId)
                    .map_err(|_| E::custom("entity id cannot be negative"))
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                value.parse::<u64>().map(CryEntityId).map_err(E::custom)
            }
        }

        deserializer.deserialize_any(EntityIdVisitor)
    }
}

/// One `ParticleComponent` attached to an entity or skeleton joint in a scene slice.
///
/// glTF has no particle-system primitive, so exporters materialize this as an empty
/// node and preserve the authored emitter/library settings in node extras.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CryParticleEmitter {
    pub selected_emitter: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub particle_library_asset_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub particle_library_path: Option<String>,
    pub visible: bool,
    pub enabled: bool,
    pub attach_to_mesh: bool,
    pub load_emitter_on_activate: bool,
    pub color: [f32; 4],
    /// Authored `ParticleEmitterSettings::Target Entity` (+0x30).
    pub particle_target_entity_id: CryEntityId,
    /// Authored `GPU Edge Dissolve Target Entity` (+0x38). NewWorld 3-26 uses
    /// this entity as the mesh provider when `attach_to_mesh` is enabled.
    pub gpu_edge_dissolve_target_entity_id: CryEntityId,
    /// Particle entity placement relative to the character entity that owns it.
    pub entity_transform: CryParticleTransform,
    pub entity_parent_id: CryEntityId,
    /// Placement resolved by the scene importer. glTF serialization consumes this
    /// directly instead of reinterpreting attachment policy or choosing a skeleton.
    pub placement: CryParticlePlacement,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placement_issue: Option<CryParticlePlacementIssue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachment: Option<CryParticleAttachment>,
    /// Canonical converted ObjectStream payload retained for tools that need fields
    /// beyond the convenience projection above.
    pub authored_payload: CryParticleAuthoredPayload,
    /// SHA-1-derived canonical AZ data UUID over the lossless authored particle,
    /// mesh-layer, load-on-activate, and attachment settings used for equivalence.
    pub authored_settings_fingerprint: String,
    pub context: CryParticleEmitterContext,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum CryParticlePlacement {
    Entity {
        transform: CryParticleTransform,
    },
    TargetEntity {
        target_entity_id: CryEntityId,
        transform: CryParticleTransform,
    },
    Bone {
        target_entity_id: CryEntityId,
        skeleton_index: usize,
        bone_name: String,
        transform: CryParticleTransform,
    },
}

impl Default for CryParticlePlacement {
    fn default() -> Self {
        Self::Entity {
            transform: CryParticleTransform::default(),
        }
    }
}

impl CryParticlePlacement {
    #[must_use]
    pub const fn transform(&self) -> CryParticleTransform {
        match self {
            Self::Entity { transform }
            | Self::TargetEntity { transform, .. }
            | Self::Bone { transform, .. } => *transform,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CryParticlePlacementIssue {
    UnresolvedAttachmentTarget,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CryParticleAuthoredPayload {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub component_version: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings_version: Option<u8>,
    pub particle_component: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachment_component_version: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachment_configuration_version: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachment_component: Option<serde_json::Value>,
}

/// Explicit `AttachmentComponent` configuration co-owned by the particle entity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CryParticleAttachment {
    pub target_entity_id: CryEntityId,
    pub target_bone_name: String,
    pub target_offset: CryParticleTransform,
    pub attached_initially: bool,
    pub scale_source: u8,
    pub update_tolerance: f32,
}

/// Bone-local or entity-relative transform retained from the scene slice.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CryParticleTransform {
    pub translation: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
}

impl Default for CryParticleTransform {
    fn default() -> Self {
        Self {
            translation: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0; 3],
        }
    }
}

/// Provenance for a particle component decoded from a consumer scene slice.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CryParticleEmitterContext {
    pub source_path: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub alternate_source_paths: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity_id: Option<CryEntityId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity_name: Option<String>,
}

/// A character-owned particle whose authored non-empty target bone does not exist
/// on the exported primary skeleton. It is diagnosed but receives no glTF node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CryUnboundParticleEmitter {
    pub emitter: CryParticleEmitter,
    pub reason: CryParticleUnboundReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CryParticleUnboundReason {
    MissingTargetBone,
    MissingEntityTransform,
}

#[cfg(test)]
mod tests {
    use glam::Mat4;

    use crate::{Bone, CryAssetExtras, Gltf, Model, Skeleton};

    use super::*;

    #[test]
    fn entity_ids_serialize_losslessly_and_accept_legacy_numbers() {
        let id = CryEntityId(11_547_519_008_188_624_080);
        assert_eq!(
            serde_json::to_string(&id).unwrap(),
            r#""11547519008188624080""#
        );
        assert_eq!(
            serde_json::from_str::<CryEntityId>("17").unwrap(),
            CryEntityId(17)
        );
        assert_eq!(
            serde_json::from_str::<CryEntityId>(r#""11547519008188624080""#).unwrap(),
            id
        );
    }

    #[test]
    fn gltf_emits_particle_once_and_parents_initial_attachment_to_bone() {
        let model = model_with_bone();
        let emitter = emitter(true, "bind_right_wingC_05");
        let extras = CryAssetExtras {
            particle_emitters: vec![emitter],
            ..Default::default()
        };
        let document = gltf_json(&model, &extras);
        let nodes = document["nodes"].as_array().unwrap();
        let bone = node_index(nodes, "bind_right_wingC_05");
        let particle = node_index(nodes, "VFX_Wing_Right03");

        assert!(document["extras"].get("particleEmitters").is_none());
        assert!(nodes[particle].get("mesh").is_none());
        assert_eq!(
            nodes[particle]["extras"]["particleEmitter"]["selectedEmitter"],
            "cFX_npc_Isabella_Phase2.Wing_Idle01"
        );
        assert_eq!(
            nodes[particle]["extras"]["particleEmitter"]["attachment"]["targetEntityId"],
            "42"
        );
        assert!(
            nodes[bone]["children"]
                .as_array()
                .unwrap()
                .iter()
                .any(|child| child.as_u64() == Some(particle as u64))
        );
    }

    #[test]
    fn unattached_initial_state_uses_entity_transform_not_bone_parent() {
        let model = model_with_bone();
        let emitter = emitter(false, "bind_right_wingC_05");
        let extras = CryAssetExtras {
            particle_emitters: vec![emitter],
            ..Default::default()
        };
        let document = gltf_json(&model, &extras);
        let nodes = document["nodes"].as_array().unwrap();
        let bone = node_index(nodes, "bind_right_wingC_05");
        let particle = node_index(nodes, "VFX_Wing_Right03");

        assert_eq!(
            nodes[particle]["translation"],
            serde_json::json!([-4.0, 6.0, 5.0])
        );
        assert!(
            !nodes[bone]
                .get("children")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|children| children
                    .iter()
                    .any(|child| child.as_u64() == Some(particle as u64)))
        );
    }

    #[test]
    fn missing_attachment_uses_entity_transform() {
        let model = model_with_bone();
        let mut emitter = emitter(true, "bind_right_wingC_05");
        emitter.attachment = None;
        emitter.placement = CryParticlePlacement::Entity {
            transform: emitter.entity_transform,
        };
        let extras = CryAssetExtras {
            particle_emitters: vec![emitter],
            ..Default::default()
        };
        let document = gltf_json(&model, &extras);
        let nodes = document["nodes"].as_array().unwrap();
        let particle = node_index(nodes, "VFX_Wing_Right03");

        assert_eq!(
            nodes[particle]["translation"],
            serde_json::json!([-4.0, 6.0, 5.0])
        );
    }

    #[test]
    fn unresolved_non_empty_bone_never_becomes_a_root_node() {
        let model = model_with_bone();
        let extras = CryAssetExtras {
            particle_emitters: vec![emitter(true, "bind_missing_wing")],
            ..Default::default()
        };
        let document = gltf_json(&model, &extras);
        assert!(
            document["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .all(|node| node["name"] != "VFX_Wing_Right03")
        );
    }

    #[test]
    fn gltf_uses_resolved_secondary_skeleton_index() {
        let mut model = model_with_bone();
        model.skeletons.push(Skeleton {
            bones: vec![Bone {
                name: "nested_wing".to_owned(),
                controller_id: 2,
                parent: None,
                local: Mat4::IDENTITY,
                inverse_bind: Mat4::IDENTITY,
            }],
            placement: None,
        });
        let mut emitter = emitter(true, "nested_wing");
        emitter.placement = CryParticlePlacement::Bone {
            target_entity_id: CryEntityId(42),
            skeleton_index: 1,
            bone_name: "nested_wing".to_owned(),
            transform: CryParticleTransform::default(),
        };
        let document = gltf_json(
            &model,
            &CryAssetExtras {
                particle_emitters: vec![emitter],
                ..Default::default()
            },
        );
        let nodes = document["nodes"].as_array().unwrap();
        let secondary_bone = node_index(nodes, "nested_wing");
        let particle = node_index(nodes, "VFX_Wing_Right03");

        assert!(
            nodes[secondary_bone]["children"]
                .as_array()
                .unwrap()
                .iter()
                .any(|child| child.as_u64() == Some(particle as u64))
        );
    }

    fn model_with_bone() -> Model {
        Model {
            meshes: Vec::new(),
            skeletons: vec![Skeleton {
                bones: vec![Bone {
                    name: "bind_right_wingC_05".to_owned(),
                    controller_id: 1,
                    parent: None,
                    local: Mat4::IDENTITY,
                    inverse_bind: Mat4::IDENTITY,
                }],
                placement: None,
            }],
            auxiliary_nodes: Vec::new(),
        }
    }

    fn emitter(attached_initially: bool, target_bone_name: &str) -> CryParticleEmitter {
        CryParticleEmitter {
            selected_emitter: "cFX_npc_Isabella_Phase2.Wing_Idle01".to_owned(),
            particle_library_asset_id: Some("{1E1D1F12-486E-50A5-BD4E-4B1E20076939}:0".to_owned()),
            particle_library_path: Some("libs/particles/cfx_npc_isabella_phase2.xml".to_owned()),
            visible: true,
            enabled: true,
            attach_to_mesh: false,
            load_emitter_on_activate: true,
            color: [1.0; 4],
            particle_target_entity_id: CryEntityId(u64::from(u32::MAX)),
            gpu_edge_dissolve_target_entity_id: CryEntityId(u64::from(u32::MAX)),
            entity_transform: CryParticleTransform {
                translation: [4.0, 5.0, 6.0],
                ..Default::default()
            },
            entity_parent_id: CryEntityId(42),
            placement: if attached_initially {
                CryParticlePlacement::Bone {
                    target_entity_id: CryEntityId(42),
                    skeleton_index: 0,
                    bone_name: target_bone_name.to_owned(),
                    transform: CryParticleTransform {
                        translation: [1.0, 2.0, 3.0],
                        ..Default::default()
                    },
                }
            } else {
                CryParticlePlacement::Entity {
                    transform: CryParticleTransform {
                        translation: [4.0, 5.0, 6.0],
                        ..Default::default()
                    },
                }
            },
            placement_issue: None,
            attachment: Some(CryParticleAttachment {
                target_entity_id: CryEntityId(42),
                target_bone_name: target_bone_name.to_owned(),
                target_offset: CryParticleTransform {
                    translation: [1.0, 2.0, 3.0],
                    ..Default::default()
                },
                attached_initially,
                scale_source: 0,
                update_tolerance: 0.0,
            }),
            authored_payload: CryParticleAuthoredPayload::default(),
            authored_settings_fingerprint: "fingerprint".to_owned(),
            context: CryParticleEmitterContext {
                source_path: "slices/characters/isabella_lair_phase2.dynamicslice".to_owned(),
                entity_id: Some(CryEntityId(7)),
                entity_name: Some("VFX_Wing_Right03".to_owned()),
                ..Default::default()
            },
        }
    }

    fn gltf_json(model: &Model, extras: &CryAssetExtras) -> serde_json::Value {
        let package = Gltf::new(model).extras(extras).to_gltf_package();
        let uris = (0..package.resources().len())
            .map(|index| format!("resource_{index}.bin"))
            .collect::<Vec<_>>();
        serde_json::from_str(&package.into_json(&uris).unwrap()).unwrap()
    }

    fn node_index(nodes: &[serde_json::Value], name: &str) -> usize {
        nodes.iter().position(|node| node["name"] == name).unwrap()
    }
}
