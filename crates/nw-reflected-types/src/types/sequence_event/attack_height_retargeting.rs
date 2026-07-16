use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug,
    Default,
    Clone,
    Copy,
    PartialEq,
    serde::Serialize,
    serde::Deserialize,
    bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct AttackHeightRetargeting {
    #[serde(rename = "m_overrideDefaultBlendTimes", default)]
    pub override_default_blend_times: bool,
    #[serde(rename = "m_blendInTimeOverride", default)]
    pub blend_in_time_override: f32,
    #[serde(rename = "m_blendOutTimeOverride", default)]
    pub blend_out_time_override: f32,
    #[serde(rename = "m_minAngleDegreesOverride", default)]
    pub min_angle_degrees_override: f32,
    #[serde(rename = "m_maxAngleDegreesOverride", default)]
    pub max_angle_degrees_override: f32,
    #[serde(rename = "m_lockTargetDirOnStart", default)]
    pub lock_target_dir_on_start: bool,
}

impl AzRtti for AttackHeightRetargeting {
    const NAME: &'static str = "AttackHeightRetargeting";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xBA0E7996_AC24_4E79_A172_6B7173C678AB);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x9B454E3B_282D_4089_90BE_DF25317205E7)];
}
