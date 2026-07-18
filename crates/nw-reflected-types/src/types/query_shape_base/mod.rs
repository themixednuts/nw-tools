use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

pub mod query_shape_aabb;
pub mod query_shape_box;
pub mod query_shape_capsule;
pub mod query_shape_cylinder;
pub mod query_shape_point;
pub mod query_shape_sphere;

pub use self::query_shape_aabb::QueryShapeAabb;
pub use self::query_shape_box::QueryShapeBox;
pub use self::query_shape_capsule::QueryShapeCapsule;
pub use self::query_shape_cylinder::QueryShapeCylinder;
pub use self::query_shape_point::QueryShapePoint;
pub use self::query_shape_sphere::QueryShapeSphere;

#[derive(Debug, Clone, PartialEq, bevy_reflect::Reflect)]
#[reflect(Serialize, Deserialize)]
pub enum QueryShape {
    Aabb(QueryShapeAabb),
    Box(QueryShapeBox),
    Capsule(QueryShapeCapsule),
    Cylinder(QueryShapeCylinder),
    Point(QueryShapePoint),
    Sphere(QueryShapeSphere),
}

impl ::core::default::Default for QueryShape {
    fn default() -> Self {
        Self::Point(<QueryShapePoint as ::core::default::Default>::default())
    }
}

impl ::serde::Serialize for QueryShape {
    fn serialize<S>(&self, serializer: S) -> ::core::result::Result<S::Ok, S::Error>
    where
        S: ::serde::Serializer,
    {
        match self {
            Self::Aabb(payload) => {
                let mut fields =
                    match ::serde_json::to_value(payload).map_err(::serde::ser::Error::custom)? {
                        ::serde_json::Value::Object(fields) => fields,
                        ::serde_json::Value::Null => ::serde_json::Map::new(),
                        value => {
                            let mut fields = ::serde_json::Map::new();
                            fields.insert("value".to_owned(), value);
                            fields
                        }
                    };
                fields.insert(
                    "$type".to_owned(),
                    ::serde_json::Value::String(<QueryShapeAabb as AzRtti>::TYPE_ID.to_string()),
                );
                ::serde::Serialize::serialize(&::serde_json::Value::Object(fields), serializer)
            }
            Self::Box(payload) => {
                let mut fields =
                    match ::serde_json::to_value(payload).map_err(::serde::ser::Error::custom)? {
                        ::serde_json::Value::Object(fields) => fields,
                        ::serde_json::Value::Null => ::serde_json::Map::new(),
                        value => {
                            let mut fields = ::serde_json::Map::new();
                            fields.insert("value".to_owned(), value);
                            fields
                        }
                    };
                fields.insert(
                    "$type".to_owned(),
                    ::serde_json::Value::String(<QueryShapeBox as AzRtti>::TYPE_ID.to_string()),
                );
                ::serde::Serialize::serialize(&::serde_json::Value::Object(fields), serializer)
            }
            Self::Capsule(payload) => {
                let mut fields =
                    match ::serde_json::to_value(payload).map_err(::serde::ser::Error::custom)? {
                        ::serde_json::Value::Object(fields) => fields,
                        ::serde_json::Value::Null => ::serde_json::Map::new(),
                        value => {
                            let mut fields = ::serde_json::Map::new();
                            fields.insert("value".to_owned(), value);
                            fields
                        }
                    };
                fields.insert(
                    "$type".to_owned(),
                    ::serde_json::Value::String(<QueryShapeCapsule as AzRtti>::TYPE_ID.to_string()),
                );
                ::serde::Serialize::serialize(&::serde_json::Value::Object(fields), serializer)
            }
            Self::Cylinder(payload) => {
                let mut fields =
                    match ::serde_json::to_value(payload).map_err(::serde::ser::Error::custom)? {
                        ::serde_json::Value::Object(fields) => fields,
                        ::serde_json::Value::Null => ::serde_json::Map::new(),
                        value => {
                            let mut fields = ::serde_json::Map::new();
                            fields.insert("value".to_owned(), value);
                            fields
                        }
                    };
                fields.insert(
                    "$type".to_owned(),
                    ::serde_json::Value::String(
                        <QueryShapeCylinder as AzRtti>::TYPE_ID.to_string(),
                    ),
                );
                ::serde::Serialize::serialize(&::serde_json::Value::Object(fields), serializer)
            }
            Self::Point(_) => {
                let mut fields = ::serde_json::Map::new();
                fields.insert(
                    "$type".to_owned(),
                    ::serde_json::Value::String(<QueryShapePoint as AzRtti>::TYPE_ID.to_string()),
                );
                ::serde::Serialize::serialize(&::serde_json::Value::Object(fields), serializer)
            }
            Self::Sphere(payload) => {
                let mut fields =
                    match ::serde_json::to_value(payload).map_err(::serde::ser::Error::custom)? {
                        ::serde_json::Value::Object(fields) => fields,
                        ::serde_json::Value::Null => ::serde_json::Map::new(),
                        value => {
                            let mut fields = ::serde_json::Map::new();
                            fields.insert("value".to_owned(), value);
                            fields
                        }
                    };
                fields.insert(
                    "$type".to_owned(),
                    ::serde_json::Value::String(<QueryShapeSphere as AzRtti>::TYPE_ID.to_string()),
                );
                ::serde::Serialize::serialize(&::serde_json::Value::Object(fields), serializer)
            }
        }
    }
}

