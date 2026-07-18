pub fn is_hex_color_text(value: &str) -> bool {
    let value = value.trim();
    let Some(hex) = value.strip_prefix('#') else {
        return false;
    };
    matches!(hex.len(), 6 | 8) && hex.as_bytes().iter().all(u8::is_ascii_hexdigit)
}
