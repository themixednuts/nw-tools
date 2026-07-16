use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::TaskInteractEntryData;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug,
    Default,
    Clone,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct TaskInteractData {
    #[serde(rename = "InteractTagEntries", default)]
    pub interact_tag_entries: Vec<TaskInteractEntryData>,
    #[serde(rename = "DestinationOverrideInteractTag", default)]
    pub destination_override_interact_tag: String,
}

impl AzRtti for TaskInteractData {
    const NAME: &'static str = "TaskInteractData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x1E1CA0DB_AD63_43F4_B6C0_6DF1BEE20556);
}