impl<'de> ::serde::Deserialize<'de> for QueryShape {
    fn deserialize<D>(deserializer: D) -> ::core::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        struct Visitor;
        impl<'de> ::serde::de::Visitor<'de> for Visitor {
            type Value = QueryShape;
            fn expecting(&self, formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                formatter.write_str("AZ polymorphic object with a `$type` field")
            }
            fn visit_map<A>(self, mut map: A) -> ::core::result::Result<Self::Value, A::Error>
            where
                A: ::serde::de::MapAccess<'de>,
            {
                fn merge_sum_payload_defaults(
                    value: &mut ::serde_json::Value,
                    source: ::serde_json::Value,
                ) {
                    match (value, source) {
                        (
                            ::serde_json::Value::Object(value),
                            ::serde_json::Value::Object(source),
                        ) => {
                            for (key, source_value) in source {
                                match value.get_mut(&key) {
                                    Some(value) => merge_sum_payload_defaults(value, source_value),
                                    None => {
                                        value.insert(key, source_value);
                                    }
                                }
                            }
                        }
                        (value, source) => *value = source,
                    }
                }
                let Some(key) = map.next_key::<String>()? else {
                    return Err(::serde::de::Error::missing_field("$type"));
                };
                if key != "$type" {
                    return Err(::serde::de::Error::custom(format!(
                        "expected `$type` as first field for {}, got `{}`",
                        stringify!(QueryShape),
                        key,
                    )));
                }
                let type_id = map.next_value::<String>()?;
                let type_id = AzUuid::parse_str(&type_id).map_err(::serde::de::Error::custom)?;
                if type_id == <QueryShapeAabb as AzRtti>::TYPE_ID {
                    let source_fields = <::serde_json::Map<
                        String,
                        ::serde_json::Value,
                    > as ::serde::Deserialize>::deserialize(
                        ::serde::de::value::MapAccessDeserializer::new(map),
                    )?;
                    let mut value = ::serde_json::to_value(
                        <QueryShapeAabb as ::core::default::Default>::default(),
                    )
                    .map_err(::serde::de::Error::custom)?;
                    merge_sum_payload_defaults(
                        &mut value,
                        ::serde_json::Value::Object(source_fields),
                    );
                    return ::serde_json::from_value::<QueryShapeAabb>(value)
                        .map(QueryShape::Aabb)
                        .map_err(::serde::de::Error::custom);
                }
                if type_id == <QueryShapeBox as AzRtti>::TYPE_ID {
                    let source_fields = <::serde_json::Map<
                        String,
                        ::serde_json::Value,
                    > as ::serde::Deserialize>::deserialize(
                        ::serde::de::value::MapAccessDeserializer::new(map),
                    )?;
                    let mut value = ::serde_json::to_value(
                        <QueryShapeBox as ::core::default::Default>::default(),
                    )
                    .map_err(::serde::de::Error::custom)?;
                    merge_sum_payload_defaults(
                        &mut value,
                        ::serde_json::Value::Object(source_fields),
                    );
                    return ::serde_json::from_value::<QueryShapeBox>(value)
                        .map(QueryShape::Box)
                        .map_err(::serde::de::Error::custom);
                }
                if type_id == <QueryShapeCapsule as AzRtti>::TYPE_ID {
                    let source_fields = <::serde_json::Map<
                        String,
                        ::serde_json::Value,
                    > as ::serde::Deserialize>::deserialize(
                        ::serde::de::value::MapAccessDeserializer::new(map),
                    )?;
                    let mut value = ::serde_json::to_value(
                        <QueryShapeCapsule as ::core::default::Default>::default(),
                    )
                    .map_err(::serde::de::Error::custom)?;
                    merge_sum_payload_defaults(
                        &mut value,
                        ::serde_json::Value::Object(source_fields),
                    );
                    return ::serde_json::from_value::<QueryShapeCapsule>(value)
                        .map(QueryShape::Capsule)
                        .map_err(::serde::de::Error::custom);
                }
                if type_id == <QueryShapeCylinder as AzRtti>::TYPE_ID {
                    let source_fields = <::serde_json::Map<
                        String,
                        ::serde_json::Value,
                    > as ::serde::Deserialize>::deserialize(
                        ::serde::de::value::MapAccessDeserializer::new(map),
                    )?;
                    let mut value = ::serde_json::to_value(
                        <QueryShapeCylinder as ::core::default::Default>::default(),
                    )
                    .map_err(::serde::de::Error::custom)?;
                    merge_sum_payload_defaults(
                        &mut value,
                        ::serde_json::Value::Object(source_fields),
                    );
                    return ::serde_json::from_value::<QueryShapeCylinder>(value)
                        .map(QueryShape::Cylinder)
                        .map_err(::serde::de::Error::custom);
                }
                if type_id == <QueryShapePoint as AzRtti>::TYPE_ID {
                    while let Some(_extra) = map.next_key::<String>()? {
                        let _ = map.next_value::<::serde::de::IgnoredAny>()?;
                    }
                    return Ok(QueryShape::Point(
                        <QueryShapePoint as ::core::default::Default>::default(),
                    ));
                }
                if type_id == <QueryShapeSphere as AzRtti>::TYPE_ID {
                    let source_fields = <::serde_json::Map<
                        String,
                        ::serde_json::Value,
                    > as ::serde::Deserialize>::deserialize(
                        ::serde::de::value::MapAccessDeserializer::new(map),
                    )?;
                    let mut value = ::serde_json::to_value(
                        <QueryShapeSphere as ::core::default::Default>::default(),
                    )
                    .map_err(::serde::de::Error::custom)?;
                    merge_sum_payload_defaults(
                        &mut value,
                        ::serde_json::Value::Object(source_fields),
                    );
                    return ::serde_json::from_value::<QueryShapeSphere>(value)
                        .map(QueryShape::Sphere)
                        .map_err(::serde::de::Error::custom);
                }
                Err(::serde::de::Error::custom(format!(
                    "unknown {} concrete type {}",
                    stringify!(QueryShape),
                    type_id,
                )))
            }
        }
        deserializer.deserialize_map(Visitor)
    }
}

impl AzRtti for QueryShape {
    const NAME: &'static str = "QueryShapeBase";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xCF5EB8E9_9C03_4A19_828D_ED500C732978);
}
