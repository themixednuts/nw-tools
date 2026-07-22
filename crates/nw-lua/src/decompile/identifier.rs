//! Lua identifier validation shared by naming and emission.

use bstr::BString;

/// Return whether raw bytes can be emitted as a simple Lua identifier.
#[must_use]
pub fn is_valid_identifier(bytes: &BString) -> bool {
    let bytes = bytes.as_slice();
    let Some((&first, rest)) = bytes.split_first() else {
        return false;
    };
    if !is_ident_start(first) || !rest.iter().copied().all(is_ident_continue) {
        return false;
    }
    !is_keyword(bytes)
}

fn is_ident_start(byte: u8) -> bool {
    byte == b'_' || byte.is_ascii_alphabetic()
}

fn is_ident_continue(byte: u8) -> bool {
    is_ident_start(byte) || byte.is_ascii_digit()
}

fn is_keyword(bytes: &[u8]) -> bool {
    matches!(
        bytes,
        b"and"
            | b"break"
            | b"do"
            | b"else"
            | b"elseif"
            | b"end"
            | b"false"
            | b"for"
            | b"function"
            | b"if"
            | b"in"
            | b"local"
            | b"nil"
            | b"not"
            | b"or"
            | b"repeat"
            | b"return"
            | b"then"
            | b"true"
            | b"until"
            | b"while"
    )
}
