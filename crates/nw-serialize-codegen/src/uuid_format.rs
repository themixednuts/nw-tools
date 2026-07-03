use uuid::Uuid;

pub(crate) fn uuid_u128_literal_text(type_id: Uuid) -> String {
    let hex = type_id.simple().to_string().to_ascii_uppercase();
    debug_assert_eq!(hex.len(), 32);
    format!(
        "0x{}_{}_{}_{}_{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

#[cfg(test)]
mod tests {
    use uuid::uuid;

    use super::*;

    #[test]
    fn uuid_u128_literals_follow_uuid_group_boundaries() {
        assert_eq!(
            uuid_u128_literal_text(uuid!("A85DF621-DCE0-409F-8D39-A447EA0807FF")),
            "0xA85DF621_DCE0_409F_8D39_A447EA0807FF"
        );
    }
}
