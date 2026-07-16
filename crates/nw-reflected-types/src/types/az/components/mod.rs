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
pub struct Component {
    #[serde(rename = "Id", default)]
    pub id: u64,
}

impl AzRtti for Component {
    const NAME: &'static str = "AZ::Component";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xEDFCB2CF_F75D_43BE_B26B_F35821B29247);
}
