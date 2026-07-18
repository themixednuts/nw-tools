use crate::az::asset::Asset as AzAsset;
use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::{DEPRECATEDCollisionType, QueryShape};
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct GameRigidBodyConfig {
    #[serde(rename = "m_center", default)]
    pub center: bevy_math::Vec3,
    #[serde(rename = "m_collisionType", default)]
    pub collision_type: DEPRECATEDCollisionType,
    #[serde(rename = "m_rnrAsset", default)]
    pub rnr_asset: AzAsset,
    #[serde(rename = "m_materialOverrideAsset", default)]
    pub material_override_asset: AzAsset,
    #[serde(rename = "m_collisionShape", default)]
    pub collision_shape: Option<QueryShape>,
    #[serde(rename = "m_overrideCollisionShapeMaterial", default)]
    pub override_collision_shape_material: bool,
    #[serde(rename = "m_overrideCollisionShapeMaterialName", default)]
    pub override_collision_shape_material_name: String,
    #[serde(rename = "m_shapeEntity", default)]
    pub shape_entity: u64,
    #[serde(rename = "m_isDynamic", default)]
    pub is_dynamic: bool,
    #[serde(rename = "m_mass", default)]
    pub mass: f32,
    #[serde(rename = "m_linearDamping", default)]
    pub linear_damping: f32,
    #[serde(rename = "m_angularDamping", default)]
    pub angular_damping: f32,
    #[serde(rename = "m_sleepMinEnergy", default)]
    pub sleep_min_energy: f32,
    #[serde(rename = "m_interactWithTriggers", default)]
    pub interact_with_triggers: bool,
    #[serde(rename = "m_strFilter", default)]
    pub str_filter: String,
    #[serde(rename = "m_gameplayFlags", default)]
    pub gameplay_flags: Vec<i32>,
    #[serde(rename = "m_scaleShapes", default)]
    pub scale_shapes: bool,
    #[serde(rename = "m_applyAlignmentDetails", default)]
    pub apply_alignment_details: bool,
}

impl AzRtti for GameRigidBodyConfig {
    const NAME: &'static str = "GameRigidBodyConfig";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x0BCAB3AC_6ABB_4462_9253_44565E6BD8D8);
}
