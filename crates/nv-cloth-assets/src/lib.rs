//! Offline NvCloth asset parsing.
//!
//! Runtime gems own their own simulation products. This crate owns only the
//! legacy `.cloth` and `.clothmaterial` wire formats used during extraction.

use glam::{Quat, Vec3};
use serde::Serialize;
use thiserror::Error;

pub const CLOTH_MATERIAL_SIZE: usize = 208;
pub const CLOTH_ASSET_HEADER_SIZE: usize = 16;
pub const CLOTH_ASSET_VERSION: u32 = 1;
pub const CLOTH_ASSET_EXTENSION: &str = "cloth";
pub const CLOTH_MATERIAL_EXTENSION: &str = "clothmaterial";

const FABRIC_LAYOUT_OFFSET: usize = CLOTH_ASSET_HEADER_SIZE + CLOTH_MATERIAL_SIZE;
const FABRIC_LAYOUT_SIZE: usize = 272;
const FABRIC_INTERNAL_ARRAYS: usize = 10;
const FABRIC_INTERNAL_OFFSETS: usize = 11;
const FABRIC_GEOMETRY_OFFSETS: usize = 11;
const CLOTH_BARYCENTRIC_MAPPING: u32 = 0x0100_0000;
const CLOTH_HAS_BACKSTOP_OFFSETS: u32 = 0x0000_0100;
const CLOTH_HAS_BACKSTOP_RADII: u32 = 0x0001_0000;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClothMaterial {
    pub phase_configs: FabricPhaseConfigs,
    pub stiffness_frequency: f32,
    pub motion_constraints: MotionConstraintConfig,
    pub self_collision: SelfCollisionConfig,
    pub tether_constraints: TetherConstraintConfig,
    pub solver_frequency: f32,
    pub acceleration_filter_width: f32,
    pub continuous_collision: bool,
    pub damping: Vec3,
    pub linear_drag: Vec3,
    pub angular_drag: Vec3,
    pub linear_inertia: Vec3,
    pub angular_inertia: Vec3,
    pub centrifugal_inertia: Vec3,
}

