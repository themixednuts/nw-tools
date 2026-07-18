//! Parser for RockNRoll `.rnr` shape assets.
//!
//! The format is owned by New World's RockNRoll physics layer.

use glam::{Vec3, Vec4};
use thiserror::Error;

pub const MAGIC: u32 = 0x1234_ABCD;
const SHAPE_STREAM_VERSION: u32 = 0x69;
const MAX_RECURSION_DEPTH: usize = 64;

pub type ShapeTransform = [Vec4; 3];

#[derive(Debug, Clone, PartialEq)]
pub struct ShapeAsset<'a> {
    pub version: u32,
    pub objects: Vec<ShapeObject<'a>>,
    pub asset_guid: Option<[u8; 16]>,
    pub material_filter: MaterialFilter<'a>,
    pub shapes: Vec<PhysicalShape<'a>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShapeObject<'a> {
    pub name: &'a str,
    pub material_indices: U16LeSlice<'a>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MaterialFilter<'a> {
    pub enabled: bool,
    pub secondary_geometry: bool,
    pub indices: U16LeSlice<'a>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PhysicalShape<'a> {
    pub data: ShapeData<'a>,
    pub extra: Option<&'a [u8]>,
}

impl PhysicalShape<'_> {
    #[must_use]
    pub const fn kind(&self) -> ShapeKind {
        self.data.kind()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum ShapeKind {
    Box = 1,
    Sphere = 2,
    ConvexHull = 3,
    Cylinder = 4,
    CylinderUnaligned = 5,
    Capsule = 6,
    CapsuleUnaligned = 7,
    Triangle = 8,
    Mesh = 10,
    Compound = 11,
    Transform = 12,
    SoftBody = 13,
    Plane = 17,
    ScaleConvexPolytope = 18,
    ScaleMesh = 19,
    HeightField = 20,
}

impl TryFrom<u32> for ShapeKind {
    type Error = u32;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Box),
            2 => Ok(Self::Sphere),
            3 => Ok(Self::ConvexHull),
            4 => Ok(Self::Cylinder),
            5 => Ok(Self::CylinderUnaligned),
            6 => Ok(Self::Capsule),
            7 => Ok(Self::CapsuleUnaligned),
            8 => Ok(Self::Triangle),
            10 => Ok(Self::Mesh),
            11 => Ok(Self::Compound),
            12 => Ok(Self::Transform),
            13 => Ok(Self::SoftBody),
            17 => Ok(Self::Plane),
            18 => Ok(Self::ScaleConvexPolytope),
            19 => Ok(Self::ScaleMesh),
            20 => Ok(Self::HeightField),
            _ => Err(value),
        }
    }
}

