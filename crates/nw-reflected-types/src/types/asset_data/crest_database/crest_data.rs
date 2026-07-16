use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::{FactionType, SimpleAssetReferenceTextureAsset};
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
pub struct CrestData {
    #[serde(rename = "Name", default)]
    pub name: String,
    #[serde(rename = "EntitlementId", default)]
    pub entitlement_id: String,
    #[serde(rename = "Image", default)]
    pub image: SimpleAssetReferenceTextureAsset,
    #[serde(rename = "IsEntitlement", default)]
    pub is_entitlement: bool,
    #[serde(rename = "IsSelectable", default)]
    pub is_selectable: bool,
    #[serde(rename = "Faction", default)]
    pub faction: FactionType,
}

impl AzRtti for CrestData {
    const NAME: &'static str = "CrestData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x64AB17AB_0592_47E8_820D_81D89429A8D6);
}
