//! The `AZ::Uuid` primitive: `CreateName` / `CreateData` / `operator+`.
//!
//! `nw-objectstream` already depends on `nw-asset`, so this crate is the
//! foundational owner of the primitive: SHA-1 hash the input bytes directly
//! (no RFC namespace), take the first 16 digest bytes, then stamp the
//! version/variant masks. The same primitive over `lhs || rhs` folds
//! template type IDs (`operator+`).
//!
//! The public surface is [`AzUuidExt`], which mirrors `AZ::Uuid`'s native
//! API: call sites read `Uuid::create_name(...)`, matching the engine's own
//! `AZ::Uuid::CreateName` spelling.
//!
//! [`const_impl`] is `#[doc(hidden)]` and exists only so other crates (namely
//! `nw-objectstream`'s const type-id folding helpers) can evaluate the same
//! primitive in `const fn` context, which trait methods cannot be called
//! from. Prefer [`AzUuidExt`] everywhere else.

use uuid::Uuid;

/// Extension trait mirroring `AZ::Uuid`'s native name/data/combine API.
pub trait AzUuidExt: Sized {
    /// `AZ::Uuid::CreateName(name)`.
    #[must_use]
    fn create_name(name: &[u8]) -> Self;

    /// `AZ::Uuid::CreateData(data, size)`.
    #[must_use]
    fn create_data(data: &[u8]) -> Self;

    /// `AZ::Uuid::operator+`.
    #[must_use]
    fn combine(self, rhs: Self) -> Self;
}

impl AzUuidExt for Uuid {
    #[inline]
    fn create_name(name: &[u8]) -> Self {
        const_impl::create_name(name)
    }

    #[inline]
    fn create_data(data: &[u8]) -> Self {
        const_impl::create_data(data)
    }

    #[inline]
    fn combine(self, rhs: Self) -> Self {
        const_impl::combine(self, rhs)
    }
}

/// `const fn` implementation of the `AZ::Uuid` primitive.
///
/// Hidden from docs: [`AzUuidExt`] is the public API. This module exists so
/// `nw-objectstream`'s const type-id folding helpers (which must themselves
/// be `const fn`) can evaluate the primitive without going through a trait
/// method call.
#[doc(hidden)]
pub mod const_impl {
    use uuid::Uuid;

    /// `AZ::Uuid::CreateData(data, size)`.
    ///
    /// This hashes `bytes` directly, without an RFC namespace. Empty input
    /// returns the nil UUID, matching `AZ::Uuid::CreateNull`.
    #[inline]
    #[must_use]
    pub const fn create_data(bytes: &[u8]) -> Uuid {
        if bytes.is_empty() {
            return Uuid::from_u128(0);
        }

        let digest = sha1(bytes);
        let mut data = [0u8; 16];
        let mut index = 0;
        while index < data.len() {
            data[index] = digest[index];
            index += 1;
        }

        // Native masks: VAR_RFC_4122 and VER_NAME_SHA1.
        // These are equivalent to the usual RFC 4122 v5 bit stamping after
        // the digest is truncated.
        data[8] &= 0xbf;
        data[8] |= 0x80;
        data[6] &= 0x5f;
        data[6] |= 0x50;

        uuid_from_bytes(data)
    }

    /// `AZ::Uuid::CreateName(name)`.
    #[inline]
    #[must_use]
    pub const fn create_name(name: &[u8]) -> Uuid {
        create_data(name)
    }

    /// `AZ::Uuid::operator+`.
    #[inline]
    #[must_use]
    pub const fn combine(lhs: Uuid, rhs: Uuid) -> Uuid {
        let mut bytes = [0u8; 32];
        let lhs = uuid_bytes(lhs);
        let rhs = uuid_bytes(rhs);
        let mut index = 0;
        while index < 16 {
            bytes[index] = lhs[index];
            bytes[16 + index] = rhs[index];
            index += 1;
        }
        create_data(&bytes)
    }

