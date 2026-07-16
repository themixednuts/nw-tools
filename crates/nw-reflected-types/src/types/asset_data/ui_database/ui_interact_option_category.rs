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
pub struct UiInteractOptionCategory {
    #[serde(rename = "Ui Interact Option Category", default)]
    pub ui_interact_option_category: i32,
}

impl AzRtti for UiInteractOptionCategory {
    const NAME: &'static str = "UiInteractOptionCategory";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xDCE8218D_42FB_48B5_BAD1_3ED6A88A185D);
}
