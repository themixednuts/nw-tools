//! Serde-like ObjectStream object traversal.
//!
//! The `value` module owns leaf payload conversion. This module owns
//! object-shaped field traversal so callers describe field intent instead of
//! manually driving `FieldCursor`.

use uuid::Uuid;

use crate::value::{DecodeAzValue, FieldCursor, ObjectStreamValueError};
use crate::{Element, types};

pub trait Deserialize<'a>: Sized {
    fn deserialize(fields: &mut ObjectFields<'a>) -> Result<Self, ObjectStreamValueError>;
}

pub trait Serialize {
    fn serialize(&self) -> Element;
}

pub fn serialize<T>(value: &T) -> Element
where
    T: Serialize + ?Sized,
{
    value.serialize()
}

#[derive(Debug, Clone)]
pub struct ObjectFields<'a> {
    fields: FieldCursor<'a>,
    path: String,
}

impl<'a> ObjectFields<'a> {
    #[inline]
    #[must_use]
    pub fn from_element(element: &'a Element) -> Self {
        Self::from_element_at(
            element,
            element.field().map_or("<object>", |field| field.as_str()),
        )
    }

    #[inline]
    #[must_use]
    pub fn from_element_at(element: &'a Element, path: impl Into<String>) -> Self {
        Self {
            fields: FieldCursor::from_element(element),
            path: path.into(),
        }
    }

    pub fn required<T>(&mut self, field: &str) -> Result<T, ObjectStreamValueError>
    where
        T: DecodeAzValue<'a>,
    {
        let element = self.required_element(field)?;
        T::decode_az_value(element)
    }

    pub fn required_with<T>(
        &mut self,
        field: &str,
        read: impl FnOnce(&'a Element) -> Result<T, ObjectStreamValueError>,
    ) -> Result<T, ObjectStreamValueError> {
        read(self.required_element(field)?)
    }

    pub fn optional<T>(&mut self, field: &str) -> Result<Option<T>, ObjectStreamValueError>
    where
        T: DecodeAzValue<'a>,
    {
        self.fields.find(field).map(T::decode_az_value).transpose()
    }

    pub fn defaulted<T>(&mut self, field: &str) -> Result<T, ObjectStreamValueError>
    where
        T: DecodeAzValue<'a> + Default,
    {
        self.optional(field).map(Option::unwrap_or_default)
    }

    pub fn required_element(&mut self, field: &str) -> Result<&'a Element, ObjectStreamValueError> {
        self.fields
            .find(field)
            .ok_or_else(|| ObjectStreamValueError::MissingField {
                field: self.field_path(field),
            })
    }

