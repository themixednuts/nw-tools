use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

const MAX_REF_DEPTH: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ReferenceKey {
    String(String),
    Number(u64),
}

impl ReferenceKey {
    #[must_use]
    pub fn parse(reference: &str) -> Self {
        let key = reference.strip_prefix('#').unwrap_or(reference);
        key.parse::<u64>()
            .map(Self::Number)
            .unwrap_or_else(|_| Self::String(key.to_owned()))
    }

    #[must_use]
    pub fn display(&self) -> String {
        match self {
            Self::String(value) => value.clone(),
            Self::Number(value) => value.to_string(),
        }
    }
}

#[derive(Debug)]
pub struct ReferenceIndex<'a> {
    values: BTreeMap<ReferenceKey, &'a Value>,
    duplicate_ids: BTreeSet<ReferenceKey>,
    references: BTreeSet<ReferenceKey>,
}

impl<'a> ReferenceIndex<'a> {
    #[must_use]
    pub fn new(root: &'a Value) -> Self {
        let mut index = Self {
            values: BTreeMap::new(),
            duplicate_ids: BTreeSet::new(),
            references: BTreeSet::new(),
        };
        index.collect(root);
        index
    }

    #[must_use]
    pub fn resolve(&self, value: &'a Value) -> &'a Value {
        let mut current = value;
        let mut seen = BTreeSet::new();
        for _ in 0..MAX_REF_DEPTH {
            let Some(reference) = current.get("$ref").and_then(Value::as_str) else {
                return current;
            };
            let key = ReferenceKey::parse(reference);
            if !seen.insert(key.clone()) {
                return current;
            }
            let Some(next) = self.values.get(&key).copied() else {
                return current;
            };
            current = next;
        }
        current
    }

    #[must_use]
    pub fn resolve_key(&self, key: &ReferenceKey) -> Option<&'a Value> {
        self.values
            .get(key)
            .copied()
            .map(|value| self.resolve(value))
    }

