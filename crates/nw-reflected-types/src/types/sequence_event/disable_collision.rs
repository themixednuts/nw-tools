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
pub struct DisableCollision {
    #[serde(rename = "m_modifyActorCollision", default)]
    pub modify_actor_collision: bool,
    #[serde(rename = "m_modifyStaticCollision", default)]
    pub modify_static_collision: bool,
    #[serde(rename = "m_modifyPlayerToPlayerCollision", default)]
    pub modify_player_to_player_collision: bool,
}

impl AzRtti for DisableCollision {
    const NAME: &'static str = "DisableCollision";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x7A998EB7_E1AC_4A4E_A1BD_65EE70F6341E);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x9B454E3B_282D_4089_90BE_DF25317205E7)];
}
