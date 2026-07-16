use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::{Component, SimpleAssetReferenceMannequinAnimationDatabaseAsset};
use bevy_ecs::reflect::ReflectComponent;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    bevy_ecs::component::Component,
    Debug,
    Default,
    Clone,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    bevy_reflect::Reflect,
)]
#[reflect(Component, Serialize, Deserialize)]
pub struct MannequinScopeComponent {
    #[serde(rename = "BaseClass1", default)]
    pub az_component: Component,
    #[serde(rename = "Animation Database", default)]
    pub animation_database: SimpleAssetReferenceMannequinAnimationDatabaseAsset,
    #[serde(rename = "Context Name", default)]
    pub context_name: String,
    #[serde(rename = "Target Entity", default)]
    pub target_entity: u64,
}

impl AzRtti for MannequinScopeComponent {
    const NAME: &'static str = "MannequinScopeComponent";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xAB4FDB4A_D742_4EF8_B36E_9A1775FA6FA5);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0xEDFCB2CF_F75D_43BE_B26B_F35821B29247)];
}
