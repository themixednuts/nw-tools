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
pub struct HitStun {
    #[serde(rename = "m_drawDebug", default)]
    pub draw_debug: bool,
}

impl AzRtti for HitStun {
    const NAME: &'static str = "HitStun";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x21633383_6478_4BED_A9C9_B352FE21C5AA);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x9B454E3B_282D_4089_90BE_DF25317205E7)];
}
