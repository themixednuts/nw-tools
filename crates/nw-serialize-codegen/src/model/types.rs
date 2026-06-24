use std::collections::BTreeMap;

use uuid::Uuid;

use crate::reference::ReferenceKey;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassNameIndexEntry {
    pub name_crc: Option<u32>,
    pub type_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReflectedClass {
    pub reference_id: Option<ReferenceKey>,
    pub map_key_type_id: Option<Uuid>,
    pub name: String,
    pub type_id: Uuid,
    pub version: Option<u32>,
    pub factory: Option<String>,
    pub persistent_id: Option<String>,
    pub do_save: Option<String>,
    pub serializer: Option<String>,
    pub az_rtti: Option<ReflectedAzRtti>,
    pub container: Option<String>,
    pub converter: Option<String>,
    pub data_converter: Option<String>,
    pub event_handler: Option<String>,
    pub members: Vec<ReflectedMember>,
    pub attributes: Vec<ReflectedAttribute>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReflectedMember {
    pub reference_id: Option<ReferenceKey>,
    pub name: String,
    pub name_crc: Option<u32>,
    pub type_id: Uuid,
    pub data_size: Option<u32>,
    pub offset: Option<u32>,
    pub attribute_ownership: Option<u32>,
    pub flags: Option<u32>,
    pub is_pointer: bool,
    pub is_base_class: bool,
    pub no_default_value: bool,
    pub is_dynamic_field: bool,
    pub is_ui_element: bool,
    pub az_rtti: Option<ReflectedAzRtti>,
    pub generic_class: Option<Box<ReflectedGenericClass>>,
    pub attributes: Vec<ReflectedAttribute>,
}

impl ReflectedMember {
    #[must_use]
    pub fn enum_type_id(&self) -> Option<Uuid> {
        self.attributes
            .iter()
            .find(|attribute| attribute.name.as_deref() == Some("EnumType"))
            .and_then(|attribute| attribute.value.as_ref())
            .and_then(|value| value.value_string.as_deref())
            .and_then(|value| Uuid::parse_str(value.trim_matches(['{', '}'])).ok())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReflectedGenericClass {
    pub reference_id: Option<ReferenceKey>,
    pub map_key_type_id: Option<Uuid>,
    pub type_id: Option<Uuid>,
    pub registered_type_ids: Vec<Uuid>,
    pub templated_argument_count: Option<u32>,
    pub templated_type_ids: Vec<Uuid>,
    pub type_id_fold_type_ids: Vec<Uuid>,
    pub specialized_type_id: Option<Uuid>,
    pub generic_type_id: Option<Uuid>,
    pub legacy_specialized_type_id: Option<Uuid>,
    pub non_type_template_arguments: BTreeMap<String, ReflectedNonTypeTemplateArgument>,
    pub class_type_id: Option<Uuid>,
    pub class_name: Option<String>,
    pub members: Vec<ReflectedMember>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReflectedAzRtti {
    pub reference_id: Option<ReferenceKey>,
    pub address: Option<String>,
    pub type_id: Option<Uuid>,
    pub type_name: Option<String>,
    pub hierarchy: Vec<ReflectedAzRttiHierarchyEntry>,
    pub is_abstract: Option<bool>,
}

impl ReflectedAzRtti {
    #[must_use]
    pub fn address(&self) -> Option<&str> {
        self.address.as_deref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReflectedAzRttiHierarchyEntry {
    pub type_id: Uuid,
    pub type_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReflectedBodylessType {
    pub type_id: Uuid,
    pub name: String,
    pub is_abstract: Option<bool>,
}

impl ReflectedGenericClass {
    pub fn concrete_type_ids(&self) -> impl Iterator<Item = Uuid> + '_ {
        self.map_key_type_id
            .into_iter()
            .chain(self.type_id)
            .chain(self.specialized_type_id)
            .chain(self.legacy_specialized_type_id)
            .chain(self.registered_type_ids.iter().copied())
    }

    #[must_use]
    pub fn argument_type_ids(&self) -> Vec<Uuid> {
        if !self.templated_type_ids.is_empty() {
            return self.templated_type_ids.clone();
        }
        let value1 = self
            .members
            .iter()
            .find(|member| member.name == "value1")
            .map(|member| member.type_id);
        let value2 = self
            .members
            .iter()
            .find(|member| member.name == "value2")
            .map(|member| member.type_id);
        if let (Some(value1), Some(value2)) = (value1, value2) {
            return vec![value1, value2];
        }
        self.members
            .iter()
            .find(|member| member.name == "element")
            .or_else(|| self.members.first())
            .map(|member| vec![member.type_id])
            .unwrap_or_default()
    }

    #[must_use]
    pub fn non_type_capacity(&self) -> Option<usize> {
        self.non_type_template_arguments
            .get("capacity")
            .or_else(|| self.non_type_template_arguments.get("size"))
            .and_then(|value| {
                value
                    .as_u64_lossy()
                    .and_then(|value| usize::try_from(value).ok())
            })
            .or_else(|| {
                self.non_type_template_arguments
                    .get("values")
                    .and_then(ReflectedNonTypeTemplateArgument::as_array)
                    .and_then(|values| values.first())
                    .and_then(|value| {
                        value
                            .as_u64_lossy()
                            .and_then(|value| usize::try_from(value).ok())
                    })
            })
    }

    #[must_use]
    pub fn non_type_integer_bounds(&self) -> Option<(i128, i128)> {
        let values = self.non_type_template_arguments.get("values")?.as_array()?;
        let [min, max, ..] = values else {
            return None;
        };
        Some((non_type_i128(min)?, non_type_i128(max)?))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ReflectedNonTypeTemplateArgument {
    Null,
    Bool(bool),
    U64(u64),
    I64(i64),
    F64(f64),
    String(String),
    Array(Vec<ReflectedNonTypeTemplateArgument>),
}

impl ReflectedNonTypeTemplateArgument {
    #[must_use]
    pub const fn as_u64(&self) -> Option<u64> {
        match self {
            Self::U64(value) => Some(*value),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_array(&self) -> Option<&[ReflectedNonTypeTemplateArgument]> {
        match self {
            Self::Array(values) => Some(values),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_u64_lossy(&self) -> Option<u64> {
        match self {
            Self::U64(value) => Some(*value),
            Self::I64(value) => u64::try_from(*value).ok(),
            Self::String(value) => value
                .strip_prefix("0x")
                .and_then(|value| u64::from_str_radix(value, 16).ok())
                .or_else(|| value.parse().ok()),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReflectedEnum {
    pub reference_id: Option<ReferenceKey>,
    pub type_id: Uuid,
    pub element_id: Option<u32>,
    pub name: String,
    pub description: Option<String>,
    pub deprecated_name: Option<String>,
    pub variants: Vec<ReflectedEnumVariant>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReflectedEnumVariant {
    pub name: String,
    pub value_u64: Option<u64>,
    pub value_u32: Option<u32>,
    pub value_i32: Option<i32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReflectedAttribute {
    pub reference_id: Option<ReferenceKey>,
    pub attribute_id: Option<u32>,
    pub name: Option<String>,
    pub describes_children: Option<bool>,
    pub child_class_owned: Option<bool>,
    pub value: Option<ReflectedAttributeValue>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReflectedAttributeValue {
    pub kind: Option<String>,
    pub description: Option<String>,
    pub function: Option<String>,
    pub member_function: Option<String>,
    pub value_string: Option<String>,
    pub value_bool: Option<bool>,
    pub value_u64: Option<u64>,
    pub value_u32: Option<u32>,
    pub value_i32: Option<i32>,
    pub value_high_u32: Option<u32>,
    pub value_f32: Option<f64>,
    pub value_high_f32: Option<f64>,
}

fn non_type_i128(value: &ReflectedNonTypeTemplateArgument) -> Option<i128> {
    match value {
        ReflectedNonTypeTemplateArgument::U64(value) => Some(i128::from(*value)),
        ReflectedNonTypeTemplateArgument::I64(value) => Some(i128::from(*value)),
        ReflectedNonTypeTemplateArgument::String(value) => value
            .strip_prefix("0x")
            .and_then(|value| i128::from_str_radix(value, 16).ok())
            .or_else(|| value.parse().ok()),
        _ => None,
    }
}