impl From<ShapeKind> for u32 {
    fn from(value: ShapeKind) -> Self {
        value as Self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ShapeData<'a> {
    Box(BoxShape),
    Sphere(SphereShape),
    ConvexHull(ConvexHullShape<'a>),
    Cylinder(CylinderShape),
    CylinderUnaligned(CylinderUnalignedShape),
    Capsule(CapsuleShape),
    CapsuleUnaligned(CapsuleUnalignedShape),
    Triangle(TriangleShape),
    Mesh(MeshShape<'a>),
    Compound(CompoundShape<'a>),
    Transform(TransformShape<'a>),
    SoftBody(SoftBodyShape),
    Plane(PlaneShape),
    ScaleConvexPolytope(ScaledShape<'a>),
    ScaleMesh(ScaledShape<'a>),
    HeightField(HeightFieldShape<'a>),
}

impl ShapeData<'_> {
    #[must_use]
    pub const fn kind(&self) -> ShapeKind {
        match self {
            Self::Box(_) => ShapeKind::Box,
            Self::Sphere(_) => ShapeKind::Sphere,
            Self::ConvexHull(_) => ShapeKind::ConvexHull,
            Self::Cylinder(_) => ShapeKind::Cylinder,
            Self::CylinderUnaligned(_) => ShapeKind::CylinderUnaligned,
            Self::Capsule(_) => ShapeKind::Capsule,
            Self::CapsuleUnaligned(_) => ShapeKind::CapsuleUnaligned,
            Self::Triangle(_) => ShapeKind::Triangle,
            Self::Mesh(_) => ShapeKind::Mesh,
            Self::Compound(_) => ShapeKind::Compound,
            Self::Transform(_) => ShapeKind::Transform,
            Self::SoftBody(_) => ShapeKind::SoftBody,
            Self::Plane(_) => ShapeKind::Plane,
            Self::ScaleConvexPolytope(_) => ShapeKind::ScaleConvexPolytope,
            Self::ScaleMesh(_) => ShapeKind::ScaleMesh,
            Self::HeightField(_) => ShapeKind::HeightField,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoxShape {
    pub half_extents: Vec3,
    pub convex_radius: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SphereShape {
    pub radius: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConvexHullShape<'a> {
    pub vertices: Vec3LeSlice<'a>,
    pub planes: Vec4LeSlice<'a>,
    pub convex_radius: f32,
    pub extra: Option<ConvexHullExtra<'a>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConvexHullExtra<'a> {
    pub data_a: U16LeSlice<'a>,
    pub data_b: U16LeSlice<'a>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CylinderShape {
    pub half_height: f32,
    pub radius: f32,
    pub convex_radius: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CylinderUnalignedShape {
    pub endpoint_a: Vec3,
    pub endpoint_b: Vec3,
    pub radius: f32,
    pub convex_radius: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CapsuleShape {
    pub half_height: f32,
    pub radius: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CapsuleUnalignedShape {
    pub endpoint_a: Vec3,
    pub endpoint_b: Vec3,
    pub radius: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TriangleShape {
    pub a: Vec3,
    pub b: Vec3,
    pub c: Vec3,
    pub convex_radius: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MeshShape<'a> {
    pub stream_header: u32,
    pub vertices: Vec3LeSlice<'a>,
    pub indices: U16LeSlice<'a>,
    pub adjacent_triangles: Option<U16LeSlice<'a>>,
    pub bvh: BvhTree<'a>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompoundShape<'a> {
    pub children: Vec<CompoundChild<'a>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompoundChild<'a> {
    pub transform: ShapeTransform,
    pub shape: Box<PhysicalShape<'a>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TransformShape<'a> {
    pub transform: ShapeTransform,
    pub shape: Box<PhysicalShape<'a>>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SoftBodyShape;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlaneShape {
    pub plane: Vec4,
    pub aabb_min: Vec3,
    pub aabb_max: Vec3,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScaledShape<'a> {
    pub stream_header: u32,
    pub scale: Vec3,
    pub shape: Box<PhysicalShape<'a>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HeightFieldShape<'a> {
    pub layout: u32,
    pub data: Option<HeightFieldData<'a>>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeightFieldData<'a> {
    pub version: u32,
    pub width: u32,
    pub length: u32,
    pub height_scale: f32,
    pub aabb_min: Vec3,
    pub aabb_max: Vec3,
    pub samples: &'a [u8],
}

#[derive(Debug, Clone, PartialEq)]
pub enum BvhTree<'a> {
    V1(BvhTreeParts<'a>),
    V2(BvhTreeParts<'a>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BvhTreeParts<'a> {
    pub payload: &'a [u8],
    pub quantized_nodes: &'a [u8],
    pub subtree_infos: &'a [u8],
    pub triangle_index_map: &'a [u8],
    pub quantized_node_count: u32,
    pub subtree_info_count: u16,
    pub triangle_index_count: u32,
    pub flags: u16,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct U16LeSlice<'a> {
    bytes: &'a [u8],
}

impl<'a> U16LeSlice<'a> {
    #[must_use]
    pub const fn from_bytes_unchecked(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }

    #[must_use]
    pub const fn as_bytes(self) -> &'a [u8] {
        self.bytes
    }

    #[must_use]
    pub const fn len(self) -> usize {
        self.bytes.len() / 2
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.bytes.is_empty()
    }

    #[must_use]
    pub fn get(self, index: usize) -> Option<u16> {
        let offset = index.checked_mul(2)?;
        let bytes = self.bytes.get(offset..offset + 2)?;
        Some(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    #[must_use]
    pub fn iter(self) -> U16LeIter<'a> {
        U16LeIter {
            bytes: self.bytes,
            index: 0,
        }
    }
}

pub struct U16LeIter<'a> {
    bytes: &'a [u8],
    index: usize,
}

impl Iterator for U16LeIter<'_> {
    type Item = u16;

    fn next(&mut self) -> Option<Self::Item> {
        let offset = self.index.checked_mul(2)?;
        let bytes = self.bytes.get(offset..offset + 2)?;
        self.index += 1;
        Some(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.bytes.len() / 2 - self.index;
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for U16LeIter<'_> {}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Vec3LeSlice<'a> {
    bytes: &'a [u8],
}

impl<'a> Vec3LeSlice<'a> {
    #[must_use]
    pub const fn from_bytes_unchecked(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }

    #[must_use]
    pub const fn as_bytes(self) -> &'a [u8] {
        self.bytes
    }

    #[must_use]
    pub const fn len(self) -> usize {
        self.bytes.len() / 12
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.bytes.is_empty()
    }

    #[must_use]
    pub fn get(self, index: usize) -> Option<Vec3> {
        let offset = index.checked_mul(12)?;
        let bytes = self.bytes.get(offset..offset + 12)?;
        Some(vec3_from_chunk(bytes))
    }

    #[must_use]
    pub fn iter(self) -> Vec3LeIter<'a> {
        Vec3LeIter {
            bytes: self.bytes,
            index: 0,
        }
    }
}

pub struct Vec3LeIter<'a> {
    bytes: &'a [u8],
    index: usize,
}

impl Iterator for Vec3LeIter<'_> {
    type Item = Vec3;

    fn next(&mut self) -> Option<Self::Item> {
        let offset = self.index.checked_mul(12)?;
        let bytes = self.bytes.get(offset..offset + 12)?;
        self.index += 1;
        Some(vec3_from_chunk(bytes))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.bytes.len() / 12 - self.index;
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for Vec3LeIter<'_> {}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Vec4LeSlice<'a> {
    bytes: &'a [u8],
}

impl<'a> Vec4LeSlice<'a> {
    #[must_use]
    pub const fn from_bytes_unchecked(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }

    #[must_use]
    pub const fn as_bytes(self) -> &'a [u8] {
        self.bytes
    }

    #[must_use]
    pub const fn len(self) -> usize {
        self.bytes.len() / 16
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.bytes.is_empty()
    }

    #[must_use]
    pub fn get(self, index: usize) -> Option<Vec4> {
        let offset = index.checked_mul(16)?;
        let bytes = self.bytes.get(offset..offset + 16)?;
        Some(vec4_from_chunk(bytes))
    }

    #[must_use]
    pub fn iter(self) -> Vec4LeIter<'a> {
        Vec4LeIter {
            bytes: self.bytes,
            index: 0,
        }
    }
}

pub struct Vec4LeIter<'a> {
    bytes: &'a [u8],
    index: usize,
}

impl Iterator for Vec4LeIter<'_> {
    type Item = Vec4;

    fn next(&mut self) -> Option<Self::Item> {
        let offset = self.index.checked_mul(16)?;
        let bytes = self.bytes.get(offset..offset + 16)?;
        self.index += 1;
        Some(vec4_from_chunk(bytes))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.bytes.len() / 16 - self.index;
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for Vec4LeIter<'_> {}

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("bad RockNRoll magic {found:#010x}")]
    BadMagic { found: u32 },
    #[error("unexpected end of file at {offset}, needed {needed} bytes with {remaining} remaining")]
    UnexpectedEof {
        offset: usize,
        needed: usize,
        remaining: usize,
    },
    #[error("invalid UTF-8 in {field}: {source}")]
    Utf8 {
        field: &'static str,
        #[source]
        source: std::str::Utf8Error,
    },
    #[error("{field} count {count} cannot be represented on this platform")]
    CountTooLarge { field: &'static str, count: u64 },
    #[error("{field} count {count} is negative")]
    NegativeCount { field: &'static str, count: i16 },
    #[error("{field} byte length overflow for count {count} and stride {stride}")]
    ByteLengthOverflow {
        field: &'static str,
        count: usize,
        stride: usize,
    },
    #[error("unknown RockNRoll shape kind {kind} at byte offset {offset}")]
    UnknownShapeKind { kind: u32, offset: usize },
    #[error("shape recursion limit exceeded")]
    RecursionLimit,
    #[error("unsupported BVH version {version}")]
    UnsupportedBvhVersion { version: u32 },
    #[error("invalid BVH payload: {reason}")]
    InvalidBvh { reason: &'static str },
    #[error("trailing RockNRoll bytes after parsed asset: {bytes}")]
    TrailingBytes { bytes: usize },
}

pub fn parse_shape_asset(bytes: &[u8]) -> Result<ShapeAsset<'_>, ParseError> {
    let mut cursor = Cursor::new(bytes);
    let magic = cursor.read_u32()?;
    if magic != MAGIC {
        return Err(ParseError::BadMagic { found: magic });
    }

    let version = cursor.read_u32()?;
    let objects = if version > 1 {
        let count = cursor.read_usize_u32("object")?;
        let mut objects = Vec::with_capacity(count);
        for _ in 0..count {
            let name = cursor.read_string("object name")?;
            let material_count = cursor.read_usize_u32("object material index")?;
            let material_indices =
                cursor.read_u16_slice(material_count, "object material index")?;
            objects.push(ShapeObject {
                name,
                material_indices,
            });
        }
        objects
    } else {
        Vec::new()
    };

    let asset_guid = if version > 2 {
        Some(cursor.read_array_16()?)
    } else {
        None
    };

    let mut material_filter = MaterialFilter::default();
    if version > 4 {
        material_filter.enabled = cursor.read_u8()? != 0;
        if version > 6 {
            material_filter.secondary_geometry = cursor.read_u8()? != 0;
        }
        if material_filter.enabled {
            let count = cursor.read_i16()?;
            if count < 0 {
                return Err(ParseError::NegativeCount {
                    field: "material filter index",
                    count,
                });
            }
            material_filter.indices =
                cursor.read_u16_slice(count as usize, "material filter index")?;
        }
    }

    let shape_count = if version > 3 { cursor.read_u8()? } else { 1 };
    let mut shapes = Vec::with_capacity(shape_count as usize);
    for _ in 0..shape_count {
        shapes.push(read_physical_shape(&mut cursor, 0)?);
    }

    if cursor.remaining() != 0 {
        return Err(ParseError::TrailingBytes {
            bytes: cursor.remaining(),
        });
    }

    Ok(ShapeAsset {
        version,
        objects,
        asset_guid,
        material_filter,
        shapes,
    })
}

fn read_physical_shape<'a>(
    cursor: &mut Cursor<'a>,
    depth: usize,
) -> Result<PhysicalShape<'a>, ParseError> {
    if depth > MAX_RECURSION_DEPTH {
        return Err(ParseError::RecursionLimit);
    }

    let kind_offset = cursor.position();
    let kind_value = cursor.read_u32()?;
    let kind = ShapeKind::try_from(kind_value).map_err(|kind| ParseError::UnknownShapeKind {
        kind,
        offset: kind_offset,
    })?;

    let data = match kind {
        ShapeKind::Box => ShapeData::Box(BoxShape {
            half_extents: cursor.read_vec3()?,
            convex_radius: f32::from_bits(cursor.read_u32()?),
        }),
        ShapeKind::Sphere => ShapeData::Sphere(SphereShape {
            radius: f32::from_bits(cursor.read_u32()?),
        }),
        ShapeKind::ConvexHull => {
            let vertex_count = cursor.read_usize_u32("convex hull vertex")?;
            let plane_count = cursor.read_usize_u32("convex hull plane")?;
            let vertices = cursor.read_vec3_slice(vertex_count, "convex hull vertex")?;
            let planes = cursor.read_vec4_slice(plane_count, "convex hull plane")?;
            let convex_radius = f32::from_bits(cursor.read_u32()?);
            let extra = if SHAPE_STREAM_VERSION > 0x67 && cursor.read_u8()? != 0 {
                Some(read_convex_hull_extra(cursor)?)
            } else {
                None
            };
            ShapeData::ConvexHull(ConvexHullShape {
                vertices,
                planes,
                convex_radius,
                extra,
            })
        }
        ShapeKind::Cylinder => ShapeData::Cylinder(CylinderShape {
            half_height: f32::from_bits(cursor.read_u32()?),
            radius: f32::from_bits(cursor.read_u32()?),
            convex_radius: f32::from_bits(cursor.read_u32()?),
        }),
        ShapeKind::CylinderUnaligned => ShapeData::CylinderUnaligned(CylinderUnalignedShape {
            endpoint_a: cursor.read_vec3()?,
            endpoint_b: cursor.read_vec3()?,
            radius: f32::from_bits(cursor.read_u32()?),
            convex_radius: f32::from_bits(cursor.read_u32()?),
        }),
        ShapeKind::Capsule => ShapeData::Capsule(CapsuleShape {
            half_height: f32::from_bits(cursor.read_u32()?),
            radius: f32::from_bits(cursor.read_u32()?),
        }),
        ShapeKind::CapsuleUnaligned => ShapeData::CapsuleUnaligned(CapsuleUnalignedShape {
            endpoint_a: cursor.read_vec3()?,
            endpoint_b: cursor.read_vec3()?,
            radius: f32::from_bits(cursor.read_u32()?),
        }),
        ShapeKind::Triangle => ShapeData::Triangle(TriangleShape {
            a: cursor.read_vec3()?,
            b: cursor.read_vec3()?,
            c: cursor.read_vec3()?,
            convex_radius: f32::from_bits(cursor.read_u32()?),
        }),
        ShapeKind::Mesh => ShapeData::Mesh(read_mesh_shape(cursor)?),
        ShapeKind::Compound => {
            let count = cursor.read_usize_u32("compound child")?;
            let mut children = Vec::with_capacity(count);
            for _ in 0..count {
                children.push(CompoundChild {
                    transform: cursor.read_transform()?,
                    shape: Box::new(read_physical_shape(cursor, depth + 1)?),
                });
            }
            ShapeData::Compound(CompoundShape { children })
        }
        ShapeKind::Transform => ShapeData::Transform(TransformShape {
            transform: cursor.read_transform()?,
            shape: Box::new(read_physical_shape(cursor, depth + 1)?),
        }),
        ShapeKind::SoftBody => {
            // Engine RockNRoll::PhysicalShape::read<T> @ 0x7ff6010f7920 routes id 13
            // to default:, returning null without consuming the trailing block.
            // Take the same path as any unknown/unsupported id.
            return Err(ParseError::UnknownShapeKind {
                kind: kind_value,
                offset: kind_offset,
            });
        }
        ShapeKind::Plane => ShapeData::Plane(PlaneShape {
            plane: cursor.read_vec4()?,
            aabb_min: cursor.read_vec3()?,
            aabb_max: cursor.read_vec3()?,
        }),
        ShapeKind::ScaleConvexPolytope => ShapeData::ScaleConvexPolytope(ScaledShape {
            stream_header: cursor.read_u32()?,
            scale: cursor.read_vec3()?,
            shape: Box::new(read_physical_shape(cursor, depth + 1)?),
        }),
        ShapeKind::ScaleMesh => ShapeData::ScaleMesh(ScaledShape {
            stream_header: cursor.read_u32()?,
            scale: cursor.read_vec3()?,
            shape: Box::new(read_physical_shape(cursor, depth + 1)?),
        }),
        ShapeKind::HeightField => ShapeData::HeightField(read_height_field_shape(cursor)?),
    };

    let extra = read_optional_block(cursor)?;
    Ok(PhysicalShape { data, extra })
}

fn read_mesh_shape<'a>(cursor: &mut Cursor<'a>) -> Result<MeshShape<'a>, ParseError> {
    let stream_header = cursor.read_u32()?;
    let vertex_count = cursor.read_usize_u32("mesh vertex")?;
    let triangle_count = cursor.read_usize_u32("mesh triangle")?;
    let index_count = checked_len(triangle_count, 3, "mesh index")?;
    let vertices = cursor.read_vec3_slice(vertex_count, "mesh vertex")?;
    let indices = cursor.read_u16_slice(index_count, "mesh index")?;
    let adjacent_triangles = if cursor.read_u8()? != 0 {
        Some(cursor.read_u16_slice(index_count, "mesh adjacent triangle")?)
    } else {
        None
    };
    let bvh = read_bvh_tree(cursor)?;
    Ok(MeshShape {
        stream_header,
        vertices,
        indices,
        adjacent_triangles,
        bvh,
    })
}

fn read_height_field_shape<'a>(
    cursor: &mut Cursor<'a>,
) -> Result<HeightFieldShape<'a>, ParseError> {
    let layout = cursor.read_u32()?;
    let data = if layout == 1 {
        let version = cursor.read_u32()?;
        let width = cursor.read_u32()?;
        let length = cursor.read_u32()?;
        let height_scale = f32::from_bits(cursor.read_u32()?);
        let aabb_min = cursor.read_vec3()?;
        let aabb_max = cursor.read_vec3()?;
        let byte_len = cursor.read_usize_u32("height field sample byte")?;
        let samples = cursor.read_bytes(byte_len)?;
        Some(HeightFieldData {
            version,
            width,
            length,
            height_scale,
            aabb_min,
            aabb_max,
            samples,
        })
    } else {
        None
    };
    Ok(HeightFieldShape { layout, data })
}

fn read_convex_hull_extra<'a>(cursor: &mut Cursor<'a>) -> Result<ConvexHullExtra<'a>, ParseError> {
    let count_a = cursor.read_usize_u32("convex hull extra a")?;
    let data_a = cursor.read_u16_slice(count_a, "convex hull extra a")?;
    let count_b = cursor.read_usize_u32("convex hull extra b")?;
    let data_b = cursor.read_u16_slice(count_b, "convex hull extra b")?;
    Ok(ConvexHullExtra { data_a, data_b })
}

fn read_bvh_tree<'a>(cursor: &mut Cursor<'a>) -> Result<BvhTree<'a>, ParseError> {
    let byte_len = cursor.read_usize_u32("BVH payload byte")?;
    let payload = cursor.read_bytes(byte_len)?;
    let version = u32_at(payload, 0)?;
    match version {
        1 => Ok(BvhTree::V1(parse_bvh_v1(payload)?)),
        2 => Ok(BvhTree::V2(parse_bvh_v2(payload)?)),
        version => Err(ParseError::UnsupportedBvhVersion { version }),
    }
}

fn parse_bvh_v2(payload: &[u8]) -> Result<BvhTreeParts<'_>, ParseError> {
    if payload.len() < 0x6c {
        return Err(ParseError::InvalidBvh {
            reason: "v2 header is shorter than 0x6c bytes",
        });
    }
    bvh_parts(
        payload,
        BvhLayout {
            node_offset: usize_at_u32(payload, 0x04)?,
            subtree_offset: usize_at_u32(payload, 0x08)?,
            triangle_offset: usize_at_u32(payload, 0x0c)?,
            quantized_node_count: u32_at(payload, 0x48)?,
            subtree_info_count: u16_at(payload, 0x58)?,
            triangle_index_count: u32_at(payload, 0x68)?,
            flags: u16_at(payload, 0x5a)?,
        },
    )
}

fn parse_bvh_v1(payload: &[u8]) -> Result<BvhTreeParts<'_>, ParseError> {
    if payload.len() < 0x58 {
        return Err(ParseError::InvalidBvh {
            reason: "v1 header is shorter than 0x58 bytes",
        });
    }
    bvh_parts(
        payload,
        BvhLayout {
            node_offset: usize_at_u32(payload, 0x04)?,
            subtree_offset: usize_at_u32(payload, 0x08)?,
            triangle_offset: usize_at_u32(payload, 0x0c)?,
            quantized_node_count: u32_at(payload, 0x44)?,
            subtree_info_count: u16_at(payload, 0x4c)?,
            triangle_index_count: u32_at(payload, 0x54)?,
            flags: u16_at(payload, 0x4e)?,
        },
    )
}

#[derive(Debug, Clone, Copy)]
struct BvhLayout {
    node_offset: usize,
    subtree_offset: usize,
    triangle_offset: usize,
    quantized_node_count: u32,
    subtree_info_count: u16,
    triangle_index_count: u32,
    flags: u16,
}

fn bvh_parts<'a>(payload: &'a [u8], layout: BvhLayout) -> Result<BvhTreeParts<'a>, ParseError> {
    let node_bytes = checked_len(
        usize::try_from(layout.quantized_node_count).map_err(|_| ParseError::CountTooLarge {
            field: "BVH quantized node",
            count: u64::from(layout.quantized_node_count),
        })?,
        16,
        "BVH quantized node",
    )?;
    let subtree_bytes = checked_len(
        usize::from(layout.subtree_info_count),
        32,
        "BVH subtree info",
    )?;
    let triangle_stride = if layout.flags & 2 == 0 { 4 } else { 2 };
    let triangle_bytes = checked_len(
        usize::try_from(layout.triangle_index_count).map_err(|_| ParseError::CountTooLarge {
            field: "BVH triangle index",
            count: u64::from(layout.triangle_index_count),
        })?,
        triangle_stride,
        "BVH triangle index",
    )?;

    let quantized_nodes = payload
        .get(layout.node_offset..layout.node_offset + node_bytes)
        .ok_or(ParseError::InvalidBvh {
            reason: "quantized node range is outside payload",
        })?;
    let subtree_infos = payload
        .get(layout.subtree_offset..layout.subtree_offset + subtree_bytes)
        .ok_or(ParseError::InvalidBvh {
            reason: "subtree info range is outside payload",
        })?;
    let triangle_index_map = payload
        .get(layout.triangle_offset..layout.triangle_offset + triangle_bytes)
        .ok_or(ParseError::InvalidBvh {
            reason: "triangle index map range is outside payload",
        })?;

    Ok(BvhTreeParts {
        payload,
        quantized_nodes,
        subtree_infos,
        triangle_index_map,
        quantized_node_count: layout.quantized_node_count,
        subtree_info_count: layout.subtree_info_count,
        triangle_index_count: layout.triangle_index_count,
        flags: layout.flags,
    })
}

fn read_optional_block<'a>(cursor: &mut Cursor<'a>) -> Result<Option<&'a [u8]>, ParseError> {
    if SHAPE_STREAM_VERSION <= 0x66 {
        return Ok(None);
    }
    let byte_len = cursor.read_usize_u32("shape block byte")?;
    if byte_len == 0 {
        return Ok(None);
    }
    Ok(Some(cursor.read_bytes(byte_len)?))
}

fn checked_len(count: usize, stride: usize, field: &'static str) -> Result<usize, ParseError> {
    count
        .checked_mul(stride)
        .ok_or(ParseError::ByteLengthOverflow {
            field,
            count,
            stride,
        })
}

fn vec3_from_chunk(bytes: &[u8]) -> Vec3 {
    Vec3::new(
        f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
        f32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
        f32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]),
    )
}

fn vec4_from_chunk(bytes: &[u8]) -> Vec4 {
    Vec4::new(
        f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
        f32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
        f32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]),
        f32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]),
    )
}

