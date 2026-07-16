use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::SimpleAssetReferenceMaterialOverrideAsset;
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
pub struct MaterialOverrideInfo {
    #[serde(rename = "m_name", default)]
    pub name: String,
    #[serde(rename = "m_materialAsset", default)]
    pub material_asset: SimpleAssetReferenceMaterialOverrideAsset,
}

impl AzRtti for MaterialOverrideInfo {
    const NAME: &'static str = "MaterialOverrideInfo";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xCB911795_C640_45AB_9390_841D0E079266);
}
