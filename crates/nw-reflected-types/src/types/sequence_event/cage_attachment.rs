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
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct CAGEAttachment {
    #[serde(rename = "m_attachmentSource", default)]
    pub attachment_source: i32,
    #[serde(rename = "m_attachmentName", default)]
    pub attachment_name: SlayerScriptLiteral,
    #[serde(rename = "m_customSource", default)]
    pub custom_source: i8,
    #[serde(rename = "m_attachedAnimationAlias", default)]
    pub attached_animation_alias: SlayerScriptLiteral,
}

impl AzRtti for CAGEAttachment {
    const NAME: &'static str = "CAGEAttachment";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x9C5C516D_3B1F_46C3_94B7_F2D6AC1E5E21);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x9B454E3B_282D_4089_90BE_DF25317205E7)];
}
