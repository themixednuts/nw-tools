#![allow(dead_code)]

use std::borrow::Cow;

include!("generated.rs");

const MAX_SCHEMA_VALUE_DEPTH: usize = 64;

pub fn parse(bytes: &[u8]) -> serde_json::Result<SerializeContext> {
    serde_json::from_slice(bytes)
}

pub fn parse_value(value: serde_json::Value) -> serde_json::Result<SerializeContext> {
    serde_json::from_value(value)
}

impl SerializeContext {
    pub fn uuid_map_entries(&self) -> impl Iterator<Item = (&str, &UuidMap)> {
        self.uuid_map
            .iter()
            .map(|(type_id, class)| (type_id.as_str(), class))
    }

    pub fn class_name_entries(&self) -> impl Iterator<Item = SchemaClassNameEntry<'_>> {
        self.class_name_to_uuid.iter().filter_map(|entry| {
            let [
                ClassNameToUuid::Integer(name_crc),
                ClassNameToUuid::String(type_id),
            ] = entry.as_slice()
            else {
                return None;
            };
            Some(SchemaClassNameEntry {
                name_crc: *name_crc,
                type_id,
            })
        })
    }

    pub fn uuid_generic_map_entries(&self) -> impl Iterator<Item = (&str, &UuidGenericMap)> {
        self.uuid_generic_map.iter().filter_map(|entry| {
            let [
                UuidGenericMapElement::String(type_id),
                UuidGenericMapElement::UuidGenericMap(generic),
            ] = entry.as_slice()
            else {
                return None;
            };
            Some((type_id.as_str(), generic.as_ref()))
        })
    }

    pub fn uuid_generic_class_entries(
        &self,
    ) -> impl Iterator<Item = (&str, SchemaGenericClassInfo<'_>)> {
        self.uuid_generic_map_entries()
            .map(|(type_id, generic)| (type_id, generic.as_generic_class_info()))
    }

    pub fn uuid_any_creation_entries(&self) -> impl Iterator<Item = (&str, &str)> {
        self.uuid_any_creation_map
            .iter()
            .map(|(type_id, creator)| (type_id.as_str(), creator.as_str()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchemaClassNameEntry<'a> {
    pub name_crc: i64,
    pub type_id: &'a str,
}

impl UuidMap {
    #[must_use]
    pub fn id(&self) -> i64 {
        self.id
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn type_id(&self) -> &str {
        &self.type_id
    }

    #[must_use]
    pub fn version(&self) -> Option<u32> {
        i64_to_u32(self.version)
    }

    #[must_use]
    pub fn factory(&self) -> Option<&str> {
        self.factory.as_deref().filter(|value| !value.is_empty())
    }

    #[must_use]
    pub fn persistent_id(&self) -> Option<&str> {
        self.persistent_id
            .as_deref()
            .filter(|value| !value.is_empty())
    }

    #[must_use]
    pub fn do_save(&self) -> Option<&str> {
        value_str(self.do_save.as_ref())
    }

    #[must_use]
    pub fn serializer(&self) -> Option<&str> {
        self.serializer.as_deref().filter(|value| !value.is_empty())
    }

    #[must_use]
    pub fn az_rtti(&self) -> Option<SchemaAzRttiInfo<'_>> {
        self.az_rtti.as_ref().map(SchemaAzRttiInfo::UuidMap)
    }

    #[must_use]
    pub fn container(&self) -> Option<&str> {
        self.container.as_deref().filter(|value| !value.is_empty())
    }

    #[must_use]
    pub fn converter(&self) -> Option<&str> {
        self.converter.as_deref().filter(|value| !value.is_empty())
    }

    #[must_use]
    pub fn data_converter(&self) -> Option<&str> {
        value_str(self.data_converter.as_ref())
    }

    #[must_use]
    pub fn event_handler(&self) -> Option<&str> {
        self.event_handler
            .as_deref()
            .filter(|value| !value.is_empty())
    }

    #[must_use]
    pub fn elements(&self) -> &[UuidMapElement] {
        &self.elements
    }
}

impl UuidMapElement {
    #[must_use]
    pub fn id(&self) -> i64 {
        self.id
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn type_id(&self) -> &str {
        &self.type_id
    }

    #[must_use]
    pub fn name_crc(&self) -> Option<u32> {
        i64_to_u32(self.name_crc)
    }

    #[must_use]
    pub fn data_size(&self) -> Option<u32> {
        str_to_u32(&self.data_size)
    }

    #[must_use]
    pub fn offset(&self) -> Option<u32> {
        str_to_u32(&self.offset)
    }

    #[must_use]
    pub fn attribute_ownership(&self) -> Option<u32> {
        i64_to_u32(self.attribute_ownership)
    }

    #[must_use]
    pub fn flags(&self) -> Option<u32> {
        i64_to_u32(self.flags)
    }

    #[must_use]
    pub fn is_pointer(&self) -> bool {
        self.is_pointer
    }

    #[must_use]
    pub fn is_base_class(&self) -> bool {
        self.is_base_class
    }

    #[must_use]
    pub fn no_default_value(&self) -> bool {
        self.no_default_value
    }

    #[must_use]
    pub fn is_dynamic_field(&self) -> bool {
        self.is_dynamic_field
    }

    #[must_use]
    pub fn is_ui_element(&self) -> bool {
        self.is_ui_element
    }

    #[must_use]
    pub fn az_rtti(&self) -> Option<SchemaAzRttiInfo<'_>> {
        self.az_rtti.as_ref().map(SchemaAzRttiInfo::Element)
    }

    #[must_use]
    pub fn generic_class_info(&self) -> Option<SchemaGenericClassInfo<'_>> {
        self.generic_class_info
            .as_ref()
            .map(SchemaGenericClassInfo::Purple)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SchemaGenericClassInfo<'a> {
    UuidGenericMap(&'a UuidGenericMap),
    Sticky(&'a StickyGenericClassInfo),
    Tentacled(&'a TentacledGenericClassInfo),
    Fluffy(&'a FluffyGenericClassInfo),
    Purple(&'a PurpleGenericClassInfo),
    IndigoRef(&'a IndigoGenericClassInfo),
}

impl<'a> SchemaGenericClassInfo<'a> {
    #[must_use]
    pub fn reference(&self) -> Option<&'a str> {
        match self {
            Self::UuidGenericMap(generic) => generic
                .uuid_generic_map_ref
                .as_deref()
                .filter(|value| !value.is_empty()),
            Self::Sticky(generic) => generic
                .generic_class_info_ref
                .as_deref()
                .filter(|value| !value.is_empty()),
            Self::Tentacled(generic) => generic
                .generic_class_info_ref
                .as_deref()
                .filter(|value| !value.is_empty()),
            Self::Fluffy(generic) => generic
                .generic_class_info_ref
                .as_deref()
                .filter(|value| !value.is_empty()),
            Self::Purple(generic) => generic
                .generic_class_info_ref
                .as_deref()
                .filter(|value| !value.is_empty()),
            Self::IndigoRef(generic) => Some(generic.generic_class_info_ref.as_str()),
        }
    }

    #[must_use]
    pub fn id(&self) -> Option<i64> {
        match self {
            Self::UuidGenericMap(generic) => generic.id,
            Self::Sticky(generic) => generic.id,
            Self::Tentacled(generic) => generic.id,
            Self::Fluffy(generic) => generic.id,
            Self::Purple(generic) => generic.id,
            Self::IndigoRef(_) => None,
        }
    }

    #[must_use]
    pub fn type_id(&self) -> Option<&'a str> {
        match self {
            Self::UuidGenericMap(generic) => generic.type_id.as_deref(),
            Self::Sticky(generic) => generic.type_id.as_deref(),
            Self::Tentacled(generic) => generic.type_id.as_deref(),
            Self::Fluffy(generic) => generic.type_id.as_deref(),
            Self::Purple(generic) => generic.type_id.as_deref(),
            Self::IndigoRef(_) => None,
        }
        .filter(|value| !value.is_empty())
    }

    #[must_use]
    pub fn registered_type_ids(&self) -> &'a [String] {
        match self {
            Self::UuidGenericMap(generic) => generic.registered_type_ids.as_deref(),
            Self::Sticky(generic) => generic.registered_type_ids.as_deref(),
            Self::Tentacled(generic) => generic.registered_type_ids.as_deref(),
            Self::Fluffy(generic) => generic.registered_type_ids.as_deref(),
            Self::Purple(generic) => generic.registered_type_ids.as_deref(),
            Self::IndigoRef(_) => None,
        }
        .unwrap_or(&[])
    }

    #[must_use]
    pub fn templated_argument_count(&self) -> Option<u32> {
        match self {
            Self::UuidGenericMap(generic) => generic.templated_argument_count,
            Self::Sticky(generic) => generic.templated_argument_count,
            Self::Tentacled(generic) => generic.templated_argument_count,
            Self::Fluffy(generic) => generic.templated_argument_count,
            Self::Purple(generic) => generic.templated_argument_count,
            Self::IndigoRef(_) => None,
        }
        .and_then(i64_to_u32)
    }

    #[must_use]
    pub fn templated_type_ids(&self) -> &'a [String] {
        match self {
            Self::UuidGenericMap(generic) => generic.templated_type_ids.as_deref(),
            Self::Sticky(generic) => generic.templated_type_ids.as_deref(),
            Self::Tentacled(generic) => generic.templated_type_ids.as_deref(),
            Self::Fluffy(generic) => generic.templated_type_ids.as_deref(),
            Self::Purple(generic) => generic.templated_type_ids.as_deref(),
            Self::IndigoRef(_) => None,
        }
        .unwrap_or(&[])
    }

    #[must_use]
    pub fn type_id_fold_type_ids(&self) -> &'a [String] {
        match self {
            Self::UuidGenericMap(generic) => generic.type_id_fold_type_ids.as_deref(),
            Self::Sticky(generic) => generic.type_id_fold_type_ids.as_deref(),
            Self::Tentacled(generic) => generic.type_id_fold_type_ids.as_deref(),
            Self::Fluffy(generic) => generic.type_id_fold_type_ids.as_deref(),
            Self::Purple(generic) => generic.type_id_fold_type_ids.as_deref(),
            Self::IndigoRef(_) => None,
        }
        .unwrap_or(&[])
    }

    #[must_use]
    pub fn specialized_type_id(&self) -> Option<&'a str> {
        match self {
            Self::UuidGenericMap(generic) => generic.specialized_type_id.as_deref(),
            Self::Sticky(generic) => generic.specialized_type_id.as_deref(),
            Self::Tentacled(generic) => generic.specialized_type_id.as_deref(),
            Self::Fluffy(generic) => generic.specialized_type_id.as_deref(),
            Self::Purple(generic) => generic.specialized_type_id.as_deref(),
            Self::IndigoRef(_) => None,
        }
        .filter(|value| !value.is_empty())
    }

    #[must_use]
    pub fn generic_type_id(&self) -> Option<&'a str> {
        match self {
            Self::UuidGenericMap(generic) => generic.generic_type_id.as_deref(),
            Self::Sticky(generic) => generic.generic_type_id.as_deref(),
            Self::Tentacled(generic) => generic.generic_type_id.as_deref(),
            Self::Fluffy(generic) => generic.generic_type_id.as_deref(),
            Self::Purple(generic) => generic.generic_type_id.as_deref(),
            Self::IndigoRef(_) => None,
        }
        .filter(|value| !value.is_empty())
    }

    #[must_use]
    pub fn legacy_specialized_type_id(&self) -> Option<Cow<'a, str>> {
        match self {
            Self::UuidGenericMap(generic) => generic
                .legacy_specialized_type_id
                .as_ref()
                .map(legacy_specialized_type_id_str)
                .map(Cow::Borrowed),
            Self::Sticky(generic) => generic
                .legacy_specialized_type_id
                .as_deref()
                .filter(|value| !value.is_empty())
                .map(Cow::Borrowed),
            Self::Tentacled(generic) => generic
                .legacy_specialized_type_id
                .as_deref()
                .filter(|value| !value.is_empty())
                .map(Cow::Borrowed),
            Self::Fluffy(generic) => generic
                .legacy_specialized_type_id
                .as_deref()
                .filter(|value| !value.is_empty())
                .map(Cow::Borrowed),
            Self::Purple(generic) => generic
                .legacy_specialized_type_id
                .as_deref()
                .filter(|value| !value.is_empty())
                .map(Cow::Borrowed),
            Self::IndigoRef(_) => None,
        }
    }

    #[must_use]
    pub fn class_data(&self) -> Option<&'a UuidMap> {
        match self {
            Self::UuidGenericMap(generic) => generic.class_data.as_ref(),
            Self::Sticky(generic) => generic.class_data.as_ref(),
            Self::Tentacled(generic) => generic.class_data.as_ref(),
            Self::Fluffy(generic) => generic.class_data.as_ref(),
            Self::Purple(generic) => generic.class_data.as_ref(),
            Self::IndigoRef(_) => None,
        }
    }

    #[must_use]
    pub fn elements(&self) -> Vec<SchemaElementInfo<'a>> {
        match self {
            Self::UuidGenericMap(generic) => generic
                .elements
                .as_deref()
                .unwrap_or(&[])
                .iter()
                .map(SchemaElementInfo::UuidGenericMap)
                .collect(),
            Self::Sticky(generic) => generic
                .elements
                .as_deref()
                .unwrap_or(&[])
                .iter()
                .map(SchemaElementInfo::Sticky)
                .collect(),
            Self::Tentacled(generic) => generic
                .elements
                .as_deref()
                .unwrap_or(&[])
                .iter()
                .map(SchemaElementInfo::Tentacled)
                .collect(),
            Self::Fluffy(generic) => generic
                .elements
                .as_deref()
                .unwrap_or(&[])
                .iter()
                .map(SchemaElementInfo::Fluffy)
                .collect(),
            Self::Purple(generic) => generic
                .elements
                .as_deref()
                .unwrap_or(&[])
                .iter()
                .map(SchemaElementInfo::Purple)
                .collect(),
            Self::IndigoRef(_) => Vec::new(),
        }
    }

    #[must_use]
    pub fn non_type_template_arguments(&self) -> SchemaNonTypeTemplateArguments<'a> {
        match self {
            Self::UuidGenericMap(generic) => generic
                .non_type_template_arguments
                .as_ref()
                .map(SchemaNonTypeTemplateArguments::Json)
                .unwrap_or(SchemaNonTypeTemplateArguments::None),
            Self::Sticky(generic) => generic
                .non_type_template_arguments
                .as_ref()
                .map(SchemaNonTypeTemplateArguments::Json)
                .unwrap_or(SchemaNonTypeTemplateArguments::None),
            Self::Tentacled(generic) => generic
                .non_type_template_arguments
                .as_ref()
                .map(SchemaNonTypeTemplateArguments::Purple)
                .unwrap_or(SchemaNonTypeTemplateArguments::None),
            Self::Fluffy(generic) => generic
                .non_type_template_arguments
                .as_ref()
                .map(SchemaNonTypeTemplateArguments::Json)
                .unwrap_or(SchemaNonTypeTemplateArguments::None),
            Self::Purple(generic) => generic
                .non_type_template_arguments
                .as_ref()
                .map(SchemaNonTypeTemplateArguments::Fluffy)
                .unwrap_or(SchemaNonTypeTemplateArguments::None),
            Self::IndigoRef(_) => SchemaNonTypeTemplateArguments::None,
        }
    }
}