    pub const fn sha1(bytes: &[u8]) -> [u8; 20] {
        let mut h0 = 0x6745_2301u32;
        let mut h1 = 0xefcd_ab89u32;
        let mut h2 = 0x98ba_dcfeu32;
        let mut h3 = 0x1032_5476u32;
        let mut h4 = 0xc3d2_e1f0u32;

        let total_len = sha1_padded_len(bytes.len());
        let mut chunk_start = 0;
        while chunk_start < total_len {
            let mut w = [0u32; 80];
            let mut word = 0;
            while word < 16 {
                let byte = chunk_start + word * 4;
                w[word] = ((sha1_padded_byte(bytes, byte, total_len) as u32) << 24)
                    | ((sha1_padded_byte(bytes, byte + 1, total_len) as u32) << 16)
                    | ((sha1_padded_byte(bytes, byte + 2, total_len) as u32) << 8)
                    | sha1_padded_byte(bytes, byte + 3, total_len) as u32;
                word += 1;
            }

            word = 16;
            while word < 80 {
                w[word] = (w[word - 3] ^ w[word - 8] ^ w[word - 14] ^ w[word - 16]).rotate_left(1);
                word += 1;
            }

            let mut a = h0;
            let mut b = h1;
            let mut c = h2;
            let mut d = h3;
            let mut e = h4;

            word = 0;
            while word < 80 {
                let (f, k) = if word < 20 {
                    ((b & c) | ((!b) & d), 0x5a82_7999)
                } else if word < 40 {
                    (b ^ c ^ d, 0x6ed9_eba1)
                } else if word < 60 {
                    ((b & c) | (b & d) | (c & d), 0x8f1b_bcdc)
                } else {
                    (b ^ c ^ d, 0xca62_c1d6)
                };

                let temp = a
                    .rotate_left(5)
                    .wrapping_add(f)
                    .wrapping_add(e)
                    .wrapping_add(k)
                    .wrapping_add(w[word]);
                e = d;
                d = c;
                c = b.rotate_left(30);
                b = a;
                a = temp;
                word += 1;
            }

            h0 = h0.wrapping_add(a);
            h1 = h1.wrapping_add(b);
            h2 = h2.wrapping_add(c);
            h3 = h3.wrapping_add(d);
            h4 = h4.wrapping_add(e);

            chunk_start += 64;
        }

        let mut digest = [0u8; 20];
        write_be_u32(&mut digest, 0, h0);
        write_be_u32(&mut digest, 4, h1);
        write_be_u32(&mut digest, 8, h2);
        write_be_u32(&mut digest, 12, h3);
        write_be_u32(&mut digest, 16, h4);
        digest
    }

    pub const fn sha1_padded_len(len: usize) -> usize {
        let rem = len % 64;
        if rem < 56 {
            len + (56 - rem) + 8
        } else {
            len + (64 - rem) + 56 + 8
        }
    }

    pub const fn sha1_padded_byte(bytes: &[u8], index: usize, total_len: usize) -> u8 {
        let len = bytes.len();
        if index < len {
            bytes[index]
        } else if index == len {
            0x80
        } else if index >= total_len - 8 {
            let bit_len = (len as u64).wrapping_mul(8);
            let shift = (total_len - 1 - index) * 8;
            ((bit_len >> shift) & 0xff).to_le_bytes()[0]
        } else {
            0
        }
    }

    pub const fn write_be_u32(bytes: &mut [u8; 20], offset: usize, value: u32) {
        let raw = value.to_be_bytes();
        bytes[offset] = raw[0];
        bytes[offset + 1] = raw[1];
        bytes[offset + 2] = raw[2];
        bytes[offset + 3] = raw[3];
    }

    pub const fn uuid_bytes(uuid: Uuid) -> [u8; 16] {
        uuid.as_u128().to_be_bytes()
    }

    pub const fn uuid_from_bytes(bytes: [u8; 16]) -> Uuid {
        Uuid::from_u128(u128::from_be_bytes(bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::uuid;

    #[test]
    fn create_name_matches_azcore_masking() {
        assert_eq!(
            Uuid::create_name(b"hello"),
            uuid!("aaf4c61d-dcc5-58a2-9abe-de0f3b482cd9")
        );
    }

    #[test]
    fn create_name_of_empty_input_is_nil() {
        assert_eq!(Uuid::create_name(b""), Uuid::nil());
    }

    #[test]
    fn create_data_of_empty_input_is_nil() {
        assert_eq!(Uuid::create_data(b""), Uuid::nil());
    }

    #[test]
    fn combine_matches_uuid_operator_plus() {
        // AZ::TypeId<s32> + AZ::TypeId<u8> (folded template-argument combine).
        let s32 = uuid!("72039442-EB38-4D42-A1AD-CB68F7E0EEF6");
        let u8 = uuid!("72B9409A-7D1A-4831-9CFE-FCB3FADD3426");
        assert_eq!(
            s32.combine(u8),
            uuid!("2554130f-2bb8-5a25-8cc4-319329151f28")
        );
    }
}
