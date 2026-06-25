//! Assemble Cry chunk meshes ([`cry_chunk`]) into editable glTF source assets.
//!
//! Pipeline: a [`Model`] is built from a parsed `.cgf` (+ its `.cgfheap`) via
//! `Model::from((&CgfFile, heap))`, then serialized with the [`Gltf`] exporter.
//! Materials ([`MaterialSet`], parsed with [`str::parse`]) and textures attach to
//! the same exporter; skins and animations build on the same [`Model`] over time.

mod geometry;
mod gltf;
mod material;
mod math;

pub use geometry::{Mesh, Model, Primitive};
pub use gltf::{Gltf, NoMaterials, TextureData, WithMaterials};
pub use material::{MapSlot, MaterialSet, SubMaterial, TextureRef};

/// Errors from building a model.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Chunk(#[from] cry_chunk::CgfParseError),
    #[error("no drawable geometry found in chunk file")]
    NoGeometry,
}

/// Parse a `.cgf` and its heap and assemble a [`Model`] in one step.
///
/// # Errors
///
/// Returns [`Error`] if the chunk file fails to parse or has no geometry.
pub fn model_from_bytes(cgf: &[u8], heap: &[u8]) -> Result<Model, Error> {
    let file = cry_chunk::CgfFile::parse(cgf)?;
    let model = Model::from((&file, heap));
    if model.is_empty() {
        return Err(Error::NoGeometry);
    }
    Ok(model)
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
            skeleton: None,
            meshes: vec![Mesh {
                name: "tri".to_string(),
                skinned: false,
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
            skeleton: None,
            meshes: vec![Mesh {
                name: "m".to_string(),
                skinned: false,
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
}
