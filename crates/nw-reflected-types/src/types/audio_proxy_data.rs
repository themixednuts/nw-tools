use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct AudioProxyData {
    #[serde(rename = "m_attachmentJointId", default)]
    pub attachment_joint_id: i32,
    #[serde(rename = "m_refCount", default)]
    pub ref_count: i32,
    #[serde(rename = "m_needUpdateCount", default)]
    pub need_update_count: i32,
    #[serde(rename = "m_attachmentJointCurAbsPos", default)]
    pub attachment_joint_cur_abs_pos: bevy_math::Vec3,
    #[serde(rename = "m_attachmentJointPrevRelPos", default)]
    pub attachment_joint_prev_rel_pos: bevy_math::Vec3,
    #[serde(rename = "m_attachmentJointPrevRelSpeed", default)]
    pub attachment_joint_prev_rel_speed: f32,
    #[serde(rename = "m_attachmentJointRtpcName", default)]
    pub attachment_joint_rtpc_name: String,
}

impl AzRtti for AudioProxyData {
    const NAME: &'static str = "AudioProxyData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x3C3F143D_E106_4CFF_9E90_6544BF6771EF);
}
