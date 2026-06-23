use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde_json::Value;
use thiserror::Error;

use crate::reference::{ReferenceExpansionContext, ReferenceIndex, ReferencePathSegment};
use crate::schema;

#[derive(Debug, Clone)]
pub struct SerializeContextDocument {
    root: Value,
    schema: Option<schema::SerializeContext>,
}

impl SerializeContextDocument {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, SerializeContextDocumentError> {
        let path = path.as_ref();
        let bytes = fs::read(path).map_err(|source| SerializeContextDocumentError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        Self::from_slice(&bytes).map_err(|source| SerializeContextDocumentError::Parse {
            path: path.to_path_buf(),
            source,
        })
    }

    pub fn from_slice(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        let root = serde_json::from_slice(bytes)?;
        let references = ReferenceIndex::new(&root);
        let schema_root = if references.has_references() {
            references.expand_with_policy(&root, should_expand_schema_reference)
        } else {
            root.clone()
        };
        let schema = schema::parse_value(schema_root)?;
        Ok(Self {
            root,
            schema: Some(schema),
        })
    }

    #[must_use]
    pub fn from_value_unchecked(root: Value) -> Self {
        Self { root, schema: None }
    }

    #[must_use]
    pub const fn root(&self) -> &Value {
        &self.root
    }

    #[must_use]
    pub fn schema(&self) -> Option<&schema::SerializeContext> {
        self.schema.as_ref()
    }

    #[must_use]
    pub fn references(&self) -> ReferenceIndex<'_> {
        ReferenceIndex::new(&self.root)
    }
}

fn should_expand_schema_reference(context: ReferenceExpansionContext<'_>) -> bool {
    GeneratedSchemaReferenceSlot::from_path(context.path).is_none()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GeneratedSchemaReferenceSlot {
    GenericClassInfo,
    UuidGenericMapEntry,
}

impl GeneratedSchemaReferenceSlot {
    fn from_path(path: &[ReferencePathSegment]) -> Option<Self> {
        if matches!(
            path.last(),
            Some(ReferencePathSegment::Field(field)) if field == "genericClassInfo"
        ) {
            return Some(Self::GenericClassInfo);
        }

        if matches!(
            path,
            [
                ..,
                ReferencePathSegment::Field(field),
                ReferencePathSegment::Index(_),
                ReferencePathSegment::Index(1),
            ] if field == "uuidGenericMap"
        ) {
            return Some(Self::UuidGenericMapEntry);
        }

        None
    }
}

#[derive(Debug, Error)]
pub enum SerializeContextDocumentError {
    #[error("failed to read {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to parse {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde_json::json;
    use uuid::uuid;

    use crate::model::SerializeContextModel;

    use super::*;

    #[test]
    fn unchecked_document_exposes_reference_index_over_owned_json() {
        let document = SerializeContextDocument::from_value_unchecked(json!({
            "$id": 1,
            "target": { "$id": 2, "name": "Component" },
            "alias": { "$ref": "#2" }
        }));

        assert_eq!(
            document.references().resolve(&document.root()["alias"])["name"],
            "Component"
        );
        assert!(document.schema().is_none());
    }

    #[test]
    fn parsed_document_keeps_generated_schema_model() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("resources")
            .join("serialize.json");
        let document =
            SerializeContextDocument::from_path(path).expect("project serialize schema fixture");

        assert!(document.schema().is_some());
    }

    #[test]
    fn typed_schema_parse_expands_references_outside_generated_schema_ref_slots() {
        let bytes = json!({
            "$id": 1,
            "uuidMap": {
                "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA": { "$ref": "#10" }
            },
            "classNameToUuid": [],
            "uuidGenericMap": [],
            "uuidAnyCreationMap": {},
            "editContext": {
                "$id": 2,
                "classData": [],
                "enumData": [[
                    "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB",
                    { "$ref": "#50" }
                ]]
            },
            "enumTypeIdToUnderlyingTypeIdMap": {},
            "definitions": {
                "class": {
                    "$id": 10,
                    "name": "Example::CounterComponent",
                    "typeId": "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
                    "version": 1,
                    "doSave": null,
                    "dataConverter": null,
                    "editData": null,
                    "elements": [{ "$ref": "#20" }],
                    "attributes": []
                },
                "member": {
                    "$id": 20,
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
                    "attributes": [[
                        99,
                        { "$ref": "#30" }
                    ]]
                },
                "memberAttribute": {
                    "$id": 30,
                    "attributeId": 99,
                    "attributeName": "EnumType",
                    "describesChildren": false,
                    "value": { "$ref": "#40" }
                },
                "memberAttributeValue": {
                    "$id": 40,
                    "kind": "Uuid",
                    "value": "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB"
                },
                "enum": {
                    "$id": 50,
                    "elementId": 7,
                    "name": "Example::Mode",
                    "description": "Mode enum",
                    "deprecatedName": null,
                    "attributes": [[
                        1,
                        { "$ref": "#60" }
                    ]]
                },
                "enumAttribute": {
                    "$id": 60,
                    "attributeId": 1,
                    "attributeName": "EnumValue",
                    "describesChildren": false,
                    "value": { "$ref": "#70" }
                },
                "enumValue": {
                    "$id": 70,
                    "kind": "enumConstant",
                    "valueU64": "7",
                    "valueU32": 7,
                    "valueI32": 7,
                    "valueHighU32": 0,
                    "valueF32": null,
                    "valueHighF32": 0.0,
                    "description": "Seven"
                }
            }
        })
        .to_string();

        let document = SerializeContextDocument::from_slice(bytes.as_bytes())
            .expect("typed schema parse should expand document-wide refs");
        let model = SerializeContextModel::from_document(&document);

        let class = model
            .classes
            .get(&uuid!("AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA"))
            .expect("class from uuidMap ref");
        assert_eq!(class.name, "Example::CounterComponent");
        assert_eq!(class.members[0].name, "m_count");
        assert_eq!(class.members[0].attributes[0].attribute_id, Some(99));
        assert_eq!(
            class.members[0].attributes[0]
                .value
                .as_ref()
                .and_then(|value| value.value_string.as_deref()),
            Some("BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB")
        );

        let reflected_enum = model
            .enums
            .get(&uuid!("BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB"))
            .expect("enum from editContext ref");
        assert_eq!(reflected_enum.name, "Example::Mode");
        assert_eq!(reflected_enum.variants[0].name, "Seven");
        assert_eq!(reflected_enum.variants[0].value_u32, Some(7));
    }

    #[test]
    fn schema_reference_policy_preserves_only_generated_ref_slots() {
        let root = json!({
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
                        "typeId": "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB",
                        "genericClassInfo": { "$ref": "#30" },
                        "toolMetadata": { "$ref": "#40" }
                    }],
                    "attributes": []
                }
            },
            "uuidGenericMap": [[
                "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB",
                { "$ref": "#30" }
            ]],
            "definitions": {
                "generic": {
                    "$id": 30,
                    "typeId": "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB"
                },
                "metadata": {
                    "$id": 40,
                    "name": "expanded metadata"
                }
            }
        });
        let refs = ReferenceIndex::new(&root);

        let expanded = refs.expand_with_policy(&root, should_expand_schema_reference);

        let member = &expanded["uuidMap"]["AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA"]["elements"][0];
        assert_eq!(member["genericClassInfo"]["$ref"], "#30");
        assert_eq!(member["toolMetadata"]["name"], "expanded metadata");
        assert_eq!(expanded["uuidGenericMap"][0][1]["$ref"], "#30");
    }
}
