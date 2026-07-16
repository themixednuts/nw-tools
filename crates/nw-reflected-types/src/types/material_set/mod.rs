use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

pub mod material_entry;
pub mod material_properties;
pub mod material_set_asset;

pub use self::material_entry::MaterialEntry;
pub use self::material_properties::MaterialProperties;
pub use self::material_set_asset::MaterialSetAsset;

#[derive(
    Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct MaterialSet {
    #[serde(rename = "DefaultMaterial", default)]
    pub default_material: MaterialProperties,
    #[serde(rename = "Materials", default)]
    pub materials: Vec<MaterialEntry>,
}

impl AzRtti for MaterialSet {
    const NAME: &'static str = "MaterialSet";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x84399E75_18AB_4000_8DCA_07B9D4E0F8E8);
}
