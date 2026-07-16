use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::FactionType;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct CrestColorData {
    #[serde(rename = "Name", default)]
    pub name: String,
    #[serde(rename = "EntitlementId", default)]
    pub entitlement_id: String,
    #[serde(rename = "Color", default)]
    pub color: bevy_color::LinearRgba,
    #[serde(rename = "IsEntitlement", default)]
    pub is_entitlement: bool,
    #[serde(rename = "IsSelectable", default)]
    pub is_selectable: bool,
    #[serde(rename = "Faction", default)]
    pub faction: FactionType,
}

impl AzRtti for CrestColorData {
    const NAME: &'static str = "CrestColorData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xE13466E0_02EB_488D_9A83_1423E8490C30);
}
