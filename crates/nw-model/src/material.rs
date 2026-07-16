//! Lossless, typed projection of CryEngine/Lumberyard `.mtl` XML.
//!
//! The typed fields follow `EEfResTextures`, `IMaterial.h`, and
//! `MaterialHelpers.cpp`. The original XML tree is retained so shader-specific
//! public parameters, texture modifiers, and future engine fields are never
//! discarded by an export/import round trip.

use std::str::FromStr;

use cry_xml::XmlElement;
use glam::Vec4;
use nw_asset::{AssetDependencies, AssetDependency};

/// `EMaterialFlags` values from `CryCommon/IMaterial.h`.
pub const MTL_FLAG_2SIDED: i64 = 0x0002;
pub const MTL_FLAG_ADDITIVE: i64 = 0x0004;
pub const MTL_FLAG_NOSHADOW: i64 = 0x0020;
pub const MTL_FLAG_NODRAW: i64 = 0x0400;
pub const MTL_FLAG_REQUIRE_FORWARD_RENDERING: i64 = 0x8000;

/// `EEfResTextures` in engine order. Unknown authored names retain their exact
/// spelling instead of collapsing into an untyped catch-all.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MapSlot {
    Diffuse,
    Normals,
    Specular,
    Environment,
    DetailOverlay,
    SecondSmoothness,
    Height,
    DecalOverlay,
    Subsurface,
    Custom,
    CustomSecondary,
    Opacity,
    Smoothness,
    Emittance,
    Occlusion,
    Specular2,
    Unknown(String),
}

impl MapSlot {
    #[must_use]
    pub fn from_authored_name(value: &str) -> Self {
        match value.to_ascii_lowercase().as_str() {
            "diffuse" => Self::Diffuse,
            // Lumberyard retains these aliases for backwards compatibility.
            "bumpmap" | "normal" => Self::Normals,
            "specular" => Self::Specular,
            "environment" => Self::Environment,
            "detail" => Self::DetailOverlay,
            "secondsmoothness" => Self::SecondSmoothness,
            "heightmap" | "height" => Self::Height,
            "decal" | "decaloverlay" => Self::DecalOverlay,
            "subsurface" => Self::Subsurface,
            "custom" | "custommap" => Self::Custom,
            "[1] custom" | "customsecondary" | "customsecondarymap" => Self::CustomSecondary,
            "opacity" => Self::Opacity,
            "smoothness" | "glossnormala" => Self::Smoothness,
            "emittance" | "emissive" => Self::Emittance,
            "occlusion" => Self::Occlusion,
            "specular2" => Self::Specular2,
            _ => Self::Unknown(value.to_owned()),
        }
    }

    /// Canonical authored name from Lumberyard's `MaterialHelpers` table.
    #[must_use]
    pub fn authored_name(&self) -> &str {
        match self {
            Self::Diffuse => "Diffuse",
            Self::Normals => "Bumpmap",
            Self::Specular => "Specular",
            Self::Environment => "Environment",
            Self::DetailOverlay => "Detail",
            Self::SecondSmoothness => "SecondSmoothness",
            Self::Height => "Heightmap",
            Self::DecalOverlay => "Decal",
            Self::Subsurface => "SubSurface",
            Self::Custom => "Custom",
            Self::CustomSecondary => "[1] Custom",
            Self::Opacity => "Opacity",
            Self::Smoothness => "Smoothness",
            Self::Emittance => "Emittance",
            Self::Occlusion => "Occlusion",
            Self::Specular2 => "Specular2",
            Self::Unknown(value) => value,
        }
    }
}

/// A texture map plus its complete authored XML (`TexMod` children included).
#[derive(Debug, Clone, PartialEq)]
pub struct TextureRef {
    pub slot: MapSlot,
    /// Exact value of the `Map` attribute, including aliases/unknown slots.
    pub authored_slot: String,
    /// Source path as authored (often `.tif`; shipped products are commonly `.dds`).
    pub file: String,
    pub source: XmlElement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureSourceKind {
    Asset,
    RuntimeDefined,
}

impl TextureRef {
    /// Lumberyard MaterialBuilder treats `$` names and extensionless names
    /// (for example `nearest_cubemap`) as runtime-defined textures.
    #[must_use]
    pub fn source_kind(&self) -> TextureSourceKind {
        let filename = self.file.rsplit(['/', '\\']).next().unwrap_or(&self.file);
        if self.file.starts_with('$')
            || filename
                .rsplit_once('.')
                .is_none_or(|(_, extension)| extension.is_empty())
        {
            TextureSourceKind::RuntimeDefined
        } else {
            TextureSourceKind::Asset
        }
    }
}

/// One material or sub-material projected into engine-defined fields.
#[derive(Debug, Clone, PartialEq)]
pub struct SubMaterial {
    pub name: String,
    pub shader: String,
    pub surface_type: String,
    pub diffuse: Vec4,
    pub specular: Vec4,
    pub emittance: Vec4,
    pub opacity: f32,
    /// Cry smoothness value (`Shininess` in material XML).
    pub shininess: f32,
    pub alpha_test: f32,
    pub flags: i64,
    /// Kept as text because Cry supports both legacy and 64-bit shader masks.
    pub shader_generation_mask: Option<String>,
    pub textures: Vec<TextureRef>,
    /// Complete source element, including public parameters and unknown fields.
    pub source: XmlElement,
}

impl SubMaterial {
    /// Whether this material is explicitly non-visual. Shadow proxies are kept as
    /// model metadata elsewhere, but their proxy primitives are not rendered.
    #[must_use]
    pub fn is_skippable(&self) -> bool {
        let name = self.name.to_ascii_lowercase();
        self.shader.eq_ignore_ascii_case("nodraw")
            || self.flags & MTL_FLAG_NODRAW != 0
            || name.contains("shadowproxy")
            || name.contains("shadow_proxy")
            // A fully invisible shadow caster is a shadow-only proxy.
            || (self.opacity <= 0.0 && self.flags & MTL_FLAG_NOSHADOW == 0)
    }

