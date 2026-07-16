use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::Component;
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
pub struct AudioPreloadComponent {
    #[serde(rename = "BaseClass1", default)]
    pub az_component: Component,
    #[serde(rename = "Preload Name", default)]
    pub preload_name: String,
    #[serde(rename = "Load Type", default)]
    pub load_type: u32,
}

impl AzRtti for AudioPreloadComponent {
    const NAME: &'static str = "AudioPreloadComponent";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xCBBB1234_4DCA_427E_80FF_E2BB0866EEB1);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0xEDFCB2CF_F75D_43BE_B26B_F35821B29247)];
}
