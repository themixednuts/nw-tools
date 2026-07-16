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
pub struct CharacterActionGridAliasRef;

impl AzRtti for CharacterActionGridAliasRef {
    const NAME: &'static str = "CharacterActionGridAliasRef";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x53F1EDF8_58EC_4A6F_BB20_8DA4A84FA665);
}
