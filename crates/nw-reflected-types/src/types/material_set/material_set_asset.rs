use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::MaterialSet;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct MaterialSetAsset {
    #[serde(rename = "BaseClass1", default)]
    pub material_set: MaterialSet,
}

impl AzRtti for MaterialSetAsset {
    const NAME: &'static str = "MaterialSetAsset";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x9E366D8C_33BB_4825_9A1F_FA3ADBE11D0F);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x84399E75_18AB_4000_8DCA_07B9D4E0F8E8)];
}
