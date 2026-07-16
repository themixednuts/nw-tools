use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

pub mod event_notification_data;

pub use self::event_notification_data::EventNotificationData;

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
pub struct EventNotificationDatabase {
    #[serde(rename = "Structure Placed Notification Id", default)]
    pub structure_placed_notification_id: String,
    #[serde(rename = "Interact Focus Notification Ids", default)]
    pub interact_focus_notification_ids: Vec<EventNotificationData>,
    #[serde(rename = "Other Player Encountered Notification Id", default)]
    pub other_player_encountered_notification_id: String,
    #[serde(rename = "Damage Ignored Not Flagged For pvp Notification Id", default)]
    pub damage_ignored_not_flagged_for_pvp_notification_id: String,
    #[serde(rename = "Damage Ignored Sanctuary Notification Id", default)]
    pub damage_ignored_sanctuary_notification_id: String,
    #[serde(
        rename = "Damage Ignored Siege Capture Points Unclaimed Notification Id",
        default
    )]
    pub damage_ignored_siege_capture_points_unclaimed_notification_id: String,
    #[serde(rename = "Damage Ignored Not In Duel Notification Id", default)]
    pub damage_ignored_not_in_duel_notification_id: String,
    #[serde(rename = "Currency Sent To Player Notification Id", default)]
    pub currency_sent_to_player_notification_id: String,
    #[serde(rename = "Currency Received From Player Notification Id", default)]
    pub currency_received_from_player_notification_id: String,
    #[serde(rename = "Currency Send Failed Notification Id", default)]
    pub currency_send_failed_notification_id: String,
    #[serde(rename = "TradingPost Insufficient Quantity Notification Id", default)]
    pub trading_post_insufficient_quantity_notification_id: String,
    #[serde(rename = "TradingPost Price Change Notification Id", default)]
    pub trading_post_price_change_notification_id: String,
    #[serde(rename = "Trade Failure NotificationId", default)]
    pub trade_failure_notification_id: String,
    #[serde(
        rename = "Trade Failure No Withdrawal Permissions Notification Id",
        default
    )]
    pub trade_failure_no_withdrawal_permissions_notification_id: String,
    #[serde(
        rename = "Trade Failure No Deposit Permissions Notification Id",
        default
    )]
    pub trade_failure_no_deposit_permissions_notification_id: String,
    #[serde(rename = "Trade Failure Not Enough Space Notification Id", default)]
    pub trade_failure_not_enough_space_notification_id: String,
    #[serde(
        rename = "Trade Failure Global Storage Not Enough Currency Notification Id",
        default
    )]
    pub trade_failure_global_storage_not_enough_currency_notification_id: String,
    #[serde(rename = "Trade Failure Not In FFA State Notification Id", default)]
    pub trade_failure_not_in_ffa_state_notification_id: String,
    #[serde(rename = "Trade Failure Not In PVP State Notification Id", default)]
    pub trade_failure_not_in_pvp_state_notification_id: String,
    #[serde(
        rename = "Trade Failure Global Storage Wrong Faction Notification Id",
        default
    )]
    pub trade_failure_global_storage_wrong_faction_notification_id: String,
    #[serde(
        rename = "Trade Failure Global Storage Invalid Deposit Notification Id",
        default
    )]
    pub trade_failure_global_storage_invalid_deposit_notification_id: String,
    #[serde(
        rename = "Trade Failure Global Storage Invalid Deposit RFI Notification Id",
        default
    )]
    pub trade_failure_global_storage_invalid_deposit_rfi_notification_id: String,
    #[serde(
        rename = "Trade Failure Global Storage Invalid Deposit Storage Inaccessible Id",
        default
    )]
    pub trade_failure_global_storage_invalid_deposit_storage_inaccessible_id: String,
    #[serde(rename = "Teleport Pending Notification Id", default)]
    pub teleport_pending_notification_id: String,
    #[serde(rename = "Teleport Denied Notification Id", default)]
    pub teleport_denied_notification_id: String,
    #[serde(rename = "Teleport Failed Notification Id", default)]
    pub teleport_failed_notification_id: String,
    #[serde(rename = "New Items Dropped Notification Id", default)]
    pub new_items_dropped_notification_id: String,
    #[serde(rename = "Coin Transferal Disabled Notification Id", default)]
    pub coin_transferal_disabled_notification_id: String,
    #[serde(rename = "Coin Generation Limited Notification Id", default)]
    pub coin_generation_limited_notification_id: String,
    #[serde(rename = "Crafting Disabled Notification Id", default)]
    pub crafting_disabled_notification_id: String,
    #[serde(rename = "Salvaging Disabled Notification Id", default)]
    pub salvaging_disabled_notification_id: String,
    #[serde(rename = "Item Salvaging Disabled Notification Id", default)]
    pub item_salvaging_disabled_notification_id: String,
    #[serde(rename = "Recipe Salvaging Disabled Notification Id", default)]
    pub recipe_salvaging_disabled_notification_id: String,
    #[serde(rename = "Open Loot Disabled Notification Id", default)]
    pub open_loot_disabled_notification_id: String,
}

impl AzRtti for EventNotificationDatabase {
    const NAME: &'static str = "EventNotificationDatabase";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x5C624458_C56D_417D_A7A8_16A98EEE22A6);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0xAF3F7D32_1536_422A_89F3_A11E1F5B5A9C)];
}
