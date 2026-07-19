use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::Component;
use bevy_ecs::reflect::ReflectComponent;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

pub mod attachment_configuration;

pub use self::attachment_configuration::AttachmentConfiguration;

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
pub struct AttachmentComponent {
    #[serde(rename = "BaseClass1", default)]
    pub az_component: Component,
    #[serde(rename = "Configuration", default)]
    pub configuration: AttachmentConfiguration,
}

impl AzRtti for AttachmentComponent {
    const NAME: &'static str = "AttachmentComponent";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x2D17A64A_7AC5_4C02_AC36_C5E8141FFDDF);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0xEDFCB2CF_F75D_43BE_B26B_F35821B29247)];
}
