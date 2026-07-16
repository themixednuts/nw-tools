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
pub struct CharacterActionGridCell;

impl AzRtti for CharacterActionGridCell {
    const NAME: &'static str = "CharacterActionGridCell";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xBDED4B21_9E3B_4FEA_8F63_3DA71897E4F1);
}