fn u16_at(bytes: &[u8], offset: usize) -> Result<u16, ParseError> {
    let bytes = bytes
        .get(offset..offset + 2)
        .ok_or(ParseError::InvalidBvh {
            reason: "u16 field is outside payload",
        })?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn u32_at(bytes: &[u8], offset: usize) -> Result<u32, ParseError> {
    let bytes = bytes
        .get(offset..offset + 4)
        .ok_or(ParseError::InvalidBvh {
            reason: "u32 field is outside payload",
        })?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn usize_at_u32(bytes: &[u8], offset: usize) -> Result<usize, ParseError> {
    let value = u32_at(bytes, offset)?;
    usize::try_from(value).map_err(|_| ParseError::CountTooLarge {
        field: "BVH offset",
        count: u64::from(value),
    })
}

struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    const fn position(&self) -> usize {
        self.offset
    }

    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.offset)
    }

    fn read_bytes(&mut self, len: usize) -> Result<&'a [u8], ParseError> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or(ParseError::UnexpectedEof {
                offset: self.offset,
                needed: len,
                remaining: self.remaining(),
            })?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or(ParseError::UnexpectedEof {
                offset: self.offset,
                needed: len,
                remaining: self.remaining(),
            })?;
        self.offset = end;
        Ok(bytes)
    }

    fn read_u8(&mut self) -> Result<u8, ParseError> {
        Ok(self.read_bytes(1)?[0])
    }

    fn read_i16(&mut self) -> Result<i16, ParseError> {
        let bytes = self.read_bytes(2)?;
        Ok(i16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn read_u32(&mut self) -> Result<u32, ParseError> {
        let bytes = self.read_bytes(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_usize_u32(&mut self, field: &'static str) -> Result<usize, ParseError> {
        let value = self.read_u32()?;
        usize::try_from(value).map_err(|_| ParseError::CountTooLarge {
            field,
            count: u64::from(value),
        })
    }

    fn read_array_16(&mut self) -> Result<[u8; 16], ParseError> {
        let bytes = self.read_bytes(16)?;
        let mut out = [0; 16];
        out.copy_from_slice(bytes);
        Ok(out)
    }

    fn read_string(&mut self, field: &'static str) -> Result<&'a str, ParseError> {
        let len = self.read_usize_u32(field)?;
        let bytes = self.read_bytes(len)?;
        std::str::from_utf8(bytes).map_err(|source| ParseError::Utf8 { field, source })
    }

    fn read_u16_slice(
        &mut self,
        count: usize,
        field: &'static str,
    ) -> Result<U16LeSlice<'a>, ParseError> {
        let byte_len = checked_len(count, 2, field)?;
        Ok(U16LeSlice::from_bytes_unchecked(self.read_bytes(byte_len)?))
    }

    fn read_vec3_slice(
        &mut self,
        count: usize,
        field: &'static str,
    ) -> Result<Vec3LeSlice<'a>, ParseError> {
        let byte_len = checked_len(count, 12, field)?;
        Ok(Vec3LeSlice::from_bytes_unchecked(
            self.read_bytes(byte_len)?,
        ))
    }

    fn read_vec4_slice(
        &mut self,
        count: usize,
        field: &'static str,
    ) -> Result<Vec4LeSlice<'a>, ParseError> {
        let byte_len = checked_len(count, 16, field)?;
        Ok(Vec4LeSlice::from_bytes_unchecked(
            self.read_bytes(byte_len)?,
        ))
    }

    fn read_vec3(&mut self) -> Result<Vec3, ParseError> {
        Ok(vec3_from_chunk(self.read_bytes(12)?))
    }

    fn read_vec4(&mut self) -> Result<Vec4, ParseError> {
        Ok(vec4_from_chunk(self.read_bytes(16)?))
    }

    fn read_transform(&mut self) -> Result<ShapeTransform, ParseError> {
        Ok([self.read_vec4()?, self.read_vec4()?, self.read_vec4()?])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_box_shape() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&MAGIC.to_le_bytes());
        bytes.extend_from_slice(&7u32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&[0; 16]);
        bytes.push(0);
        bytes.push(0);
        bytes.push(1);
        bytes.extend_from_slice(&(ShapeKind::Box as u32).to_le_bytes());
        for value in [1.0f32, 2.0, 3.0] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.extend_from_slice(&4.0f32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());

        let asset = parse_shape_asset(&bytes).unwrap();

        assert_eq!(asset.version, 7);
        assert_eq!(asset.shapes.len(), 1);
        match &asset.shapes[0].data {
            ShapeData::Box(shape) => {
                assert_eq!(shape.half_extents, Vec3::new(1.0, 2.0, 3.0));
                assert_eq!(shape.convex_radius, 4.0);
            }
            data => panic!("expected box shape, got {data:?}"),
        }
    }

    #[test]
    fn u16_slice_reads_little_endian_values() {
        let slice = U16LeSlice::from_bytes_unchecked(&[1, 0, 2, 0, 0xff, 0x7f]);

        assert_eq!(slice.len(), 3);
        assert_eq!(slice.iter().collect::<Vec<_>>(), vec![1, 2, 0x7fff]);
    }
}