impl UuidGenericMap {
    #[must_use]
    pub fn as_generic_class_info(&self) -> SchemaGenericClassInfo<'_> {
        SchemaGenericClassInfo::UuidGenericMap(self)
    }
}

impl StickyGenericClassInfo {
    #[must_use]
    pub fn as_generic_class_info(&self) -> SchemaGenericClassInfo<'_> {
        SchemaGenericClassInfo::Sticky(self)
    }
}

impl TentacledGenericClassInfo {
    #[must_use]
    pub fn as_generic_class_info(&self) -> SchemaGenericClassInfo<'_> {
        SchemaGenericClassInfo::Tentacled(self)
    }
}

impl FluffyGenericClassInfo {
    #[must_use]
    pub fn as_generic_class_info(&self) -> SchemaGenericClassInfo<'_> {
        SchemaGenericClassInfo::Fluffy(self)
    }
}

impl PurpleGenericClassInfo {
    #[must_use]
    pub fn as_generic_class_info(&self) -> SchemaGenericClassInfo<'_> {
        SchemaGenericClassInfo::Purple(self)
    }
}

impl IndigoGenericClassInfo {
    #[must_use]
    pub fn as_generic_class_info(&self) -> SchemaGenericClassInfo<'_> {
        SchemaGenericClassInfo::IndigoRef(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SchemaElementInfo<'a> {
    UuidMap(&'a UuidMapElement),
    UuidGenericMap(&'a UuidGenericMapElementClass),
    Sticky(&'a StickyElement),
    Tentacled(&'a TentacledElement),
    Fluffy(&'a FluffyElement),
    Purple(&'a PurpleElement),
}

impl<'a> SchemaElementInfo<'a> {
    #[must_use]
    pub fn id(&self) -> i64 {
        match self {
            Self::UuidMap(element) => element.id,
            Self::UuidGenericMap(element) => element.id,
            Self::Sticky(element) => element.id,
            Self::Tentacled(element) => element.id,
            Self::Fluffy(element) => element.id,
            Self::Purple(element) => element.id,
        }
    }

    #[must_use]
    pub fn name(&self) -> Cow<'a, str> {
        match self {
            Self::UuidMap(element) => Cow::Borrowed(element.name.as_str()),
            Self::UuidGenericMap(element) => Cow::Borrowed(schema_name(&element.name)),
            Self::Sticky(element) => Cow::Borrowed(schema_name(&element.name)),
            Self::Tentacled(element) => Cow::Borrowed(schema_name(&element.name)),
            Self::Fluffy(element) => Cow::Borrowed(schema_name(&element.name)),
            Self::Purple(element) => Cow::Borrowed(schema_name(&element.name)),
        }
    }

    #[must_use]
    pub fn type_id(&self) -> &'a str {
        match self {
            Self::UuidMap(element) => element.type_id.as_str(),
            Self::UuidGenericMap(element) => element.type_id.as_str(),
            Self::Sticky(element) => element.type_id.as_str(),
            Self::Tentacled(element) => element.type_id.as_str(),
            Self::Fluffy(element) => element.type_id.as_str(),
            Self::Purple(element) => element.type_id.as_str(),
        }
    }

    #[must_use]
    pub fn name_crc(&self) -> Option<u32> {
        match self {
            Self::UuidMap(element) => element.name_crc(),
            Self::UuidGenericMap(element) => i64_to_u32(element.name_crc),
            Self::Sticky(element) => i64_to_u32(element.name_crc),
            Self::Tentacled(element) => i64_to_u32(element.name_crc),
            Self::Fluffy(element) => i64_to_u32(element.name_crc),
            Self::Purple(element) => i64_to_u32(element.name_crc),
        }
    }

    #[must_use]
    pub fn data_size(&self) -> Option<u32> {
        match self {
            Self::UuidMap(element) => element.data_size(),
            Self::UuidGenericMap(element) => str_to_u32(&element.data_size),
            Self::Sticky(element) => str_to_u32(&element.data_size),
            Self::Tentacled(element) => str_to_u32(&element.data_size),
            Self::Fluffy(element) => str_to_u32(&element.data_size),
            Self::Purple(element) => str_to_u32(&element.data_size),
        }
    }

    #[must_use]
    pub fn offset(&self) -> Option<u32> {
        match self {
            Self::UuidMap(element) => element.offset(),
            Self::UuidGenericMap(element) => str_to_u32(&element.offset),
            Self::Sticky(element) => str_to_u32(&element.offset),
            Self::Tentacled(element) => str_to_u32(&element.offset),
            Self::Fluffy(element) => str_to_u32(&element.offset),
            Self::Purple(element) => str_to_u32(&element.offset),
        }
    }

    #[must_use]
    pub fn attribute_ownership(&self) -> Option<u32> {
        match self {
            Self::UuidMap(element) => element.attribute_ownership(),
            Self::UuidGenericMap(element) => i64_to_u32(element.attribute_ownership),
            Self::Sticky(element) => i64_to_u32(element.attribute_ownership),
            Self::Tentacled(element) => i64_to_u32(element.attribute_ownership),
            Self::Fluffy(element) => i64_to_u32(element.attribute_ownership),
            Self::Purple(element) => i64_to_u32(element.attribute_ownership),
        }
    }

    #[must_use]
    pub fn flags(&self) -> Option<u32> {
        match self {
            Self::UuidMap(element) => element.flags(),
            Self::UuidGenericMap(element) => i64_to_u32(element.flags),
            Self::Sticky(element) => i64_to_u32(element.flags),
            Self::Tentacled(element) => i64_to_u32(element.flags),
            Self::Fluffy(element) => i64_to_u32(element.flags),
            Self::Purple(element) => i64_to_u32(element.flags),
        }
    }

    #[must_use]
    pub fn is_pointer(&self) -> bool {
        match self {
            Self::UuidMap(element) => element.is_pointer,
            Self::UuidGenericMap(element) => element.is_pointer,
            Self::Sticky(element) => element.is_pointer,
            Self::Tentacled(element) => element.is_pointer,
            Self::Fluffy(element) => element.is_pointer,
            Self::Purple(element) => element.is_pointer,
        }
    }

    #[must_use]
    pub fn is_base_class(&self) -> bool {
        match self {
            Self::UuidMap(element) => element.is_base_class,
            Self::UuidGenericMap(element) => element.is_base_class,
            Self::Sticky(element) => element.is_base_class,
            Self::Tentacled(element) => element.is_base_class,
            Self::Fluffy(element) => element.is_base_class,
            Self::Purple(element) => element.is_base_class,
        }
    }

    #[must_use]
    pub fn no_default_value(&self) -> bool {
        match self {
            Self::UuidMap(element) => element.no_default_value,
            Self::UuidGenericMap(element) => element.no_default_value,
            Self::Sticky(element) => element.no_default_value,
            Self::Tentacled(element) => element.no_default_value,
            Self::Fluffy(element) => element.no_default_value,
            Self::Purple(element) => element.no_default_value,
        }
    }

    #[must_use]
    pub fn is_dynamic_field(&self) -> bool {
        match self {
            Self::UuidMap(element) => element.is_dynamic_field,
            Self::UuidGenericMap(element) => element.is_dynamic_field,
            Self::Sticky(element) => element.is_dynamic_field,
            Self::Tentacled(element) => element.is_dynamic_field,
            Self::Fluffy(element) => element.is_dynamic_field,
            Self::Purple(element) => element.is_dynamic_field,
        }
    }

    #[must_use]
    pub fn is_ui_element(&self) -> bool {
        match self {
            Self::UuidMap(element) => element.is_ui_element,
            Self::UuidGenericMap(element) => element.is_ui_element,
            Self::Sticky(element) => element.is_ui_element,
            Self::Tentacled(element) => element.is_ui_element,
            Self::Fluffy(element) => element.is_ui_element,
            Self::Purple(element) => element.is_ui_element,
        }
    }

    #[must_use]
    pub fn az_rtti(&self) -> Option<SchemaAzRttiInfo<'a>> {
        match self {
            Self::UuidMap(element) => element.az_rtti.as_ref(),
            Self::UuidGenericMap(element) => element.az_rtti.as_ref(),
            Self::Sticky(element) => element.az_rtti.as_ref(),
            Self::Tentacled(element) => element.az_rtti.as_ref(),
            Self::Fluffy(element) => element.az_rtti.as_ref(),
            Self::Purple(element) => element.az_rtti.as_ref(),
        }
        .map(SchemaAzRttiInfo::Element)
    }

    #[must_use]
    pub fn generic_class_info(&self) -> Option<SchemaGenericClassInfo<'a>> {
        match self {
            Self::UuidMap(element) => element
                .generic_class_info
                .as_ref()
                .map(SchemaGenericClassInfo::Purple),
            Self::UuidGenericMap(element) => element
                .generic_class_info
                .as_ref()
                .map(SchemaGenericClassInfo::Sticky),
            Self::Sticky(element) => element
                .generic_class_info
                .as_ref()
                .map(SchemaGenericClassInfo::IndigoRef),
            Self::Tentacled(element) => element
                .generic_class_info
                .as_ref()
                .map(SchemaGenericClassInfo::UuidGenericMap),
            Self::Fluffy(element) => element
                .generic_class_info
                .as_ref()
                .map(SchemaGenericClassInfo::Tentacled),
            Self::Purple(element) => element
                .generic_class_info
                .as_ref()
                .map(SchemaGenericClassInfo::Fluffy),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SchemaAzRttiInfo<'a> {
    UuidMap(&'a UuidMapAzRtti),
    Element(&'a ElementAzRtti),
}

impl<'a> SchemaAzRttiInfo<'a> {
    #[must_use]
    pub fn id(&self) -> Option<i64> {
        match self {
            Self::UuidMap(_) => None,
            Self::Element(rtti) => rtti.id,
        }
    }

    #[must_use]
    pub fn reference(&self) -> Option<&'a str> {
        match self {
            Self::UuidMap(_) => None,
            Self::Element(rtti) => rtti.az_rtti_ref.as_deref(),
        }
        .filter(|value| !value.is_empty())
    }

    #[must_use]
    pub fn address(&self) -> Option<&'a str> {
        match self {
            Self::UuidMap(rtti) => Some(rtti.address.as_str()),
            Self::Element(rtti) => rtti.address.as_deref(),
        }
        .filter(|value| !value.is_empty())
    }

    #[must_use]
    pub fn type_id(&self) -> Option<&'a str> {
        match self {
            Self::UuidMap(rtti) => Some(rtti.type_id.as_str()),
            Self::Element(rtti) => rtti.type_id.as_deref(),
        }
        .filter(|value| !value.is_empty())
    }

    #[must_use]
    pub fn type_name(&self) -> Option<&'a str> {
        match self {
            Self::UuidMap(rtti) => rtti.type_name.as_deref(),
            Self::Element(rtti) => rtti.type_name.as_deref(),
        }
        .filter(|value| !value.is_empty())
    }

    #[must_use]
    pub fn hierarchy(&self) -> Vec<SchemaAzRttiHierarchyEntry<'a>> {
        match self {
            Self::UuidMap(rtti) => rtti.hierarchy.as_deref(),
            Self::Element(rtti) => rtti.hierarchy.as_deref(),
        }
        .unwrap_or(&[])
        .iter()
        .map(|entry| SchemaAzRttiHierarchyEntry {
            type_id: entry.type_id.as_str(),
            type_name: entry.type_name.as_deref().filter(|value| !value.is_empty()),
        })
        .collect()
    }

    #[must_use]
    pub fn is_abstract(&self) -> Option<bool> {
        match self {
            Self::UuidMap(rtti) => Some(rtti.is_abstract),
            Self::Element(rtti) => rtti.is_abstract,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchemaAzRttiHierarchyEntry<'a> {
    pub type_id: &'a str,
    pub type_name: Option<&'a str>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SchemaNonTypeTemplateArguments<'a> {
    None,
    Json(&'a serde_json::Value),
    Purple(&'a PurpleNonTypeTemplateArguments),
    Fluffy(&'a FluffyNonTypeTemplateArguments),
}

impl<'a> SchemaNonTypeTemplateArguments<'a> {
    #[must_use]
    pub fn entries(&self) -> Vec<(Cow<'a, str>, SchemaNonTypeTemplateArgument<'a>)> {
        match self {
            Self::None => Vec::new(),
            Self::Json(value) => value
                .as_object()
                .map(|args| {
                    args.iter()
                        .map(|(key, value)| {
                            (
                                Cow::Owned(key.clone()),
                                SchemaNonTypeTemplateArgument::from_json(value),
                            )
                        })
                        .collect()
                })
                .unwrap_or_default(),
            Self::Purple(value) => vec![(
                Cow::Borrowed("capacity"),
                SchemaNonTypeTemplateArgument::I64(value.capacity),
            )],
            Self::Fluffy(value) => {
                let mut entries = Vec::new();
                if let Some(capacity) = value.capacity {
                    entries.push((
                        Cow::Borrowed("capacity"),
                        SchemaNonTypeTemplateArgument::I64(capacity),
                    ));
                }
                if let Some(values) = &value.values {
                    entries.push((
                        Cow::Borrowed("values"),
                        SchemaNonTypeTemplateArgument::Array(
                            values
                                .iter()
                                .map(SchemaNonTypeTemplateArgument::from_class_name_to_uuid)
                                .collect(),
                        ),
                    ));
                }
                entries
            }
        }
    }

    #[must_use]
    pub fn capacity(&self) -> Option<u64> {
        match self {
            Self::None => None,
            Self::Json(value) => value
                .get("capacity")
                .and_then(serde_json::Value::as_u64)
                .or_else(|| {
                    value
                        .get("capacity")
                        .and_then(serde_json::Value::as_i64)
                        .and_then(|value| u64::try_from(value).ok())
                }),
            Self::Purple(value) => u64::try_from(value.capacity).ok(),
            Self::Fluffy(value) => value.capacity.and_then(|value| u64::try_from(value).ok()),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SchemaNonTypeTemplateArgument<'a> {
    Null,
    Bool(bool),
    U64(u64),
    I64(i64),
    F64(f64),
    String(Cow<'a, str>),
    Array(Vec<SchemaNonTypeTemplateArgument<'a>>),
}

impl<'a> SchemaNonTypeTemplateArgument<'a> {
    #[must_use]
    pub fn from_json(value: &'a serde_json::Value) -> Self {
        Self::from_json_with_depth(value, 0)
    }

    fn from_json_with_depth(value: &'a serde_json::Value, depth: usize) -> Self {
        if depth >= MAX_SCHEMA_VALUE_DEPTH {
            return Self::String(Cow::Owned(value.to_string()));
        }
        match value {
            serde_json::Value::Null => Self::Null,
            serde_json::Value::Bool(value) => Self::Bool(*value),
            serde_json::Value::Number(value) => value
                .as_u64()
                .map(Self::U64)
                .or_else(|| value.as_i64().map(Self::I64))
                .or_else(|| value.as_f64().map(Self::F64))
                .unwrap_or(Self::Null),
            serde_json::Value::String(value) => value
                .strip_prefix("0x")
                .and_then(|value| u64::from_str_radix(value, 16).ok())
                .or_else(|| value.parse::<u64>().ok())
                .map(Self::U64)
                .unwrap_or_else(|| Self::String(Cow::Borrowed(value))),
            serde_json::Value::Array(values) => Self::Array(
                values
                    .iter()
                    .map(|value| Self::from_json_with_depth(value, depth + 1))
                    .collect(),
            ),
            serde_json::Value::Object(_) => Self::String(Cow::Owned(value.to_string())),
        }
    }

    #[must_use]
    pub fn from_class_name_to_uuid(value: &'a ClassNameToUuid) -> Self {
        match value {
            ClassNameToUuid::Integer(value) => Self::I64(*value),
            ClassNameToUuid::String(value) => value
                .strip_prefix("0x")
                .and_then(|value| u64::from_str_radix(value, 16).ok())
                .or_else(|| value.parse::<u64>().ok())
                .map(Self::U64)
                .unwrap_or_else(|| Self::String(Cow::Borrowed(value))),
        }
    }
}

fn value_str(value: Option<&serde_json::Value>) -> Option<&str> {
    value
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
}

fn i64_to_u32(value: i64) -> Option<u32> {
    u32::try_from(value).ok()
}

fn str_to_u32(value: &str) -> Option<u32> {
    value.parse().ok()
}

fn legacy_specialized_type_id_str(value: &LegacySpecializedTypeId) -> &'static str {
    match value {
        LegacySpecializedTypeId::The00Eb73A2F67F00003Dd0F4A9F67F0000 => {
            "00EB73A2-F67F-0000-3DD0-F4A9F67F0000"
        }
        LegacySpecializedTypeId::The3Dd0F4A9F67F00003Dd0F4A9F67F0000 => {
            "3DD0F4A9-F67F-0000-3DD0-F4A9F67F0000"
        }
    }
}

fn schema_name(value: &Name) -> &'static str {
    match value {
        Name::Element => "element",
        Name::NameValue1 | Name::Value1 => "value1",
        Name::NameValue2 | Name::Value2 => "value2",
        Name::Value => "value",
        Name::Value3 => "value3",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_typed_top_level_indexes_without_raw_json_callers() {
        let schema = parse(
            br#"{
                "$id": 1,
                "uuidMap": {
                    "11111111-1111-1111-1111-111111111111": {
                        "$id": 10,
                        "name": "Example::CounterComponent",
                        "typeId": "11111111-1111-1111-1111-111111111111",
                        "version": 1,
                        "doSave": null,
                        "dataConverter": null,
                        "editData": null,
                        "elements": [{
                            "$id": 11,
                            "name": "m_count",
                            "nameCrc": 1,
                            "typeId": "43DA906B-7DEF-4CA8-9790-854106D3F983",
                            "dataSize": "4",
                            "offset": "0",
                            "attributeOwnership": 0,
                            "flags": 0,
                            "is_pointer": false,
                            "is_base_class": false,
                            "no_default_value": false,
                            "is_dynamic_field": false,
                            "is_ui_element": false,
                            "editData": null,
                            "attributes": []
                        }],
                        "attributes": []
                    }
                },
                "classNameToUuid": [[123, "11111111-1111-1111-1111-111111111111"]],
                "uuidGenericMap": [],
                "uuidAnyCreationMap": {
                    "11111111-1111-1111-1111-111111111111": "0x1234"
                },
                "editContext": {"$id": 2, "classData": [], "enumData": []},
                "enumTypeIdToUnderlyingTypeIdMap": {}
            }"#,
        )
        .expect("schema");

        let class = schema.uuid_map_entries().next().expect("uuid map class").1;
        assert_eq!(class.name(), "Example::CounterComponent");
        assert_eq!(class.version(), Some(1));
        assert_eq!(class.elements()[0].name(), "m_count");
        assert_eq!(class.elements()[0].data_size(), Some(4));
        assert_eq!(schema.class_name_entries().next().unwrap().name_crc, 123);
        assert_eq!(
            schema.uuid_any_creation_entries().next(),
            Some(("11111111-1111-1111-1111-111111111111", "0x1234"))
        );
    }

    #[test]
    fn exposes_normalized_generic_class_info_from_generated_schema() {
        let schema = parse(
            br#"{
                "$id": 1,
                "uuidMap": {
                    "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA": {
                        "$id": 10,
                        "name": "Owner",
                        "typeId": "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
                        "version": 0,
                        "doSave": null,
                        "dataConverter": null,
                        "editData": null,
                        "elements": [{
                            "$id": 11,
                            "name": "m_values",
                            "nameCrc": 1,
                            "typeId": "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB",
                            "dataSize": "24",
                            "offset": "0",
                            "attributeOwnership": 0,
                            "flags": 0,
                            "is_pointer": false,
                            "is_base_class": false,
                            "no_default_value": false,
                            "is_dynamic_field": false,
                            "is_ui_element": false,
                            "genericClassInfo": {
                                "$id": 30,
                                "typeId": "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB",
                                "registeredTypeIds": ["BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB"],
                                "templatedArgumentCount": 1,
                                "templatedTypeIds": ["43DA906B-7DEF-4CA8-9790-854106D3F983"],
                                "typeIdFoldTypeIds": null,
                                "specializedTypeId": "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB",
                                "genericTypeId": "2BADE35A-6F1B-4698-B2BC-3373D010020C",
                                "legacySpecializedTypeId": null,
                                "nonTypeTemplateArguments": {"capacity": 4},
                                "classData": {
                                    "$id": 31,
                                    "name": "AZStd::fixed_vector",
                                    "typeId": "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB",
                                    "version": 0,
                                    "doSave": null,
                                    "dataConverter": null,
                                    "editData": null,
                                    "elements": [],
                                    "attributes": []
                                },
                                "elements": []
                            },
                            "editData": null,
                            "attributes": []
                        }],
                        "attributes": []
                    }
                },
                "classNameToUuid": [],
                "uuidGenericMap": [[
                    "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB",
                    {
                        "$id": 40,
                        "typeId": "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB",
                        "registeredTypeIds": ["BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB"],
                        "templatedArgumentCount": 1,
                        "templatedTypeIds": ["43DA906B-7DEF-4CA8-9790-854106D3F983"],
                        "typeIdFoldTypeIds": null,
                        "specializedTypeId": "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB",
                        "genericTypeId": "2BADE35A-6F1B-4698-B2BC-3373D010020C",
                        "legacySpecializedTypeId": null,
                        "nonTypeTemplateArguments": {"capacity": 8},
                        "classData": {
                            "$id": 41,
                            "name": "AZStd::fixed_vector",
                            "typeId": "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB",
                            "version": 0,
                            "doSave": null,
                            "dataConverter": null,
                            "editData": null,
                            "elements": [],
                            "attributes": []
                        },
                        "elements": []
                    }
                ]],
                "uuidAnyCreationMap": {},
                "editContext": {"$id": 2, "classData": [], "enumData": []},
                "enumTypeIdToUnderlyingTypeIdMap": {}
            }"#,
        )
        .expect("schema");

        let top_level = schema
            .uuid_generic_class_entries()
            .next()
            .expect("top-level generic")
            .1;
        assert_eq!(top_level.id(), Some(40));
        assert_eq!(top_level.templated_argument_count(), Some(1));
        assert_eq!(top_level.non_type_template_arguments().capacity(), Some(8));
        assert_eq!(
            top_level.class_data().map(UuidMap::name),
            Some("AZStd::fixed_vector")
        );

        let member_generic = schema
            .uuid_map_entries()
            .next()
            .expect("class")
            .1
            .elements()[0]
            .generic_class_info()
            .expect("member generic");
        assert_eq!(member_generic.id(), Some(30));
        assert_eq!(
            member_generic.templated_type_ids(),
            ["43DA906B-7DEF-4CA8-9790-854106D3F983"]
        );
        assert_eq!(
            member_generic.non_type_template_arguments().capacity(),
            Some(4)
        );
    }

    #[test]
    fn non_type_json_argument_conversion_is_depth_bounded() {
        let mut value = serde_json::json!(5);
        for _ in 0..(MAX_SCHEMA_VALUE_DEPTH + 8) {
            value = serde_json::json!([value]);
        }

        let parsed = SchemaNonTypeTemplateArgument::from_json(&value);

        assert!(contains_depth_limit_string(&parsed));
    }

    fn contains_depth_limit_string(value: &SchemaNonTypeTemplateArgument<'_>) -> bool {
        match value {
            SchemaNonTypeTemplateArgument::String(value) => value.contains('['),
            SchemaNonTypeTemplateArgument::Array(values) => {
                values.iter().any(contains_depth_limit_string)
            }
            _ => false,
        }
    }
}
