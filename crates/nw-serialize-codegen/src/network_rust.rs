use std::collections::{BTreeMap, BTreeSet};

use quote::{format_ident, quote};
use serde::{Deserialize, Serialize};
use syn::{LitInt, LitStr};
use thiserror::Error;
use uuid::Uuid;

use crate::CodegenContext;
use crate::ir::{SerializeCodegenItem, SerializeCodegenItemKind, collect_resolved_named_type_ids};
use crate::naming::{rust_field_ident, rust_module_ident, rust_type_ident};
use crate::network_schema::parse::{
    NetworkMemberWireShape, collection_element_wire_shape, nested_member_wire_shapes,
    nested_shape_by_wire_name, parse_network_member_wire_shape, parse_network_wire_scalar_shape,
    type_name_leaf,
};
use crate::network_schema::{
    NetworkConfidence, NetworkContainerCodec, NetworkField, NetworkFieldHandlerVtable,
    NetworkFragmentMetadata, NetworkNativeTypeInfoEvidence, NetworkPackedPositionWireShape,
    NetworkReplicatedContainerPlan, NetworkReplicatedContainerStorageKind,
    NetworkReplicatedContainerWireShape, NetworkSchema, NetworkSerializeKind, NetworkSerializeRole,
    NetworkSerializeType, NetworkType, NetworkTypeCapability,
    NetworkWireScalarShape as SchemaWireScalarShape, NetworkWireShape as SchemaWireShape,
};
use crate::rust::types::{RustTypeOptions, RustTypeRenderer};
use crate::types::{ResolvedType, ScalarType};

mod containers;
mod conversions;
mod descriptor_emit;
mod emitter;
mod evidence;
mod field_plan;
mod fixed_sequence;
mod identity;
mod message_emit;
mod model;
mod planning;
mod rust_types;
mod state_emit;
mod structured_values;
mod wire_types;

use containers::*;
use conversions::*;
use descriptor_emit::*;
pub use evidence::{NetworkEvidenceIssue, NetworkEvidenceIssueKind};
use evidence::{message_evidence_issues, state_evidence_issues};
use field_plan::*;
pub use fixed_sequence::NetworkFixedSequenceFieldReport;
use fixed_sequence::{fixed_sequence_field_report, fixed_sequence_vtable_for_field};
use identity::*;
use message_emit::*;
pub use model::*;
use planning::*;
use rust_types::*;
use state_emit::*;
use structured_values::*;
use wire_types::*;

#[cfg(test)]
mod tests;
