use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct ParticleEmitterSettings {
    #[serde(rename = "Visible", default)]
    pub visible: bool,
    #[serde(rename = "Enable", default)]
    pub enable: bool,
    #[serde(rename = "AttachToMesh", default)]
    pub attach_to_mesh: bool,
    #[serde(rename = "AttachToDissolvingEdge", default)]
    pub attach_to_dissolving_edge: bool,
    #[serde(rename = "SelectedEmitter", default)]
    pub selected_emitter: String,
    #[serde(rename = "Color", default)]
    pub color: bevy_color::LinearRgba,
    #[serde(rename = "Alpha Scale", default)]
    pub alpha_scale: f32,
    #[serde(rename = "Pre-roll", default)]
    pub pre_roll: bool,
    #[serde(rename = "Particle Count Scale", default)]
    pub particle_count_scale: f32,
    #[serde(rename = "Time Scale", default)]
    pub time_scale: f32,
    #[serde(rename = "Pulse Period", default)]
    pub pulse_period: f32,
    #[serde(rename = "GlobalSizeScale", default)]
    pub global_size_scale: f32,
    #[serde(rename = "ParticleSizeX", default)]
    pub particle_size_x: f32,
    #[serde(rename = "ParticleSizeY", default)]
    pub particle_size_y: f32,
    #[serde(rename = "ParticleSizeZ", default)]
    pub particle_size_z: f32,
    #[serde(rename = "ParticleSizeRandom", default)]
    pub particle_size_random: f32,
    #[serde(rename = "Speed Scale", default)]
    pub speed_scale: f32,
    #[serde(rename = "Strength", default)]
    pub strength: f32,
    #[serde(rename = "Ignore Rotation", default)]
    pub ignore_rotation: bool,
    #[serde(rename = "Not Attached", default)]
    pub not_attached: bool,
    #[serde(rename = "Register by Bounding Box", default)]
    pub register_by_bounding_box: bool,
    #[serde(rename = "Use LOD", default)]
    pub use_lod: bool,
    #[serde(rename = "Target Entity", default)]
    pub target_entity: u64,
    #[serde(rename = "GPU Edge Dissolve Target Entity", default)]
    pub gpu_edge_dissolve_target_entity: u64,
    #[serde(rename = "Enable Audio", default)]
    pub enable_audio: bool,
    #[serde(rename = "Audio RTPC", default)]
    pub audio_rtpc: String,
    #[serde(rename = "View Distance Multiplier", default)]
    pub view_distance_multiplier: f32,
    #[serde(rename = "Use VisArea", default)]
    pub use_vis_area: bool,
    #[serde(rename = "Accept Decals", default)]
    pub accept_decals: bool,
    #[serde(rename = "Accept Snow", default)]
    pub accept_snow: bool,
    #[serde(rename = "Accept Silhouette", default)]
    pub accept_silhouette: bool,
    #[serde(rename = "Render Always", default)]
    pub render_always: bool,
    #[serde(rename = "Kill On Deactivate", default)]
    pub kill_on_deactivate: bool,
    #[serde(rename = "Force Highest Contextual Priority", default)]
    pub force_highest_contextual_priority: bool,
}

impl AzRtti for ParticleEmitterSettings {
    const NAME: &'static str = "ParticleEmitterSettings";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xA1E34557_30DB_4716_B4CE_39D52A113D0C);
}
