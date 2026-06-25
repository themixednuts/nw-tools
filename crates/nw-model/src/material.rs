//! Parse Cry/Lumberyard `.mtl` material XML into a flat material set.
//!
//! A `.mtl` is either a single `<Material>` or a `<Material>` wrapping
//! `<SubMaterials>`; a mesh subset's `material_id` indexes the sub-material list.
//! Each sub-material carries shader/colour params and texture-map references.
//!
//! Parsing uses quick-xml's streaming event reader (no serde, no intermediate
//! allocations beyond the kept fields) for the fastest path over many `.mtl`s.

use std::str::FromStr;

use glam::Vec4;
use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

/// `MTL_FLAG_NODRAW` — the sub-material draws nothing (collision/proxy).
const MTL_FLAG_NODRAW: i64 = 0x10;
/// `MTL_FLAG_NOSHADOW`.
const MTL_FLAG_NOSHADOW: i64 = 0x20;

/// A texture map slot we map into glTF.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapSlot {
    Diffuse,
    Bumpmap,
    Specular,
    Emittance,
    Other,
}

impl MapSlot {
    fn from_bytes(value: &[u8]) -> Self {
        match value {
            b"Diffuse" => Self::Diffuse,
            b"Bumpmap" => Self::Bumpmap,
            b"Specular" => Self::Specular,
            b"Emittance" => Self::Emittance,
            _ => Self::Other,
        }
    }
}

/// A texture reference from a sub-material.
#[derive(Debug, Clone)]
pub struct TextureRef {
    pub slot: MapSlot,
    /// Source path as authored (usually `.tif`; the shipped asset is `.dds`).
    pub file: String,
}

/// One sub-material resolved to render-relevant fields.
#[derive(Debug, Clone)]
pub struct SubMaterial {
    pub name: String,
    pub shader: String,
    pub diffuse: Vec4,
    pub emittance: Vec4,
    pub opacity: f32,
    pub flags: i64,
    pub textures: Vec<TextureRef>,
}

impl SubMaterial {
    /// Whether this sub-material should be dropped from a visual export: Nodraw
    /// shaders, the `nodraw` flag, and shadow-proxy conventions.
    #[must_use]
    pub fn is_skippable(&self) -> bool {
        let name = self.name.to_ascii_lowercase();
        self.shader.eq_ignore_ascii_case("nodraw")
            || self.flags & MTL_FLAG_NODRAW != 0
            || name.contains("shadowproxy")
            || name.contains("shadow_proxy")
            // Material shadow-only proxy: invisible but still casts (No Shadow unset).
            || (self.opacity <= 0.0 && self.flags & MTL_FLAG_NOSHADOW == 0)
    }

    #[must_use]
    pub fn texture(&self, slot: MapSlot) -> Option<&TextureRef> {
        self.textures.iter().find(|texture| texture.slot == slot)
    }

    /// Treat the material as transparent (glTF `BLEND`) when not fully opaque.
    #[must_use]
    pub fn is_transparent(&self) -> bool {
        self.opacity < 1.0
            || self.shader.eq_ignore_ascii_case("glass")
            || self.shader.to_ascii_lowercase().contains("transparent")
    }
}

/// The full material set parsed from a `.mtl`.
#[derive(Debug, Clone, Default)]
pub struct MaterialSet {
    pub sub_materials: Vec<SubMaterial>,
}

impl FromStr for MaterialSet {
    type Err = Error;

    /// Parse a `.mtl` document.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] if the XML cannot be parsed.
    fn from_str(xml: &str) -> Result<Self, Self::Err> {
        let mut reader = Reader::from_str(xml);
        reader.config_mut().trim_text(true);

        // A stack of open `<Material>` builders; `Texture` children attach to the
        // innermost open one. Each builder records its nesting depth so we can tell
        // the outer container from its sub-materials.
        let mut stack: Vec<Builder> = Vec::new();
        let mut finished: Vec<Builder> = Vec::new();

        loop {
            match reader.read_event()? {
                Event::Start(e) if is(&e, b"Material") => {
                    let depth = stack.len();
                    stack.push(Builder::from_element(&e, depth));
                }
                Event::Empty(e) if is(&e, b"Material") => {
                    let depth = stack.len();
                    finished.push(Builder::from_element(&e, depth));
                }
                Event::Start(e) | Event::Empty(e) if is(&e, b"Texture") => {
                    if let Some(top) = stack.last_mut() {
                        top.push_texture(&e);
                    }
                }
                Event::End(e) if e.name().as_ref() == b"Material" => {
                    if let Some(builder) = stack.pop() {
                        finished.push(builder);
                    }
                }
                Event::Eof => break,
                _ => {}
            }
        }

        // Sub-materials are the nested ones (depth > 0), in document order (they
        // close before the outer container). A file with no `<SubMaterials>` has a
        // single depth-0 material which is itself the sole sub-material.
        let mut sub_materials: Vec<SubMaterial> = finished
            .iter()
            .filter(|b| b.depth > 0)
            .map(Builder::build)
            .collect();
        if sub_materials.is_empty() {
            sub_materials = finished.into_iter().map(|b| b.build()).collect();
        }
        Ok(Self { sub_materials })
    }
}

