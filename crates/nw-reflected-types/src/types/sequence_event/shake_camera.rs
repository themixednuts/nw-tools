use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::SlayerScriptLiteral;
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
pub struct ShakeCamera {
    #[serde(rename = "m_cameraShakeID", default)]
    pub camera_shake_id: SlayerScriptLiteral,
    #[serde(rename = "m_cameraShakeRange", default)]
    pub camera_shake_range: f32,
}

impl AzRtti for ShakeCamera {
    const NAME: &'static str = "ShakeCamera";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x42AD7BF7_B29F_4E03_8D53_9BF144711D0B);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x9B454E3B_282D_4089_90BE_DF25317205E7)];
}
