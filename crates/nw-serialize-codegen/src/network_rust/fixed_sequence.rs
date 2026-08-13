use std::collections::BTreeMap;

use quote::{format_ident, quote};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::naming::rust_type_ident;
use crate::network_schema::{
    NetworkField, NetworkFieldHandlerVtable, NetworkNativeTypeInfoEvidence, NetworkSerializeType,
    NetworkWireScalarShape, NetworkWireShape,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkFixedSequenceFieldReport {
    pub capacity: u16,
    pub element_stride: u64,
    pub element_wire_shape: NetworkWireScalarShape,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element_type_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element_type_id: Option<Uuid>,
    pub element_rust_type: String,
    pub generates_support_type: bool,
}

impl NetworkFixedSequenceFieldReport {
    pub(super) fn value_type(&self) -> String {
        format!(
            "::arrayvec::ArrayVec<{}, {}>",
            self.element_rust_type, self.capacity
        )
    }

    pub(super) fn field_type(&self) -> String {
        format!(
            "::nw_network::serialize::ReplicatedFieldHandler<{}>",
            self.value_type()
        )
    }

    pub(super) fn support_tokens(&self) -> Option<proc_macro2::TokenStream> {
        if !self.generates_support_type {
            return None;
        }
        let NetworkWireScalarShape::FixedBytes(width) = self.element_wire_shape else {
            return None;
        };
        let type_name = self.element_type_name.as_deref()?;
        let type_id = self.element_type_id?;
        let type_ident = format_ident!("{}", rust_type_ident(type_name));
        let type_id = syn::LitStr::new(
            &type_id.hyphenated().to_string().to_ascii_uppercase(),
            proc_macro2::Span::call_site(),
        );
        let width = syn::LitInt::new(&width.to_string(), proc_macro2::Span::call_site());
        Some(quote! {
            #[repr(transparent)]
            #[az_rtti(#type_id)]
            #[derive(
                Debug,
                Clone,
                Copy,
                Default,
                PartialEq,
                Eq,
                PartialOrd,
                Ord,
                Hash,
                Marshaler,
            )]
            pub struct #type_ident([u8; #width]);

            impl #type_ident {
                #[must_use]
                pub const fn from_bytes(bytes: [u8; #width]) -> Self {
                    Self(bytes)
                }

                #[must_use]
                pub const fn as_bytes(&self) -> &[u8; #width] {
                    &self.0
                }

                #[must_use]
                pub const fn into_bytes(self) -> [u8; #width] {
                    self.0
                }
            }

            impl From<[u8; #width]> for #type_ident {
                fn from(bytes: [u8; #width]) -> Self {
                    Self::from_bytes(bytes)
                }
            }

            impl From<#type_ident> for [u8; #width] {
                fn from(value: #type_ident) -> Self {
                    value.into_bytes()
                }
            }
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FixedSequencePlanError {
    MissingPlan,
    WireShapeMismatch,
    MissingElementType,
    ElementSizeMismatch,
    UnsupportedElementType,
}

impl FixedSequencePlanError {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::MissingPlan => "missing-fixed-sequence-plan",
            Self::WireShapeMismatch => "fixed-sequence-shape-mismatch",
            Self::MissingElementType => "missing-fixed-sequence-element-type",
            Self::ElementSizeMismatch => "fixed-sequence-element-size-mismatch",
            Self::UnsupportedElementType => "unsupported-fixed-sequence-element-type",
        }
    }
}

pub(super) fn fixed_sequence_vtable_for_field<'a>(
    field: &NetworkField,
    handler_vtables: &'a BTreeMap<&str, &NetworkFieldHandlerVtable>,
) -> Option<&'a NetworkFieldHandlerVtable> {
    field
        .handler_vtable
        .as_deref()
        .and_then(|address| handler_vtables.get(address).copied())
        .filter(|vtable| vtable.fixed_sequence_shape.is_some())
}

