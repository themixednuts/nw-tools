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
pub struct GuaranteedItemTransferData {
    #[serde(rename = "ItemName", default)]
    pub item_name: String,
    #[serde(rename = "ItemQuantity", default)]
    pub item_quantity: u32,
}

impl AzRtti for GuaranteedItemTransferData {
    const NAME: &'static str = "GuaranteedItemTransferData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x6583011A_4493_4FC9_9407_6EA993257296);
}
