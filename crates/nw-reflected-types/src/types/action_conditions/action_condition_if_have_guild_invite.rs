use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
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
pub struct ActionConditionIfHaveGuildInvite {}

impl AzRtti for ActionConditionIfHaveGuildInvite {
    const NAME: &'static str = "ActionConditionIfHaveGuildInvite";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x563EC85D_F945_451C_AFD2_98AFB859F271);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x401EA5B5_DDE2_4848_BE17_FD45660FF8C5)];
}
