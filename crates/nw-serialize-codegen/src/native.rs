use std::collections::BTreeMap;

use uuid::Uuid;

use crate::model::{ReflectedAttribute, ReflectedClass, ReflectedMember, SerializeContextModel};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeReflectedTypeFact {
    pub type_id: Uuid,
    pub source_name: &'static str,
}

#[must_use]
pub fn native_reflected_type_name(type_id: Uuid) -> Option<&'static str> {
    NATIVE_REFLECTED_TYPE_FACTS
        .iter()
        .find(|fact| fact.type_id == type_id)
        .map(|fact| fact.source_name)
}

#[must_use]
pub fn native_reflected_type_facts() -> &'static [NativeReflectedTypeFact] {
    NATIVE_REFLECTED_TYPE_FACTS
}

const NATIVE_REFLECTED_TYPE_FACTS: &[NativeReflectedTypeFact] = &[
    // Shared engine RTTI interfaces used by reflected containers but not
    // emitted as normal SerializeContext classes.
    NativeReflectedTypeFact {
        type_id: Uuid::from_u128(0x0A096354_7F26_4B18_B8C0_8F10A3E0440A),
        source_name: "IAnimNode",
    },
    NativeReflectedTypeFact {
        type_id: Uuid::from_u128(0x11C16CEC_4C03_4342_B4A7_62790E48CBD5),
        source_name: "IUiAnimTrack",
    },
    NativeReflectedTypeFact {
        type_id: Uuid::from_u128(0x298180CC_B577_440C_8466_A01ABC8CC00A),
        source_name: "IUiAnimNode",
    },
    NativeReflectedTypeFact {
        type_id: Uuid::from_u128(0x74EFA085_7758_4275_98A1_4D40DC6F55B8),
        source_name: "IUiAnimSequence",
    },
    NativeReflectedTypeFact {
        type_id: Uuid::from_u128(0xA60F95F5_5A4A_47DB_B3BB_525BBC0BC8DB),
        source_name: "IAnimSequence",
    },
    NativeReflectedTypeFact {
        type_id: Uuid::from_u128(0xAA0D5170_FB28_426F_BA13_7EFF6BB3AC67),
        source_name: "IAnimTrack",
    },
    NativeReflectedTypeFact {
        type_id: Uuid::from_u128(0xCCA8EFE4_C4D6_49AD_80A1_70700118A9ED),
        source_name: "AZ::UserSettings",
    },
    NativeReflectedTypeFact {
        type_id: Uuid::from_u128(0xD86C82E1_E027_453F_A43B_BD801CF88391),
        source_name: "UiInteractableStateAction",
    },
    // Project RTTI/type facts used by reflected fields without a reflected
    // class body.
    NativeReflectedTypeFact {
        type_id: Uuid::from_u128(0x11E60F2F_8CEA_4978_91E2_9EDABF91B9AF),
        source_name: "StatModVitalsData",
    },
    NativeReflectedTypeFact {
        type_id: Uuid::from_u128(0x2298CE4C_4675_46F8_BEEB_07C8041B8A9A),
        source_name: "ContributionEvent",
    },
    NativeReflectedTypeFact {
        type_id: Uuid::from_u128(0x489FAD0A_3B3E_4C82_A33B_EC7C0887A90C),
        source_name: "AbilityData",
    },
    NativeReflectedTypeFact {
        type_id: Uuid::from_u128(0x6682289A_61C7_47CE_8D13_F99CA3A8DAAA),
        source_name: "Aws::Transaction::Model::InitializeTransactionRequest",
    },
    NativeReflectedTypeFact {
        type_id: Uuid::from_u128(0xD2EBA850_70EF_4B9F_B6CE_42AE137939F9),
        source_name: "GuildInfluenceData",
    },
    NativeReflectedTypeFact {
        type_id: Uuid::from_u128(0xD43E3D53_10B1_43EC_A503_E83D4208BC30),
        source_name: "UnifiedInteractOption",
    },
    NativeReflectedTypeFact {
        type_id: Uuid::from_u128(0xE95D9900_33D1_4696_9B40_4640B1DA544C),
        source_name: "ClientView",
    },
];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NativeSymbolIndex {
    symbols_by_address: BTreeMap<String, NativeSymbol>,
}

