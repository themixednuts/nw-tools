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
pub struct GameRigidBodyServerFacetConfig {
    #[serde(rename = "m_logGridInfo", default)]
    pub log_grid_info: bool,
}

impl AzRtti for GameRigidBodyServerFacetConfig {
    const NAME: &'static str = "GameRigidBodyServerFacetConfig";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x0C1F07A3_01D4_426F_B853_3B2FF9979913);
}
