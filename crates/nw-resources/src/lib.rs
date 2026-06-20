pub const SERIALIZE_JSON: &[u8] = include_bytes!("../../../resources/serialize.json");

#[derive(Debug, Clone, Copy)]
pub struct EmbeddedResource {
    pub path: &'static str,
    pub bytes: &'static [u8],
}

pub fn all() -> impl Iterator<Item = EmbeddedResource> {
    [EmbeddedResource {
        path: "serialize.json",
        bytes: SERIALIZE_JSON,
    }]
    .into_iter()
}
