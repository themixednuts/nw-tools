use crate::az::asset::AssetId as AzAssetId;
use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::Component;
use bevy_ecs::reflect::ReflectComponent;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

pub mod particle_emit_bone_layer;
pub mod particle_emitter_settings;

pub use self::particle_emit_bone_layer::ParticleEmitBoneLayer;
pub use self::particle_emitter_settings::ParticleEmitterSettings;

#[derive(
    bevy_ecs::component::Component,
    Debug,
    Default,
    Clone,
    PartialEq,
    serde::Serialize,
    serde::Deserialize,
    bevy_reflect::Reflect,
)]
#[reflect(Component, Serialize, Deserialize)]
pub struct ParticleComponent {
    #[serde(rename = "BaseClass1", default)]
    pub az_component: Component,
    #[serde(rename = "Particle", default)]
    pub particle: ParticleEmitterSettings,
    #[serde(rename = "ParticleLibraryAssetId", default)]
    pub particle_library_asset_id: AzAssetId,
    #[serde(rename = "MeshParticle", default)]
    pub mesh_particle: Vec<ParticleEmitBoneLayer>,
    #[serde(rename = "Load Emitter On Activate", default)]
    pub load_emitter_on_activate: bool,
}

impl AzRtti for ParticleComponent {
    const NAME: &'static str = "ParticleComponent";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x65BC817A_ABF6_440F_AD4F_581C40F92795);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0xEDFCB2CF_F75D_43BE_B26B_F35821B29247)];
}
