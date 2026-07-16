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
pub struct CharacterActionList;

impl AzRtti for CharacterActionList {
    const NAME: &'static str = "CharacterActionList";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x127C4F8F_C512_41A0_B85C_2BF7C3363D2A);
}
