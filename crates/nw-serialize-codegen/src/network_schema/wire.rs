use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkNativeTypeInfoEvidence {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub native_size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub native_size_source: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NetworkReplicatedContainerStorageKind {
    Map,
    Vec,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NetworkPackedPositionWireShape {
    pub minimum_bits: u32,
    pub maximum_bits: u32,
}

impl NetworkPackedPositionWireShape {
    #[must_use]
    pub const fn minimum(self) -> f32 {
        f32::from_bits(self.minimum_bits)
    }

    #[must_use]
    pub const fn maximum(self) -> f32 {
        f32::from_bits(self.maximum_bits)
    }

    pub(crate) fn wire_string(self) -> String {
        format!(
            "packed-position<0x{:08x},0x{:08x}>",
            self.minimum_bits, self.maximum_bits
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkWireScalarShape {
    Bool,
    U8,
    U16,
    U32,
    U64,
    F32,
    F64,
    HalfF32,
    VlqU32,
    VlqU64,
    SequenceNumber,
    Vec2,
    Vec3,
    Vec4,
    Quat,
    QuatCompNorm,
    Vec2Comp,
    Vec3Comp,
    Vec3CompNorm,
    Vec3SmallestThree,
    QuatComp,
    QuatSmallestThree,
    NonUniformScaleComp,
    DeltaVec3(u32),
    RemoteServerGdeRef,
    PackedPosition(NetworkPackedPositionWireShape),
    TransformCompressor,
    PackedSize,
    Mat3,
    Affine3,
    Aabb2d,
    Aabb3d,
    ActorRef,
    EntityRef,
    FixedBytes(u16),
    Bytes,
    String,
}

impl Serialize for NetworkWireScalarShape {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.wire_string())
    }
}

impl<'de> Deserialize<'de> for NetworkWireScalarShape {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        parse_network_wire_scalar_shape(&value)
            .ok_or_else(|| serde::de::Error::custom(format!("unknown wire scalar shape `{value}`")))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NetworkReplicatedContainerWireShape {
    pub key: NetworkWireScalarShape,
    pub value: NetworkWireScalarShape,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkWireShape {
    Bool,
    U8,
    U16,
    U32,
    U64,
    F32,
    F64,
    HalfF32,
    VlqU32,
    VlqU64,
    SequenceNumber,
    Vec2,
    Vec3,
    Vec4,
    Quat,
    QuatCompNorm,
    Vec2Comp,
    Vec3Comp,
    Vec3CompNorm,
    Vec3SmallestThree,
    QuatComp,
    QuatSmallestThree,
    NonUniformScaleComp,
    DeltaVec3(u32),
    RemoteServerGdeRef,
    PackedPosition(NetworkPackedPositionWireShape),
    TransformCompressor,
    PackedSize,
    Mat3,
    Affine3,
    Aabb2d,
    Aabb3d,
    ActorRef,
    EntityRef,
    FixedBytes(u16),
    Bytes,
    String,
    Composite(Vec<Self>),
    Optional(Box<Self>),
    ReplicatedContainer(NetworkReplicatedContainerWireShape),
    FixedSequence(NetworkFixedSequenceWireShape),
}

impl NetworkWireScalarShape {
    fn as_static_str(self) -> Option<&'static str> {
        match self {
            Self::Bool => Some("bool"),
            Self::U8 => Some("u8"),
            Self::U16 => Some("u16"),
            Self::U32 => Some("u32"),
            Self::U64 => Some("u64"),
            Self::F32 => Some("f32"),
            Self::F64 => Some("f64"),
            Self::HalfF32 => Some("half-f32"),
            Self::VlqU32 => Some("vlq-u32"),
            Self::VlqU64 => Some("vlq-u64"),
            Self::SequenceNumber => Some("sequence-number"),
            Self::Vec2 => Some("vec2"),
            Self::Vec3 => Some("vec3"),
            Self::Vec4 => Some("vec4"),
            Self::Quat => Some("quat"),
            Self::QuatCompNorm => Some("quat-comp-norm"),
            Self::Vec2Comp => Some("vec2-comp"),
            Self::Vec3Comp => Some("vec3-comp"),
            Self::Vec3CompNorm => Some("vec3-comp-norm"),
            Self::Vec3SmallestThree => Some("vec3-smallest-three"),
            Self::QuatComp => Some("quat-comp"),
            Self::QuatSmallestThree => Some("quat-smallest-three"),
            Self::NonUniformScaleComp => Some("non-uniform-scale-comp"),
            Self::DeltaVec3(_) => None,
            Self::RemoteServerGdeRef => Some("remote-server-gde-ref"),
            Self::PackedPosition(_) => None,
            Self::TransformCompressor => Some("transform-compressor"),
            Self::PackedSize => Some("packed-size"),
            Self::Mat3 => Some("mat3"),
            Self::Affine3 => Some("affine3"),
            Self::Aabb2d => Some("aabb2d"),
            Self::Aabb3d => Some("aabb3d"),
            Self::ActorRef => Some("actor-ref"),
            Self::EntityRef => Some("entity-ref"),
            Self::FixedBytes(_) => None,
            Self::Bytes => Some("length-prefixed-bytes"),
            Self::String => Some("string"),
        }
    }

    pub(crate) fn wire_string(self) -> String {
        self.as_static_str().map_or_else(
            || match self {
                Self::FixedBytes(len) => format!("fixed-bytes-{len}"),
                Self::DeltaVec3(range) => format!("delta-vec3<{range}>"),
                Self::PackedPosition(shape) => shape.wire_string(),
                _ => unreachable!("non-static wire scalar handled above"),
            },
            ToOwned::to_owned,
        )
    }
}

impl From<NetworkWireScalarShape> for NetworkWireShape {
    fn from(value: NetworkWireScalarShape) -> Self {
        match value {
            NetworkWireScalarShape::Bool => Self::Bool,
            NetworkWireScalarShape::U8 => Self::U8,
            NetworkWireScalarShape::U16 => Self::U16,
            NetworkWireScalarShape::U32 => Self::U32,
            NetworkWireScalarShape::U64 => Self::U64,
            NetworkWireScalarShape::F32 => Self::F32,
            NetworkWireScalarShape::F64 => Self::F64,
            NetworkWireScalarShape::HalfF32 => Self::HalfF32,
            NetworkWireScalarShape::VlqU32 => Self::VlqU32,
            NetworkWireScalarShape::VlqU64 => Self::VlqU64,
            NetworkWireScalarShape::SequenceNumber => Self::SequenceNumber,
            NetworkWireScalarShape::Vec2 => Self::Vec2,
            NetworkWireScalarShape::Vec3 => Self::Vec3,
            NetworkWireScalarShape::Vec4 => Self::Vec4,
            NetworkWireScalarShape::Quat => Self::Quat,
            NetworkWireScalarShape::QuatCompNorm => Self::QuatCompNorm,
            NetworkWireScalarShape::Vec2Comp => Self::Vec2Comp,
            NetworkWireScalarShape::Vec3Comp => Self::Vec3Comp,
            NetworkWireScalarShape::Vec3CompNorm => Self::Vec3CompNorm,
            NetworkWireScalarShape::Vec3SmallestThree => Self::Vec3SmallestThree,
            NetworkWireScalarShape::QuatComp => Self::QuatComp,
            NetworkWireScalarShape::QuatSmallestThree => Self::QuatSmallestThree,
            NetworkWireScalarShape::NonUniformScaleComp => Self::NonUniformScaleComp,
            NetworkWireScalarShape::DeltaVec3(range) => Self::DeltaVec3(range),
            NetworkWireScalarShape::RemoteServerGdeRef => Self::RemoteServerGdeRef,
            NetworkWireScalarShape::PackedPosition(shape) => Self::PackedPosition(shape),
            NetworkWireScalarShape::TransformCompressor => Self::TransformCompressor,
            NetworkWireScalarShape::PackedSize => Self::PackedSize,
            NetworkWireScalarShape::Mat3 => Self::Mat3,
            NetworkWireScalarShape::Affine3 => Self::Affine3,
            NetworkWireScalarShape::Aabb2d => Self::Aabb2d,
            NetworkWireScalarShape::Aabb3d => Self::Aabb3d,
            NetworkWireScalarShape::ActorRef => Self::ActorRef,
            NetworkWireScalarShape::EntityRef => Self::EntityRef,
            NetworkWireScalarShape::FixedBytes(len) => Self::FixedBytes(len),
            NetworkWireScalarShape::Bytes => Self::Bytes,
            NetworkWireScalarShape::String => Self::String,
        }
    }
}

impl NetworkWireShape {
    pub(crate) const fn from_self_describing_layout(layout: &str) -> Option<Self> {
        match layout.as_bytes() {
            b"length-prefixed-bytes" => Some(Self::Bytes),
            _ => None,
        }
    }

    #[must_use]
    pub const fn is_replicated_container(&self) -> bool {
        matches!(self, Self::ReplicatedContainer(_))
    }

    fn as_static_str(&self) -> Option<&'static str> {
        match self {
            Self::Bool => Some("bool"),
            Self::U8 => Some("u8"),
            Self::U16 => Some("u16"),
            Self::U32 => Some("u32"),
            Self::U64 => Some("u64"),
            Self::F32 => Some("f32"),
            Self::F64 => Some("f64"),
            Self::HalfF32 => Some("half-f32"),
            Self::VlqU32 => Some("vlq-u32"),
            Self::VlqU64 => Some("vlq-u64"),
            Self::SequenceNumber => Some("sequence-number"),
            Self::Vec2 => Some("vec2"),
            Self::Vec3 => Some("vec3"),
            Self::Vec4 => Some("vec4"),
            Self::Quat => Some("quat"),
            Self::QuatCompNorm => Some("quat-comp-norm"),
            Self::Vec2Comp => Some("vec2-comp"),
            Self::Vec3Comp => Some("vec3-comp"),
            Self::Vec3CompNorm => Some("vec3-comp-norm"),
            Self::Vec3SmallestThree => Some("vec3-smallest-three"),
            Self::QuatComp => Some("quat-comp"),
            Self::QuatSmallestThree => Some("quat-smallest-three"),
            Self::NonUniformScaleComp => Some("non-uniform-scale-comp"),
            Self::RemoteServerGdeRef => Some("remote-server-gde-ref"),
            Self::TransformCompressor => Some("transform-compressor"),
            Self::PackedSize => Some("packed-size"),
            Self::Mat3 => Some("mat3"),
            Self::Affine3 => Some("affine3"),
            Self::Aabb2d => Some("aabb2d"),
            Self::Aabb3d => Some("aabb3d"),
            Self::ActorRef => Some("actor-ref"),
            Self::EntityRef => Some("entity-ref"),
            Self::Bytes => Some("length-prefixed-bytes"),
            Self::FixedBytes(_)
            | Self::DeltaVec3(_)
            | Self::PackedPosition(_)
            | Self::Composite(_)
            | Self::Optional(_)
            | Self::ReplicatedContainer(_)
            | Self::FixedSequence(_) => None,
            Self::String => Some("string"),
        }
    }

    pub(super) fn wire_string(&self) -> String {
        self.as_static_str().map_or_else(
            || match self {
                Self::FixedBytes(len) => format!("fixed-bytes-{len}"),
                Self::DeltaVec3(range) => format!("delta-vec3<{range}>"),
                Self::PackedPosition(shape) => shape.wire_string(),
                Self::Composite(members) => format!(
                    "composite<{}>",
                    members
                        .iter()
                        .map(Self::wire_string)
                        .collect::<Vec<_>>()
                        .join(",")
                ),
                Self::Optional(value) => format!("optional<{}>", value.wire_string()),
                Self::ReplicatedContainer(container) => format!(
                    "replicated-container<{},{}>",
                    container.key.wire_string(),
                    container.value.wire_string()
                ),
                Self::FixedSequence(sequence) => format!(
                    "fixed-vector<{},{}>",
                    sequence.element.wire_string(),
                    sequence.capacity
                ),
                _ => unreachable!("non-static wire shape handled above"),
            },
            ToOwned::to_owned,
        )
    }
}

impl Serialize for NetworkWireShape {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if let Some(value) = self.as_static_str() {
            return serializer.serialize_str(value);
        }
        match self {
            Self::FixedBytes(len) => serializer.serialize_str(&format!("fixed-bytes-{len}")),
            Self::DeltaVec3(range) => serializer.serialize_str(&format!("delta-vec3<{range}>")),
            Self::PackedPosition(shape) => serializer.serialize_str(&shape.wire_string()),
            Self::Composite(members) => serializer.serialize_str(&format!(
                "composite<{}>",
                members
                    .iter()
                    .map(Self::wire_string)
                    .collect::<Vec<_>>()
                    .join(",")
            )),
            Self::Optional(value) => {
                serializer.serialize_str(&format!("optional<{}>", value.wire_string()))
            }
            Self::ReplicatedContainer(container) => serializer.serialize_str(&format!(
                "replicated-container<{},{}>",
                container.key.wire_string(),
                container.value.wire_string()
            )),
            Self::FixedSequence(sequence) => serializer.serialize_str(&format!(
                "fixed-vector<{},{}>",
                sequence.element.wire_string(),
                sequence.capacity
            )),
            _ => unreachable!("non-static wire shape handled above"),
        }
    }
}

impl<'de> Deserialize<'de> for NetworkWireShape {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        parse_network_wire_shape(&value)
            .ok_or_else(|| serde::de::Error::custom(format!("unknown wire shape `{value}`")))
    }
}
