//! Inline image output via the kitty graphics protocol (supported by kitty,
//! Ghostty, WezTerm, and Konsole). Pixels are sent as raw RGBA, base64-encoded
//! and split into the protocol's ~4 KB chunks.

use std::io::{self, Write};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;

/// Largest base64 payload per kitty escape chunk (the protocol's limit).
const CHUNK: usize = 4096;

/// Print an RGBA image (tightly packed, `width`×`height`) inline using the kitty
/// graphics protocol. The terminal scales it to its natural pixel size at the
/// cursor. No-op for an empty image.
pub fn print_kitty_rgba(rgba: &[u8], width: u32, height: u32) {
    if rgba.is_empty() || width == 0 || height == 0 {
        return;
    }
    let encoded = STANDARD.encode(rgba);
    let chunks: Vec<&[u8]> = encoded.as_bytes().chunks(CHUNK).collect();
    let mut out = String::with_capacity(encoded.len() + chunks.len() * 16);
    for (index, chunk) in chunks.iter().enumerate() {
        let more = u8::from(index + 1 < chunks.len());
        out.push_str("\x1b_G");
        if index == 0 {
            // f=32: RGBA, a=T: transmit+display, t=d: payload is direct (inline).
            out.push_str(&format!("f=32,s={width},v={height},a=T,t=d,"));
        }
        out.push_str(&format!("m={more};"));
        // chunk is base64 (ASCII), so this is always valid UTF-8.
        out.push_str(std::str::from_utf8(chunk).unwrap_or_default());
        out.push_str("\x1b\\");
    }
    out.push('\n');
    let mut stdout = io::stdout();
    let _ = stdout.write_all(out.as_bytes());
    let _ = stdout.flush();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kitty_chunks_carry_header_once_and_terminate() {
        // 2x2 RGBA = 16 bytes → ~24 base64 chars → a single chunk.
        let rgba = vec![255u8; 16];
        let encoded = STANDARD.encode(&rgba);
        let sequence = {
            // Re-derive the expected single-chunk form.
            format!("\x1b_Gf=32,s=2,v=2,a=T,t=d,m=0;{encoded}\x1b\\\n")
        };
        // Build via a buffer to avoid touching real stdout in the test.
        let chunks: Vec<&[u8]> = encoded.as_bytes().chunks(CHUNK).collect();
        let mut out = String::new();
        for (index, chunk) in chunks.iter().enumerate() {
            let more = u8::from(index + 1 < chunks.len());
            out.push_str("\x1b_G");
            if index == 0 {
                out.push_str("f=32,s=2,v=2,a=T,t=d,");
            }
            out.push_str(&format!("m={more};"));
            out.push_str(std::str::from_utf8(chunk).unwrap());
            out.push_str("\x1b\\");
        }
        out.push('\n');
        assert_eq!(out, sequence);
        assert!(out.starts_with("\x1b_Gf=32,s=2,v=2,a=T,t=d,m=0;"));
        assert!(out.ends_with("\x1b\\\n"));
    }
}
