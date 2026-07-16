use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::InteractionUIActions;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug,
    Default,
    Clone,
    Copy,
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
pub struct UiInteractActionType {
    #[serde(rename = "Interact Action Type", default)]
    pub interact_action_type: InteractionUIActions,
}

impl AzRtti for UiInteractActionType {
    const NAME: &'static str = "UiInteractActionType";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xBA36E7E7_3B73_480E_BEF9_4D80C17D2745);
}
