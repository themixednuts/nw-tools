use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::{PlayerDimensions, PlayerDynamics};
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
pub struct CryPlayerPhysicsConfiguration {
    #[serde(rename = "Player Dimensions", default)]
    pub player_dimensions: PlayerDimensions,
    #[serde(rename = "Player Dynamics", default)]
    pub player_dynamics: PlayerDynamics,
}

impl AzRtti for CryPlayerPhysicsConfiguration {
    const NAME: &'static str = "CryPlayerPhysicsConfiguration";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x97A1C07E_0444_4FAC_A394_8317AFE5696B);
}
