use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug,
    Default,
    Clone,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct InputMapAsset {
    #[serde(rename = "NameMap", default)]
    pub name_map: Vec<String>,
    #[serde(rename = "InputMap", default)]
    pub input_map: std::collections::HashMap<u32, i32>,
}

impl AzRtti for InputMapAsset {
    const NAME: &'static str = "InputMapAsset";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xC6ECD1FF_7B27_40C7_8104_5E1775EB8D16);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0xAF3F7D32_1536_422A_89F3_A11E1F5B5A9C)];
}
