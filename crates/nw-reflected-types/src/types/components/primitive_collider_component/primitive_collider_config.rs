use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug,
    Default,
    Clone,
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
pub struct PrimitiveColliderConfig {
    #[serde(rename = "SurfaceTypeName", default)]
    pub surface_type_name: String,
}

impl AzRtti for PrimitiveColliderConfig {
    const NAME: &'static str = "PrimitiveColliderConfig";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x85AA27D6_E019_469F_8472_89862323DBF7);
}
