use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::Component;
use bevy_ecs::reflect::ReflectComponent;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

pub mod skinned_mesh_component_render_node;
pub mod skinned_render_options;

pub use self::skinned_mesh_component_render_node::SkinnedMeshComponentRenderNode;
pub use self::skinned_render_options::SkinnedRenderOptions;

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
pub struct SkinnedMeshComponent {
    #[serde(rename = "BaseClass1", default)]
    pub az_component: Component,
    #[serde(rename = "Skinned Mesh Render Node", default)]
    pub skinned_mesh_render_node: SkinnedMeshComponentRenderNode,
    #[serde(rename = "Load Mesh On Activate", default)]
    pub load_mesh_on_activate: bool,
}

impl AzRtti for SkinnedMeshComponent {
    const NAME: &'static str = "SkinnedMeshComponent";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xC99EB110_CA74_4D95_83F0_2FCDD1FF418B);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0xEDFCB2CF_F75D_43BE_B26B_F35821B29247)];
}
