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
pub struct EditEnumItemClasses {
    #[serde(rename = "m_valueCrc", default)]
    pub value_crc: u32,
}

impl AzRtti for EditEnumItemClasses {
    const NAME: &'static str = "EditEnum<EnumType><Javelin::SBItemClass::ItemClasses >";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x32737B9C_7A9F_547A_868D_79E601645FC8);
}
