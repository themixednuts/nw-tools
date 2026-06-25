//! Cry → glTF numeric helpers built on `glam` (the math types `bevy_math`
//! re-exports): half-float decode, the coordinate-system change, and node
//! transforms.

use glam::{Mat4, Vec3, Vec4};

/// The Cry → glTF basis change as a matrix: maps `(x, y, z) → (-x, z, y)`. It is
/// its own inverse, so conjugating a transform is `SWAP * m * SWAP`.
const SWAP: Mat4 = Mat4::from_cols_array(&[
    -1.0, 0.0, 0.0, 0.0, // col 0
    0.0, 0.0, 1.0, 0.0, // col 1
    0.0, 1.0, 0.0, 0.0, // col 2
    0.0, 0.0, 0.0, 1.0, // col 3
]);

/// Decode an IEEE 754 half-precision (binary16) value to `f32`.
#[must_use]
pub fn half_to_f32(bits: u16) -> f32 {
    let sign = u32::from(bits & 0x8000) << 16;
    let exponent = (bits >> 10) & 0x1f;
    let mantissa = u32::from(bits & 0x03ff);
    let value = match exponent {
        0 => {
            if mantissa == 0 {
                0.0
            } else {
                (mantissa as f32) * 2.0f32.powi(-24)
            }
        }
        0x1f => {
            if mantissa == 0 {
                f32::INFINITY
            } else {
                f32::NAN
            }
        }
        _ => (1.0 + (mantissa as f32) / 1024.0) * 2.0f32.powi(i32::from(exponent) - 15),
    };
    // `value` is the positive magnitude; OR the sign bit back in.
    f32::from_bits(sign | value.to_bits())
}

#[cfg(test)]
mod tests {
    use super::half_to_f32;

    #[test]
    fn half_decode_applies_sign_and_magnitude() {
        assert_eq!(half_to_f32(0x0000), 0.0);
        assert_eq!(half_to_f32(0x3c00), 1.0); // 1.0
        assert_eq!(half_to_f32(0xc000), -2.0); // -2.0
        // 0xbfbf ≈ -1.9365 (the value that was collapsing to -0.0).
        let v = half_to_f32(0xbfbf);
        assert!((v - -1.9365).abs() < 1e-3, "got {v}");
    }
}

/// Cry is right-handed Z-up; glTF is right-handed Y-up. Map `(x, y, z)` →
/// `(-x, z, y)` (matches nw-buddy's `CryToGltfVec3`).
#[must_use]
pub fn cry_to_gltf(v: Vec3) -> Vec3 {
    Vec3::new(-v.x, v.z, v.y)
}

/// Build a transform from a raw Cry node matrix: scale the translation row from
/// centimetres to metres, force `w = 1`, and interpret the row-major storage as a
/// column-major `Mat4` (hence the transpose).
#[must_use]
pub fn node_matrix(mut raw: [f32; 16]) -> Mat4 {
    raw[12] *= 0.01;
    raw[13] *= 0.01;
    raw[14] *= 0.01;
    raw[15] = 1.0;
    Mat4::from_cols_array(&raw).transpose()
}

/// Convert a Cry `Matrix34` (row-major 3×4, translation in column 3) to a `Mat4`.
#[must_use]
pub fn matrix34(rows: &[[f32; 4]; 3]) -> Mat4 {
    Mat4::from_cols(
        Vec4::new(rows[0][0], rows[1][0], rows[2][0], 0.0),
        Vec4::new(rows[0][1], rows[1][1], rows[2][1], 0.0),
        Vec4::new(rows[0][2], rows[1][2], rows[2][2], 0.0),
        Vec4::new(rows[0][3], rows[1][3], rows[2][3], 1.0),
    )
}

/// Express a Cry-space transform in glTF space (basis conjugation `SWAP·m·SWAP`).
#[must_use]
pub fn cry_to_gltf_mat(m: Mat4) -> Mat4 {
    SWAP * m * SWAP
}
