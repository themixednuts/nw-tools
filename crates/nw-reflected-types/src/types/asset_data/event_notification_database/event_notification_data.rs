use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::InteractionUIActions;
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
pub struct EventNotificationData {
    #[serde(rename = "Interaction UI Action", default)]
    pub interaction_ui_action: InteractionUIActions,
    #[serde(rename = "Notification Id", default)]
    pub notification_id: String,
}

impl AzRtti for EventNotificationData {
    const NAME: &'static str = "EventNotificationData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x2958D8DB_DCD6_487C_B70F_8781F48C6524);
}
