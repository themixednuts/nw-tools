use serde_json::Value;
use uuid::Uuid;

use crate::reference::ReferenceKey;

pub(crate) fn reference_id(value: &Value) -> Option<ReferenceKey> {
    match value {
        Value::String(value) if !value.is_empty() => Some(ReferenceKey::String(value.clone())),
        Value::Number(value) => value.as_u64().map(ReferenceKey::Number),
        _ => None,
    }
}

pub(crate) fn non_empty_str(value: &Value) -> Option<&str> {
    value.as_str().filter(|value| !value.is_empty())
}

pub(crate) fn parse_uuid(value: &str) -> Option<Uuid> {
    Uuid::parse_str(value.trim_matches(['{', '}'])).ok()
}

pub(crate) fn value_uuid(value: &Value) -> Option<Uuid> {
    value.as_str().and_then(parse_uuid)
}

pub(crate) fn value_u32(value: &Value) -> Option<u32> {
    match value {
        Value::Number(value) => value.as_u64().and_then(|value| u32::try_from(value).ok()),
        Value::String(value) => value.parse().ok(),
        _ => None,
    }
}

pub(crate) fn value_i32(value: &Value) -> Option<i32> {
    match value {
        Value::Number(value) => value.as_i64().and_then(|value| i32::try_from(value).ok()),
        Value::String(value) => value.parse().ok(),
        _ => None,
    }
}

pub(crate) fn value_u64(value: &Value) -> Option<u64> {
    match value {
        Value::Number(value) => value.as_u64(),
        Value::String(value) => value
            .strip_prefix("0x")
            .and_then(|value| u64::from_str_radix(value, 16).ok())
            .or_else(|| value.parse().ok()),
        _ => None,
    }
}

pub(crate) fn value_f64(value: &Value) -> Option<f64> {
    match value {
        Value::Number(value) => value.as_f64(),
        Value::String(value) => value.parse().ok(),
        _ => None,
    }
}