impl ClothMaterial {
    fn validate(self) -> Result<(), ClothValidationError> {
        let scalars = [
            self.stiffness_frequency,
            self.motion_constraints.max_distance,
            self.motion_constraints.scale,
            self.motion_constraints.bias,
            self.motion_constraints.stiffness,
            self.self_collision.distance,
            self.self_collision.stiffness,
            self.tether_constraints.stiffness,
            self.tether_constraints.scale,
            self.solver_frequency,
            self.acceleration_filter_width,
        ];
        if scalars.into_iter().any(|value| !value.is_finite())
            || [
                self.damping,
                self.linear_drag,
                self.angular_drag,
                self.linear_inertia,
                self.angular_inertia,
                self.centrifugal_inertia,
            ]
            .into_iter()
            .any(|value| !value.is_finite())
            || self
                .phase_configs
                .iter()
                .flat_map(PhaseConfig::values)
                .any(|value| !value.is_finite())
        {
            return Err(ClothValidationError::NonFiniteMaterial);
        }
        // NewWorld 3-26 applies the authored frequencies directly. In particular,
        // shipped fabrics use a zero solver frequency, so range policy belongs to
        // the consuming runtime rather than this legacy wire-format validator.
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FabricPhaseConfigs {
    pub horizontal: PhaseConfig,
    pub vertical: PhaseConfig,
    pub bending: PhaseConfig,
    pub shearing: PhaseConfig,
}

impl FabricPhaseConfigs {
    fn iter(self) -> impl Iterator<Item = PhaseConfig> {
        [self.horizontal, self.vertical, self.bending, self.shearing].into_iter()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhaseConfig {
    pub stiffness: f32,
    pub stiffness_multiplier: f32,
    pub compression_limit: f32,
    pub stretch_limit: f32,
}

impl PhaseConfig {
    fn values(self) -> impl Iterator<Item = f32> {
        [
            self.stiffness,
            self.stiffness_multiplier,
            self.compression_limit,
            self.stretch_limit,
        ]
        .into_iter()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MotionConstraintConfig {
    pub max_distance: f32,
    pub scale: f32,
    pub bias: f32,
    pub stiffness: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelfCollisionConfig {
    pub distance: f32,
    pub stiffness: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TetherConstraintConfig {
    pub stiffness: f32,
    pub scale: f32,
}

/// Borrowed `.cloth` envelope. Use [`parse_cloth_fabric`] for the complete
/// owned projection used by exporters.
#[derive(Debug)]
pub struct ClothAsset<'a> {
    pub header: ClothAssetHeader,
    pub material: ClothMaterial,
    pub internal: FabricCookedDataBytes<'a>,
    pub geometry: ClothGeometryLayout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClothAssetHeader {
    pub version: u32,
    pub flags: [u32; 3],
}

#[derive(Debug, Clone, Copy)]
pub struct FabricCookedDataBytes<'a> {
    pub num_particles: u32,
    pub phase_indices: &'a [u8],
    pub phase_types: &'a [u8],
    pub sets: &'a [u8],
    pub rest_values: &'a [u8],
    pub stiffness_values: &'a [u8],
    pub indices: &'a [u8],
    pub anchors: &'a [u8],
    pub tether_lengths: &'a [u8],
    pub triangles: &'a [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClothGeometryLayout {
    pub counts: [u32; 4],
    pub offsets: [u64; FABRIC_GEOMETRY_OFFSETS],
    pub flags: [u32; 2],
}

/// Complete extraction-owned representation of one cooked cloth fabric.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClothFabric {
    pub render_model: String,
    pub material: Option<String>,
    pub embedded_material: ClothMaterial,
    pub mesh: ClothMesh,
    pub cooked: FabricCookedData,
}

impl ClothFabric {
    pub fn validate(&self) -> Result<(), ClothValidationError> {
        self.embedded_material.validate()?;
        self.mesh.validate()?;
        self.cooked.validate(self.mesh.vertices.len())?;
        if self.mesh.indices != self.cooked.triangles {
            return Err(ClothValidationError::TriangleTopologyMismatch);
        }
        Ok(())
    }

    pub fn dependency_paths(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.render_model.as_str()).chain(self.material.as_deref())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClothMesh {
    pub vertices: Vec<ClothSimulationVertex>,
    pub indices: Vec<u32>,
    pub render_mapping: ClothRenderMapping,
    pub paint: ClothPaintMaps,
}

impl ClothMesh {
    fn validate(&self) -> Result<(), ClothValidationError> {
        if self.vertices.is_empty() {
            return Err(ClothValidationError::EmptyMesh);
        }
        validate_triangles(&self.indices, self.vertices.len())?;
        for (index, vertex) in self.vertices.iter().enumerate() {
            if !vertex.position.is_finite() || !vertex.tangent_frame.is_finite() {
                return Err(ClothValidationError::NonFiniteVertex { vertex: index });
            }
            let sum = vertex
                .joint_weights
                .iter()
                .copied()
                .map(u16::from)
                .sum::<u16>();
            if sum != u16::from(u8::MAX) {
                return Err(ClothValidationError::InvalidSkinWeights { vertex: index, sum });
            }
        }
        self.render_mapping
            .validate(self.vertices.len(), self.indices.len() / 3)?;
        self.paint.validate(self.vertices.len())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClothSimulationVertex {
    pub position: Vec3,
    pub tangent_frame: Quat,
    pub joint_indices: [u16; 4],
    pub joint_weights: [u8; 4],
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ClothRenderMapping {
    Direct {
        particle_indices: Vec<u32>,
    },
    Barycentric {
        entries: Vec<ClothSkinMapEntry>,
        ranges: Vec<ClothSkinMapRange>,
    },
}

impl ClothRenderMapping {
    fn validate(
        &self,
        particle_count: usize,
        triangle_count: usize,
    ) -> Result<(), ClothValidationError> {
        match self {
            Self::Direct { particle_indices } => {
                if particle_indices.len() != particle_count {
                    return Err(ClothValidationError::DirectMapLength {
                        expected: particle_count,
                        actual: particle_indices.len(),
                    });
                }
                if let Some((vertex, particle)) = particle_indices
                    .iter()
                    .copied()
                    .enumerate()
                    .find(|(_, particle)| *particle as usize >= particle_count)
                {
                    return Err(ClothValidationError::DirectMapParticle {
                        vertex,
                        particle,
                        particle_count,
                    });
                }
            }
            Self::Barycentric { entries, ranges } => {
                for (vertex, entry) in entries.iter().enumerate() {
                    if !entry.barycentric.is_finite() || !entry.height.is_finite() {
                        return Err(ClothValidationError::NonFiniteSkinMap { vertex });
                    }
                    let sum = entry.barycentric.element_sum();
                    if (sum - 1.0).abs() > 1.0e-3 {
                        return Err(ClothValidationError::InvalidBarycentricSum { vertex, sum });
                    }
                    if entry.triangle as usize >= triangle_count {
                        return Err(ClothValidationError::SkinMapTriangle {
                            vertex,
                            triangle: entry.triangle,
                            triangle_count,
                        });
                    }
                }
                let mut next = 0_usize;
                for range in ranges {
                    if range.first_vertex as usize != next {
                        return Err(ClothValidationError::NonContiguousSkinMapRanges);
                    }
                    next = next
                        .checked_add(range.vertex_count as usize)
                        .ok_or(ClothValidationError::NonContiguousSkinMapRanges)?;
                }
                if next != entries.len() {
                    return Err(ClothValidationError::SkinMapRangeCoverage {
                        entries: entries.len(),
                        covered: next,
                    });
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClothSkinMapEntry {
    pub barycentric: Vec3,
    pub height: f32,
    pub triangle: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClothSkinMapRange {
    pub first_vertex: u32,
    pub vertex_count: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClothPaintMaps {
    pub motion_constraint_max_distances: Vec<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backstop_offsets: Option<Vec<f32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backstop_radii: Option<Vec<f32>>,
}

impl ClothPaintMaps {
    fn validate(&self, particles: usize) -> Result<(), ClothValidationError> {
        validate_paint_map(
            "motion constraint max distance",
            &self.motion_constraint_max_distances,
            particles,
        )?;
        if let Some(values) = &self.backstop_offsets {
            validate_paint_map("backstop offset", values, particles)?;
        }
        if let Some(values) = &self.backstop_radii {
            validate_paint_map("backstop radius", values, particles)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FabricCookedData {
    pub phase_indices: Vec<u32>,
    pub phase_types: Vec<FabricPhaseType>,
    pub sets: Vec<u32>,
    pub rest_values: Vec<f32>,
    pub stiffness_values: Vec<f32>,
    pub constraint_indices: Vec<u32>,
    pub anchors: Vec<u32>,
    pub tether_lengths: Vec<f32>,
    pub triangles: Vec<u32>,
}

impl FabricCookedData {
    fn validate(&self, particles: usize) -> Result<(), ClothValidationError> {
        if self.phase_indices.len() != self.phase_types.len() {
            return Err(ClothValidationError::PhaseLengthMismatch {
                indices: self.phase_indices.len(),
                types: self.phase_types.len(),
            });
        }
        if self.constraint_indices.len() != self.rest_values.len().saturating_mul(2) {
            return Err(ClothValidationError::ConstraintLengthMismatch {
                rest_values: self.rest_values.len(),
                indices: self.constraint_indices.len(),
            });
        }
        if !self.stiffness_values.is_empty()
            && self.stiffness_values.len() != self.rest_values.len()
        {
            return Err(ClothValidationError::StiffnessLengthMismatch {
                rest_values: self.rest_values.len(),
                stiffness_values: self.stiffness_values.len(),
            });
        }
        if self.anchors.len() != self.tether_lengths.len() {
            return Err(ClothValidationError::TetherLengthMismatch {
                anchors: self.anchors.len(),
                lengths: self.tether_lengths.len(),
            });
        }
        if self
            .constraint_indices
            .iter()
            .chain(&self.anchors)
            .any(|index| *index as usize >= particles)
        {
            return Err(ClothValidationError::FabricParticleIndex { particles });
        }
        if self
            .phase_indices
            .iter()
            .any(|phase| *phase as usize >= self.sets.len())
        {
            return Err(ClothValidationError::FabricPhaseIndex {
                sets: self.sets.len(),
            });
        }
        if self.sets.windows(2).any(|window| window[0] > window[1])
            || self
                .sets
                .last()
                .is_some_and(|last| *last as usize > self.rest_values.len())
        {
            return Err(ClothValidationError::InvalidFabricSets);
        }
        if self
            .rest_values
            .iter()
            .chain(&self.stiffness_values)
            .chain(&self.tether_lengths)
            .any(|value| !value.is_finite())
        {
            return Err(ClothValidationError::NonFiniteFabric);
        }
        validate_triangles(&self.triangles, particles)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
#[repr(i32)]
pub enum FabricPhaseType {
    Horizontal = 1,
    Vertical = 2,
    Bending = 3,
    Shearing = 4,
}

impl TryFrom<i32> for FabricPhaseType {
    type Error = ClothValidationError;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Horizontal),
            2 => Ok(Self::Vertical),
            3 => Ok(Self::Bending),
            4 => Ok(Self::Shearing),
            value => Err(ClothValidationError::UnknownPhaseType { value }),
        }
    }
}

/// Parse the fixed 208-byte `.clothmaterial` payload.
pub fn parse_cloth_material(bytes: &[u8]) -> Result<ClothMaterial, ClothMaterialParseError> {
    if bytes.len() != CLOTH_MATERIAL_SIZE {
        return Err(ClothMaterialParseError::InvalidSize {
            actual: bytes.len(),
            expected: CLOTH_MATERIAL_SIZE,
        });
    }

    let mut reader = FloatReader::new(bytes);
    let phase_configs = FabricPhaseConfigs {
        horizontal: reader.read_phase_config(),
        vertical: reader.read_phase_config(),
        bending: reader.read_phase_config(),
        shearing: reader.read_phase_config(),
    };
    let stiffness_frequency = reader.read_f32();
    let motion_constraints = MotionConstraintConfig {
        max_distance: reader.read_f32(),
        scale: reader.read_f32(),
        bias: reader.read_f32(),
        stiffness: reader.read_f32(),
    };
    let self_collision = SelfCollisionConfig {
        distance: reader.read_f32(),
        stiffness: reader.read_f32(),
    };
    let tether_constraints = TetherConstraintConfig {
        stiffness: reader.read_f32(),
        scale: reader.read_f32(),
    };
    let solver_frequency = reader.read_f32();
    let acceleration_filter_width = reader.read_f32();
    let continuous_collision = reader.read_bool_u32("continuous collision")?;

    Ok(ClothMaterial {
        phase_configs,
        stiffness_frequency,
        motion_constraints,
        self_collision,
        tether_constraints,
        solver_frequency,
        acceleration_filter_width,
        continuous_collision,
        damping: reader.read_padded_vec3("damping")?,
        linear_drag: reader.read_padded_vec3("linear_drag")?,
        angular_drag: reader.read_padded_vec3("angular_drag")?,
        linear_inertia: reader.read_padded_vec3("linear_inertia")?,
        angular_inertia: reader.read_padded_vec3("angular_inertia")?,
        centrifugal_inertia: reader.read_padded_vec3("centrifugal_inertia")?,
    })
}

/// Parse the complete borrowed `.cloth` layout.
pub fn parse_cloth_asset(bytes: &[u8]) -> Result<ClothAsset<'_>, ClothAssetParseError> {
    let version = read_u32(bytes, 0, "cloth version")?;
    if version != CLOTH_ASSET_VERSION {
        return Err(ClothAssetParseError::UnsupportedVersion { version });
    }
    let header = ClothAssetHeader {
        version,
        flags: [
            read_u32(bytes, 4, "cloth header flags[0]")?,
            read_u32(bytes, 8, "cloth header flags[1]")?,
            read_u32(bytes, 12, "cloth header flags[2]")?,
        ],
    };
    let material = parse_cloth_material(slice(
        bytes,
        CLOTH_ASSET_HEADER_SIZE,
        CLOTH_MATERIAL_SIZE,
        "cloth material",
    )?)?;
    let layout = FabricCookedDataLayout::parse(slice(
        bytes,
        FABRIC_LAYOUT_OFFSET,
        FABRIC_LAYOUT_SIZE,
        "fabric layout",
    )?)?;
    let internal = layout.internal_data(bytes)?;
    let geometry = layout.geometry();
    geometry.validate(bytes.len())?;
    Ok(ClothAsset {
        header,
        material,
        internal,
        geometry,
    })
}

/// Parse and validate all simulation, skinning, paint, and cooked-fabric data.
pub fn parse_cloth_fabric(bytes: &[u8]) -> Result<ClothFabric, ClothFabricParseError> {
    let asset = parse_cloth_asset(bytes)?;
    let geometry = asset.geometry;
    let particle_count = usize::try_from(geometry.counts[0])
        .map_err(|_| ClothFabricParseError::CountOverflow("simulation vertices"))?;
    if particle_count != asset.internal.num_particles as usize {
        return Err(ClothFabricParseError::ParticleCountMismatch {
            fabric: asset.internal.num_particles,
            mesh: geometry.counts[0],
        });
    }

    let vertices = parse_simulation_vertices(block(
        bytes,
        geometry.offsets[0],
        particle_count,
        64,
        "simulation vertices",
    )?)?;
    let indices = decode_u32s(block(
        bytes,
        geometry.offsets[1],
        geometry.counts[1] as usize,
        4,
        "triangle indices",
    )?)?;
    let render_mapping = parse_render_mapping(bytes, geometry, particle_count)?;
    if geometry.flags[1] != 4 {
        return Err(ClothFabricParseError::UnsupportedInfluenceCount {
            count: geometry.flags[1],
        });
    }
    let motion_constraint_max_distances = decode_f32s(block(
        bytes,
        geometry.offsets[8],
        particle_count,
        4,
        "motion-constraint paint",
    )?)?;
    let backstop_offsets = (geometry.flags[0] & CLOTH_HAS_BACKSTOP_OFFSETS != 0)
        .then(|| {
            block(
                bytes,
                geometry.offsets[9],
                particle_count,
                4,
                "backstop-offset paint",
            )
            .and_then(decode_f32s)
        })
        .transpose()?;
    let backstop_radii = (geometry.flags[0] & CLOTH_HAS_BACKSTOP_RADII != 0)
        .then(|| {
            block(
                bytes,
                geometry.offsets[10],
                particle_count,
                4,
                "backstop-radius paint",
            )
            .and_then(decode_f32s)
        })
        .transpose()?;

    let material_path = normalize_path(read_c_string(
        bytes,
        geometry.offsets[7],
        "cloth material path",
    )?);
    let fabric = ClothFabric {
        render_model: normalize_path(read_c_string(
            bytes,
            geometry.offsets[6],
            "render skin path",
        )?),
        material: (!material_path.is_empty()).then_some(material_path),
        embedded_material: asset.material,
        mesh: ClothMesh {
            vertices,
            indices,
            render_mapping,
            paint: ClothPaintMaps {
                motion_constraint_max_distances,
                backstop_offsets,
                backstop_radii,
            },
        },
        cooked: FabricCookedData {
            phase_indices: decode_u32s(asset.internal.phase_indices)?,
            phase_types: decode_i32s(asset.internal.phase_types)?
                .into_iter()
                .map(FabricPhaseType::try_from)
                .collect::<Result<_, _>>()?,
            sets: decode_u32s(asset.internal.sets)?,
            rest_values: decode_f32s(asset.internal.rest_values)?,
            stiffness_values: decode_f32s(asset.internal.stiffness_values)?,
            constraint_indices: decode_u32s(asset.internal.indices)?,
            anchors: decode_u32s(asset.internal.anchors)?,
            tether_lengths: decode_f32s(asset.internal.tether_lengths)?,
            triangles: decode_u32s(asset.internal.triangles)?,
        },
    };
    fabric.validate()?;
    Ok(fabric)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FabricCookedDataLayout {
    num_particles: u32,
    counts: [u32; FABRIC_INTERNAL_ARRAYS],
    offsets: [u64; FABRIC_INTERNAL_OFFSETS],
    geometry_counts: [u32; 4],
    geometry_offsets: [u64; FABRIC_GEOMETRY_OFFSETS],
    geometry_flags: [u32; 2],
}

impl FabricCookedDataLayout {
    fn parse(bytes: &[u8]) -> Result<Self, ClothAssetParseError> {
        let num_particles = read_u32(bytes, 0, "fabric particle count")?;
        let mut counts = [0; FABRIC_INTERNAL_ARRAYS];
        counts[0] = num_particles;
        for (index, count) in counts.iter_mut().enumerate().skip(1) {
            *count = read_u32(bytes, index * 4, "fabric internal count")?;
        }
        let mut offsets = [0; FABRIC_INTERNAL_OFFSETS];
        for (index, offset) in offsets.iter_mut().enumerate() {
            *offset = read_u64(bytes, 40 + index * 8, "fabric internal offset")?;
        }
        let geometry_base = 40 + FABRIC_INTERNAL_OFFSETS * 8;
        let geometry_counts = [
            read_u32(bytes, geometry_base, "cloth geometry count[0]")?,
            read_u32(bytes, geometry_base + 4, "cloth geometry count[1]")?,
            read_u32(bytes, geometry_base + 8, "cloth geometry count[2]")?,
            read_u32(bytes, geometry_base + 12, "cloth geometry count[3]")?,
        ];
        let mut geometry_offsets = [0; FABRIC_GEOMETRY_OFFSETS];
        let geometry_offsets_base = geometry_base + 16;
        for (index, offset) in geometry_offsets.iter_mut().enumerate() {
            *offset = read_u64(
                bytes,
                geometry_offsets_base + index * 8,
                "cloth geometry offset",
            )?;
        }
        let flags_base = geometry_offsets_base + FABRIC_GEOMETRY_OFFSETS * 8;
        Ok(Self {
            num_particles,
            counts,
            offsets,
            geometry_counts,
            geometry_offsets,
            geometry_flags: [
                read_u32(bytes, flags_base, "cloth geometry flags[0]")?,
                read_u32(bytes, flags_base + 4, "cloth geometry flags[1]")?,
            ],
        })
    }

    fn internal_data<'a>(
        &self,
        bytes: &'a [u8],
    ) -> Result<FabricCookedDataBytes<'a>, ClothAssetParseError> {
        Ok(FabricCookedDataBytes {
            num_particles: self.num_particles,
            phase_indices: self.range(bytes, 1, 4, "phase indices")?,
            phase_types: self.range(bytes, 2, 4, "phase types")?,
            sets: self.range(bytes, 3, 4, "sets")?,
            rest_values: self.range(bytes, 4, 4, "rest values")?,
            stiffness_values: self.range(bytes, 5, 4, "stiffness values")?,
            indices: self.range(bytes, 6, 4, "indices")?,
            anchors: self.range(bytes, 7, 4, "anchors")?,
            tether_lengths: self.range(bytes, 8, 4, "tether lengths")?,
            triangles: self.range(bytes, 9, 4, "triangles")?,
        })
    }

    fn range<'a>(
        &self,
        bytes: &'a [u8],
        index: usize,
        stride: usize,
        field: &'static str,
    ) -> Result<&'a [u8], ClothAssetParseError> {
        let offset = usize::try_from(self.offsets[index]).map_err(|_| {
            ClothAssetParseError::OutOfBounds {
                field,
                offset: self.offsets[index],
                len: bytes.len(),
            }
        })?;
        let len = (self.counts[index] as usize).checked_mul(stride).ok_or(
            ClothAssetParseError::OutOfBounds {
                field,
                offset: self.offsets[index],
                len: bytes.len(),
            },
        )?;
        slice(bytes, offset, len, field)
    }

    const fn geometry(&self) -> ClothGeometryLayout {
        ClothGeometryLayout {
            counts: self.geometry_counts,
            offsets: self.geometry_offsets,
            flags: self.geometry_flags,
        }
    }
}

impl ClothGeometryLayout {
    fn validate(&self, file_len: usize) -> Result<(), ClothAssetParseError> {
        for &offset in &self.offsets {
            if offset != 0 && usize::try_from(offset).map_or(true, |value| value > file_len) {
                return Err(ClothAssetParseError::OutOfBounds {
                    field: "geometry",
                    offset,
                    len: file_len,
                });
            }
        }
        Ok(())
    }
}

fn parse_simulation_vertices(
    bytes: &[u8],
) -> Result<Vec<ClothSimulationVertex>, ClothFabricParseError> {
    bytes
        .chunks_exact(64)
        .enumerate()
        .map(|(index, record)| {
            ensure_zero_padding(record, 12..16, "simulation vertex position", index)?;
            ensure_zero_padding(record, 40..48, "simulation vertex joints", index)?;
            ensure_zero_padding(record, 52..64, "simulation vertex weights", index)?;
            Ok(ClothSimulationVertex {
                position: Vec3::new(
                    read_f32_at(record, 0)?,
                    read_f32_at(record, 4)?,
                    read_f32_at(record, 8)?,
                ),
                tangent_frame: Quat::from_xyzw(
                    read_f32_at(record, 16)?,
                    read_f32_at(record, 20)?,
                    read_f32_at(record, 24)?,
                    read_f32_at(record, 28)?,
                ),
                joint_indices: [
                    read_u16_at(record, 32)?,
                    read_u16_at(record, 34)?,
                    read_u16_at(record, 36)?,
                    read_u16_at(record, 38)?,
                ],
                joint_weights: record[48..52].try_into().map_err(|_| {
                    ClothFabricParseError::InvalidRecord {
                        field: "simulation vertex weights",
                        index,
                    }
                })?,
            })
        })
        .collect()
}

fn parse_render_mapping(
    bytes: &[u8],
    geometry: ClothGeometryLayout,
    particle_count: usize,
) -> Result<ClothRenderMapping, ClothFabricParseError> {
    if geometry.flags[0] & CLOTH_BARYCENTRIC_MAPPING == 0 {
        return Ok(ClothRenderMapping::Direct {
            particle_indices: decode_u32s(block(
                bytes,
                geometry.offsets[2],
                geometry.counts[2] as usize,
                4,
                "direct render mapping",
            )?)?,
        });
    }

    let range_count = geometry.counts[3] as usize;
    let mut ranges = Vec::with_capacity(range_count);
    for (index, record) in block(
        bytes,
        geometry.offsets[3],
        range_count,
        16,
        "barycentric map ranges",
    )?
    .chunks_exact(16)
    .enumerate()
    {
        ensure_zero_padding(record, 8..16, "barycentric map range", index)?;
        ranges.push(ClothSkinMapRange {
            first_vertex: read_u32_at(record, 0)?,
            vertex_count: read_u32_at(record, 4)?,
        });
    }
    let entry_count = ranges
        .last()
        .map(|range| range.first_vertex as usize + range.vertex_count as usize)
        .unwrap_or(particle_count);
    let mut entries = Vec::with_capacity(entry_count);
    for (index, record) in block(
        bytes,
        geometry.offsets[2],
        entry_count,
        32,
        "barycentric render mapping",
    )?
    .chunks_exact(32)
    .enumerate()
    {
        ensure_zero_padding(record, 20..32, "barycentric map entry", index)?;
        entries.push(ClothSkinMapEntry {
            barycentric: Vec3::new(
                read_f32_at(record, 0)?,
                read_f32_at(record, 4)?,
                read_f32_at(record, 8)?,
            ),
            height: read_f32_at(record, 12)?,
            triangle: read_u32_at(record, 16)?,
        });
    }
    Ok(ClothRenderMapping::Barycentric { entries, ranges })
}

fn normalize_path(value: &str) -> String {
    value.trim().replace('\\', "/").to_ascii_lowercase()
}

fn read_c_string<'a>(
    bytes: &'a [u8],
    offset: u64,
    field: &'static str,
) -> Result<&'a str, ClothFabricParseError> {
    let offset = usize::try_from(offset)
        .map_err(|_| ClothFabricParseError::InvalidStringOffset { field, offset })?;
    let tail = bytes
        .get(offset..)
        .ok_or(ClothFabricParseError::InvalidStringOffset {
            field,
            offset: offset as u64,
        })?;
    let length = tail
        .iter()
        .position(|byte| *byte == 0)
        .ok_or(ClothFabricParseError::UnterminatedString { field })?;
    std::str::from_utf8(&tail[..length])
        .map_err(|source| ClothFabricParseError::Utf8 { field, source })
}

fn block<'a>(
    bytes: &'a [u8],
    offset: u64,
    count: usize,
    stride: usize,
    field: &'static str,
) -> Result<&'a [u8], ClothFabricParseError> {
    let offset = usize::try_from(offset)
        .map_err(|_| ClothFabricParseError::BlockOutOfBounds { field, offset })?;
    let length = count
        .checked_mul(stride)
        .ok_or(ClothFabricParseError::CountOverflow(field))?;
    slice(bytes, offset, length, field).map_err(ClothFabricParseError::Asset)
}

fn decode_u32s(bytes: &[u8]) -> Result<Vec<u32>, ClothFabricParseError> {
    bytes
        .chunks_exact(4)
        .map(|value| {
            value
                .try_into()
                .map(u32::from_le_bytes)
                .map_err(|_| ClothFabricParseError::InvalidScalarArray)
        })
        .collect()
}

fn decode_i32s(bytes: &[u8]) -> Result<Vec<i32>, ClothFabricParseError> {
    bytes
        .chunks_exact(4)
        .map(|value| {
            value
                .try_into()
                .map(i32::from_le_bytes)
                .map_err(|_| ClothFabricParseError::InvalidScalarArray)
        })
        .collect()
}

fn decode_f32s(bytes: &[u8]) -> Result<Vec<f32>, ClothFabricParseError> {
    bytes
        .chunks_exact(4)
        .map(|value| {
            value
                .try_into()
                .map(f32::from_le_bytes)
                .map_err(|_| ClothFabricParseError::InvalidScalarArray)
        })
        .collect()
}

fn read_u16_at(bytes: &[u8], offset: usize) -> Result<u16, ClothFabricParseError> {
    bytes
        .get(offset..offset + 2)
        .and_then(|value| value.try_into().ok())
        .map(u16::from_le_bytes)
        .ok_or(ClothFabricParseError::InvalidScalarArray)
}

fn read_u32_at(bytes: &[u8], offset: usize) -> Result<u32, ClothFabricParseError> {
    bytes
        .get(offset..offset + 4)
        .and_then(|value| value.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or(ClothFabricParseError::InvalidScalarArray)
}

fn read_f32_at(bytes: &[u8], offset: usize) -> Result<f32, ClothFabricParseError> {
    read_u32_at(bytes, offset).map(f32::from_bits)
}

fn ensure_zero_padding(
    bytes: &[u8],
    range: std::ops::Range<usize>,
    field: &'static str,
    index: usize,
) -> Result<(), ClothFabricParseError> {
    if bytes
        .get(range)
        .is_some_and(|padding| padding.iter().all(|byte| *byte == 0))
    {
        Ok(())
    } else {
        Err(ClothFabricParseError::NonZeroPadding { field, index })
    }
}

fn validate_triangles(indices: &[u32], vertices: usize) -> Result<(), ClothValidationError> {
    if indices.len() % 3 != 0 {
        return Err(ClothValidationError::TriangleIndexCount {
            indices: indices.len(),
        });
    }
    if let Some((slot, index)) = indices
        .iter()
        .copied()
        .enumerate()
        .find(|(_, index)| *index as usize >= vertices)
    {
        return Err(ClothValidationError::TriangleVertex {
            slot,
            index,
            vertices,
        });
    }
    Ok(())
}

fn validate_paint_map(
    name: &'static str,
    values: &[f32],
    particles: usize,
) -> Result<(), ClothValidationError> {
    if values.len() != particles {
        return Err(ClothValidationError::PaintMapLength {
            name,
            expected: particles,
            actual: values.len(),
        });
    }
    if values.iter().any(|value| !value.is_finite()) {
        return Err(ClothValidationError::NonFinitePaintMap { name });
    }
    Ok(())
}

fn read_u32(
    bytes: &[u8],
    offset: usize,
    context: &'static str,
) -> Result<u32, ClothAssetParseError> {
    Ok(u32::from_le_bytes(read_array(bytes, offset, context)?))
}

fn read_u64(
    bytes: &[u8],
    offset: usize,
    context: &'static str,
) -> Result<u64, ClothAssetParseError> {
    Ok(u64::from_le_bytes(read_array(bytes, offset, context)?))
}

fn read_array<const N: usize>(
    bytes: &[u8],
    offset: usize,
    context: &'static str,
) -> Result<[u8; N], ClothAssetParseError> {
    bytes
        .get(offset..offset + N)
        .ok_or(ClothAssetParseError::UnexpectedEof { context })?
        .try_into()
        .map_err(|_| ClothAssetParseError::UnexpectedEof { context })
}

fn slice<'a>(
    bytes: &'a [u8],
    offset: usize,
    len: usize,
    field: &'static str,
) -> Result<&'a [u8], ClothAssetParseError> {
    let end = offset
        .checked_add(len)
        .ok_or(ClothAssetParseError::OutOfBounds {
            field,
            offset: offset as u64,
            len: bytes.len(),
        })?;
    bytes
        .get(offset..end)
        .ok_or(ClothAssetParseError::OutOfBounds {
            field,
            offset: offset as u64,
            len: bytes.len(),
        })
}

struct FloatReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> FloatReader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn read_phase_config(&mut self) -> PhaseConfig {
        PhaseConfig {
            stiffness: self.read_f32(),
            stiffness_multiplier: self.read_f32(),
            compression_limit: self.read_f32(),
            stretch_limit: self.read_f32(),
        }
    }

    fn read_padded_vec3(&mut self, field: &'static str) -> Result<Vec3, ClothMaterialParseError> {
        let value = Vec3::new(self.read_f32(), self.read_f32(), self.read_f32());
        let padding = self.read_f32();
        if padding != 0.0 {
            return Err(ClothMaterialParseError::NonZeroVectorPadding {
                field,
                value: padding,
            });
        }
        Ok(value)
    }

    fn read_f32(&mut self) -> f32 {
        let value = f32::from_le_bytes(
            self.bytes[self.offset..self.offset + 4]
                .try_into()
                .expect("cloth material size was validated"),
        );
        self.offset += 4;
        value
    }

    fn read_bool_u32(&mut self, field: &'static str) -> Result<bool, ClothMaterialParseError> {
        let value = u32::from_le_bytes(
            self.bytes[self.offset..self.offset + 4]
                .try_into()
                .expect("cloth material size was validated"),
        );
        self.offset += 4;
        match value {
            0 => Ok(false),
            1 => Ok(true),
            value => Err(ClothMaterialParseError::InvalidBoolean { field, value }),
        }
    }
}

#[derive(Debug, Error, PartialEq)]
pub enum ClothMaterialParseError {
    #[error("invalid cloth material size: expected {expected} bytes, got {actual}")]
    InvalidSize { actual: usize, expected: usize },
    #[error("{field} vector padding is {value}, expected 0")]
    NonZeroVectorPadding { field: &'static str, value: f32 },
    #[error("{field} boolean is {value}, expected 0 or 1")]
    InvalidBoolean { field: &'static str, value: u32 },
}

#[derive(Debug, Error, PartialEq)]
pub enum ClothAssetParseError {
    #[error("unexpected end of file while reading {context}")]
    UnexpectedEof { context: &'static str },
    #[error("unsupported cloth asset version {version}")]
    UnsupportedVersion { version: u32 },
    #[error("{field} points outside the cloth asset: offset {offset:#x}, file length {len:#x}")]
    OutOfBounds {
        field: &'static str,
        offset: u64,
        len: usize,
    },
    #[error(transparent)]
    Material(#[from] ClothMaterialParseError),
}

#[derive(Debug, Error)]
pub enum ClothFabricParseError {
    #[error(transparent)]
    Asset(#[from] ClothAssetParseError),
    #[error(transparent)]
    Validation(#[from] ClothValidationError),
    #[error("cloth {0} count cannot be represented")]
    CountOverflow(&'static str),
    #[error("cooked fabric has {fabric} particles but the mesh has {mesh}")]
    ParticleCountMismatch { fabric: u32, mesh: u32 },
    #[error("cloth {field} block at {offset:#x} is outside the file")]
    BlockOutOfBounds { field: &'static str, offset: u64 },
    #[error("cloth {field} record {index} is malformed")]
    InvalidRecord { field: &'static str, index: usize },
    #[error("cloth {field} record {index} contains non-zero padding")]
    NonZeroPadding { field: &'static str, index: usize },
    #[error("cloth scalar array is malformed")]
    InvalidScalarArray,
    #[error("cloth {field} string offset {offset:#x} is outside the file")]
    InvalidStringOffset { field: &'static str, offset: u64 },
    #[error("cloth {field} string is not terminated")]
    UnterminatedString { field: &'static str },
    #[error("cloth {field} string is not UTF-8")]
    Utf8 {
        field: &'static str,
        #[source]
        source: std::str::Utf8Error,
    },
    #[error("cloth asset uses {count} skin influences, expected four")]
    UnsupportedInfluenceCount { count: u32 },
}

#[derive(Debug, Error, Clone, PartialEq)]
pub enum ClothValidationError {
    #[error("cloth mesh has no simulation vertices")]
    EmptyMesh,
    #[error("cloth material contains a non-finite value")]
    NonFiniteMaterial,
    #[error("cloth vertex {vertex} contains a non-finite position or tangent frame")]
    NonFiniteVertex { vertex: usize },
    #[error("cloth vertex {vertex} skin weights sum to {sum}, expected 255")]
    InvalidSkinWeights { vertex: usize, sum: u16 },
    #[error("triangle index count {indices} is not divisible by three")]
    TriangleIndexCount { indices: usize },
    #[error("triangle slot {slot} references vertex {index}, but only {vertices} exist")]
    TriangleVertex {
        slot: usize,
        index: u32,
        vertices: usize,
    },
    #[error("the mesh and cooked-fabric triangle topology differ")]
    TriangleTopologyMismatch,
    #[error("direct render map has {actual} entries, expected {expected}")]
    DirectMapLength { expected: usize, actual: usize },
    #[error("direct render vertex {vertex} references particle {particle} of {particle_count}")]
    DirectMapParticle {
        vertex: usize,
        particle: u32,
        particle_count: usize,
    },
    #[error("barycentric render vertex {vertex} contains a non-finite value")]
    NonFiniteSkinMap { vertex: usize },
    #[error("barycentric render vertex {vertex} weights sum to {sum}")]
    InvalidBarycentricSum { vertex: usize, sum: f32 },
    #[error(
        "barycentric render vertex {vertex} references triangle {triangle} of {triangle_count}"
    )]
    SkinMapTriangle {
        vertex: usize,
        triangle: u32,
        triangle_count: usize,
    },
    #[error("barycentric render ranges are not contiguous")]
    NonContiguousSkinMapRanges,
    #[error("barycentric render ranges cover {covered} of {entries} entries")]
    SkinMapRangeCoverage { entries: usize, covered: usize },
    #[error("{name} paint map has {actual} values, expected {expected}")]
    PaintMapLength {
        name: &'static str,
        expected: usize,
        actual: usize,
    },
    #[error("{name} paint map contains a non-finite value")]
    NonFinitePaintMap { name: &'static str },
    #[error("fabric has {indices} phase indices and {types} phase types")]
    PhaseLengthMismatch { indices: usize, types: usize },
    #[error("fabric has {rest_values} rest values but {indices} constraint indices")]
    ConstraintLengthMismatch { rest_values: usize, indices: usize },
    #[error("fabric has {rest_values} rest values but {stiffness_values} stiffness values")]
    StiffnessLengthMismatch {
        rest_values: usize,
        stiffness_values: usize,
    },
    #[error("fabric has {anchors} tether anchors but {lengths} tether lengths")]
    TetherLengthMismatch { anchors: usize, lengths: usize },
    #[error("fabric references a particle outside its {particles} particles")]
    FabricParticleIndex { particles: usize },
    #[error("fabric phase references a missing set among {sets} sets")]
    FabricPhaseIndex { sets: usize },
    #[error("fabric constraint sets are invalid")]
    InvalidFabricSets,
    #[error("fabric contains a non-finite constraint value")]
    NonFiniteFabric,
    #[error("fabric uses unknown phase type {value}")]
    UnknownPhaseType { value: i32 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_complete_cloth_material_block() {
        let bytes = material_bytes();
        let material = parse_cloth_material(&bytes).unwrap();

        assert_eq!(material.phase_configs.horizontal.stiffness, 1.0);
        assert_eq!(material.stiffness_frequency, 50.0);
        assert_eq!(material.solver_frequency, 120.0);
        assert!(material.continuous_collision);
        assert_eq!(material.damping, Vec3::splat(0.1));
        assert_eq!(material.linear_inertia, Vec3::ONE);
    }

    #[test]
    fn preserves_zero_solver_frequency_used_by_shipped_cloth() {
        let mut bytes = material_bytes();
        bytes[100..104].copy_from_slice(&0.0_f32.to_le_bytes());

        let material = parse_cloth_material(&bytes).unwrap();

        assert_eq!(material.solver_frequency, 0.0);
        assert!(material.validate().is_ok());
    }

    fn material_bytes() -> Vec<u8> {
        let mut bytes = Vec::with_capacity(CLOTH_MATERIAL_SIZE);
        for value in [
            1.0_f32,
            2.0,
            0.5,
            1.0,
            1.0,
            2.0,
            0.5,
            1.0,
            1.0,
            2.0,
            0.5,
            1.0,
            1.0,
            2.0,
            0.5,
            1.0,
            50.0,
            0.5,
            0.5,
            0.5,
            1.0,
            0.0,
            0.0,
            1.0,
            1.0,
            120.0,
            30.0,
            f32::from_bits(1),
            0.1,
            0.1,
            0.1,
            0.0,
            0.1,
            0.1,
            0.1,
            0.0,
            0.1,
            0.1,
            0.1,
            0.0,
            1.0,
            1.0,
            1.0,
            0.0,
            1.0,
            1.0,
            1.0,
            0.0,
            1.0,
            1.0,
            1.0,
            0.0,
        ] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes
    }
}