pub(super) fn fixed_sequence_field_report(
    vtable: &NetworkFieldHandlerVtable,
    field_wire_shape: Option<&NetworkWireShape>,
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
) -> Result<NetworkFixedSequenceFieldReport, FixedSequencePlanError> {
    let shape = vtable
        .fixed_sequence_shape
        .as_ref()
        .ok_or(FixedSequencePlanError::MissingPlan)?;
    if field_wire_shape != Some(&NetworkWireShape::FixedSequence(shape.wire_shape())) {
        return Err(FixedSequencePlanError::WireShapeMismatch);
    }
    if let Some(type_info) = shape.element_type_info.as_ref() {
        validate_element_size(type_info, shape.element_stride)?;
    }
    let type_identity = shape.element_type_info.as_ref().and_then(|type_info| {
        let type_id = type_info.type_id?;
        let type_name = type_info
            .name
            .as_deref()
            .filter(|name| !name.trim().is_empty())?;
        Some((type_id, type_name))
    });

    let (element_type_id, element_type_name, element_rust_type, generates_support_type) =
        if let Some((type_id, type_name)) = type_identity {
            if let Some(serialize_type) = serialize_types.get(&type_id) {
                (
                    Some(type_id),
                    Some(type_name.to_owned()),
                    format!(
                        "::nw_network::source::{}",
                        rust_type_ident(&serialize_type.name)
                    ),
                    false,
                )
            } else if matches!(
                shape.element_wire_shape,
                NetworkWireScalarShape::FixedBytes(_)
            ) {
                (
                    Some(type_id),
                    Some(type_name.to_owned()),
                    rust_type_ident(type_name),
                    true,
                )
            } else {
                return Err(FixedSequencePlanError::UnsupportedElementType);
            }
        } else if let Some(rust_type) =
            intrinsic_fixed_sequence_element_type(shape.element_wire_shape, shape.element_stride)
        {
            (None, None, rust_type, false)
        } else {
            return Err(FixedSequencePlanError::MissingElementType);
        };

    Ok(NetworkFixedSequenceFieldReport {
        capacity: shape.capacity,
        element_stride: shape.element_stride,
        element_wire_shape: shape.element_wire_shape,
        element_type_name,
        element_type_id,
        element_rust_type,
        generates_support_type,
    })
}

fn intrinsic_fixed_sequence_element_type(
    shape: NetworkWireScalarShape,
    element_stride: u64,
) -> Option<String> {
    let native_size = match shape {
        NetworkWireScalarShape::Bool | NetworkWireScalarShape::U8 => 1,
        NetworkWireScalarShape::U16 => 2,
        NetworkWireScalarShape::U32 | NetworkWireScalarShape::F32 => 4,
        NetworkWireScalarShape::U64
        | NetworkWireScalarShape::F64
        | NetworkWireScalarShape::Vec2 => 8,
        NetworkWireScalarShape::Vec3 => 12,
        NetworkWireScalarShape::Vec4 | NetworkWireScalarShape::Quat => 16,
        NetworkWireScalarShape::FixedBytes(width) => u64::from(width),
        _ => return None,
    };
    (element_stride == native_size).then(|| super::scalar_rust_type(shape))
}

fn validate_element_size(
    type_info: &NetworkNativeTypeInfoEvidence,
    element_stride: u64,
) -> Result<(), FixedSequencePlanError> {
    match type_info.native_size {
        Some(native_size) if native_size == element_stride => Ok(()),
        Some(_) => Err(FixedSequencePlanError::ElementSizeMismatch),
        None => Err(FixedSequencePlanError::MissingElementType),
    }
}

#[cfg(test)]
mod tests {
    use crate::network_rust::NetworkRustEmitter;
    use crate::network_schema::NetworkSchema;

    use super::*;

    #[test]
    fn plans_rtti_backed_fixed_bytes_as_semantic_arrayvec() {
        let vtable = serde_json::from_value::<NetworkFieldHandlerVtable>(serde_json::json!({
            "address": "NewWorld+0x81e2338",
            "fieldCount": 1,
            "fixedSequenceShape": {
                "storageKind": "inline-fixed",
                "capacity": 10,
                "elementStride": 32,
                "dataOffset": 16,
                "endOffset": 336,
                "elementWireShape": "fixed-bytes-16",
                "elementWireShapeSource": "marshal+unmarshal-pcode-agreement",
                "elementTypeInfo": {
                    "address": "NewWorld+0x808a050",
                    "name": "GroupId",
                    "typeId": "A40B891F-7B5F-434A-9E79-F7456844E5F3",
                    "source": "fixed-sequence-element-constructor-vptr+native-size",
                    "nameSource": "pcode-return-string",
                    "nativeSize": 32,
                    "nativeSizeSource": "serialize-field-layout"
                },
                "countCallsite": "NewWorld+0x34ca43c",
                "loopHeader": "NewWorld+0x34ca4dc",
                "source": "marshal+unmarshal-fixed-sequence-agreement"
            },
            "slots": [],
            "evidence": []
        }))
        .unwrap();

        let wire_shape = NetworkWireShape::FixedSequence(
            vtable.fixed_sequence_shape.as_ref().unwrap().wire_shape(),
        );
        let report =
            fixed_sequence_field_report(&vtable, Some(&wire_shape), &BTreeMap::new()).unwrap();

        assert_eq!(report.value_type(), "::arrayvec::ArrayVec<GroupId, 10>");
        assert!(report.generates_support_type);
        assert!(report.support_tokens().is_some());
    }

