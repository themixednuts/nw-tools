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
pub struct CharacterActionGridListCache;

impl AzRtti for CharacterActionGridListCache {
    const NAME: &'static str = "CharacterActionGridListCache";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x3071DCEE_D37D_4E6C_B055_7D92BA0897F6);
}