    #[must_use]
    pub fn texture(&self, slot: MapSlot) -> Option<&TextureRef> {
        self.textures.iter().find(|texture| texture.slot == slot)
    }

    #[must_use]
    pub fn is_transparent(&self) -> bool {
        self.opacity < 1.0
            || self.flags & (MTL_FLAG_ADDITIVE | MTL_FLAG_REQUIRE_FORWARD_RENDERING) != 0
            || self.shader.eq_ignore_ascii_case("glass")
            || self.shader.to_ascii_lowercase().contains("transparent")
    }

    #[must_use]
    pub fn is_alpha_masked(&self) -> bool {
        self.alpha_test > 0.0
    }

    #[must_use]
    pub fn is_double_sided(&self) -> bool {
        self.flags & MTL_FLAG_2SIDED != 0
    }
}

/// Full material set. `source` retains the root multi-material XML.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MaterialSet {
    pub source: Option<XmlElement>,
    pub sub_materials: Vec<SubMaterial>,
}

impl MaterialSet {
    #[must_use]
    pub fn len(&self) -> usize {
        self.sub_materials.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sub_materials.is_empty()
    }

    /// Append another material table and return the index offset its mesh
    /// primitives must add to their material IDs.
    pub fn append(&mut self, mut other: Self) -> usize {
        let offset = self.sub_materials.len();
        self.sub_materials.append(&mut other.sub_materials);
        offset
    }
}

impl AssetDependencies for MaterialSet {
    fn asset_dependencies(&self) -> Vec<AssetDependency> {
        self.sub_materials
            .iter()
            .flat_map(|material| &material.textures)
            .filter(|texture| texture.source_kind() == TextureSourceKind::Asset)
            .map(|texture| AssetDependency::required_path("material.texture", &texture.file))
            .collect()
    }
}

impl FromStr for MaterialSet {
    type Err = Error;

    fn from_str(xml: &str) -> Result<Self, Self::Err> {
        let root = cry_xml::parse(xml)?;
        let elements = material_elements(&root);
        let sub_materials = elements.into_iter().map(project_material).collect();
        Ok(Self {
            source: Some(root),
            sub_materials,
        })
    }
}

/// Material parsing errors.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to parse .mtl XML: {0}")]
    Xml(#[from] cry_xml::XmlError),
}

fn material_elements(root: &XmlElement) -> Vec<&XmlElement> {
    let nested: Vec<_> = root
        .children_named("SubMaterials")
        .flat_map(|container| container.children_named("Material"))
        .collect();
    if !nested.is_empty() {
        return nested;
    }
    if root.name == "Material" {
        return vec![root];
    }
    root.children_named("Material").collect()
}

fn project_material(element: &XmlElement) -> SubMaterial {
    let attribute = |name: &str| element.attribute(name).unwrap_or_default();
    let textures = element
        .children_named("Textures")
        .flat_map(|container| container.children_named("Texture"))
        .filter_map(|texture| {
            let file = texture.attribute("File").unwrap_or_default().trim();
            if file.is_empty() {
                return None;
            }
            let authored_slot = texture.attribute("Map").unwrap_or_default().to_owned();
            Some(TextureRef {
                slot: MapSlot::from_authored_name(&authored_slot),
                authored_slot,
                file: file.to_owned(),
                source: texture.clone(),
            })
        })
        .collect();

    SubMaterial {
        name: attribute("Name").to_owned(),
        shader: attribute("Shader").to_owned(),
        surface_type: attribute("SurfaceType").to_owned(),
        diffuse: parse_color(attribute("Diffuse"), Vec4::ONE),
        specular: parse_color(attribute("Specular"), Vec4::ONE),
        emittance: parse_color(
            element
                .attribute("Emittance")
                .or_else(|| element.attribute("Emissive"))
                .unwrap_or_default(),
            Vec4::ZERO,
        ),
        opacity: parse_f32(attribute("Opacity"), 1.0).clamp(0.0, 1.0),
        shininess: parse_f32(attribute("Shininess"), 0.0).max(0.0),
        alpha_test: parse_f32(attribute("AlphaTest"), 0.0).clamp(0.0, 1.0),
        flags: attribute("MtlFlags").trim().parse().unwrap_or_default(),
        shader_generation_mask: element
            .attribute("GenMask64")
            .or_else(|| element.attribute("GenMask"))
            .map(str::to_owned),
        textures,
        source: element.clone(),
    }
}

