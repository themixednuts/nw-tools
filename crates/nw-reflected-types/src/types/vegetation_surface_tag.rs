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
pub struct VegetationSurfaceTag {
    #[serde(rename = "SurfaceTagCrc", default)]
    pub surface_tag_crc: u32,
}

impl AzRtti for VegetationSurfaceTag {
    const NAME: &'static str = "VegetationSurfaceTag";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x67C8C6ED_F32A_443E_A777_1CAE48B22CD7);
}
