use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

pub mod interact_option_data;
pub mod ui_additional_info_type;
pub mod ui_delayed_interaction_data;
pub mod ui_interact_action_type;
pub mod ui_interact_availability_data;
pub mod ui_interact_input_type;
pub mod ui_interact_option_category;
pub mod ui_interact_privilege_id;
pub mod unified_interact_data;

pub use self::interact_option_data::InteractOptionData;
pub use self::ui_additional_info_type::UiAdditionalInfoType;
pub use self::ui_delayed_interaction_data::UiDelayedInteractionData;
pub use self::ui_interact_action_type::UiInteractActionType;
pub use self::ui_interact_availability_data::UiInteractAvailabilityData;
pub use self::ui_interact_input_type::UiInteractInputType;
pub use self::ui_interact_option_category::UiInteractOptionCategory;
pub use self::ui_interact_privilege_id::UiInteractPrivilegeId;
pub use self::unified_interact_data::UnifiedInteractData;

#[derive(
    Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct UiDatabase {
    #[serde(rename = "Unified Interact Data", default)]
    pub unified_interact_data: UnifiedInteractData,
}

impl AzRtti for UiDatabase {
    const NAME: &'static str = "UiDatabase";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x7CC2B992_1C5B_4B27_BCB9_790175F09DA6);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0xAF3F7D32_1536_422A_89F3_A11E1F5B5A9C)];
}