/// Material-parsing errors.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to parse .mtl XML: {0}")]
    Xml(#[from] quick_xml::Error),
}

/// Whether an element's local name matches `name`.
fn is(element: &BytesStart<'_>, name: &[u8]) -> bool {
    element.name().as_ref() == name
}

/// Accumulates one `<Material>` element's fields as events stream in.
struct Builder {
    depth: usize,
    name: String,
    shader: String,
    diffuse: Vec4,
    emittance: Vec4,
    opacity: f32,
    flags: i64,
    textures: Vec<TextureRef>,
}

impl Builder {
    fn from_element(element: &BytesStart<'_>, depth: usize) -> Self {
        let mut builder = Self {
            depth,
            name: String::new(),
            shader: String::new(),
            diffuse: Vec4::ONE,
            emittance: Vec4::ZERO,
            opacity: 1.0,
            flags: 0,
            textures: Vec::new(),
        };
        // Single pass over attributes.
        for attribute in element.attributes().flatten() {
            let value = attribute.value.as_ref();
            match attribute.key.as_ref() {
                b"Name" => builder.name = decode(value),
                b"Shader" => builder.shader = decode(value),
                b"MtlFlags" => builder.flags = decode(value).trim().parse().unwrap_or(0),
                b"Diffuse" => builder.diffuse = parse_color(value, builder.diffuse),
                b"Emittance" => builder.emittance = parse_color(value, builder.emittance),
                b"Opacity" => builder.opacity = decode(value).trim().parse().unwrap_or(1.0),
                _ => {}
            }
        }
        builder
    }

    fn push_texture(&mut self, element: &BytesStart<'_>) {
        let mut slot = MapSlot::Other;
        let mut file = String::new();
        for attribute in element.attributes().flatten() {
            match attribute.key.as_ref() {
                b"Map" => slot = MapSlot::from_bytes(attribute.value.as_ref()),
                b"File" => file = decode(attribute.value.as_ref()),
                _ => {}
            }
        }
        if !file.trim().is_empty() {
            self.textures.push(TextureRef { slot, file });
        }
    }

    fn build(&self) -> SubMaterial {
        SubMaterial {
            name: self.name.clone(),
            shader: self.shader.clone(),
            diffuse: self.diffuse,
            emittance: self.emittance,
            opacity: self.opacity,
            flags: self.flags,
            textures: self.textures.clone(),
        }
    }
}

/// Decode an attribute value (paths/numbers are ASCII; entities are vanishingly
/// rare in `.mtl`, so a lossy UTF-8 view is both correct here and allocation-light).
fn decode(value: &[u8]) -> String {
    String::from_utf8_lossy(value).into_owned()
}

/// Parse a Cry `"r,g,b,a"` colour, clamping negatives/non-finite (some exports
/// emit garbage like `Emittance="-3.4e26,0,0,0"`).
fn parse_color(value: &[u8], fallback: Vec4) -> Vec4 {
    let text = String::from_utf8_lossy(value);
    let mut out = fallback.to_array();
    for (slot, part) in out.iter_mut().zip(text.split(',')) {
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

    const SAMPLE: &str = r#"<Material MtlFlags="557312">
        <SubMaterials>
            <Material Name="sarcophagus_mat" MtlFlags="524416" Shader="Illum" Diffuse="0.5,0.5,0.5,1" Emittance="1,1,1,30" Opacity="1">
                <Textures>
                    <Texture Map="Diffuse" File="a/b/sarcophagus_diff.tif"/>
                    <Texture Map="Bumpmap" File="a/b/sarcophagus_ddna.tif"/>
                </Textures>
            </Material>
            <Material Name="coll01" Shader="Nodraw" MtlFlags="558208" Opacity="1">
                <Textures/>
            </Material>
        </SubMaterials>
    </Material>"#;

    const SINGLE: &str = r#"<Material Name="solo" Shader="Illum" Diffuse="1,0,0,1" Opacity="1">
        <Textures><Texture Map="Diffuse" File="x/y.tif"/></Textures>
    </Material>"#;

    #[test]
    fn parses_submaterials_and_textures() {
        let set = MaterialSet::from_str(SAMPLE).unwrap();
        assert_eq!(set.sub_materials.len(), 2);
        let base = &set.sub_materials[0];
        assert_eq!(base.name, "sarcophagus_mat");
        assert_eq!(base.diffuse, glam::Vec4::new(0.5, 0.5, 0.5, 1.0));
        assert!(!base.is_skippable());
        assert_eq!(
            base.texture(MapSlot::Diffuse).unwrap().file,
            "a/b/sarcophagus_diff.tif"
        );
        assert!(base.texture(MapSlot::Bumpmap).is_some());
        assert!(set.sub_materials[1].is_skippable());
    }

    #[test]
    fn parses_single_material_file() {
        let set = MaterialSet::from_str(SINGLE).unwrap();
        assert_eq!(set.sub_materials.len(), 1);
        assert_eq!(set.sub_materials[0].name, "solo");
        assert_eq!(set.sub_materials[0].diffuse, glam::Vec4::new(1.0, 0.0, 0.0, 1.0));
    }
}
