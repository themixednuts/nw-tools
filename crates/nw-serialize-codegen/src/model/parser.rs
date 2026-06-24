use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;
use uuid::Uuid;

use super::{
    ClassNameIndexEntry, ReflectedAttribute, ReflectedAttributeValue, ReflectedAzRtti,
    ReflectedAzRttiHierarchyEntry, ReflectedClass, ReflectedEnum, ReflectedEnumVariant,
    ReflectedGenericClass, ReflectedMember, ReflectedNonTypeTemplateArgument,
    SerializeContextModel,
};
use crate::document::SerializeContextDocument;
use crate::reference::{ReferenceIndex, ReferenceKey};
use crate::schema::{
    SchemaAzRttiInfo, SchemaElementInfo, SchemaGenericClassInfo, SchemaNonTypeTemplateArgument,
    SchemaNonTypeTemplateArguments, SerializeContext, UuidMap,
};
use crate::value::{
    non_empty_str, parse_uuid, reference_id, value_f64, value_i32, value_u32, value_u64, value_uuid,
};

pub(super) fn parse_document(document: &SerializeContextDocument) -> SerializeContextModel {
    let refs = ReferenceIndex::new(document.root());
    let mut parser = ModelParser {
        refs: &refs,
        model: SerializeContextModel::default(),
        generic_parse_stack: BTreeSet::new(),
        generic_parse_depth: 0,
    };
    if let Some(schema) = document.schema() {
        parser.parse_schema_document(document.root(), schema);
    } else {
        parser.parse(document.root());
    }
    parser.model
}

pub(super) fn parse_root(root: &Value) -> SerializeContextModel {
    let refs = ReferenceIndex::new(root);
    let mut parser = ModelParser {
        refs: &refs,
        model: SerializeContextModel::default(),
        generic_parse_stack: BTreeSet::new(),
        generic_parse_depth: 0,
    };
    parser.parse(root);
    parser.model
}

const MAX_GENERIC_PARSE_DEPTH: usize = 64;
const MAX_VALUE_PARSE_DEPTH: usize = 64;

struct ModelParser<'a> {
    refs: &'a ReferenceIndex<'a>,
    model: SerializeContextModel,
    generic_parse_stack: BTreeSet<ReferenceKey>,
    generic_parse_depth: usize,
}

#[derive(Debug, Default)]
struct ValueParseState {
    reference_stack: BTreeSet<ReferenceKey>,
    depth: usize,
}

impl ValueParseState {
    fn enter(&mut self, value: &Value) -> bool {
        if self.depth >= MAX_VALUE_PARSE_DEPTH {
            return false;
        }
        let reference_id = value.get("$id").and_then(reference_id);
        if let Some(reference_id) = reference_id.as_ref()
            && !self.reference_stack.insert(reference_id.clone())
        {
            return false;
        }
        self.depth += 1;
        true
    }

    fn exit(&mut self, value: &Value) {
        if self.depth > 0 {
            self.depth -= 1;
        }
        if let Some(reference_id) = value.get("$id").and_then(reference_id) {
            self.reference_stack.remove(&reference_id);
        }
    }
}

impl<'a> ModelParser<'a> {
    fn parse(&mut self, root: &'a Value) {
        self.parse_classes(root);
        self.parse_class_name_index(root);
        self.parse_generic_classes(root);
        self.parse_enums(root);
        self.parse_any_creators(root);
        self.parse_enum_underlying_types(root);
    }

    fn parse_schema_document(&mut self, root: &'a Value, schema: &SerializeContext) {
        self.parse_schema_classes(root, schema);
        self.parse_schema_generic_classes(root, schema);
        self.parse_enums(root);
        self.parse_schema_indexes(schema);
        self.parse_enum_underlying_types(root);
    }

    fn parse_schema_indexes(&mut self, schema: &SerializeContext) {
        self.model.class_name_index = schema
            .class_name_entries()
            .map(|entry| ClassNameIndexEntry {
                name_crc: u32::try_from(entry.name_crc).ok(),
                type_id: parse_uuid(entry.type_id),
            })
            .collect();
        self.model.any_creators = schema
            .uuid_any_creation_entries()
            .filter_map(|(type_id, creator)| Some((parse_uuid(type_id)?, creator.to_owned())))
            .collect();
    }

