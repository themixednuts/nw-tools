use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::ItemType;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug,
    Default,
    Clone,
    Copy,
    PartialEq,
    serde::Serialize,
    serde::Deserialize,
    bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct RemoteStorageItemTypeMultiplierData {
    #[serde(rename = "Item Tier Base Fees", default)]
    pub item_tier_base_fees: ItemType,
    #[serde(rename = "Item Type Fee Multipliers", default)]
    pub item_type_fee_multipliers: f32,
}

impl AzRtti for RemoteStorageItemTypeMultiplierData {
    const NAME: &'static str = "RemoteStorageItemTypeMultiplierData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xA440FA71_1598_4ED4_BCDE_5E3DF1EBD81B);
}
