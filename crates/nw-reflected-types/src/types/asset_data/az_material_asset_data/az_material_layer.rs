use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug,
    Default,
    Clone,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct AzMaterialLayer {
    #[serde(rename = "ShaderName", default)]
    pub shader_name: String,
    #[serde(rename = "NoDraw", default)]
    pub no_draw: bool,
    #[serde(rename = "FadeOut", default)]
    pub fade_out: bool,
    #[serde(rename = "PublicParams", default)]
    pub public_params: Vec<(String, String)>,
}

impl AzRtti for AzMaterialLayer {
    const NAME: &'static str = "AzMaterialLayer";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x09EA5003_70C6_4658_88B5_9C47911B4232);
}
