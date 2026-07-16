use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::InteractOptionData;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct UnifiedInteractData {
    #[serde(rename = "Interact Options", default)]
    pub interact_options: Vec<InteractOptionData>,
}

impl AzRtti for UnifiedInteractData {
    const NAME: &'static str = "UnifiedInteractData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xEBC0595E_4ADB_4323_9527_82D07E30908C);
}
