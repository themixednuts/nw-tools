use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::MaterialProperties;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct MaterialEntry {
    #[serde(rename = "Configuration", default)]
    pub configuration: MaterialProperties,
}

impl AzRtti for MaterialEntry {
    const NAME: &'static str = "MaterialEntry";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xC5207CC2_EF1B_4A11_BC8F_F1898282FBE5);
}
