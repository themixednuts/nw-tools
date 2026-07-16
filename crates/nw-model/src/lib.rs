//! Assemble Cry chunk meshes ([`cry_chunk`]) into editable glTF source assets.
//!
//! Pipeline: a [`Model`] is built from a parsed `.cgf` (+ its `.cgfheap`) via
//! `Model::from((&CgfFile, heap))`, then serialized with the [`Gltf`] exporter.
//! Materials ([`MaterialSet`], parsed with [`str::parse`]) and textures attach to
//! the same exporter; skins and animations build on the same [`Model`] over time.

mod geometry;
mod gltf;
mod material;
pub mod math;
pub mod reflected;

pub use geometry::{
    AuxiliaryNode, AuxiliaryNodeRole, Bone, Mesh, MeshAttachment, Model, ModelBuildError,
    Primitive, Skeleton, SkeletonPlacement,
};
pub use gltf::{
    CryAssetExtras, CryNonRenderNode, CryNonRenderNodeRole, CrySourceAsset, CrySourceAssetKind,
    CryUnboundAnimation, Gltf, GltfAnimationError, ModelAnimation, NoMaterials, TextureData,
    WithMaterials,
};
pub use material::{MapSlot, MaterialSet, SubMaterial, TextureRef, TextureSourceKind};

/// Errors from building a model.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Chunk(#[from] cry_chunk::CgfParseError),
    #[error(transparent)]
    Model(#[from] ModelBuildError),
    #[error("no drawable geometry, skeleton, or auxiliary nodes found in chunk file")]
    NoGeometry,
    #[error("no compiled skeleton found in chunk file")]
    NoSkeleton,
}

/// Parse a `.cgf` and its heap and assemble a [`Model`] in one step.
///
/// # Errors
///
/// Returns [`Error`] if the chunk file fails to parse or has no model graph.
pub fn model_from_bytes(cgf: &[u8], heap: &[u8]) -> Result<Model, Error> {
    let file = cry_chunk::CgfFile::parse(cgf)?;
    let model = Model::try_from_cgf(&file, heap)?;
    if model.is_empty() && model.skeletons.is_empty() && model.auxiliary_nodes.is_empty() {
        return Err(Error::NoGeometry);
    }
    Ok(model)
}

