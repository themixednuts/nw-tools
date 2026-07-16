use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::{
    EffectData, SimpleAssetReferenceTextureAsset, UiAdditionalInfoType, UiDelayedInteractionData,
    UiInteractActionType, UiInteractAvailabilityData, UiInteractInputType,
    UiInteractOptionCategory, UiInteractPrivilegeId,
};

use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct InteractOptionData {
    #[serde(rename = "Name", default)]
    pub name: String,
    #[serde(rename = "Display Name", default)]
    pub display_name: String,
    #[serde(rename = "Interact Input Type", default)]
    pub interact_input_type: UiInteractInputType,
    #[serde(rename = "Ui Interact Action", default)]
    pub ui_interact_action: UiInteractActionType,
    #[serde(rename = "Additional Info Type", default)]
    pub additional_info_type: UiAdditionalInfoType,
    #[serde(rename = "Interact Option Category", default)]
    pub interact_option_category: UiInteractOptionCategory,
    #[serde(rename = "Delayed Interaction Data", default)]
    pub delayed_interaction_data: UiDelayedInteractionData,
    #[serde(rename = "Interact Privilege Ids", default)]
    pub interact_privilege_ids: Vec<UiInteractPrivilegeId>,
    #[serde(rename = "Blueprint Privilege Id", default)]
    pub blueprint_privilege_id: UiInteractPrivilegeId,
    #[serde(rename = "Requires Confirmation", default)]
    pub requires_confirmation: bool,
    #[serde(rename = "Is Committed Interaction", default)]
    pub is_committed_interaction: bool,
    #[serde(rename = "Is Instant Cancel", default)]
    pub is_instant_cancel: bool,
    #[serde(rename = "Close Prompt On Interaction", default)]
    pub close_prompt_on_interaction: bool,
    #[serde(rename = "Force Secondary Interact", default)]
    pub force_secondary_interact: bool,
    #[serde(rename = "Only Show If Bound To Camp", default)]
    pub only_show_if_bound_to_camp: bool,
    #[serde(rename = "Display Priority", default)]
    pub display_priority: i32,
    #[serde(rename = "Interact Option Icon", default)]
    pub interact_option_icon: SimpleAssetReferenceTextureAsset,
    #[serde(rename = "Ui Additional Info Slice Path", default)]
    pub ui_additional_info_slice_path: String,
    #[serde(rename = "Requires Security Level Validation", default)]
    pub requires_security_level_validation: bool,
    #[serde(rename = "Mannequin Fragment", default)]
    pub mannequin_fragment: String,
    #[serde(rename = "Mannequin Tag", default)]
    pub mannequin_tag: String,
    #[serde(rename = "Align to interaction", default)]
    pub align_to_interaction: bool,
    #[serde(rename = "Hold action press time", default)]
    pub hold_action_press_time: f32,
    #[serde(rename = "Cooldown Time", default)]
    pub cooldown_time: i32,
    #[serde(rename = "Set Ownership On Interact", default)]
    pub set_ownership_on_interact: bool,
    #[serde(rename = "Required Item Name", default)]
    pub required_item_name: String,
    #[serde(rename = "Required Item Count", default)]
    pub required_item_count: i32,
    #[serde(rename = "Required Currency", default)]
    pub required_currency: i32,
    #[serde(rename = "Availability", default)]
    pub availability: UiInteractAvailabilityData,
    #[serde(rename = "Siege Warfare Game Event Name", default)]
    pub siege_warfare_game_event_name: String,
    #[serde(rename = "Added Status Effects", default)]
    pub added_status_effects: Vec<EffectData>,
    #[serde(rename = "Required Status Effects", default)]
    pub required_status_effects: Vec<EffectData>,
    #[serde(rename = "Remove Status Effects", default)]
    pub remove_status_effects: Vec<EffectData>,
    #[serde(rename = "Excluded Status Effects", default)]
    pub excluded_status_effects: Vec<EffectData>,
    #[serde(rename = "Delay Before Adding/Removing Effect", default)]
    pub delay_before_adding_removing_effect: f32,
    #[serde(rename = "Remove Added Effects On Interaction End", default)]
    pub remove_added_effects_on_interaction_end: bool,
    #[serde(rename = "Check PVP Flag Is Set", default)]
    pub check_pvp_flag_is_set: bool,
    #[serde(rename = "Faction Required", default)]
    pub faction_required: bool,
    #[serde(rename = "Show Instanced Loot Item Count", default)]
    pub show_instanced_loot_item_count: bool,
    #[serde(rename = "Required Achievement Name", default)]
    pub required_achievement_name: String,
    #[serde(rename = "Required Level", default)]
    pub required_level: u32,
    #[serde(rename = "Committed Interaction Max Usage Timeout", default)]
    pub committed_interaction_max_usage_timeout: f32,
    #[serde(
        rename = "Committed Interaction Max Usage Timeout Notification",
        default
    )]
    pub committed_interaction_max_usage_timeout_notification: String,
    #[serde(rename = "Committed Interaction Inactive Timeout", default)]
    pub committed_interaction_inactive_timeout: f32,
    #[serde(
        rename = "Committed Interaction Inactive Timeout Notification",
        default
    )]
    pub committed_interaction_inactive_timeout_notification: String,
}

impl AzRtti for InteractOptionData {
    const NAME: &'static str = "InteractOptionData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xF0887E97_5084_413C_BCE7_5C24CECB03C0);
}
