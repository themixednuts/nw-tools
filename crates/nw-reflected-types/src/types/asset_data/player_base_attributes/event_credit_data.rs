use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::CreditModifierData;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct EventCreditData {
    #[serde(rename = "Credit Time Limit Seconds", default)]
    pub credit_time_limit_seconds: i32,
    #[serde(rename = "Credit Health Threshold Percentage", default)]
    pub credit_health_threshold_percentage: f32,
    #[serde(rename = "Credit Range Limit Meters", default)]
    pub credit_range_limit_meters: i32,
    #[serde(rename = "Contribution Type Multipliers", default)]
    pub contribution_type_multipliers: std::collections::HashMap<i32, f32>,
    #[serde(rename = "Event Credit Modifiers", default)]
    pub event_credit_modifiers: Vec<CreditModifierData>,
    #[serde(rename = "Group Credit Modifiers", default)]
    pub group_credit_modifiers: Vec<CreditModifierData>,
}

impl AzRtti for EventCreditData {
    const NAME: &'static str = "EventCreditData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xAA0275B2_9B50_467E_B746_061C92ED1891);
}