/// Parse a `.chr`/`.skin`/`.cdf`-style Cry chunk file and return its first
/// compiled skeleton without requiring drawable geometry.
///
/// # Errors
///
/// Returns [`Error`] if the chunk file fails to parse or has no compiled bones.
pub fn skeleton_from_bytes(cgf: &[u8]) -> Result<Skeleton, Error> {
    let file = cry_chunk::CgfFile::parse(cgf)?;
    let Some(chunk) = file.compiled_bones().first() else {
        return Err(Error::NoSkeleton);
    };
    Ok(geometry::build_skeleton(chunk))
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn glb_has_valid_header() {
        use glam::Vec3;
        // A model with one triangle.
        let model = Model {
            skeletons: Vec::new(),
            auxiliary_nodes: Vec::new(),
            meshes: vec![Mesh {
                name: "tri".to_string(),
                skin: None,
                lod: None,
                shadow_proxy: false,
                attachment: None,
                primitives: vec![Primitive {
                    positions: vec![Vec3::ZERO, Vec3::X, Vec3::Y],
                    normals: None,
                    uvs: None,
                    indices: vec![0, 1, 2],
                    joints: None,
                    weights: None,
                    material_id: 0,
                }],
            }],
        };
        let glb = Gltf::new(&model).to_glb();
        assert_eq!(&glb[0..4], b"glTF");
        assert_eq!(u32::from_le_bytes([glb[4], glb[5], glb[6], glb[7]]), 2);
        let total = u32::from_le_bytes([glb[8], glb[9], glb[10], glb[11]]) as usize;
        assert_eq!(total, glb.len());
        // JSON chunk type tag.
        assert_eq!(&glb[16..20], b"JSON");
    }

    /// Extract and parse the JSON chunk of a GLB.
    fn glb_json(glb: &[u8]) -> serde_json::Value {
        let json_len = u32::from_le_bytes([glb[12], glb[13], glb[14], glb[15]]) as usize;
        serde_json::from_slice(&glb[20..20 + json_len]).unwrap()
    }

    #[test]
    fn glb_embeds_materials_textures_and_skips_nodraw() {
        use glam::Vec3;
        let mtl = MaterialSet::from_str(
            r#"<Material><SubMaterials>
                <Material Name="m0" Shader="Illum" Diffuse="1,1,1,1" Opacity="1">
                    <Textures><Texture Map="Diffuse" File="t/diff.tif"/></Textures>
                </Material>
                <Material Name="coll" Shader="Nodraw" Opacity="1"><Textures/></Material>
            </SubMaterials></Material>"#,
        )
        .unwrap();

        let tri = |material_id| Primitive {
            positions: vec![Vec3::ZERO, Vec3::X, Vec3::Y],
            normals: None,
            uvs: None,
            indices: vec![0, 1, 2],
            joints: None,
            weights: None,
            material_id,
        };
        let model = Model {
            skeletons: Vec::new(),
            auxiliary_nodes: Vec::new(),
            meshes: vec![Mesh {
                name: "m".to_string(),
                skin: None,
                lod: None,
                shadow_proxy: false,
                attachment: None,
                primitives: vec![tri(0), tri(1)], // second uses the Nodraw sub-material
            }],
        };

        let mut requested = Vec::new();
        let glb = Gltf::new(&model).materials(&mtl).to_glb(|file| {
            requested.push(file.to_string());
            Some(TextureData {
                bytes: vec![0x89, b'P', b'N', b'G'],
                mime: "image/png".to_string(),
            })
        });

        assert_eq!(requested, vec!["t/diff.tif"]);
        let json = glb_json(&glb);
        assert_eq!(json["materials"].as_array().unwrap().len(), 1);
        assert_eq!(json["textures"].as_array().unwrap().len(), 1);
        assert_eq!(json["images"].as_array().unwrap().len(), 1);
        // Only the non-Nodraw primitive survives.
        assert_eq!(json["meshes"][0]["primitives"].as_array().unwrap().len(), 1);
        let prim = &json["meshes"][0]["primitives"][0];
        assert_eq!(prim["material"], 0);
        assert_eq!(
            json["materials"][0]["pbrMetallicRoughness"]["baseColorTexture"]["index"],
            0
        );
    }

    #[test]
    fn glb_preserves_lods_with_msft_lod() {
        use glam::Vec3;

        let primitive = || Primitive {
            positions: vec![Vec3::ZERO, Vec3::X, Vec3::Y],
            normals: None,
            uvs: None,
            indices: vec![0, 1, 2],
            joints: None,
            weights: None,
            material_id: 0,
        };
        let model = Model {
            skeletons: Vec::new(),
            auxiliary_nodes: Vec::new(),
            meshes: vec![
                Mesh {
                    name: "body".to_owned(),
                    primitives: vec![primitive()],
                    skin: None,
                    lod: None,
                    shadow_proxy: false,
                    attachment: None,
                },
                Mesh {
                    name: "body$lod1".to_owned(),
                    primitives: vec![primitive()],
                    skin: None,
                    lod: Some(1),
                    shadow_proxy: false,
                    attachment: None,
                },
            ],
        };

        let json = glb_json(&Gltf::new(&model).to_glb());
        assert_eq!(json["extensionsUsed"], serde_json::json!(["MSFT_lod"]));
        assert_eq!(json["scenes"][0]["nodes"].as_array().unwrap().len(), 1);
        assert_eq!(json["nodes"][0]["extensions"]["MSFT_lod"]["ids"][0], 1);
        assert_eq!(json["nodes"][1]["extras"]["cryLod"], 1);
    }

    #[test]
    fn glb_emits_independent_parented_skeletons() {
        use glam::{Mat4, Vec3};

        let skeleton = |name: &str, controller_id, placement| Skeleton {
            bones: vec![Bone {
                name: name.to_owned(),
                controller_id,
                parent: None,
                local: Mat4::IDENTITY,
                inverse_bind: Mat4::IDENTITY,
            }],
            placement,
        };
        let primitive = || Primitive {
            positions: vec![Vec3::ZERO, Vec3::X, Vec3::Y],
            normals: None,
            uvs: None,
            indices: vec![0, 1, 2],
            joints: Some(vec![[0, 0, 0, 0]; 3]),
            weights: Some(vec![[1.0, 0.0, 0.0, 0.0]; 3]),
            material_id: 0,
        };
        let mesh = |name: &str, skin| Mesh {
            name: name.to_owned(),
            primitives: vec![primitive()],
            skin: Some(skin),
            lod: None,
            shadow_proxy: false,
            attachment: None,
        };
        let model = Model {
            skeletons: vec![
                skeleton("body_root", 1, None),
                skeleton(
                    "weapon_root",
                    2,
                    Some(SkeletonPlacement {
                        parent_skeleton: Some(0),
                        bone_name: Some("body_root".to_owned()),
                        local: Mat4::IDENTITY,
                    }),
                ),
            ],
            meshes: vec![mesh("body", 0), mesh("weapon", 1)],
            auxiliary_nodes: Vec::new(),
        };

        let json = glb_json(&Gltf::new(&model).to_glb());
        assert_eq!(json["skins"].as_array().unwrap().len(), 2);
        assert_eq!(json["skins"][0]["joints"], serde_json::json!([0]));
        assert_eq!(json["skins"][1]["joints"], serde_json::json!([1]));
        assert_eq!(json["nodes"][3]["skin"], 0);
        assert_eq!(json["nodes"][4]["skin"], 1);
        assert!(
            json["nodes"][0]["children"]
                .as_array()
                .unwrap()
                .contains(&serde_json::json!(2))
        );
        assert_eq!(json["nodes"][2]["children"], serde_json::json!([1]));
    }
}
