use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::{
    NetworkSchema, NetworkTypeCapability, array_values, stable_address, string, u32_value,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkReplicatedStateAbiEvidence {
    pub source: String,
    pub abi_kind: NetworkReplicatedStateAbiKind,
    pub first_slot: u32,
    pub slot_count: u32,
    pub cohort_count: u32,
    pub functions: Vec<NetworkReplicatedStateAbiFunction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NetworkReplicatedStateAbiKind {
    Shared,
    Specialized,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkReplicatedStateAbiFunction {
    pub slot: u32,
    pub function: String,
}

pub(super) fn replicated_state_abi_evidence(
    object: &Map<String, Value>,
) -> Option<NetworkReplicatedStateAbiEvidence> {
    let source = string(object, "source")?;
    let abi_kind = match object.get("abiKind")?.as_str()? {
        "shared" => NetworkReplicatedStateAbiKind::Shared,
        "specialized" => NetworkReplicatedStateAbiKind::Specialized,
        _ => return None,
    };
    let first_slot = u32_value(object, "firstSlot")?;
    let slot_count = u32_value(object, "slotCount")?;
    let cohort_count = u32_value(object, "cohortCount")?;
    let functions = array_values(object, "functions")
        .filter_map(Value::as_object)
        .map(|function| {
            Some(NetworkReplicatedStateAbiFunction {
                slot: u32_value(function, "slot")?,
                function: stable_address(function, "function")?,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    let valid_source = matches!(
        (abi_kind, source.as_str()),
        (
            NetworkReplicatedStateAbiKind::Shared,
            "register-field-receiver+dominant-fragment-vtable-signature"
        ) | (
            NetworkReplicatedStateAbiKind::Specialized,
            "register-field-receiver+specialized-fragment-vtable-signature"
        )
    );
    if !valid_source
        || first_slot != 17
        || slot_count != 6
        || cohort_count == 0
        || (abi_kind == NetworkReplicatedStateAbiKind::Shared && cohort_count < 3)
        || functions.len() != usize::try_from(slot_count).ok()?
        || functions
            .iter()
            .enumerate()
            .any(|(index, function)| function.slot != first_slot + index as u32)
    {
        return None;
    }
    Some(NetworkReplicatedStateAbiEvidence {
        source,
        abi_kind,
        first_slot,
        slot_count,
        cohort_count,
        functions,
    })
}

pub(super) fn network_type_replicated_state_abi(
    entry: &Map<String, Value>,
) -> Option<NetworkReplicatedStateAbiEvidence> {
    entry
        .get("replicatedStateAbi")
        .and_then(Value::as_object)
        .and_then(replicated_state_abi_evidence)
        .or_else(|| {
            array_values(entry, "constructorMatches")
                .filter_map(Value::as_object)
                .filter_map(|constructor| {
                    constructor
                        .get("replicatedStateAbi")
                        .and_then(Value::as_object)
                        .and_then(replicated_state_abi_evidence)
                })
                .next()
        })
}

pub(super) fn promote_replicated_state_capabilities(schema: &mut NetworkSchema) {
    let evidence_by_type_id = schema
        .field_registration_functions
        .iter()
        .filter_map(|function| {
            Some((
                function.az_rtti.as_ref()?.type_id?,
                function.replicated_state_abi.clone()?,
            ))
        })
        .collect::<BTreeMap<_, _>>();

    for network_type in &mut schema.types {
        if network_type.replicated_state_abi.is_none()
            && let Some(evidence) = network_type
                .type_id
                .and_then(|type_id| evidence_by_type_id.get(&type_id))
        {
            network_type.replicated_state_abi = Some(evidence.clone());
        }
        if network_type.replicated_state_abi.is_some()
            && !network_type
                .capabilities
                .contains(&NetworkTypeCapability::ReplicatedState)
        {
            network_type
                .capabilities
                .push(NetworkTypeCapability::ReplicatedState);
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn rejects_sparse_or_small_cohort_evidence() {
        let value = json!({
            "source": "register-field-receiver+dominant-fragment-vtable-signature",
            "abiKind": "shared",
            "firstSlot": 17,
            "slotCount": 2,
            "cohortCount": 2,
            "functions": [
                { "slot": 17, "function": "NewWorld+0x1" },
                { "slot": 18, "function": "NewWorld+0x2" }
            ]
        });

        assert!(replicated_state_abi_evidence(value.as_object().unwrap()).is_none());
    }
}