    fn parse_schema_classes(&mut self, root: &'a Value, schema: &SerializeContext) {
        let raw_uuid_map = self.field(root, "uuidMap").and_then(Value::as_object);
        for (key, class_data) in schema.uuid_map_entries() {
            let raw_class = raw_uuid_map
                .and_then(|uuid_map| uuid_map.get(key))
                .map(|class_data| self.refs.resolve(class_data));
            let map_key_type_id = parse_uuid(key);
            if let Some(class) = self.parse_schema_class(class_data, raw_class, map_key_type_id) {
                self.model.classes.entry(class.type_id).or_insert(class);
            }
        }
    }

    fn parse_schema_class(
        &mut self,
        class_data: &UuidMap,
        raw_class: Option<&'a Value>,
        map_key_type_id: Option<Uuid>,
    ) -> Option<ReflectedClass> {
        let type_id = parse_uuid(class_data.type_id()).or(map_key_type_id)?;
        Some(ReflectedClass {
            reference_id: schema_reference_id(class_data.id()),
            map_key_type_id,
            name: class_data.name().to_owned(),
            type_id,
            version: class_data.version(),
            factory: class_data.factory().map(str::to_owned),
            persistent_id: class_data.persistent_id().map(str::to_owned),
            do_save: class_data.do_save().map(str::to_owned),
            serializer: class_data.serializer().map(str::to_owned),
            az_rtti: class_data.az_rtti().and_then(reflected_schema_az_rtti),
            container: class_data.container().map(str::to_owned),
            converter: class_data.converter().map(str::to_owned),
            data_converter: class_data.data_converter().map(str::to_owned),
            event_handler: class_data.event_handler().map(str::to_owned),
            members: self.parse_schema_members(class_data, raw_class),
            attributes: raw_class
                .map(|raw_class| self.parse_attributes(raw_class.get("attributes")))
                .unwrap_or_default(),
        })
    }

    fn parse_schema_members(
        &mut self,
        class_data: &UuidMap,
        raw_class: Option<&'a Value>,
    ) -> Vec<ReflectedMember> {
        let raw_elements = raw_class
            .and_then(|raw_class| raw_class.get("elements"))
            .map(|elements| self.refs.resolve(elements))
            .and_then(Value::as_array);
        class_data
            .elements()
            .iter()
            .enumerate()
            .filter_map(|(index, member)| {
                self.parse_schema_element(
                    SchemaElementInfo::UuidMap(member),
                    raw_schema_member(self.refs, raw_elements, index, member.id()),
                )
            })
            .collect()
    }

    fn parse_schema_element(
        &mut self,
        member: SchemaElementInfo<'_>,
        raw_member: Option<&'a Value>,
    ) -> Option<ReflectedMember> {
        let type_id = parse_uuid(member.type_id())?;
        let raw_member = raw_member.map(|member| self.refs.resolve(member));
        let raw_generic = raw_member.and_then(|member| member.get("genericClassInfo"));
        let generic_class = member
            .generic_class_info()
            .and_then(|generic| self.parse_schema_generic_class(generic, raw_generic, None))
            .or_else(|| raw_generic.and_then(|generic| self.parse_generic_class(generic, None)))
            .map(|generic| {
                self.register_generic(generic.clone());
                Box::new(generic)
            });
        Some(ReflectedMember {
            reference_id: schema_reference_id(member.id()),
            name: member.name().into_owned(),
            name_crc: member.name_crc(),
            type_id,
            data_size: member.data_size(),
            offset: member.offset(),
            attribute_ownership: member.attribute_ownership(),
            flags: member.flags(),
            is_pointer: member.is_pointer(),
            is_base_class: member.is_base_class(),
            no_default_value: member.no_default_value(),
            is_dynamic_field: member.is_dynamic_field(),
            is_ui_element: member.is_ui_element(),
            az_rtti: member.az_rtti().and_then(reflected_schema_az_rtti),
            generic_class,
            attributes: raw_member
                .map(|member| self.parse_attributes(member.get("attributes")))
                .unwrap_or_default(),
        })
    }

