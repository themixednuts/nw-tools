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
pub struct QueryShapePoint {}

impl AzRtti for QueryShapePoint {
    const NAME: &'static str = "QueryShapePoint";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x44B34B6C_63B0_443C_BEEE_272EA4106EDC);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0xCF5EB8E9_9C03_4A19_828D_ED500C732978)];
}