    #[must_use]
    pub fn resolve_reference(&self, reference: &str) -> Option<&'a Value> {
        self.resolve_key(&ReferenceKey::parse(reference))
    }

    #[must_use]
    pub fn has_references(&self) -> bool {
        !self.references.is_empty()
    }

    #[must_use]
    pub fn expand(&self, value: &Value) -> Value {
        self.expand_with_policy(value, |_| true)
    }

    #[must_use]
    pub fn expand_with_policy<F>(&self, value: &Value, mut should_expand: F) -> Value
    where
        F: FnMut(ReferenceExpansionContext<'_>) -> bool,
    {
        self.expand_value(
            value,
            &mut ReferenceExpansionState::default(),
            &mut should_expand,
        )
    }

    #[must_use]
    pub fn report(&self) -> ReferenceReport {
        let ids = self.values.keys().cloned().collect::<BTreeSet<_>>();
        let missing_refs = self.references.difference(&ids).cloned().collect();
        ReferenceReport {
            ids: self.values.len(),
            refs: self.references.len(),
            duplicate_ids: self.duplicate_ids.iter().cloned().collect(),
            missing_refs,
            cyclic_refs: self.cyclic_references(),
        }
    }

    fn cyclic_references(&self) -> Vec<ReferenceKey> {
        self.references
            .iter()
            .filter(|key| self.reference_chain_cycles(key))
            .cloned()
            .collect()
    }

    fn reference_chain_cycles(&self, key: &ReferenceKey) -> bool {
        let mut seen = BTreeSet::new();
        let mut current = key.clone();
        for _ in 0..MAX_REF_DEPTH {
            if !seen.insert(current.clone()) {
                return true;
            }
            let Some(value) = self.values.get(&current) else {
                return false;
            };
            let Some(reference) = value.get("$ref").and_then(Value::as_str) else {
                return false;
            };
            current = ReferenceKey::parse(reference);
        }
        true
    }

    fn collect(&mut self, root: &'a Value) {
        let mut stack = vec![root];
        while let Some(value) = stack.pop() {
            match value {
                Value::Object(map) => {
                    if let Some(id) = map.get("$id").and_then(reference_id)
                        && self.values.insert(id.clone(), value).is_some()
                    {
                        self.duplicate_ids.insert(id);
                    }
                    if let Some(reference) = map.get("$ref").and_then(Value::as_str) {
                        self.references.insert(ReferenceKey::parse(reference));
                    }
                    for child in map.values() {
                        stack.push(child);
                    }
                }
                Value::Array(items) => {
                    for child in items {
                        stack.push(child);
                    }
                }
                _ => {}
            }
        }
    }

    fn expand_value<F>(
        &self,
        value: &Value,
        state: &mut ReferenceExpansionState,
        should_expand: &mut F,
    ) -> Value
    where
        F: FnMut(ReferenceExpansionContext<'_>) -> bool,
    {
        if state.depth >= MAX_REF_DEPTH {
            return value.clone();
        }

        if let Some(reference) = value.get("$ref").and_then(Value::as_str) {
            let key = ReferenceKey::parse(reference);
            let context = ReferenceExpansionContext {
                path: &state.path,
                key: &key,
                value,
            };
            if !should_expand(context) {
                return value.clone();
            }
            if !state.reference_stack.insert(key.clone()) {
                return value.clone();
            }
            let expanded = self
                .values
                .get(&key)
                .copied()
                .map(|value| {
                    state.depth += 1;
                    let expanded = self.expand_value(value, state, should_expand);
                    state.depth -= 1;
                    expanded
                })
                .unwrap_or_else(|| value.clone());
            state.reference_stack.remove(&key);
            return expanded;
        }

        match value {
            Value::Object(map) => {
                state.depth += 1;
                let expanded = map
                    .iter()
                    .map(|(key, value)| {
                        state.path.push(ReferencePathSegment::Field(key.clone()));
                        let expanded = self.expand_value(value, state, should_expand);
                        state.path.pop();
                        (key.clone(), expanded)
                    })
                    .collect();
                state.depth -= 1;
                Value::Object(expanded)
            }
            Value::Array(values) => {
                state.depth += 1;
                let expanded = values
                    .iter()
                    .enumerate()
                    .map(|(index, value)| {
                        state.path.push(ReferencePathSegment::Index(index));
                        let expanded = self.expand_value(value, state, should_expand);
                        state.path.pop();
                        expanded
                    })
                    .collect();
                state.depth -= 1;
                Value::Array(expanded)
            }
            _ => value.clone(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReferenceReport {
    pub ids: usize,
    pub refs: usize,
    pub duplicate_ids: Vec<ReferenceKey>,
    pub missing_refs: Vec<ReferenceKey>,
    pub cyclic_refs: Vec<ReferenceKey>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReferenceExpansionContext<'a> {
    pub path: &'a [ReferencePathSegment],
    pub key: &'a ReferenceKey,
    pub value: &'a Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReferencePathSegment {
    Field(String),
    Index(usize),
}

#[derive(Debug, Default)]
struct ReferenceExpansionState {
    reference_stack: BTreeSet<ReferenceKey>,
    path: Vec<ReferencePathSegment>,
    depth: usize,
}

fn reference_id(value: &Value) -> Option<ReferenceKey> {
    match value {
        Value::String(value) if !value.is_empty() => Some(ReferenceKey::String(value.clone())),
        Value::Number(value) => value.as_u64().map(ReferenceKey::Number),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn resolves_numeric_and_hash_refs_to_id_targets() {
        let root = json!({
            "$id": 1,
            "target": { "$id": 7, "name": "AZStd::vector" },
            "uses": [
                { "$ref": "#7" },
                { "$ref": "7" }
            ]
        });
        let refs = ReferenceIndex::new(&root);

        assert_eq!(refs.resolve(&root["uses"][0])["name"], "AZStd::vector");
        assert_eq!(refs.resolve(&root["uses"][1])["name"], "AZStd::vector");

        let report = refs.report();
        assert_eq!(report.ids, 2);
        assert_eq!(report.refs, 1);
        assert!(report.missing_refs.is_empty());
    }

    #[test]
    fn reports_missing_refs_and_duplicate_ids() {
        let root = json!({
            "$id": "root",
            "a": { "$id": "dup" },
            "b": { "$id": "dup" },
            "c": { "$ref": "#missing" }
        });

        let report = ReferenceIndex::new(&root).report();

        assert_eq!(
            report.duplicate_ids,
            vec![ReferenceKey::String("dup".to_owned())]
        );
        assert_eq!(
            report.missing_refs,
            vec![ReferenceKey::String("missing".to_owned())]
        );
    }

    #[test]
    fn reports_ref_cycles_without_unbounded_resolution() {
        let root = json!({
            "$id": "root",
            "a": { "$id": "a", "$ref": "#b" },
            "b": { "$id": "b", "$ref": "#a" },
            "uses": { "$ref": "#a" }
        });
        let refs = ReferenceIndex::new(&root);

        let resolved = refs.resolve(&root["uses"]);
        assert_eq!(resolved["$ref"], "#a");
        assert_eq!(
            refs.report().cyclic_refs,
            vec![
                ReferenceKey::String("a".to_owned()),
                ReferenceKey::String("b".to_owned())
            ]
        );
    }

    #[test]
    fn expands_references_across_the_entire_json_tree() {
        let root = json!({
            "$id": 1,
            "definitions": {
                "class": {
                    "$id": 10,
                    "name": "Example::CounterComponent",
                    "members": [{ "$ref": "#20" }]
                },
                "field": {
                    "$id": 20,
                    "name": "m_count",
                    "typeId": "43DA906B-7DEF-4CA8-9790-854106D3F983"
                }
            },
            "uuidMap": {
                "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA": { "$ref": "#10" }
            },
            "editContext": {
                "enumData": [[
                    "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB",
                    {
                        "attributes": [[1, { "value": { "$ref": "#30" } }]]
                    }
                ]]
            },
            "sharedValues": {
                "$id": 30,
                "description": "Server",
                "valueU32": 2
            }
        });
        let refs = ReferenceIndex::new(&root);

        let expanded = refs.expand(&root);

        assert_eq!(
            expanded["uuidMap"]["AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA"]["name"],
            "Example::CounterComponent"
        );
        assert_eq!(
            expanded["uuidMap"]["AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA"]["members"][0]["name"],
            "m_count"
        );
        assert_eq!(
            expanded["editContext"]["enumData"][0][1]["attributes"][0][1]["value"]["description"],
            "Server"
        );
    }

    #[test]
    fn leaves_cyclic_references_bounded_during_expansion() {
        let root = json!({
            "$id": "root",
            "a": { "$id": "a", "next": { "$ref": "#b" } },
            "b": { "$id": "b", "next": { "$ref": "#a" } },
            "uses": { "$ref": "#a" }
        });
        let refs = ReferenceIndex::new(&root);

        let expanded = refs.expand(&root);

        assert_eq!(expanded["uses"]["next"]["next"]["$ref"], "#a");
    }
}