    fn parse_classes(&mut self, root: &'a Value) {
        let Some(uuid_map) = root.get("uuidMap").and_then(Value::as_object) else {
            return;
        };
        for (key, class_data) in uuid_map {
            let map_key_type_id = parse_uuid(key);
            if let Some(class) = self.parse_class(class_data, map_key_type_id) {
                self.model.classes.entry(class.type_id).or_insert(class);
            }
        }
    }

    fn parse_class_name_index(&mut self, root: &'a Value) {
        let Some(entries) = root.get("classNameToUuid").and_then(Value::as_array) else {
            return;
        };
        self.model.class_name_index = entries
            .iter()
            .filter_map(|entry| {
                let (name_crc, type_id) = self.pair(entry)?;
                Some(ClassNameIndexEntry {
                    name_crc: value_u32(name_crc),
                    type_id: value_uuid(type_id),
                })
            })
            .collect();
    }

    fn parse_generic_classes(&mut self, root: &'a Value) {
        let Some(uuid_generic_map) = self.field(root, "uuidGenericMap").and_then(Value::as_array)
        else {
            return;
        };
        for pair in uuid_generic_map {
            let Some((key, generic_data)) = self.string_pair(pair) else {
                continue;
            };
            let map_key_type_id = parse_uuid(key);
            if let Some(generic) = self.parse_generic_class(generic_data, map_key_type_id) {
                self.register_generic(generic);
            }
        }
    }

    fn parse_schema_generic_classes(&mut self, root: &'a Value, schema: &SerializeContext) {
        let raw_uuid_generic_map = self.field(root, "uuidGenericMap").and_then(Value::as_array);
        for (key, generic_data) in schema.uuid_generic_class_entries() {
            let raw_generic = raw_schema_generic_class(self.refs, raw_uuid_generic_map, key);
            let map_key_type_id = parse_uuid(key);
            if let Some(generic) =
                self.parse_schema_generic_class(generic_data, raw_generic, map_key_type_id)
            {
                self.register_generic(generic);
            }
        }
    }

    fn parse_enums(&mut self, root: &'a Value) {
        let Some(enum_data) = root
            .get("editContext")
            .map(|value| self.refs.resolve(value))
            .and_then(|value| self.field(value, "enumData"))
            .and_then(Value::as_array)
        else {
            return;
        };
        for pair in enum_data {
            let Some((type_id, enum_data)) = self.string_pair(pair) else {
                continue;
            };
            let Some(type_id) = parse_uuid(type_id) else {
                continue;
            };
            if let Some(enumeration) = self.parse_enum(enum_data, type_id) {
                self.model.enums.insert(type_id, enumeration);
            }
        }
    }

    fn parse_any_creators(&mut self, root: &'a Value) {
        let Some(creators) = self
            .field(root, "uuidAnyCreationMap")
            .and_then(Value::as_object)
        else {
            return;
        };
        self.model.any_creators = creators
            .iter()
            .filter_map(|(type_id, address)| {
                Some((
                    parse_uuid(type_id)?,
                    non_empty_str(self.refs.resolve(address))?.to_owned(),
                ))
            })
            .collect();
    }

    fn parse_enum_underlying_types(&mut self, root: &'a Value) {
        let Some(entries) = root
            .get("enumTypeIdToUnderlyingTypeIdMap")
            .map(|value| self.refs.resolve(value))
            .and_then(Value::as_object)
        else {
            return;
        };
        self.model.enum_underlying_types = entries
            .iter()
            .filter_map(|(enum_type_id, underlying_type_id)| {
                Some((
                    parse_uuid(enum_type_id)?,
                    value_uuid(self.refs.resolve(underlying_type_id))?,
                ))
            })
            .collect();
    }

