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
pub struct CharacterActionGrid {}

impl AzRtti for CharacterActionGrid {
    const NAME: &'static str = "CharacterActionGrid";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x44CBAB2A_97EB_47B8_ABBF_06D3825FEB7A);
}