    pub fn optional_element(&mut self, field: &str) -> Option<&'a Element> {
        self.fields.find(field)
    }

    pub fn object<T>(&mut self, field: &str) -> Result<T, ObjectStreamValueError>
    where
        T: Deserialize<'a>,
    {
        let element = self.required_element(field)?;
        let mut fields = ObjectFields::from_element_at(element, self.field_path(field));
        let value = T::deserialize(&mut fields)?;
        fields.finish()?;
        Ok(value)
    }

    pub fn object_with<T>(
        &mut self,
        field: &str,
        read: impl FnOnce(&'a Element) -> Result<T, ObjectStreamValueError>,
    ) -> Result<T, ObjectStreamValueError> {
        read(self.required_element(field)?)
    }

    pub fn list_with<T>(
        &mut self,
        field: &str,
        item_type: Uuid,
        read: impl FnMut(&'a Element) -> Result<T, ObjectStreamValueError>,
    ) -> Result<Vec<T>, ObjectStreamValueError> {
        self.required_element(field)?
            .children()
            .iter()
            .filter(|child| child.id() == &item_type)
            .map(read)
            .collect()
    }

    pub fn map_with<K, V>(
        &mut self,
        field: &str,
        mut read_key: impl FnMut(&'a Element) -> Result<K, ObjectStreamValueError>,
        mut read_value: impl FnMut(&'a Element) -> Result<V, ObjectStreamValueError>,
    ) -> Result<Vec<(K, V)>, ObjectStreamValueError> {
        self.required_element(field)?
            .children()
            .iter()
            .map(|pair| {
                let (key, value) = map_pair(pair)?;
                Ok((read_key(key)?, read_value(value)?))
            })
            .collect()
    }

    pub fn finish(self) -> Result<(), ObjectStreamValueError> {
        if let Some(element) = self.fields.remaining().first() {
            return Err(ObjectStreamValueError::UnknownField {
                field: self.field_path(element.field().map_or("<unnamed>", |field| field.as_str())),
            });
        }
        Ok(())
    }

    fn field_path(&self, field: &str) -> String {
        if self.path == "<object>" {
            field.to_string()
        } else {
            format!("{}.{}", self.path, field)
        }
    }
}

pub fn deserialize<'a, T>(element: &'a Element) -> Result<T, ObjectStreamValueError>
where
    T: Deserialize<'a>,
{
    let mut fields = ObjectFields::from_element(element);
    let value = T::deserialize(&mut fields)?;
    fields.finish()?;
    Ok(value)
}

pub fn map_pair(pair: &Element) -> Result<(&Element, &Element), ObjectStreamValueError> {
    let [key, value] = pair.children() else {
        return Err(ObjectStreamValueError::UnexpectedType {
            field: pair
                .field()
                .map_or_else(|| "<map pair>".to_string(), ToString::to_string),
            expected: "AZStd::pair with key and value children",
            actual: *pair.id(),
        });
    };
    Ok((key, value))
}

pub fn ensure_type(
    element: &Element,
    expected: Uuid,
    expected_name: &'static str,
) -> Result<(), ObjectStreamValueError> {
    if element.id() == &expected {
        Ok(())
    } else {
        Err(ObjectStreamValueError::UnexpectedType {
            field: element
                .field()
                .map_or_else(|| "<object>".to_string(), ToString::to_string),
            expected: expected_name,
            actual: *element.id(),
        })
    }
}

pub fn string_key(element: &Element) -> Result<Box<str>, ObjectStreamValueError> {
    element.decode()
}

pub fn object_vector<'a, T>(
    element: &'a Element,
    item_type: Uuid,
    read: impl FnMut(&'a Element) -> Result<T, ObjectStreamValueError>,
) -> Result<Vec<T>, ObjectStreamValueError> {
    ensure_type(element, types::AZSTD_VECTOR, "AZStd::vector")?;
    element
        .children()
        .iter()
        .filter(|child| child.id() == &item_type)
        .map(read)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types;

    #[derive(Debug, PartialEq)]
    struct Shape {
        name: Box<str>,
        enabled: bool,
        count: u32,
    }

    #[derive(Debug, PartialEq)]
    struct Container {
        shape: Shape,
    }

    impl<'a> Deserialize<'a> for Shape {
        fn deserialize(fields: &mut ObjectFields<'a>) -> Result<Self, ObjectStreamValueError> {
            Ok(Self {
                name: fields.required("Name")?,
                enabled: fields.defaulted("Enabled")?,
                count: fields.required("Count")?,
            })
        }
    }

    impl<'a> Deserialize<'a> for Container {
        fn deserialize(fields: &mut ObjectFields<'a>) -> Result<Self, ObjectStreamValueError> {
            Ok(Self {
                shape: fields.object("Shape")?,
            })
        }
    }

    fn leaf(field: &str, id: Uuid, data: impl Into<Vec<u8>>) -> Element {
        Element::new(id).with_field(field).with_data(data)
    }

    #[test]
    fn object_deserialize_reads_required_and_defaulted_fields() {
        let element = Element::new(types::AZSTD_VECTOR).with_children([
            leaf("Name", types::AZSTD_STRING, b"Crate"),
            leaf("Count", types::UNSIGNED_INT, 7_u32.to_be_bytes()),
        ]);

        let shape: Shape = deserialize(&element).unwrap();

        assert_eq!(
            shape,
            Shape {
                name: Box::<str>::from("Crate"),
                enabled: false,
                count: 7,
            }
        );
    }

    #[test]
    fn object_deserialize_rejects_unread_fields() {
        let element = Element::new(types::AZSTD_VECTOR).with_children([
            leaf("Name", types::AZSTD_STRING, b"Crate"),
            leaf("Count", types::UNSIGNED_INT, 7_u32.to_be_bytes()),
            leaf("Extra", types::BOOL, [1]),
        ]);

        let err = deserialize::<Shape>(&element).unwrap_err();

        assert!(matches!(err, ObjectStreamValueError::UnknownField { .. }));
    }

    #[test]
    fn nested_object_errors_include_field_path() {
        let element =
            Element::new(types::AZSTD_VECTOR).with_children([Element::new(types::AZSTD_VECTOR)
                .with_field("Shape")
                .with_children([
                    leaf("Name", types::AZSTD_STRING, b"Crate"),
                    leaf("Count", types::UNSIGNED_INT, 7_u32.to_be_bytes()),
                    leaf("Extra", types::BOOL, [1]),
                ])]);

        let err = deserialize::<Container>(&element).unwrap_err();

        assert!(matches!(
            err,
            ObjectStreamValueError::UnknownField { field } if field == "Shape.Extra"
        ));
    }
}
