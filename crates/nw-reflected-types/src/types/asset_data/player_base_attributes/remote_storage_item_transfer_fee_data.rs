use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::RemoteStorageItemTypeMultiplierData;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct RemoteStorageItemTransferFeeData {
    #[serde(rename = "Item Tier Base Fees", default)]
    pub item_tier_base_fees: Vec<u32>,
    #[serde(rename = "Item Type Fee Multipliers", default)]
    pub item_type_fee_multipliers: Vec<RemoteStorageItemTypeMultiplierData>,
    #[serde(rename = "Distance Interval Meters", default)]
    pub distance_interval_meters: f32,
    #[serde(rename = "Fee Multiplier Per Interval", default)]
    pub fee_multiplier_per_interval: f32,
}

impl AzRtti for RemoteStorageItemTransferFeeData {
    const NAME: &'static str = "RemoteStorageItemTransferFeeData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x42E861DC_5038_490F_A16A_6FF0D226E3B2);
}
