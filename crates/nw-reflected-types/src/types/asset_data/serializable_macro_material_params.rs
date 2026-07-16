use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
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
pub struct SerializableMacroMaterialParams {
    #[serde(rename = "MacroColorScale", default)]
    pub macro_color_scale: f32,
    #[serde(rename = "MacroColor", default)]
    pub macro_color: bevy_color::LinearRgba,
    #[serde(rename = "MacroGlossScale", default)]
    pub macro_gloss_scale: f32,
    #[serde(rename = "MacroNormalScale", default)]
    pub macro_normal_scale: f32,
    #[serde(rename = "MacroSpecularReflectance", default)]
    pub macro_specular_reflectance: f32,
}

impl AzRtti for SerializableMacroMaterialParams {
    const NAME: &'static str = "SerializableMacroMaterialParams";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xA99A56EF_2BC9_406A_855D_BD36F5DF2638);
}