impl NativeSymbolIndex {
    #[must_use]
    pub fn from_model(model: &SerializeContextModel) -> Self {
        let mut index = Self::default();

        for class in model.classes.values() {
            index.add_class_symbol(
                class,
                class.factory.as_deref(),
                NativeSymbolUseKind::Factory,
            );
            index.add_class_symbol(
                class,
                class.serializer.as_deref(),
                NativeSymbolUseKind::Serializer,
            );
            index.add_class_symbol(
                class,
                class.az_rtti.as_ref().and_then(|rtti| rtti.address()),
                NativeSymbolUseKind::AzRtti,
            );
            index.add_class_symbol(
                class,
                class.container.as_deref(),
                NativeSymbolUseKind::Container,
            );
            index.add_class_symbol(
                class,
                class.converter.as_deref(),
                NativeSymbolUseKind::Converter,
            );
            index.add_class_symbol(
                class,
                class.data_converter.as_deref(),
                NativeSymbolUseKind::DataConverter,
            );
            index.add_class_symbol(
                class,
                class.event_handler.as_deref(),
                NativeSymbolUseKind::EventHandler,
            );
            index.add_class_symbol(
                class,
                class.persistent_id.as_deref(),
                NativeSymbolUseKind::PersistentId,
            );
            index.add_class_symbol(class, class.do_save.as_deref(), NativeSymbolUseKind::DoSave);
            index.add_attributes(class, None, &class.attributes);

            for member in &class.members {
                index.add_member_symbol(
                    class,
                    member,
                    member.az_rtti.as_ref().and_then(|rtti| rtti.address()),
                );
                index.add_attributes(class, Some(member), &member.attributes);
            }
        }

        for (type_id, address) in &model.any_creators {
            let owner_name = model.type_name(*type_id).map(str::to_owned);
            index.add(
                address,
                NativeSymbolUse {
                    kind: NativeSymbolUseKind::AnyCreator,
                    owner_type_id: Some(*type_id),
                    owner_name,
                    member_name: None,
                    attribute_name: None,
                },
            );
        }

        index
    }

    #[must_use]
    pub fn symbols(&self) -> impl Iterator<Item = &NativeSymbol> {
        self.symbols_by_address.values()
    }

    #[must_use]
    pub fn symbol(&self, address: &str) -> Option<&NativeSymbol> {
        self.symbols_by_address.get(address)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.symbols_by_address.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.symbols_by_address.is_empty()
    }

    fn add_class_symbol(
        &mut self,
        class: &ReflectedClass,
        address: Option<&str>,
        kind: NativeSymbolUseKind,
    ) {
        let Some(address) = address else {
            return;
        };
        self.add(
            address,
            NativeSymbolUse {
                kind,
                owner_type_id: Some(class.type_id),
                owner_name: Some(class.name.clone()),
                member_name: None,
                attribute_name: None,
            },
        );
    }

    fn add_member_symbol(
        &mut self,
        class: &ReflectedClass,
        member: &ReflectedMember,
        address: Option<&str>,
    ) {
        let Some(address) = address else {
            return;
        };
        self.add(
            address,
            NativeSymbolUse {
                kind: NativeSymbolUseKind::MemberAzRtti,
                owner_type_id: Some(class.type_id),
                owner_name: Some(class.name.clone()),
                member_name: Some(member.name.clone()),
                attribute_name: None,
            },
        );
    }

    fn add_attributes(
        &mut self,
        class: &ReflectedClass,
        member: Option<&ReflectedMember>,
        attributes: &[ReflectedAttribute],
    ) {
        for attribute in attributes {
            let Some(value) = &attribute.value else {
                continue;
            };
            if let Some(address) = value.function.as_deref() {
                self.add_attribute_symbol(
                    class,
                    member,
                    attribute,
                    address,
                    NativeSymbolUseKind::AttributeFunction,
                );
            }
            if let Some(address) = value.member_function.as_deref() {
                self.add_attribute_symbol(
                    class,
                    member,
                    attribute,
                    address,
                    NativeSymbolUseKind::AttributeMemberFunction,
                );
            }
        }
    }

    fn add_attribute_symbol(
        &mut self,
        class: &ReflectedClass,
        member: Option<&ReflectedMember>,
        attribute: &ReflectedAttribute,
        address: &str,
        kind: NativeSymbolUseKind,
    ) {
        self.add(
            address,
            NativeSymbolUse {
                kind,
                owner_type_id: Some(class.type_id),
                owner_name: Some(class.name.clone()),
                member_name: member.map(|member| member.name.clone()),
                attribute_name: attribute.name.clone(),
            },
        );
    }

