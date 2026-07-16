use crate::az::asset::AssetId as AzAssetId;
use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
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
#[reflect(Serialize, Deserialize)]
pub struct SliceData {
    #[serde(rename = "SliceAssetId", default)]
    pub slice_asset_id: AzAssetId,
    #[serde(rename = "SlicePath", default)]
    pub slice_path: String,
}

impl AzRtti for SliceData {
    const NAME: &'static str = "SliceData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x6568A55A_5F18_43FE_8AEF_060FFCA6E90B);
}
