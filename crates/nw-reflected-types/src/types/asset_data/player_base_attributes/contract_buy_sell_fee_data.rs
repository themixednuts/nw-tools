use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
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
pub struct ContractBuySellFeeData {
    #[serde(rename = "Flat Fee", default)]
    pub flat_fee: u32,
    #[serde(rename = "Percentage Fee", default)]
    pub percentage_fee: f32,
}

impl AzRtti for ContractBuySellFeeData {
    const NAME: &'static str = "ContractBuySellFeeData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x06F4F968_0A4A_4FAC_AF23_D6B5B018069D);
}