    #[test]
    fn plans_intrinsic_u8_fixed_sequence_without_rtti() {
        let vtable = serde_json::from_value::<NetworkFieldHandlerVtable>(serde_json::json!({
            "address": "NewWorld+0x80ff820",
            "fieldCount": 1,
            "fixedSequenceShape": {
                "storageKind": "inline-fixed",
                "capacity": 32,
                "elementStride": 1,
                "dataOffset": 16,
                "endOffset": 48,
                "elementWireShape": "u8",
                "elementWireShapeSource": "unmarshal-codec-specialization+fixed-width-pcode",
                "source": "cfg-fixed-capacity-sequence"
            },
            "slots": [],
            "evidence": []
        }))
        .unwrap();

        let wire_shape = NetworkWireShape::FixedSequence(
            vtable.fixed_sequence_shape.as_ref().unwrap().wire_shape(),
        );
        let report =
            fixed_sequence_field_report(&vtable, Some(&wire_shape), &BTreeMap::new()).unwrap();

        assert_eq!(report.value_type(), "::arrayvec::ArrayVec<u8, 32>");
        assert_eq!(report.element_type_name, None);
        assert_eq!(report.element_type_id, None);
        assert!(!report.generates_support_type);
    }

    #[test]
    fn emits_semantic_group_id_arrayvec_from_fixed_sequence_evidence() {
        let schema = NetworkSchema::from_ghidra_static_network_report(&serde_json::json!({
            "registryEntries": [{
                "uuid": "A85DF621-DCE0-409F-8D39-A447EA0807FF",
                "typeIndex": 28,
                "typeName": "Javelin::RaidDataComponentReplicatedState",
                "fields": [{
                    "index": 4,
                    "group": 0,
                    "name": "groupIds",
                    "handlerVtable": "NewWorld+0x81e2338",
                    "confidence": "register-field-call"
                }]
            }],
            "fieldRegistrationFunctions": [],
            "fieldHandlerVtables": [{
                "address": "NewWorld+0x81e2338",
                "fieldCount": 1,
                "wireShape": "fixed-vector<fixed-bytes-16,10>",
                "wireShapeSource": "marshal+unmarshal-fixed-sequence-agreement",
                "fixedSequenceShape": {
                    "storageKind": "inline-fixed",
                    "capacity": 10,
                    "elementStride": 32,
                    "dataOffset": 16,
                    "endOffset": 336,
                    "elementWireShape": "fixed-bytes-16",
                    "elementWireShapeSource": "marshal+unmarshal-pcode-agreement",
                    "elementTypeInfo": {
                        "address": "NewWorld+0x808a050",
                        "name": "GroupId",
                        "typeId": "A40B891F-7B5F-434A-9E79-F7456844E5F3",
                        "source": "fixed-sequence-element-constructor-vptr+native-size",
                        "nameSource": "pcode-return-string",
                        "nativeSize": 32,
                        "nativeSizeSource": "serialize-field-layout"
                    }
                },
                "slots": []
            }]
        }))
        .unwrap();

        let output = NetworkRustEmitter::emit_replicated_states(&schema, [28]).unwrap();
        let plan = &output.report.state_generation_plans[0];

        assert!(plan.can_generate, "{plan:#?}");
        assert_eq!(output.report.support_type_count, 1);
        assert!(output.source.contains("pub struct GroupId([u8; 16])"));
        assert!(output.source.contains("::arrayvec::ArrayVec<GroupId, 10>"));
    }

    #[test]
    fn emits_proven_structured_fixed_vector_without_scalar_storage_plan() {
        let schema = NetworkSchema::from_ghidra_static_network_report(&serde_json::json!({
            "registryEntries": [{
                "uuid": "C5E8790C-7A92-4B68-A05A-AB0871AEFA68",
                "typeIndex": 2133,
                "typeName": "MB::EncounterComponentReplicatedState",
                "fields": [{
                    "index": 0,
                    "group": 0,
                    "name": "status",
                    "handlerVtable": "NewWorld+0x80af538",
                    "confidence": "register-field-call"
                }]
            }],
            "fieldRegistrationFunctions": [],
            "fieldHandlerVtables": [{
                "address": "NewWorld+0x80af538",
                "fieldCount": 1,
                "wireShape": "fixed-vector<composite<u32,u32>,10>",
                "wireShapeSource": "cfg-bounded-inline-sequence",
                "wireLayout": "vec<composite<u32,u32>>",
                "wireLayoutSource": "cfg-bounded-inline-sequence",
                "slots": []
            }]
        }))
        .expect("structured fixed-vector schema");

        let output = NetworkRustEmitter::emit_replicated_states(&schema, [2133]).unwrap();
        let plan = &output.report.state_generation_plans[0];

        assert!(plan.can_generate, "{plan:#?}");
        assert_eq!(
            plan.fields[0].rust_value_type.as_deref(),
            Some("::arrayvec::ArrayVec<(u32, u32), 10>")
        );
        assert_eq!(plan.fields[0].fixed_sequence, None);
    }
}