fn parse_f32(value: &str, fallback: f32) -> f32 {
    value
        .trim()
        .parse::<f32>()
        .ok()
        .filter(|value| value.is_finite())
        .unwrap_or(fallback)
}

/// Parse a Cry `r,g,b,a` colour while sanitizing non-finite/negative legacy data.
fn parse_color(value: &str, fallback: Vec4) -> Vec4 {
    let mut out = fallback.to_array();
    for (slot, part) in out.iter_mut().zip(value.split(',')) {
        if let Ok(parsed) = part.trim().parse::<f32>() {
            *slot = if parsed.is_finite() && parsed >= 0.0 {
                parsed
            } else {
                0.0
            };
        }
    }
    Vec4::from_array(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<Material MtlFlags="557312" custom="retained">
        <SubMaterials>
            <Material Name="sarcophagus_mat" MtlFlags="524418" Shader="Illum" SurfaceType="mat_stone" Diffuse="0.5,0.5,0.5,1" Specular="0.2,0.3,0.4,1" Emittance="1,1,1,30" Opacity="1" Shininess="128" AlphaTest="0.4" GenMask64="123">
                <Textures>
                    <Texture Map="Diffuse" File="a/b/sarcophagus_diff.tif" custom="yes"><TexMod TileU="2"/></Texture>
                    <Texture Map="Bumpmap" File="a/b/sarcophagus_ddna.tif"/>
                    <Texture Map="FutureSlot" File="a/b/future.tif"/>
                </Textures>
                <PublicParams unknown="retained"/>
            </Material>
            <Material Name="coll01" Shader="Nodraw" MtlFlags="558208" Opacity="1"><Textures/></Material>
        </SubMaterials>
    </Material>"#;

    #[test]
    fn projects_all_engine_slots_and_retains_source_xml() {
        let set = MaterialSet::from_str(SAMPLE).unwrap();
        assert_eq!(set.sub_materials.len(), 2);
        assert_eq!(
            set.source.as_ref().unwrap().attributes["custom"],
            "retained"
        );
        let base = &set.sub_materials[0];
        assert_eq!(base.surface_type, "mat_stone");
        assert_eq!(base.specular, Vec4::new(0.2, 0.3, 0.4, 1.0));
        assert_eq!(base.shader_generation_mask.as_deref(), Some("123"));
        assert!(base.is_double_sided());
        assert!(base.is_alpha_masked());
        assert_eq!(
            base.texture(MapSlot::Diffuse).unwrap().file,
            "a/b/sarcophagus_diff.tif"
        );
        assert!(base.texture(MapSlot::Normals).is_some());
        let unknown = &base.textures[2];
        assert_eq!(unknown.slot, MapSlot::Unknown("FutureSlot".to_owned()));
        assert_eq!(base.textures[0].source.children[0].name, "TexMod");
        assert_eq!(base.source.children_named("PublicParams").count(), 1);
        assert!(set.sub_materials[1].is_skippable());
    }

    #[test]
    fn lighting_flag_is_not_mistaken_for_nodraw() {
        let set = MaterialSet::from_str(
            r#"<Material Name="lit" Shader="Illum" MtlFlags="16" Diffuse="1,0,0,1"/>"#,
        )
        .unwrap();
        assert!(!set.sub_materials[0].is_skippable());
    }

    #[test]
    fn classifies_lumberyard_runtime_textures() {
        let set = MaterialSet::from_str(
            r#"<Material><Textures>
                <Texture Map="Environment" File="nearest_cubemap"/>
                <Texture Map="Custom" File="$SceneTarget"/>
                <Texture Map="Diffuse" File="textures/body_diff.tif"/>
            </Textures></Material>"#,
        )
        .unwrap();
        let textures = &set.sub_materials[0].textures;
        assert_eq!(textures[0].source_kind(), TextureSourceKind::RuntimeDefined);
        assert_eq!(textures[1].source_kind(), TextureSourceKind::RuntimeDefined);
        assert_eq!(textures[2].source_kind(), TextureSourceKind::Asset);
    }
}
