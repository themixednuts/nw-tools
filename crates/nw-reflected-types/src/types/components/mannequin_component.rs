use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::{Component, SimpleAssetReferenceMannequinControllerDefinitionAsset};
use bevy_ecs::reflect::ReflectComponent;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    bevy_ecs::component::Component,
    Debug,
    Default,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    bevy_reflect::Reflect,
)]
#[reflect(Component, Serialize, Deserialize)]
pub struct MannequinComponent {
    #[serde(rename = "BaseClass1", default)]
    pub az_component: Component,
    #[serde(rename = "Controller Definition", default)]
    pub controller_definition: SimpleAssetReferenceMannequinControllerDefinitionAsset,
    #[serde(rename = "Initial Fragment", default)]
    pub initial_fragment: String,
}

impl AzRtti for MannequinComponent {
    const NAME: &'static str = "MannequinComponent";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x83E4AC4C_2184_49D1_AAD0_A0687EEE1405);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0xEDFCB2CF_F75D_43BE_B26B_F35821B29247)];
}
