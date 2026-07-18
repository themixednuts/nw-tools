use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::{Component, NetBindable};
use bevy_ecs::reflect::ReflectComponent;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    bevy_ecs::component::Component,
    Debug,
    Default,
    Clone,
    Copy,
    PartialEq,
    serde::Serialize,
    serde::Deserialize,
    bevy_reflect::Reflect,
)]
#[reflect(Component, Serialize, Deserialize)]
pub struct TransformComponent {
    #[serde(rename = "BaseClass1", default)]
    pub az_component: Component,
    #[serde(rename = "BaseClass2", default)]
    pub net_bindable: NetBindable,
    #[serde(rename = "Parent", default)]
    pub parent: u64,
    #[serde(rename = "Transform", default)]
    pub transform: bevy_transform::components::Transform,
    #[serde(rename = "LocalTransform", default)]
    pub local_transform: bevy_transform::components::Transform,
    #[serde(rename = "ParentActivationTransformMode", default)]
    pub parent_activation_transform_mode: u32,
    #[serde(rename = "IsStatic", default)]
    pub is_static: bool,
    #[serde(rename = "InterpolatePosition", default)]
    pub interpolate_position: u32,
    #[serde(rename = "InterpolateRotation", default)]
    pub interpolate_rotation: u32,
}

impl AzRtti for TransformComponent {
    const NAME: &'static str = "TransformComponent";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x22B10178_39B6_4C12_BB37_77DB45FDD3B6);
    const BASE_TYPE_IDS: &'static [AzUuid] = &[
        AzUuid::from_u128(0xEDFCB2CF_F75D_43BE_B26B_F35821B29247),
        AzUuid::from_u128(0x80206665_D429_4703_B42E_94434F82F381),
    ];
}
