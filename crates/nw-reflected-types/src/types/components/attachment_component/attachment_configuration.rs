use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct AttachmentConfiguration {
    #[serde(rename = "Target ID", default)]
    pub target_id: u64,
    #[serde(rename = "Target Bone Name", default)]
    pub target_bone_name: String,
    #[serde(rename = "Target Offset", default)]
    pub target_offset: bevy_transform::components::Transform,
    #[serde(rename = "Attached Initially", default)]
    pub attached_initially: bool,
    #[serde(rename = "Scale Source", default)]
    pub scale_source: u8,
    #[serde(rename = "Update Tolerance", default)]
    pub update_tolerance: f32,
}

impl AzRtti for AttachmentConfiguration {
    const NAME: &'static str = "AttachmentConfiguration";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x74B5DC69_DE44_4640_836A_55339E116795);
}
