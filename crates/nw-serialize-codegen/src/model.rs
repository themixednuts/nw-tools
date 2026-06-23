use std::collections::BTreeMap;

use serde_json::Value;
use uuid::Uuid;

use crate::document::SerializeContextDocument;

mod parser;
mod types;

pub use types::{
    ClassNameIndexEntry, ReflectedAttribute, ReflectedAttributeValue, ReflectedAzRtti,
    ReflectedAzRttiHierarchyEntry, ReflectedBodylessType, ReflectedClass, ReflectedEnum,
    ReflectedEnumVariant, ReflectedGenericClass, ReflectedMember, ReflectedNonTypeTemplateArgument,
};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SerializeContextModel {
    pub classes: BTreeMap<Uuid, ReflectedClass>,
    pub class_name_index: Vec<ClassNameIndexEntry>,
    pub generic_classes: BTreeMap<Uuid, ReflectedGenericClass>,
    pub enums: BTreeMap<Uuid, ReflectedEnum>,
    pub any_creators: BTreeMap<Uuid, String>,
    pub enum_underlying_types: BTreeMap<Uuid, Uuid>,
}

impl SerializeContextModel {
    #[must_use]
    pub fn from_document(document: &SerializeContextDocument) -> Self {
        parser::parse_document(document)
    }

    #[must_use]
    pub fn from_root(root: &Value) -> Self {
        parser::parse_root(root)
    }

    #[must_use]
    pub fn class_name(&self, type_id: Uuid) -> Option<&str> {
        self.classes.get(&type_id).map(|class| class.name.as_str())
    }

    #[must_use]
    pub fn generic_class(&self, type_id: Uuid) -> Option<&ReflectedGenericClass> {
        self.generic_classes.get(&type_id)
    }

    #[must_use]
    pub fn type_name(&self, type_id: Uuid) -> Option<&str> {
        self.class_name(type_id)
            .or_else(|| self.enums.get(&type_id).map(|ty| ty.name.as_str()))
            .or_else(|| {
                self.generic_classes
                    .get(&type_id)
                    .and_then(|generic| generic.class_name.as_deref())
            })
    }

    #[must_use]
    pub fn bodyless_rtti_types(&self) -> BTreeMap<Uuid, ReflectedBodylessType> {
        let mut bodyless = BTreeMap::new();
        for class in self.classes.values() {
            collect_bodyless_rtti_hierarchy_types(self, class.az_rtti.as_ref(), &mut bodyless);
            for member in &class.members {
                collect_bodyless_member_types(self, member, &mut bodyless);
            }
        }
        bodyless
    }

    #[must_use]
    pub fn bodyless_rtti_type_name(&self, type_id: Uuid) -> Option<String> {
        self.bodyless_rtti_types()
            .remove(&type_id)
            .map(|bodyless| bodyless.name)
    }
}

fn collect_bodyless_rtti_hierarchy_types(
    model: &SerializeContextModel,
    rtti: Option<&ReflectedAzRtti>,
    bodyless: &mut BTreeMap<Uuid, ReflectedBodylessType>,
) {
    let Some(rtti) = rtti else {
        return;
    };

    for (index, entry) in rtti.hierarchy.iter().enumerate() {
        if index == 0 && Some(entry.type_id) == rtti.type_id {
            continue;
        }
        let Some(name) = entry.type_name.as_deref().filter(|name| !name.is_empty()) else {
            continue;
        };
        insert_bodyless_type(model, bodyless, entry.type_id, name, Some(true));
    }
}

fn collect_bodyless_member_types(
    model: &SerializeContextModel,
    member: &ReflectedMember,
    bodyless: &mut BTreeMap<Uuid, ReflectedBodylessType>,
) {
    if let Some(rtti) = &member.az_rtti {
        if member.is_base_class || rtti.is_abstract == Some(true) {
            if let Some(name) = member_rtti_name(member) {
                insert_bodyless_type(model, bodyless, member.type_id, name, rtti.is_abstract);
            }
        }
        collect_bodyless_rtti_hierarchy_types(model, Some(rtti), bodyless);
    }

    if let Some(generic) = member.generic_class.as_deref() {
        collect_bodyless_generic_types(model, generic, bodyless);
    }
}

fn collect_bodyless_generic_types(
    model: &SerializeContextModel,
    generic: &ReflectedGenericClass,
    bodyless: &mut BTreeMap<Uuid, ReflectedBodylessType>,
) {
    for member in &generic.members {
        collect_bodyless_member_types(model, member, bodyless);
    }
}

fn insert_bodyless_type(
    model: &SerializeContextModel,
    bodyless: &mut BTreeMap<Uuid, ReflectedBodylessType>,
    type_id: Uuid,
    name: &str,
    is_abstract: Option<bool>,
) {
    if type_id.is_nil()
        || model.classes.contains_key(&type_id)
        || model.enums.contains_key(&type_id)
        || model.generic_classes.contains_key(&type_id)
    {
        return;
    }

    bodyless
        .entry(type_id)
        .and_modify(|existing| {
            existing.is_abstract = merge_optional_abstractness(existing.is_abstract, is_abstract);
        })
        .or_insert_with(|| ReflectedBodylessType {
            type_id,
            name: name.to_owned(),
            is_abstract,
        });
}

const fn merge_optional_abstractness(
    current: Option<bool>,
    incoming: Option<bool>,
) -> Option<bool> {
    match (current, incoming) {
        (Some(true), _) | (_, Some(true)) => Some(true),
        (Some(false), _) | (_, Some(false)) => Some(false),
        (None, None) => None,
    }
}

fn member_rtti_name(member: &ReflectedMember) -> Option<&str> {
    let rtti = member.az_rtti.as_ref()?;
    rtti.type_name
        .as_deref()
        .filter(|name| !name.is_empty())
        .or_else(|| {
            rtti.hierarchy
                .iter()
                .find(|entry| entry.type_id == member.type_id)
                .and_then(|entry| entry.type_name.as_deref())
                .filter(|name| !name.is_empty())
        })
}

#[cfg(test)]
mod tests;