    fn parse_class(
        &mut self,
        class_data: &'a Value,
        map_key_type_id: Option<Uuid>,
    ) -> Option<ReflectedClass> {
        let class_data = self.refs.resolve(class_data);
        let type_id = class_data
            .get("typeId")
            .map(|value| self.refs.resolve(value))
            .and_then(value_uuid)
            .or(map_key_type_id)?;
        let name = self
            .field(class_data, "name")
            .and_then(non_empty_str)?
            .to_owned();
        Some(ReflectedClass {
            reference_id: class_data.get("$id").and_then(reference_id),
            map_key_type_id,
            name,
            type_id,
            version: self.field(class_data, "version").and_then(value_u32),
            factory: self.optional_string(class_data, "factory"),
            persistent_id: self.optional_string(class_data, "persistentId"),
            do_save: self.optional_string(class_data, "doSave"),
            serializer: self.optional_string(class_data, "serializer"),
            az_rtti: self.parse_az_rtti(class_data.get("azRtti")),
            container: self.optional_string(class_data, "container"),
            converter: self.optional_string(class_data, "converter"),
            data_converter: self.optional_string(class_data, "dataConverter"),
            event_handler: self.optional_string(class_data, "eventHandler"),
            members: self.parse_members(class_data),
            attributes: self.parse_attributes(class_data.get("attributes")),
        })
    }

