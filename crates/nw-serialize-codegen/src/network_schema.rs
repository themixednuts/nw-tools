use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};
use thiserror::Error;
use uuid::Uuid;

use crate::CodegenContext;
use crate::ir::{
    SerializeCodegenIndex, SerializeCodegenItem, SerializeCodegenItemKind, SerializeCodegenUnit,
    collect_resolved_named_type_ids,
};
use crate::role::ReflectedTypeRole;
use crate::types::{ResolvedType, ScalarType};

mod container_plan;
mod fixed_sequence;
mod ingest;
mod merge;
mod model;
mod overlay;
pub(crate) mod parse;
mod replicated_state;
mod runtime;
mod schema;
mod wire;

pub use container_plan::{
    NetworkContainerCodec, NetworkContainerMemberSemantics, NetworkContainerPlanDiagnostic,
    NetworkReplicatedContainerPlan,
};
pub use fixed_sequence::{
    NetworkFixedSequenceShape, NetworkFixedSequenceStorageKind, NetworkFixedSequenceWireShape,
};
pub use model::*;
pub use overlay::NetworkGhidraOverlayMergeReport;
pub use replicated_state::{
    NetworkReplicatedStateAbiEvidence, NetworkReplicatedStateAbiFunction,
    NetworkReplicatedStateAbiKind,
};
pub use runtime::*;
pub use wire::*;

use fixed_sequence::{parse_fixed_sequence_shape, parse_fixed_sequence_wire_shape};
use ingest::*;
use merge::*;
use parse::*;
use replicated_state::{
    network_type_replicated_state_abi, promote_replicated_state_capabilities,
    replicated_state_abi_evidence,
};

#[cfg(test)]
mod tests;
