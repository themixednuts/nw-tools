use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

pub mod attachment_component;
pub mod audio_area_environment_component;
pub mod audio_environment_component;
pub mod audio_listener_component;
pub mod audio_override_component;
pub mod audio_preload_component;
pub mod audio_rtpc_component;
pub mod audio_shape_component;
pub mod audio_spline_component;
pub mod audio_switch_component;
pub mod audio_trigger_component;
pub mod box_shape_component;
pub mod capsule_shape_component;
pub mod character_animation_manager_component;
pub mod character_controller_component;
pub mod character_physics_component;
pub mod faceted_components;
pub mod mannequin_component;
pub mod mannequin_scope_component;
pub mod mesh_collider_component;
pub mod motion_parameter_smoothing_settings;
pub mod physics_components;
pub mod physics_system_component;
pub mod primitive_collider_component;
pub mod rigid_body_component;
pub mod simple_animation_component;
pub mod skinned_mesh_component;
pub mod sphere_shape_component;
pub mod transform_component;

pub use self::attachment_component::AttachmentConfiguration;
pub use self::audio_area_environment_component::AudioAreaEnvironmentComponent;
pub use self::audio_environment_component::AudioEnvironmentComponent;
pub use self::audio_listener_component::AudioListenerComponent;
pub use self::audio_override_component::AudioOverrideComponent;
pub use self::audio_preload_component::AudioPreloadComponent;
pub use self::audio_rtpc_component::AudioRtpcComponent;
pub use self::audio_shape_component::AudioShapeComponent;
pub use self::audio_spline_component::AudioSplineComponent;
pub use self::audio_switch_component::AudioSwitchComponent;
pub use self::audio_trigger_component::AudioTriggerComponent;
pub use self::box_shape_component::{BoxShapeComponent, BoxShapeConfig};
pub use self::capsule_shape_component::{CapsuleShapeComponent, CapsuleShapeConfig};
pub use self::character_animation_manager_component::CharacterAnimationManagerComponent;
pub use self::character_controller_component::{
    CharacterControllerComponent, CharacterControllerConfig,
};

pub use self::character_physics_component::{
    CharacterPhysicsComponent, CryPlayerPhysicsConfiguration, PlayerDimensions, PlayerDynamics,
};

pub use self::faceted_components::{
    AudioSetTriggerOverrideComponent, AudioSetTriggerOverrideComponentClientFacet,
    AudioSetTriggerOverrideComponentServerFacet, ClientFacet, Facet, FacetedComponent,
    GameRigidBodyComponent, GameRigidBodyComponentClientFacet, GameRigidBodyComponentServerFacet,
    GameRigidBodyServerFacetConfig, GameTransformComponent, GameTransformComponentClientFacet,
    GameTransformComponentServerFacet, HitVolumeComponent, HitVolumeComponentClientFacet,
    HitVolumeComponentServerFacet, MaterialOverrideInfo, ServerFacet, TriggerOverridePair,
};

pub use self::mannequin_component::MannequinComponent;
pub use self::mannequin_scope_component::MannequinScopeComponent;
pub use self::mesh_collider_component::MeshColliderComponent;
pub use self::motion_parameter_smoothing_settings::MotionParameterSmoothingSettings;
pub use self::physics_components::{
    PhysicsComponent, RigidPhysicsComponent, RigidPhysicsConfig, StaticPhysicsComponent,
    StaticPhysicsConfig,
};

pub use self::physics_system_component::PhysicsSystemComponent;
pub use self::primitive_collider_component::{PrimitiveColliderComponent, PrimitiveColliderConfig};

pub use self::rigid_body_component::{RigidBodyComponent, RigidBodyConfiguration};
pub use self::simple_animation_component::{AnimatedLayer, SimpleAnimationComponent};
pub use self::skinned_mesh_component::{
    SkinnedMeshComponent, SkinnedMeshComponentRenderNode, SkinnedRenderOptions,
};

pub use self::sphere_shape_component::{SphereShapeComponent, SphereShapeConfig};
pub use self::transform_component::TransformComponent;

#[derive(
    Debug,
    Default,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct NetBindable {
    #[serde(rename = "m_isNetSyncEnabled", default)]
    pub is_net_sync_enabled: bool,
}

impl AzRtti for NetBindable {
    const NAME: &'static str = "NetBindable";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x80206665_D429_4703_B42E_94434F82F381);
}
