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
pub struct UiInteractPrivilegeId {
    #[serde(rename = "Privileges Type", default)]
    pub privileges_type: u32,
}

impl AzRtti for UiInteractPrivilegeId {
    const NAME: &'static str = "UiInteractPrivilegeId";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x75EFB374_E8B8_478E_921D_37D58C6605EF);
}
