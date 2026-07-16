use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::ContractBuySellFeeData;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct ContractConfigData {
    #[serde(rename = "Base Number of Buy/Sell Contracts", default)]
    pub base_number_of_buy_sell_contracts: u32,
    #[serde(rename = "Base Number of Other Contracts", default)]
    pub base_number_of_other_contracts: u32,
    #[serde(rename = "Buy Contract Duration Fee Map", default)]
    pub buy_contract_duration_fee_map: std::collections::BTreeMap<i32, ContractBuySellFeeData>,
    #[serde(rename = "Sell Contract Duration Fee Map", default)]
    pub sell_contract_duration_fee_map: std::collections::BTreeMap<i32, ContractBuySellFeeData>,
    #[serde(rename = "Default Contract Duration Days", default)]
    pub default_contract_duration_days: i32,
    #[serde(rename = "Buy Contract Transaction Tax", default)]
    pub buy_contract_transaction_tax: f32,
    #[serde(rename = "Sell Contract Transaction Tax", default)]
    pub sell_contract_transaction_tax: f32,
}

impl AzRtti for ContractConfigData {
    const NAME: &'static str = "ContractConfigData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x8FBA1347_A061_43C0_8950_2DD9A1E15B34);
}
