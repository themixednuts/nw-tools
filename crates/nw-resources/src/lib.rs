pub const SERIALIZE_JSON: &[u8] = include_bytes!("../../../resources/serialize.json");
pub const STEAM_API64_DLL: &[u8] = include_bytes!("../../../resources/steam_api64.dll");

#[derive(Debug, Clone, Copy)]
pub struct EmbeddedResource {
    pub path: &'static str,
    pub bytes: &'static [u8],
}

pub fn all() -> impl Iterator<Item = EmbeddedResource> {
    [
        EmbeddedResource {
            path: "serialize.json",
            bytes: SERIALIZE_JSON,
        },
        EmbeddedResource {
            path: "steam_api64.dll",
            bytes: STEAM_API64_DLL,
        },
    ]
    .into_iter()
}
