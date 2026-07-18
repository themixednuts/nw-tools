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
mod physics;
pub mod reflected;

pub use geometry::{
    AuxiliaryNode, AuxiliaryNodeRole, Bone, Mesh, MeshAttachment, MeshRole, Model, ModelBuildError,
    Primitive, Skeleton, SkeletonPlacement,
};
pub use gltf::{
    AudioTriggerMediaRef, AudioTriggerPlayback, AudioTriggerResolution, AudioTriggerSurfaceMedia,
    AudioTriggerWwiseEvent,
    CryAssetExtras, CryEmbeddedResource, CryEmbeddedResourceKind, CryMannequinAnimationAudio,
    CryMannequinAudioClip, CryNonRenderNode,
    CryNonRenderNodeRole, CryResourcePayload, CrySourceAsset, CrySourceAssetKind,
    CryUnboundAnimation, Gltf, GltfAnimationError, GltfPackage, GltfPackageError, GltfResource,
    ModelAnimation, NoMaterials, TextureData, WithMaterials,
};
pub use material::{MapSlot, MaterialSet, SubMaterial, TextureRef, TextureSourceKind};
pub use physics::{
    BvhTree, CompoundChild, ConvexHullExtra, HeightFieldData, HitVolume, PhysicalShape,
    PhysicsComponentContext, PhysicsMaterialFilter, PhysicsScene, PhysicsSceneError,
    PhysicsShapeAsset, PhysicsShapeData, PhysicsShapeObject, PhysicsTransform, QueryShape,
    RigidBody, ShapeTransform,
};

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
                role: MeshRole::Render,
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
                    joints2: None,
                    weights2: None,
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

    #[test]
    fn structured_gltf_keeps_reusable_mesh_buffers_separate() {
        use glam::Vec3;

        let primitive = || Primitive {
            positions: vec![Vec3::ZERO, Vec3::X, Vec3::Y],
            normals: None,
            uvs: None,
            indices: vec![0, 1, 2],
            joints: None,
            weights: None,
            joints2: None,
            weights2: None,
            material_id: 0,
        };
        let mesh = |name: &str| Mesh {
            name: name.to_string(),
            role: MeshRole::Render,
            skin: None,
            lod: None,
            shadow_proxy: false,
            attachment: None,
            primitives: vec![primitive()],
        };
        let model = Model {
            skeletons: Vec::new(),
            auxiliary_nodes: Vec::new(),
            meshes: vec![mesh("left"), mesh("right")],
        };
        let package = Gltf::new(&model).to_gltf_package();

        assert_eq!(package.resources().len(), 2);
        assert_eq!(package.resources()[0].extension(), "bin");
        // Anonymous geometry buffers carry no catalog path; the exporter names
        // them from the manifest.
        assert_eq!(package.resources()[0].path(), None);
        assert_eq!(
            package.resources()[0].bytes(),
            package.resources()[1].bytes()
        );
        let shared_uri = "../shared.bin".to_string();
        let json: serde_json::Value = serde_json::from_str(
            &package
                .into_json(&[shared_uri.clone(), shared_uri.clone()])
                .unwrap(),
        )
        .unwrap();
        assert_eq!(json["buffers"].as_array().unwrap().len(), 2);
        assert_eq!(json["buffers"][0]["uri"], shared_uri);
        assert_eq!(json["buffers"][1]["uri"], shared_uri);
        assert_eq!(json["bufferViews"][0]["buffer"], 0);
        assert_eq!(json["bufferViews"][2]["buffer"], 1);
    }

    #[test]
    fn structured_gltf_deduplicates_resource_identity_and_mirrors_catalog_paths() {
        use glam::Vec3;

        let model = Model {
            skeletons: Vec::new(),
            auxiliary_nodes: Vec::new(),
            meshes: vec![Mesh {
                name: "tree".to_owned(),
                role: MeshRole::Render,
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
                    joints2: None,
                    weights2: None,
                    material_id: 0,
                }],
            }],
        };
        let payload = b"same reflected slice metadata".to_vec();
        let extras = CryAssetExtras {
            resource_payloads: vec![
                CryResourcePayload::new(
                    "slices/tree_oak.slice.meta",
                    CryEmbeddedResourceKind::SliceMetadata,
                    payload.clone(),
                ),
                // Same authored identity must not allocate a second buffer.
                CryResourcePayload::new(
                    "SLICES/TREE_OAK.SLICE.META",
                    CryEmbeddedResourceKind::SliceMetadata,
                    payload.clone(),
                ),
                // A second authored asset may reference the same content hash.
                CryResourcePayload::new(
                    "slices/tree_fall.slice.meta",
                    CryEmbeddedResourceKind::SliceMetadata,
                    payload,
                ),
            ],
            ..CryAssetExtras::default()
        };
        let package = Gltf::new(&model).extras(&extras).to_gltf_package();

        // Mesh buffer + two distinct authored resource identities.
        assert_eq!(package.resources().len(), 3);
        assert_eq!(package.resources()[1].extension(), "slice.meta");
        // Raw dependency payloads keep their exact pak paths.
        assert_eq!(
            package.resources()[1].path(),
            Some("slices/tree_oak.slice.meta")
        );
        assert_eq!(
            package.resources()[2].path(),
            Some("slices/tree_fall.slice.meta")
        );
        assert_eq!(
            package.resources()[1].bytes(),
            package.resources()[2].bytes()
        );
        let json: serde_json::Value = serde_json::from_str(
            &package
                .into_json(&[
                    "alligator.bin".to_owned(),
                    "slices/tree_oak.slice.meta".to_owned(),
                    "slices/tree_fall.slice.meta".to_owned(),
                ])
                .unwrap(),
        )
        .unwrap();
        assert_eq!(json["buffers"].as_array().unwrap().len(), 3);
        // Distinct authored identities mirror distinct catalog paths.
        assert_eq!(json["buffers"][1]["uri"], "slices/tree_oak.slice.meta");
        assert_eq!(json["buffers"][2]["uri"], "slices/tree_fall.slice.meta");
        assert_eq!(
            json["extras"]["embeddedResources"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
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
            joints2: None,
            weights2: None,
            material_id,
        };
        let model = Model {
            skeletons: Vec::new(),
            auxiliary_nodes: Vec::new(),
            meshes: vec![Mesh {
                name: "m".to_string(),
                role: MeshRole::Render,
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
                path: None,
                derived_roughness: None,
            })
        });

        assert_eq!(requested, vec!["t/diff.tif"]);
        let json = glb_json(&glb);
        assert_eq!(json["materials"].as_array().unwrap().len(), 1);
        assert_eq!(json["textures"].as_array().unwrap().len(), 1);
        assert_eq!(json["images"].as_array().unwrap().len(), 1);
        assert!(json["images"][0]["bufferView"].is_number());
        assert!(json["images"][0].get("uri").is_none());
        // Only the non-Nodraw primitive survives.
        assert_eq!(json["meshes"][0]["primitives"].as_array().unwrap().len(), 1);
        let prim = &json["meshes"][0]["primitives"][0];
        assert_eq!(prim["material"], 0);
        assert_eq!(
            json["materials"][0]["pbrMetallicRoughness"]["baseColorTexture"]["index"],
            0
        );

        let package = Gltf::new(&model).materials(&mtl).to_gltf_package(|_| {
            Some(TextureData {
                bytes: vec![0x89, b'P', b'N', b'G'],
                mime: "image/png".to_string(),
                path: None,
                derived_roughness: None,
            })
        });
        assert_eq!(package.resources().len(), 2);
        assert_eq!(package.resources()[1].extension(), "png");
        // A loader that supplies no path leaves the exporter to name the image.
        assert_eq!(package.resources()[1].path(), None);
        let structured: serde_json::Value = serde_json::from_str(
            &package
                .into_json(&[
                    "shared/mesh.bin".to_string(),
                    "shared/image.png".to_string(),
                ])
                .unwrap(),
        )
        .unwrap();
        assert_eq!(structured["images"][0]["uri"], "shared/image.png");
        assert!(structured["images"][0].get("bufferView").is_none());
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
            joints2: None,
            weights2: None,
            material_id: 0,
        };
        let model = Model {
            skeletons: Vec::new(),
            auxiliary_nodes: Vec::new(),
            meshes: vec![
                Mesh {
                    name: "body".to_owned(),
                    primitives: vec![primitive()],
                    role: MeshRole::Render,
                    skin: None,
                    lod: None,
                    shadow_proxy: false,
                    attachment: None,
                },
                Mesh {
                    name: "body$lod1".to_owned(),
                    primitives: vec![primitive()],
                    role: MeshRole::Render,
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
            joints2: None,
            weights2: None,
            material_id: 0,
        };
        let mesh = |name: &str, skin| Mesh {
            name: name.to_owned(),
            primitives: vec![primitive()],
            role: MeshRole::Render,
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

    /// A single-bone character skeleton whose controller a test clip targets.
    fn single_bone_model() -> Model {
        use glam::Mat4;
        Model {
            skeletons: vec![Skeleton {
                bones: vec![Bone {
                    name: "root".to_owned(),
                    controller_id: 1,
                    parent: None,
                    local: Mat4::IDENTITY,
                    inverse_bind: Mat4::IDENTITY,
                }],
                placement: None,
            }],
            meshes: Vec::new(),
            auxiliary_nodes: Vec::new(),
        }
    }

    /// A minimal CAF clip with one position key on `controller_id`, sourced from
    /// `source_path` (its clip name is the CAF stem, matching `animation_name`).
    fn caf_clip(source_path: &str, controller_id: u32) -> ModelAnimation {
        use cry_animation::{
            AnimationClip, CafAnimation, CafAnimationHeader, CafController, PositionKey,
        };
        let name = source_path
            .rsplit('/')
            .next()
            .and_then(|file| file.strip_suffix(".caf"))
            .unwrap_or(source_path)
            .to_owned();
        ModelAnimation {
            skeleton: 0,
            clip: AnimationClip {
                source_path: source_path.to_owned(),
                name,
                caf: CafAnimation {
                    header: CafAnimationHeader {
                        flags: 0,
                        start_sec: 0.0,
                        end_sec: 1.0,
                        total_duration: 1.0,
                        controller_count: 1,
                        source: cry_chunk::CafAnimationHeaderSource::Timing,
                        file_path: None,
                    },
                    sample_rate: 30.0,
                    controllers: vec![CafController {
                        controller_id,
                        flags: 0,
                        rotations: Vec::new(),
                        positions: vec![PositionKey {
                            time: 0.0,
                            value: [0.0, 0.0, 0.0],
                        }],
                        scales: Vec::new(),
                    }],
                    root_motion: None,
                },
                events: Vec::new(),
            },
        }
    }

    #[test]
    fn structured_gltf_places_animation_buffer_at_caf_catalog_path() {
        let model = single_bone_model();
        let clips = vec![caf_clip("animations/alligator/alligator_idle.caf", 1)];
        let package = Gltf::new(&model)
            .animations(&clips)
            .unwrap()
            .to_gltf_package();

        // The channel buffer IS the animation asset at its exact CAF catalog
        // path.
        assert!(
            package
                .resources()
                .iter()
                .any(|resource| resource.path()
                    == Some("animations/alligator/alligator_idle.caf")),
            "animation buffer must take the source CAF's catalog path"
        );
        // No derived `.caf.bin` sibling and no raw CryAnimation payload ship.
        assert!(
            package.resources().iter().all(|resource| resource.path()
                != Some("animations/alligator/alligator_idle.caf.bin")),
            "no `.caf.bin` sibling may exist"
        );
    }

    #[test]
    fn hit_volume_parents_under_target_bone() {
        use glam::{Mat4, Vec3};

        // Two-joint skeleton: the volume targets `head`, which sits above `root`.
        let model = Model {
            skeletons: vec![Skeleton {
                bones: vec![
                    Bone {
                        name: "root".to_owned(),
                        controller_id: 1,
                        parent: None,
                        local: Mat4::IDENTITY,
                        inverse_bind: Mat4::IDENTITY,
                    },
                    Bone {
                        name: "head".to_owned(),
                        controller_id: 2,
                        parent: Some(0),
                        local: Mat4::from_translation(Vec3::new(0.0, 1.0, 0.0)),
                        inverse_bind: Mat4::IDENTITY,
                    },
                ],
                placement: None,
            }],
            meshes: Vec::new(),
            auxiliary_nodes: Vec::new(),
        };

        let center = Vec3::new(0.1, 0.2, 0.3);
        let scene = PhysicsScene {
            hit_volumes: vec![HitVolume {
                context: PhysicsComponentContext::default(),
                center,
                shape: QueryShape::Sphere { radius: 0.5 },
                damage_multiplier: 1.0,
                is_headshot: false,
                is_legshot: false,
                volume_name: "head_capsule".to_owned(),
                filter: String::new(),
                target_bone_name: "HEAD".to_owned(),
                hit_category: String::new(),
                lightweight_character_entity_id: 0,
                source: serde_json::Value::Null,
            }],
            ..PhysicsScene::default()
        };

        let json = glb_json(&Gltf::new(&model).physics(&scene).unwrap().to_glb());
        let nodes = json["nodes"].as_array().unwrap();
        let head = nodes
            .iter()
            .position(|node| node["name"] == "head")
            .expect("head joint node");
        let volume = nodes
            .iter()
            .position(|node| node["name"] == "head_capsule")
            .expect("hit-volume node");

        // The volume node is a child of the `head` joint (case-insensitive match).
        assert!(
            nodes[head]["children"]
                .as_array()
                .unwrap()
                .contains(&serde_json::json!(volume))
        );
        // Its local transform is the bone-local center only — no root pooling.
        // The authored Cry-space center is expressed in the same Y-up basis as
        // the joint it parents under (cry_to_gltf: (x, y, z) → (-x, z, y)).
        let translation = nodes[volume]["translation"].as_array().unwrap();
        let component = |index: usize| translation[index].as_f64().unwrap() as f32;
        assert_eq!(
            [component(0), component(1), component(2)],
            math::cry_to_gltf(center).to_array()
        );
        assert!(nodes[volume]["rotation"].is_null());
        assert!(nodes[volume]["scale"].is_null());
    }

    #[test]
    fn query_shape_visuals_share_physics_debug_material() {
        use glam::Vec3;

        // One hit volume (query shape), one rigid body (query shape), and one
        // RockNRoll `.rnr` shape asset. Only the first two share the engine
        // debug-draw material; the `.rnr` visual keeps no material.
        let model = Model {
            skeletons: Vec::new(),
            meshes: Vec::new(),
            auxiliary_nodes: Vec::new(),
        };
        let scene = PhysicsScene {
            shape_assets: vec![PhysicsShapeAsset {
                source_path: "physics/test.rnr".to_owned(),
                version: 1,
                asset_guid: None,
                objects: vec![PhysicsShapeObject {
                    name: "rnr_sphere".to_owned(),
                    material_indices: Vec::new(),
                }],
                material_filter: PhysicsMaterialFilter::default(),
                shapes: vec![PhysicalShape {
                    data: PhysicsShapeData::Sphere { radius: 0.5 },
                    extra_byte_length: 0,
                }],
                source_bytes: std::sync::Arc::from(&b"rnr-bytes"[..]),
            }],
            hit_volumes: vec![HitVolume {
                context: PhysicsComponentContext::default(),
                center: Vec3::ZERO,
                shape: QueryShape::Sphere { radius: 0.5 },
                damage_multiplier: 1.0,
                is_headshot: false,
                is_legshot: false,
                volume_name: "hit_sphere".to_owned(),
                filter: String::new(),
                target_bone_name: String::new(),
                hit_category: String::new(),
                lightweight_character_entity_id: 0,
                source: serde_json::Value::Null,
            }],
            rigid_bodies: vec![RigidBody {
                context: PhysicsComponentContext::default(),
                name: "rigid_box".to_owned(),
                center: Vec3::ZERO,
                shape: Some(QueryShape::Box { size: Vec3::ONE }),
                shape_asset_path: None,
                source: serde_json::Value::Null,
            }],
        };

        let json = glb_json(&Gltf::new(&model).physics(&scene).unwrap().to_glb());

        // Exactly one shared physics_debug material.
        let materials = json["materials"].as_array().unwrap();
        let debug: Vec<usize> = materials
            .iter()
            .enumerate()
            .filter(|(_, material)| material["name"] == "physics_debug")
            .map(|(index, _)| index)
            .collect();
        assert_eq!(debug.len(), 1, "one shared physics_debug material");
        let debug_index = debug[0];
        let material = &materials[debug_index];

        // Engine debug-draw look: unlit BLEND, double-sided, wire-pass RGBA.
        let approx = |value: &serde_json::Value, expected: f64| {
            (value.as_f64().unwrap() - expected).abs() < 1e-6
        };
        let base = material["pbrMetallicRoughness"]["baseColorFactor"]
            .as_array()
            .unwrap();
        assert!(
            approx(&base[0], 0.22)
                && approx(&base[1], 1.0)
                && approx(&base[2], 0.38)
                && approx(&base[3], 0.4),
            "baseColorFactor {base:?}"
        );
        assert_eq!(material["alphaMode"], "BLEND");
        assert_eq!(material["doubleSided"], true);
        assert!(approx(&material["pbrMetallicRoughness"]["metallicFactor"], 0.0));
        assert!(approx(
            &material["pbrMetallicRoughness"]["roughnessFactor"],
            1.0
        ));
        assert!(
            material["extensions"]["KHR_materials_unlit"].is_object(),
            "unlit extension present"
        );

        // Extras record the engine's exact two-pass values.
        let draw = &material["extras"]["cryDebugDraw"];
        let rgb = draw["rgb"].as_array().unwrap();
        assert!(approx(&rgb[0], 0.22) && approx(&rgb[1], 1.0) && approx(&rgb[2], 0.38));
        assert!(approx(&draw["fillAlpha"], 0.05));
        assert!(approx(&draw["wireAlpha"], 0.4));

        // Hit-volume and rigid-body meshes reference it; the `.rnr` mesh does not.
        let meshes = json["meshes"].as_array().unwrap();
        let mesh_material = |name: &str| {
            meshes
                .iter()
                .find(|mesh| mesh["name"] == name)
                .and_then(|mesh| mesh["primitives"][0]["material"].as_u64())
        };
        assert_eq!(mesh_material("hit_sphere"), Some(debug_index as u64));
        assert_eq!(mesh_material("rigid_box"), Some(debug_index as u64));
        assert_eq!(mesh_material("rnr_sphere"), None);

        // The extension is registered only when the material is emitted.
        let extensions = json["extensionsUsed"].as_array().unwrap();
        assert!(extensions.iter().any(|ext| ext == "KHR_materials_unlit"));
    }

    #[test]
    fn animations_order_idle_first_then_by_name() {
        let model = single_bone_model();
        // Deliberately unsorted, with idle neither first nor name-sorted first.
        let clips = vec![
            caf_clip("animations/x/walk.caf", 1),
            caf_clip("animations/x/attack.caf", 1),
            caf_clip("animations/x/idle.caf", 1),
        ];
        let json = glb_json(&Gltf::new(&model).animations(&clips).unwrap().to_glb());
        let names = json["animations"]
            .as_array()
            .unwrap()
            .iter()
            .map(|animation| animation["name"].as_str().unwrap())
            .collect::<Vec<_>>();
        // Blender activates `animations[0]`, so idle leads; the rest stay
        // name-sorted (case-insensitive).
        assert_eq!(names, vec!["idle", "attack", "walk"]);
    }

    #[test]
    fn audio_media_ref_serializes_default_branch_only_when_true() {
        // A default-branch media ref carries `defaultBranch: true`; the flag is
        // omitted entirely when false so the common case stays terse.
        let default = AudioTriggerMediaRef {
            media_id: 42,
            bank: Some("sounds/wwise/ftsp_alligator.bnk".to_owned()),
            path: None,
            default_branch: true,
        };
        let json = serde_json::to_value(&default).unwrap();
        assert_eq!(json["mediaId"], 42);
        assert_eq!(json["defaultBranch"], serde_json::json!(true));

        let other = AudioTriggerMediaRef {
            media_id: 7,
            bank: None,
            path: None,
            default_branch: false,
        };
        let json = serde_json::to_value(&other).unwrap();
        assert!(
            json.get("defaultBranch").is_none(),
            "defaultBranch must be skipped when false: {json}"
        );
    }

    #[test]
    fn surface_media_serializes_the_per_surface_breakdown() {
        // The default branch carries a resolved surface name, its media pool, and
        // the deterministic weighted sequence; a non-default branch omits the
        // `default`/`sequence` keys and an unresolved branch omits `surface`.
        let resolution = AudioTriggerResolution {
            trigger: "blend_ftsp_alligator".to_owned(),
            wwise_events: Vec::new(),
            banks: Vec::new(),
            media: Vec::new(),
            surfaces: Vec::new(),
            surface_media: vec![
                AudioTriggerSurfaceMedia {
                    surface: Some("metal".to_owned()),
                    switch_id: 2_473_969_246,
                    default: true,
                    media: vec![10, 20],
                    sequence: vec![20, 10, 20],
                },
                AudioTriggerSurfaceMedia {
                    surface: None,
                    switch_id: 712_161_704,
                    default: false,
                    media: vec![30],
                    sequence: Vec::new(),
                },
            ],
            playback: None,
        };
        let json = serde_json::to_value(&resolution).unwrap();
        let branches = json["surfaceMedia"].as_array().unwrap();
        assert_eq!(branches[0]["surface"], "metal");
        assert_eq!(branches[0]["switchId"], 2_473_969_246u32);
        assert_eq!(branches[0]["default"], serde_json::json!(true));
        assert_eq!(branches[0]["sequence"], serde_json::json!([20, 10, 20]));
        // Non-default, unresolved branch: no `surface`, no `default`, no `sequence`.
        assert!(branches[1].get("surface").is_none());
        assert!(branches[1].get("default").is_none());
        assert!(branches[1].get("sequence").is_none());
        assert_eq!(branches[1]["switchId"], 712_161_704u32);
    }
}
