//! NvCloth `.cloth` attachment resolution.
//!
//! CDF `CA_CLOTH` attachments bind an NvCloth `.cloth` asset (AZ-serialized,
//! `01 00 00 00` magic) rather than a Cry chunk file. This module routes those
//! bindings through [`nv_cloth_assets`]: it ships the raw fabric (and its
//! `.clothmaterial`), resolves the referenced render `.skin` through the normal
//! chunk pipeline, and contributes the cooked simulation mesh so the cloak's
//! skinned sim geometry is present in the export.

use glam::Vec3;

use super::materials::{append_material_table, load_material, resolve_primary_materials};
use super::*;

/// Resolve a `CA_CLOTH` attachment whose Binding is an NvCloth `.cloth` asset.
///
/// Contributes the cooked simulation mesh to `model`, resolves the fabric's
/// render `.skin` as character-skinned geometry, and records the fabric plus its
/// `.clothmaterial` as embedded resources / dependencies.
// Resolving one attachment requires the complete mutable export context.
#[allow(clippy::too_many_arguments)]
pub(super) fn resolve_cloth_attachment(
    source: &dyn AssetSource,
    binding: &str,
    binding_bytes: &[u8],
    attachment: &cry_character::CharacterAttachment,
    no_materials: bool,
    model: &mut nw_model::Model,
    materials: &mut Option<nw_model::MaterialSet>,
    extras: &mut nw_model::CryAssetExtras,
) -> Result<()> {
    let fabric = nv_cloth_assets::parse_cloth_fabric(binding_bytes)
        .with_context(|| format!("parse NvCloth fabric {binding}"))?;

    // Ship the raw fabric plus its decoded document at the catalog path.
    if !has_source_asset(extras, binding) {
        extras.source_assets.push(nw_model::CrySourceAsset {
            path: normalize_path(binding),
            kind: nw_model::CrySourceAssetKind::NvClothFabric,
            document: serde_json::to_value(&fabric)
                .with_context(|| format!("serialize NvCloth fabric {binding}"))?,
        });
    }
    add_resource(
        extras,
        binding,
        nw_model::CryEmbeddedResourceKind::NvClothFabric,
        Arc::<[u8]>::from(binding_bytes),
    );
    add_dependency(extras, binding);

    // Ship the fabric's embedded cloth material, when it names one.
    if let Some(material_path) = fabric.material.as_deref() {
        let material_path = normalize_path(material_path);
        if let Some(material_bytes) = source.read(&material_path) {
            add_resource(
                extras,
                &material_path,
                nw_model::CryEmbeddedResourceKind::NvClothMaterial,
                material_bytes,
            );
        }
        add_dependency(extras, &material_path);
    }

    // Contribute the referenced render skin as character-skinned geometry.
    let render_binding = normalize_path(&fabric.render_model);
    if source_extension(&render_binding) == "skin" {
        let render_bytes = read_required(source, &render_binding)?;
        let render_heap = source
            .read(&format!("{render_binding}heap"))
            .unwrap_or_default();
        let render_file = cry_chunk::CgfFile::parse(&render_bytes)
            .with_context(|| format!("parse NvCloth render skin {render_binding}"))?;
        let mut render = nw_model::Model::try_from_cgf(&render_file, &render_heap)
            .with_context(|| format!("assemble NvCloth render skin {render_binding}"))?;
        if !no_materials && render.has_render_geometry() {
            let explicit = attachment
                .material
                .as_deref()
                .or_else(|| attachment.material_lods.get(&0).map(String::as_str));
            let set = if let Some(path) = explicit {
                load_material(source, path)?
            } else {
                resolve_primary_materials(
                    source,
                    &render_bytes,
                    &MeshRef::for_key(&render_binding),
                    None,
                    false,
                    true,
                )?
                .with_context(|| {
                    format!("resolve material for NvCloth render skin {render_binding}")
                })?
            };
            append_material_table(&mut render, materials, set)?;
        }
        model.append_skinned_geometry(render, 0)?;
        add_dependency(extras, &render_binding);
    }

    // Append the cooked simulation mesh: positions + character-skeleton skinning.
    // The fabric's joint indices reference the character skeleton directly, so
    // `append_cloth_simulation_geometry` validates them against it.
    let simulation = cloth_simulation_model(&fabric);
    if !simulation.is_empty() {
        model
            .append_cloth_simulation_geometry(simulation, 0)
            .with_context(|| format!("append NvCloth simulation mesh {binding}"))?;
    }
    Ok(())
}

/// Build a single-mesh [`nw_model::Model`] from a cooked cloth fabric's
/// simulation vertices and triangle list, converted into glTF space.
fn cloth_simulation_model(fabric: &nv_cloth_assets::ClothFabric) -> nw_model::Model {
    let vertices = &fabric.mesh.vertices;
    let positions = vertices
        .iter()
        .map(|vertex| nw_model::math::cry_to_gltf(vertex.position))
        .collect::<Vec<_>>();
    let normals = vertices
        .iter()
        .map(|vertex| {
            // The tangent frame's rotated Z axis is the surface normal, matching
            // the qtangent decode used for chunk meshes.
            let mut normal = nw_model::math::cry_to_gltf(vertex.tangent_frame * Vec3::Z);
            if vertex.tangent_frame.w < 0.0 {
                normal = -normal;
            }
            normal.normalize_or_zero()
        })
        .collect::<Vec<_>>();
    let joints = vertices
        .iter()
        .map(|vertex| vertex.joint_indices)
        .collect::<Vec<_>>();
    let weights = vertices
        .iter()
        .map(|vertex| {
            // NvCloth stores four u8 skin weights summing to 255.
            vertex
                .joint_weights
                .map(|weight| f32::from(weight) / f32::from(u8::MAX))
        })
        .collect::<Vec<_>>();

    let primitive = nw_model::Primitive {
        positions,
        normals: Some(normals),
        uvs: None,
        indices: fabric.mesh.indices.clone(),
        joints: Some(joints),
        weights: Some(weights),
        joints2: None,
        weights2: None,
        material_id: -1,
    };
    let mesh = nw_model::Mesh {
        name: "cloth_simulation".to_owned(),
        physics_data: Vec::new(),
        primitives: vec![primitive],
        role: nw_model::MeshRole::ClothSimulation,
        skin: Some(0),
        lod: None,
        shadow_proxy: false,
        attachment: None,
    };
    nw_model::Model {
        meshes: vec![mesh],
        skeletons: Vec::new(),
        auxiliary_nodes: Vec::new(),
    }
}
