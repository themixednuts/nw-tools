use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClassRegistrationTraceIndex {
    pub record_count: usize,
    pub skipped_record_count: usize,
    pub records_by_type_id: BTreeMap<Uuid, ClassRegistrationTraceRecord>,
    pub duplicate_return_addresses_by_type_id: BTreeMap<Uuid, BTreeSet<String>>,
}

impl ClassRegistrationTraceIndex {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.record_count == 0
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.records_by_type_id.len()
    }

    #[must_use]
    pub fn duplicates_len(&self) -> usize {
        self.duplicate_return_addresses_by_type_id.len()
    }

    #[must_use]
    pub fn record_by_type_id(&self, type_id: Uuid) -> Option<&ClassRegistrationTraceRecord> {
        if self
            .duplicate_return_addresses_by_type_id
            .contains_key(&type_id)
        {
            return None;
        }
        self.records_by_type_id.get(&type_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassRegistrationTraceRecord {
    pub sequence: Option<u64>,
    pub type_id: Uuid,
    pub type_name: Option<String>,
    pub return_address: Option<String>,
    pub class_data_factory: Option<String>,
    pub class_data_az_rtti: Option<String>,
    pub any_creator: Option<String>,
}

#[must_use]
pub fn class_registration_trace_index(root: Option<&Value>) -> ClassRegistrationTraceIndex {
    let Some(root) = root else {
        return ClassRegistrationTraceIndex::default();
    };
    let Some(records) = root.as_array() else {
        return ClassRegistrationTraceIndex::default();
    };

    let mut index = ClassRegistrationTraceIndex::default();
    for record in records {
        match class_registration_trace_record(record) {
            Some(record) => remember_record(&mut index, record),
            None => index.skipped_record_count += 1,
        }
    }
    index
}

pub fn class_registration_trace_root_from_jsonl_str(
    text: &str,
) -> Result<Value, serde_json::Error> {
    let records = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(serde_json::from_str)
        .collect::<Result<Vec<Value>, _>>()?;
    Ok(Value::Array(records))
}

fn class_registration_trace_record(value: &Value) -> Option<ClassRegistrationTraceRecord> {
    let object = value.as_object()?;
    let type_id = object
        .get("typeId")
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())?;

    Some(ClassRegistrationTraceRecord {
        sequence: object.get("sequence").and_then(Value::as_u64),
        type_id,
        type_name: string_field(object, "typeName"),
        return_address: string_field(object, "returnAddress"),
        class_data_factory: string_field(object, "classDataFactory"),
        class_data_az_rtti: string_field(object, "classDataAzRtti"),
        any_creator: string_field(object, "anyCreator"),
    })
}

fn string_field(object: &serde_json::Map<String, Value>, name: &str) -> Option<String> {
    object
        .get(name)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
}

fn remember_record(index: &mut ClassRegistrationTraceIndex, record: ClassRegistrationTraceRecord) {
    index.record_count += 1;
    let Some(existing) = index.records_by_type_id.get(&record.type_id) else {
        index.records_by_type_id.insert(record.type_id, record);
        return;
    };

    if existing.return_address == record.return_address {
        return;
    }

    let duplicate_addresses = index
        .duplicate_return_addresses_by_type_id
        .entry(record.type_id)
        .or_default();
    duplicate_addresses.insert(display_address(existing.return_address.as_deref()));
    duplicate_addresses.insert(display_address(record.return_address.as_deref()));
}

fn display_address(address: Option<&str>) -> String {
    address.unwrap_or("<missing>").to_owned()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn indexes_unique_class_registration_records() {
        let root = Value::Array(vec![json!({
            "sequence": 7,
            "typeName": "Example",
            "typeId": "11111111-2222-3333-4444-555555555555",
            "returnAddress": "NewWorld+0x1234",
            "classDataFactory": "NewWorld+0x2000",
            "classDataAzRtti": "NewWorld+0x3000",
            "anyCreator": "NewWorld+0x4000"
        })]);

        let index = class_registration_trace_index(Some(&root));
        let type_id = uuid::uuid!("11111111-2222-3333-4444-555555555555");
        let record = index
            .record_by_type_id(type_id)
            .expect("registration record");

        assert_eq!(index.record_count, 1);
        assert_eq!(index.len(), 1);
        assert_eq!(record.sequence, Some(7));
        assert_eq!(record.type_name.as_deref(), Some("Example"));
        assert_eq!(record.return_address.as_deref(), Some("NewWorld+0x1234"));
    }

    #[test]
    fn refuses_ambiguous_registration_return_addresses() {
        let root = Value::Array(vec![
            json!({
                "typeId": "11111111-2222-3333-4444-555555555555",
                "returnAddress": "NewWorld+0x1234"
            }),
            json!({
                "typeId": "11111111-2222-3333-4444-555555555555",
                "returnAddress": "NewWorld+0x5678"
            }),
        ]);

        let index = class_registration_trace_index(Some(&root));
        let type_id = uuid::uuid!("11111111-2222-3333-4444-555555555555");

        assert!(index.record_by_type_id(type_id).is_none());
        assert_eq!(index.duplicates_len(), 1);
    }

    #[test]
    fn parses_jsonl_trace_root() {
        let root = class_registration_trace_root_from_jsonl_str(
            r#"{"typeId":"11111111-2222-3333-4444-555555555555"}
{"typeId":"aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"}
"#,
        )
        .expect("jsonl trace");

        let records = root.as_array().expect("array root");
        assert_eq!(records.len(), 2);
    }
}