    fn parse_members(&mut self, owner: &'a Value) -> Vec<ReflectedMember> {
        owner
            .get("elements")
            .map(|value| self.refs.resolve(value))
            .and_then(Value::as_array)
            .map(|elements| {
                elements
                    .iter()
                    .filter_map(|element| self.parse_member(element))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn parse_member(&mut self, member: &'a Value) -> Option<ReflectedMember> {
        let member = self.refs.resolve(member);
        let type_id = self.field(member, "typeId").and_then(value_uuid)?;
        let generic_class = member
            .get("genericClassInfo")
            .and_then(|generic| self.parse_generic_class(generic, None))
            .map(|generic| {
                self.register_generic(generic.clone());
                Box::new(generic)
            });
        Some(ReflectedMember {
            reference_id: member.get("$id").and_then(reference_id),
            name: member
                .get("name")
                .map(|value| self.refs.resolve(value))
                .and_then(non_empty_str)
                .unwrap_or("field")
                .to_owned(),
            name_crc: self.field(member, "nameCrc").and_then(value_u32),
            type_id,
            data_size: self.field(member, "dataSize").and_then(value_u32),
            offset: self.field(member, "offset").and_then(value_u32),
            attribute_ownership: self.field(member, "attributeOwnership").and_then(value_u32),
            flags: self.field(member, "flags").and_then(value_u32),
            is_pointer: self.bool_field(member, "is_pointer"),
            is_base_class: self.bool_field(member, "is_base_class"),
            no_default_value: self.bool_field(member, "no_default_value"),
            is_dynamic_field: self.bool_field(member, "is_dynamic_field"),
            is_ui_element: self.bool_field(member, "is_ui_element"),
            az_rtti: self.parse_az_rtti(member.get("azRtti")),
            generic_class,
            attributes: self.parse_attributes(member.get("attributes")),
        })
    }

    fn parse_generic_class(
        &mut self,
        generic_data: &'a Value,
        map_key_type_id: Option<Uuid>,
    ) -> Option<ReflectedGenericClass> {
        let generic_data = self.refs.resolve(generic_data);
        if generic_data.is_null() {
            return None;
        }
        let reference_id = generic_data.get("$id").and_then(reference_id);
        if !self.enter_generic_parse(reference_id.as_ref()) {
            return None;
        }
        let class_data = generic_data
            .get("classData")
            .map(|value| self.refs.resolve(value));
        let generic = ReflectedGenericClass {
            reference_id: reference_id.clone(),
            map_key_type_id,
            type_id: self.field(generic_data, "typeId").and_then(value_uuid),
            registered_type_ids: self.uuid_array(generic_data.get("registeredTypeIds")),
            templated_argument_count: generic_data
                .get("templatedArgumentCount")
                .map(|value| self.refs.resolve(value))
                .and_then(value_u32),
            templated_type_ids: self.uuid_array(generic_data.get("templatedTypeIds")),
            type_id_fold_type_ids: self.uuid_array(generic_data.get("typeIdFoldTypeIds")),
            specialized_type_id: self
                .field(generic_data, "specializedTypeId")
                .and_then(value_uuid),
            generic_type_id: self
                .field(generic_data, "genericTypeId")
                .and_then(value_uuid),
            legacy_specialized_type_id: generic_data
                .get("legacySpecializedTypeId")
                .map(|value| self.refs.resolve(value))
                .and_then(value_uuid),
            non_type_template_arguments: generic_data
                .get("nonTypeTemplateArguments")
                .and_then(|value| self.refs.resolve(value).as_object())
                .map(|args| {
                    args.iter()
                        .map(|(key, value)| {
                            (
                                key.clone(),
                                self.non_type_template_argument(
                                    value,
                                    &mut ValueParseState::default(),
                                ),
                            )
                        })
                        .collect()
                })
                .unwrap_or_default(),
            class_type_id: class_data
                .and_then(|value| value.get("typeId"))
                .map(|value| self.refs.resolve(value))
                .and_then(value_uuid),
            class_name: class_data
                .and_then(|value| value.get("name"))
                .map(|value| self.refs.resolve(value))
                .and_then(non_empty_str)
                .map(str::to_owned),
            members: self.parse_members(generic_data),
        };
        self.exit_generic_parse(reference_id.as_ref());
        Some(generic)
    }

    fn parse_schema_generic_class(
        &mut self,
        generic_data: SchemaGenericClassInfo<'_>,
        raw_generic: Option<&'a Value>,
        map_key_type_id: Option<Uuid>,
    ) -> Option<ReflectedGenericClass> {
        let raw_generic = raw_generic
            .map(|generic| self.refs.resolve(generic))
            .or_else(|| {
                generic_data
                    .reference()
                    .and_then(|reference| self.refs.resolve_reference(reference))
            });

        if schema_generic_is_reference_only(generic_data) {
            return raw_generic
                .and_then(|generic| self.parse_generic_class(generic, map_key_type_id));
        }

        let reference_id = generic_data.id().and_then(schema_reference_id);
        if !self.enter_generic_parse(reference_id.as_ref()) {
            return None;
        }
        let generic = ReflectedGenericClass {
            reference_id: reference_id.clone(),
            map_key_type_id,
            type_id: generic_data.type_id().and_then(parse_uuid),
            registered_type_ids: parse_uuid_strings(generic_data.registered_type_ids()),
            templated_argument_count: generic_data.templated_argument_count(),
            templated_type_ids: parse_uuid_strings(generic_data.templated_type_ids()),
            type_id_fold_type_ids: parse_uuid_strings(generic_data.type_id_fold_type_ids()),
            specialized_type_id: generic_data.specialized_type_id().and_then(parse_uuid),
            generic_type_id: generic_data.generic_type_id().and_then(parse_uuid),
            legacy_specialized_type_id: generic_data
                .legacy_specialized_type_id()
                .as_deref()
                .and_then(parse_uuid),
            non_type_template_arguments: self
                .schema_non_type_template_arguments(generic_data.non_type_template_arguments()),
            class_type_id: generic_data
                .class_data()
                .and_then(|class_data| parse_uuid(class_data.type_id())),
            class_name: generic_data
                .class_data()
                .map(|class_data| class_data.name().to_owned()),
            members: generic_data
                .elements()
                .into_iter()
                .filter_map(|element| self.parse_schema_element(element, None))
                .collect(),
        };
        self.exit_generic_parse(reference_id.as_ref());
        Some(generic)
    }

    fn enter_generic_parse(&mut self, reference_id: Option<&ReferenceKey>) -> bool {
        if self.generic_parse_depth >= MAX_GENERIC_PARSE_DEPTH {
            return false;
        }
        if let Some(reference_id) = reference_id
            && !self.generic_parse_stack.insert(reference_id.clone())
        {
            return false;
        }
        self.generic_parse_depth += 1;
        true
    }

    fn exit_generic_parse(&mut self, reference_id: Option<&ReferenceKey>) {
        if self.generic_parse_depth > 0 {
            self.generic_parse_depth -= 1;
        }
        if let Some(reference_id) = reference_id {
            self.generic_parse_stack.remove(reference_id);
        }
    }

    fn register_generic(&mut self, generic: ReflectedGenericClass) {
        for concrete_type_id in generic.concrete_type_ids().collect::<Vec<_>>() {
            self.model
                .generic_classes
                .entry(concrete_type_id)
                .or_insert_with(|| generic.clone());
        }
    }

    fn parse_enum(&self, enum_data: &'a Value, type_id: Uuid) -> Option<ReflectedEnum> {
        let enum_data = self.refs.resolve(enum_data);
        Some(ReflectedEnum {
            reference_id: enum_data.get("$id").and_then(reference_id),
            type_id,
            element_id: self.field(enum_data, "elementId").and_then(value_u32),
            name: self
                .field(enum_data, "name")
                .and_then(non_empty_str)?
                .to_owned(),
            description: self.optional_string(enum_data, "description"),
            deprecated_name: self.optional_string(enum_data, "deprecatedName"),
            variants: self.enum_variants(enum_data),
        })
    }

    fn enum_variants(&self, enum_data: &'a Value) -> Vec<ReflectedEnumVariant> {
        self.parse_attributes(enum_data.get("attributes"))
            .into_iter()
            .filter_map(|attribute| {
                let value = attribute.value?;
                Some(ReflectedEnumVariant {
                    name: value.description.unwrap_or_else(|| "Unnamed".to_owned()),
                    value_u64: value.value_u64.or_else(|| value.value_u32.map(u64::from)),
                    value_u32: value.value_u32,
                    value_i32: value.value_i32,
                })
            })
            .collect()
    }

    fn parse_attributes(&self, attributes: Option<&'a Value>) -> Vec<ReflectedAttribute> {
        attributes
            .map(|value| self.refs.resolve(value))
            .and_then(Value::as_array)
            .map(|attributes| {
                attributes
                    .iter()
                    .filter_map(|attribute| self.parse_attribute_pair(attribute))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn parse_attribute_pair(&self, attribute: &'a Value) -> Option<ReflectedAttribute> {
        let pair = attribute.as_array()?;
        let [attribute_id, attribute] = pair.as_slice() else {
            return None;
        };
        let attribute = self.refs.resolve(attribute);
        Some(ReflectedAttribute {
            reference_id: attribute.get("$id").and_then(reference_id),
            attribute_id: self
                .field(attribute, "attributeId")
                .and_then(value_u32)
                .or_else(|| value_u32(self.refs.resolve(attribute_id))),
            name: self.optional_string(attribute, "attributeName"),
            describes_children: self
                .field(attribute, "describesChildren")
                .and_then(Value::as_bool),
            child_class_owned: self
                .field(attribute, "childClassOwned")
                .and_then(Value::as_bool),
            value: attribute
                .get("value")
                .map(|value| self.parse_attribute_value(value)),
        })
    }

    fn parse_attribute_value(&self, value: &'a Value) -> ReflectedAttributeValue {
        let value = self.refs.resolve(value);
        let scalar_value = value.get("value").map(|value| self.refs.resolve(value));
        ReflectedAttributeValue {
            kind: self.optional_string(value, "kind"),
            description: self.optional_string(value, "description"),
            function: self.optional_string(value, "function"),
            member_function: self.optional_string(value, "memberFunction"),
            value_string: scalar_value.and_then(non_empty_str).map(str::to_owned),
            value_bool: scalar_value.and_then(Value::as_bool),
            value_u64: self.field(value, "valueU64").and_then(value_u64),
            value_u32: self.field(value, "valueU32").and_then(value_u32),
            value_i32: self.field(value, "valueI32").and_then(value_i32),
            value_high_u32: self.field(value, "valueHighU32").and_then(value_u32),
            value_f32: self.field(value, "valueF32").and_then(value_f64),
            value_high_f32: self.field(value, "valueHighF32").and_then(value_f64),
        }
    }

    fn schema_non_type_template_arguments(
        &self,
        arguments: SchemaNonTypeTemplateArguments<'a>,
    ) -> BTreeMap<String, ReflectedNonTypeTemplateArgument> {
        match arguments {
            SchemaNonTypeTemplateArguments::None => BTreeMap::new(),
            SchemaNonTypeTemplateArguments::Json(value) => self
                .refs
                .resolve(value)
                .as_object()
                .map(|args| {
                    args.iter()
                        .map(|(key, value)| {
                            (
                                key.clone(),
                                self.non_type_template_argument(
                                    value,
                                    &mut ValueParseState::default(),
                                ),
                            )
                        })
                        .collect()
                })
                .unwrap_or_default(),
            arguments => arguments
                .entries()
                .into_iter()
                .map(|(key, value)| {
                    (
                        key.into_owned(),
                        reflected_non_type_template_argument(value),
                    )
                })
                .collect(),
        }
    }

    fn non_type_template_argument(
        &self,
        value: &'a Value,
        state: &mut ValueParseState,
    ) -> ReflectedNonTypeTemplateArgument {
        let value = self.refs.resolve(value);
        if !state.enter(value) {
            return ReflectedNonTypeTemplateArgument::String(value.to_string());
        }
        let parsed = match value {
            Value::Null => ReflectedNonTypeTemplateArgument::Null,
            Value::Bool(value) => ReflectedNonTypeTemplateArgument::Bool(*value),
            Value::Number(value) => value
                .as_u64()
                .map(ReflectedNonTypeTemplateArgument::U64)
                .or_else(|| value.as_i64().map(ReflectedNonTypeTemplateArgument::I64))
                .or_else(|| value.as_f64().map(ReflectedNonTypeTemplateArgument::F64))
                .unwrap_or(ReflectedNonTypeTemplateArgument::Null),
            Value::String(value) => value
                .strip_prefix("0x")
                .and_then(|value| u64::from_str_radix(value, 16).ok())
                .or_else(|| value.parse::<u64>().ok())
                .map(ReflectedNonTypeTemplateArgument::U64)
                .unwrap_or_else(|| ReflectedNonTypeTemplateArgument::String(value.clone())),
            Value::Array(values) => ReflectedNonTypeTemplateArgument::Array(
                values
                    .iter()
                    .map(|value| self.non_type_template_argument(value, state))
                    .collect(),
            ),
            Value::Object(_) => ReflectedNonTypeTemplateArgument::String(value.to_string()),
        };
        state.exit(value);
        parsed
    }

    fn field(&self, value: &'a Value, key: &str) -> Option<&'a Value> {
        value.get(key).map(|value| self.refs.resolve(value))
    }

    fn pair(&self, value: &'a Value) -> Option<(&'a Value, &'a Value)> {
        let pair = self.refs.resolve(value).as_array()?;
        let [key, value] = pair.as_slice() else {
            return None;
        };
        Some((self.refs.resolve(key), self.refs.resolve(value)))
    }

    fn string_pair(&self, value: &'a Value) -> Option<(&'a str, &'a Value)> {
        let (key, value) = self.pair(value)?;
        Some((key.as_str()?, value))
    }

    fn uuid_array(&self, value: Option<&'a Value>) -> Vec<Uuid> {
        value
            .map(|value| self.refs.resolve(value))
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(|value| value_uuid(self.refs.resolve(value)))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn optional_string(&self, value: &'a Value, key: &str) -> Option<String> {
        self.field(value, key)
            .and_then(non_empty_str)
            .map(str::to_owned)
    }

    fn bool_field(&self, value: &'a Value, key: &str) -> bool {
        self.field(value, key)
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }

    fn parse_az_rtti(&self, value: Option<&'a Value>) -> Option<ReflectedAzRtti> {
        let value = self.refs.resolve(value?);
        if value.is_null() {
            return None;
        }
        let hierarchy = value
            .get("hierarchy")
            .map(|value| self.refs.resolve(value))
            .and_then(Value::as_array)
            .map(|entries| {
                entries
                    .iter()
                    .filter_map(|entry| self.parse_az_rtti_hierarchy_entry(entry))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let has_data = value.get("$id").is_some()
            || self.optional_string(value, "address").is_some()
            || self.field(value, "typeId").and_then(value_uuid).is_some()
            || self.optional_string(value, "typeName").is_some()
            || self
                .field(value, "isAbstract")
                .and_then(Value::as_bool)
                .is_some()
            || !hierarchy.is_empty();
        has_data.then(|| ReflectedAzRtti {
            reference_id: value.get("$id").and_then(reference_id),
            address: self.optional_string(value, "address"),
            type_id: self.field(value, "typeId").and_then(value_uuid),
            type_name: self.optional_string(value, "typeName"),
            hierarchy,
            is_abstract: self.field(value, "isAbstract").and_then(Value::as_bool),
        })
    }

    fn parse_az_rtti_hierarchy_entry(
        &self,
        value: &'a Value,
    ) -> Option<ReflectedAzRttiHierarchyEntry> {
        let value = self.refs.resolve(value);
        Some(ReflectedAzRttiHierarchyEntry {
            type_id: self.field(value, "typeId").and_then(value_uuid)?,
            type_name: self.optional_string(value, "typeName"),
        })
    }
}

fn reflected_non_type_template_argument(
    value: SchemaNonTypeTemplateArgument<'_>,
) -> ReflectedNonTypeTemplateArgument {
    match value {
        SchemaNonTypeTemplateArgument::Null => ReflectedNonTypeTemplateArgument::Null,
        SchemaNonTypeTemplateArgument::Bool(value) => ReflectedNonTypeTemplateArgument::Bool(value),
        SchemaNonTypeTemplateArgument::U64(value) => ReflectedNonTypeTemplateArgument::U64(value),
        SchemaNonTypeTemplateArgument::I64(value) => ReflectedNonTypeTemplateArgument::I64(value),
        SchemaNonTypeTemplateArgument::F64(value) => ReflectedNonTypeTemplateArgument::F64(value),
        SchemaNonTypeTemplateArgument::String(value) => {
            ReflectedNonTypeTemplateArgument::String(value.into_owned())
        }
        SchemaNonTypeTemplateArgument::Array(values) => ReflectedNonTypeTemplateArgument::Array(
            values
                .into_iter()
                .map(reflected_non_type_template_argument)
                .collect(),
        ),
    }
}

fn reflected_schema_az_rtti(rtti: SchemaAzRttiInfo<'_>) -> Option<ReflectedAzRtti> {
    debug_assert!(
        rtti.reference().is_none(),
        "azRtti references should be expanded before typed schema parse"
    );
    let has_data = rtti.address().is_some()
        || rtti.type_id().is_some()
        || rtti.type_name().is_some()
        || rtti.is_abstract().is_some()
        || !rtti.hierarchy().is_empty();
    has_data.then(|| ReflectedAzRtti {
        reference_id: rtti.id().and_then(schema_reference_id),
        address: rtti.address().map(str::to_owned),
        type_id: rtti.type_id().and_then(parse_uuid),
        type_name: rtti.type_name().map(str::to_owned),
        hierarchy: rtti
            .hierarchy()
            .into_iter()
            .filter_map(|entry| {
                Some(ReflectedAzRttiHierarchyEntry {
                    type_id: parse_uuid(entry.type_id)?,
                    type_name: entry.type_name.map(str::to_owned),
                })
            })
            .collect(),
        is_abstract: rtti.is_abstract(),
    })
}

fn parse_uuid_strings(values: &[String]) -> Vec<Uuid> {
    values
        .iter()
        .filter_map(|value| parse_uuid(value))
        .collect()
}

fn schema_reference_id(id: i64) -> Option<ReferenceKey> {
    u64::try_from(id).ok().map(ReferenceKey::Number)
}

fn schema_generic_is_reference_only(generic: SchemaGenericClassInfo<'_>) -> bool {
    generic.reference().is_some()
        && generic.type_id().is_none()
        && generic.registered_type_ids().is_empty()
        && generic.templated_type_ids().is_empty()
        && generic.class_data().is_none()
        && generic.elements().is_empty()
}

fn raw_schema_member<'a>(
    refs: &ReferenceIndex<'a>,
    raw_elements: Option<&'a Vec<Value>>,
    index: usize,
    member_id: i64,
) -> Option<&'a Value> {
    let raw_elements = raw_elements?;
    let id = u64::try_from(member_id).ok()?;
    raw_elements
        .get(index)
        .map(|value| refs.resolve(value))
        .filter(|value| value.get("$id").and_then(value_u64) == Some(id))
        .or_else(|| {
            raw_elements
                .iter()
                .map(|value| refs.resolve(value))
                .find(|value| value.get("$id").and_then(value_u64) == Some(id))
        })
}

fn raw_schema_generic_class<'a>(
    refs: &ReferenceIndex<'a>,
    raw_uuid_generic_map: Option<&'a Vec<Value>>,
    key: &str,
) -> Option<&'a Value> {
    raw_uuid_generic_map?.iter().find_map(|entry| {
        let pair = refs.resolve(entry).as_array()?;
        let [entry_key, value] = pair.as_slice() else {
            return None;
        };
        (refs.resolve(entry_key).as_str() == Some(key)).then_some(refs.resolve(value))
    })
}