    fn add(&mut self, address: &str, usage: NativeSymbolUse) {
        if address.is_empty() {
            return;
        }
        self.symbols_by_address
            .entry(address.to_owned())
            .or_insert_with(|| NativeSymbol {
                address: address.to_owned(),
                uses: Vec::new(),
            })
            .uses
            .push(usage);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeSymbol {
    pub address: String,
    pub uses: Vec<NativeSymbolUse>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeSymbolUse {
    pub kind: NativeSymbolUseKind,
    pub owner_type_id: Option<Uuid>,
    pub owner_name: Option<String>,
    pub member_name: Option<String>,
    pub attribute_name: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeSymbolUseKind {
    Factory,
    Serializer,
    AzRtti,
    Container,
    Converter,
    DataConverter,
    EventHandler,
    PersistentId,
    DoSave,
    MemberAzRtti,
    AttributeFunction,
    AttributeMemberFunction,
    AnyCreator,
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use uuid::uuid;

    use crate::{SerializeContextDocument, model::SerializeContextModel};

    use super::*;

    #[test]
    fn indexes_class_member_and_attribute_native_symbols() {
        let document = SerializeContextDocument::from_slice(
            json!({
            "$id": 1,
            "uuidMap": {
                "11111111-1111-1111-1111-111111111111": {
                    "$id": 10,
                    "name": "ExampleComponent",
                    "typeId": "11111111-1111-1111-1111-111111111111",
                    "version": 1,
                    "factory": "NewWorld+0x10",
                    "serializer": null,
                    "azRtti": {
                        "address": "NewWorld+0x20",
                        "typeId": "11111111-1111-1111-1111-111111111111",
                        "typeName": "ExampleComponent",
                        "hierarchy": [{
                            "typeId": "11111111-1111-1111-1111-111111111111",
                            "typeName": "ExampleComponent"
                        }],
                        "isAbstract": false
                    },
                    "container": null,
                    "converter": null,
                    "dataConverter": null,
                    "editData": null,
                    "eventHandler": null,
                    "persistentId": null,
                    "doSave": null,
                    "elements": [{
                        "$id": 11,
                        "name": "m_value",
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
                        "azRtti": {
                            "address": "NewWorld+0x30",
                            "typeId": "43DA906B-7DEF-4CA8-9790-854106D3F983",
                            "typeName": "unsigned int",
                            "hierarchy": [{
                                "typeId": "43DA906B-7DEF-4CA8-9790-854106D3F983",
                                "typeName": "unsigned int"
                            }],
                            "isAbstract": false
                        },
                        "editData": null,
                        "attributes": [[1, {
                            "$id": 12,
                            "attributeId": 1,
                            "attributeName": "ChangeNotify",
                            "describesChildren": false,
                            "childClassOwned": false,
                            "value": {
                                "kind": "function",
                                "function": null,
                                "memberFunction": "NewWorld+0x40"
                            }
                        }]]
                    }],
                    "attributes": []
                }
            },
            "classNameToUuid": [],
            "uuidGenericMap": [],
            "uuidAnyCreationMap": {
                "11111111-1111-1111-1111-111111111111": "NewWorld+0x50"
            },
            "editContext": {"$id": 2, "classData": [], "enumData": []},
            "enumTypeIdToUnderlyingTypeIdMap": {}
            })
            .to_string()
            .as_bytes(),
        )
        .expect("typed serialize context");
        let model = SerializeContextModel::from_document(&document);

        let index = NativeSymbolIndex::from_model(&model);

        assert_eq!(
            index.symbol("NewWorld+0x10").unwrap().uses[0].kind,
            NativeSymbolUseKind::Factory
        );
        assert_eq!(
            index.symbol("NewWorld+0x30").unwrap().uses[0].kind,
            NativeSymbolUseKind::MemberAzRtti
        );
        assert_eq!(
            index.symbol("NewWorld+0x40").unwrap().uses[0],
            NativeSymbolUse {
                kind: NativeSymbolUseKind::AttributeMemberFunction,
                owner_type_id: Some(uuid!("11111111-1111-1111-1111-111111111111")),
                owner_name: Some("ExampleComponent".to_owned()),
                member_name: Some("m_value".to_owned()),
                attribute_name: Some("ChangeNotify".to_owned()),
            }
        );
        assert_eq!(
            index.symbol("NewWorld+0x50").unwrap().uses[0].kind,
            NativeSymbolUseKind::AnyCreator
        );
    }
}
