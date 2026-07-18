//! Legacy RockNRoll source conversion into the glTF export-owned physics model.

use anyhow::{Context, Result, bail};
use nw_rnr::legacy as source;

pub(crate) fn physics_shape_asset(
    source_path: &str,
    bytes: &[u8],
) -> Result<nw_model::PhysicsShapeAsset> {
    let asset = source::parse_shape_asset(bytes)
        .with_context(|| format!("parse RockNRoll shape asset {source_path}"))?;
    let objects = asset
        .objects
        .iter()
        .map(|object| nw_model::PhysicsShapeObject {
            name: object.name.to_owned(),
            material_indices: object.material_indices.iter().collect(),
        })
        .collect();
    let material_filter = nw_model::PhysicsMaterialFilter {
        enabled: asset.material_filter.enabled,
        secondary_geometry: asset.material_filter.secondary_geometry,
        indices: asset.material_filter.indices.iter().collect(),
    };
    let shapes = asset
        .shapes
        .iter()
        .map(convert_shape)
        .collect::<Result<Vec<_>>>()?;
    Ok(nw_model::PhysicsShapeAsset {
        source_path: source_path.replace('\\', "/"),
        version: asset.version,
        asset_guid: asset.asset_guid,
        objects,
        material_filter,
        shapes,
        source_bytes: bytes.into(),
    })
}

fn convert_shape(shape: &source::PhysicalShape<'_>) -> Result<nw_model::PhysicalShape> {
    let data = match &shape.data {
        source::ShapeData::Box(value) => nw_model::PhysicsShapeData::Box {
            half_extents: value.half_extents,
            convex_radius: value.convex_radius,
        },
        source::ShapeData::Sphere(value) => nw_model::PhysicsShapeData::Sphere {
            radius: value.radius,
        },
        source::ShapeData::ConvexHull(value) => nw_model::PhysicsShapeData::ConvexHull {
            vertices: value.vertices.iter().collect(),
            planes: value.planes.iter().collect(),
            convex_radius: value.convex_radius,
            extra: value.extra.map(|extra| nw_model::ConvexHullExtra {
                data_a: extra.data_a.iter().collect(),
                data_b: extra.data_b.iter().collect(),
            }),
        },
        source::ShapeData::Cylinder(value) => nw_model::PhysicsShapeData::Cylinder {
            half_height: value.half_height,
            radius: value.radius,
            convex_radius: value.convex_radius,
        },
        source::ShapeData::CylinderUnaligned(value) => {
            nw_model::PhysicsShapeData::CylinderUnaligned {
                endpoint_a: value.endpoint_a,
                endpoint_b: value.endpoint_b,
                radius: value.radius,
                convex_radius: value.convex_radius,
            }
        }
        source::ShapeData::Capsule(value) => nw_model::PhysicsShapeData::Capsule {
            half_height: value.half_height,
            radius: value.radius,
        },
        source::ShapeData::CapsuleUnaligned(value) => {
            nw_model::PhysicsShapeData::CapsuleUnaligned {
                endpoint_a: value.endpoint_a,
                endpoint_b: value.endpoint_b,
                radius: value.radius,
            }
        }
        source::ShapeData::Triangle(value) => nw_model::PhysicsShapeData::Triangle {
            a: value.a,
            b: value.b,
            c: value.c,
            convex_radius: value.convex_radius,
        },
        source::ShapeData::Mesh(value) => nw_model::PhysicsShapeData::Mesh {
            stream_header: value.stream_header,
            vertices: value.vertices.iter().collect(),
            indices: value.indices.iter().collect(),
            adjacent_triangles: value
                .adjacent_triangles
                .map(|indices| indices.iter().collect()),
            bvh: convert_bvh(&value.bvh)?,
        },
        source::ShapeData::Compound(value) => nw_model::PhysicsShapeData::Compound {
            children: value
                .children
                .iter()
                .map(|child| {
                    Ok(nw_model::CompoundChild {
                        transform: child.transform,
                        shape: Box::new(convert_shape(&child.shape)?),
                    })
                })
                .collect::<Result<Vec<_>>>()?,
        },
        source::ShapeData::Transform(value) => nw_model::PhysicsShapeData::Transform {
            transform: value.transform,
            shape: Box::new(convert_shape(&value.shape)?),
        },
        source::ShapeData::SoftBody(_) => nw_model::PhysicsShapeData::SoftBody,
        source::ShapeData::Plane(value) => nw_model::PhysicsShapeData::Plane {
            plane: value.plane,
            aabb_min: value.aabb_min,
            aabb_max: value.aabb_max,
        },
        source::ShapeData::ScaleConvexPolytope(value) => {
            nw_model::PhysicsShapeData::ScaleConvexPolytope {
                stream_header: value.stream_header,
                scale: value.scale,
                shape: Box::new(convert_shape(&value.shape)?),
            }
        }
        source::ShapeData::ScaleMesh(value) => nw_model::PhysicsShapeData::ScaleMesh {
            stream_header: value.stream_header,
            scale: value.scale,
            shape: Box::new(convert_shape(&value.shape)?),
        },
        source::ShapeData::HeightField(value) => nw_model::PhysicsShapeData::HeightField {
            layout: value.layout,
            data: value.data.map(|data| nw_model::HeightFieldData {
                version: data.version,
                width: data.width,
                length: data.length,
                height_scale: data.height_scale,
                aabb_min: data.aabb_min,
                aabb_max: data.aabb_max,
                sample_byte_length: data.samples.len(),
                samples: data.samples.to_vec(),
            }),
        },
    };
    Ok(nw_model::PhysicalShape {
        data,
        extra_byte_length: shape.extra.map_or(0, <[u8]>::len),
    })
}

fn convert_bvh(value: &source::BvhTree<'_>) -> Result<nw_model::BvhTree> {
    let (version, parts) = match value {
        source::BvhTree::V1(parts) => (1, parts),
        source::BvhTree::V2(parts) => (2, parts),
    };
    Ok(nw_model::BvhTree {
        version,
        quantized_nodes_offset: payload_offset(parts.payload, parts.quantized_nodes)?,
        subtree_infos_offset: payload_offset(parts.payload, parts.subtree_infos)?,
        triangle_index_map_offset: payload_offset(parts.payload, parts.triangle_index_map)?,
        quantized_node_count: parts.quantized_node_count,
        subtree_info_count: parts.subtree_info_count,
        triangle_index_count: parts.triangle_index_count,
        flags: parts.flags,
        payload_byte_length: parts.payload.len(),
    })
}

fn payload_offset(payload: &[u8], part: &[u8]) -> Result<u32> {
    let Some(offset) = (part.as_ptr() as usize).checked_sub(payload.as_ptr() as usize) else {
        bail!("RockNRoll BVH part begins before its payload");
    };
    if offset > payload.len() || offset.saturating_add(part.len()) > payload.len() {
        bail!("RockNRoll BVH part lies outside its payload");
    }
    u32::try_from(offset).context("RockNRoll BVH offset exceeds u32")
}
