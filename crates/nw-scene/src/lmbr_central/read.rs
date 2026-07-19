use bevy_math::Mat4;
use bevy_transform::components::Transform;
use nw_objectstream::Element;
use nw_objectstream::query::child_by_field_ignore_case_or_crc;
use nw_objectstream::value::{self, ObjectStreamValueError};
use nw_reflected_types::az::rtti::AzRtti;
use nw_reflected_types::types::Component;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum LmbrCentralObjectStreamError {
    #[error("unexpected ObjectStream type: expected {expected}, actual {actual}")]
    UnexpectedType {
        expected: &'static str,
        actual: Uuid,
    },
    #[error("unsupported {type_name} ObjectStream version {version}; newest supported is {newest}")]
    UnsupportedVersion {
        type_name: &'static str,
        version: u8,
        newest: u8,
    },
    #[error("legacy {type_name} version {version} is missing converter field {field}")]
    MissingLegacyField {
        type_name: &'static str,
        version: u8,
        field: &'static str,
    },
    #[error("read {field} field")]
    Field {
        field: &'static str,
        #[source]
        source: ObjectStreamValueError,
    },
    #[error("unknown {type_name} {field} value {value}")]
    InvalidEnum {
        type_name: &'static str,
        field: &'static str,
        value: u8,
    },
}

pub(super) fn ensure_type(
    element: &Element,
    expected_id: Uuid,
    expected: &'static str,
) -> Result<(), LmbrCentralObjectStreamError> {
    if *element.id() == expected_id {
        Ok(())
    } else {
        Err(LmbrCentralObjectStreamError::UnexpectedType {
            expected,
            actual: *element.id(),
        })
    }
}

pub(super) fn checked_version(
    element: &Element,
    type_name: &'static str,
    newest: u8,
) -> Result<u8, LmbrCentralObjectStreamError> {
    match element.version() {
        Some(version) if version > newest => {
            Err(LmbrCentralObjectStreamError::UnsupportedVersion {
                type_name,
                version,
                newest,
            })
        }
        Some(version) => Ok(version),
        None => Ok(newest),
    }
}

#[inline]
pub(super) fn child<'a>(element: &'a Element, field: &str) -> Option<&'a Element> {
    child_by_field_ignore_case_or_crc(element, field, value::az_field_name_crc(field))
}

pub(super) fn required_legacy_child<'a>(
    element: &'a Element,
    type_name: &'static str,
    version: u8,
    field: &'static str,
) -> Result<&'a Element, LmbrCentralObjectStreamError> {
    child(element, field).ok_or(LmbrCentralObjectStreamError::MissingLegacyField {
        type_name,
        version,
        field,
    })
}

pub(super) fn read_optional<T>(
    element: &Element,
    field: &'static str,
    read: impl FnOnce(&Element) -> Result<T, ObjectStreamValueError>,
) -> Result<Option<T>, LmbrCentralObjectStreamError> {
    child(element, field)
        .map(read)
        .transpose()
        .map_err(|source| LmbrCentralObjectStreamError::Field { field, source })
}

pub(super) fn read_required<T>(
    element: &Element,
    type_name: &'static str,
    version: u8,
    field: &'static str,
    read: impl FnOnce(&Element) -> Result<T, ObjectStreamValueError>,
) -> Result<T, LmbrCentralObjectStreamError> {
    let field_element = required_legacy_child(element, type_name, version, field)?;
    read(field_element).map_err(|source| LmbrCentralObjectStreamError::Field { field, source })
}

pub(super) fn read_exact_string(
    element: &Element,
    field: &'static str,
) -> Result<Option<String>, LmbrCentralObjectStreamError> {
    let Some(field_element) = child(element, field) else {
        return Ok(None);
    };
    value::read_string(field_element)
        .map(|value| Some(value.to_owned()))
        .map_err(|source| LmbrCentralObjectStreamError::Field { field, source })
}

pub(super) fn read_transform(element: &Element) -> Result<Transform, ObjectStreamValueError> {
    let values = value::read_transform(element)?;
    Ok(Transform::from_matrix(Mat4::from_cols_array(&[
        values[0], values[1], values[2], 0.0, values[3], values[4], values[5], 0.0, values[6],
        values[7], values[8], 0.0, values[9], values[10], values[11], 1.0,
    ])))
}

pub(super) fn read_component_base(
    element: &Element,
) -> Result<Component, LmbrCentralObjectStreamError> {
    let Some(base) = child(element, "BaseClass1") else {
        return Ok(Component::default());
    };
    ensure_type(base, *Component::TYPE_ID.as_inner(), Component::NAME)?;
    let id = read_optional(base, "Id", value::read_u64_scalar)?.unwrap_or_default();
    Ok(Component { id })
}
