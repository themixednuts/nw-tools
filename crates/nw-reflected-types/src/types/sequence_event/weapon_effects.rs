use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::{PaperdollSlotAlias, SequenceEventOptions};
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
pub struct WeaponEffects {
    #[serde(rename = "m_itemSlotAlias", default)]
    pub item_slot_alias: PaperdollSlotAlias,
    #[serde(rename = "m_effectGroupName", default)]
    pub effect_group_name: i8,
    #[serde(rename = "m_optionOnEnter", default)]
    pub option_on_enter: SequenceEventOptions,
    #[serde(rename = "m_optionOnExit", default)]
    pub option_on_exit: SequenceEventOptions,
}

impl AzRtti for WeaponEffects {
    const NAME: &'static str = "WeaponEffects";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x50A1344B_6AB0_44FD_8897_4E9EB168A7C3);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x9B454E3B_282D_4089_90BE_DF25317205E7)];
}
