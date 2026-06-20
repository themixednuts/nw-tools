//! ObjectStream UUID derivation and type-id folding.
//!
//! ObjectStream type IDs use the native `AZ::Uuid::CreateName` /
//! `CreateData` behavior. It SHA-1 hashes the input bytes directly,
//! takes the first 16 digest bytes, then applies the version and variant
//! masks. The same primitive over `lhs || rhs` is used to fold template
//! type IDs.

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

/// Fold a non-empty sequence of `TypeIds` the way `AggregateTypes<T...>`
/// folds template arguments.
#[must_use]
pub fn aggregate_type_ids(type_ids: impl IntoIterator<Item = Uuid>) -> Option<Uuid> {
    let mut iter = type_ids.into_iter();
    let mut acc = iter.next()?;
    for type_id in iter {
        acc = combine(acc, type_id);
    }
    Some(acc)
}

/// Fold a non-empty sequence of `TypeIds` the way `AggregateTypes<T...>`
/// folds variadic template arguments.
#[must_use]
pub const fn aggregate_type_ids_right(type_ids: &[Uuid]) -> Option<Uuid> {
    if type_ids.is_empty() {
        return None;
    }

    let mut index = type_ids.len() - 1;
    let mut acc = type_ids[index];
    while index > 0 {
        index -= 1;
        acc = combine(type_ids[index], acc);
    }

    Some(acc)
}

/// Fold a template specialization that uses `AzCore`'s prefix macro:
/// `template_base + aggregate(args)`.
#[must_use]
pub const fn specialized_template_prefix(template_base: Uuid, args: &[Uuid]) -> Option<Uuid> {
    match aggregate_type_ids_const(args) {
        Some(args) => Some(combine(template_base, args)),
        None => None,
    }
}

/// Fold a template specialization that uses `AzCore`'s postfix macro:
/// `aggregate(args) + template_base`.
#[must_use]
pub const fn specialized_template_postfix(template_base: Uuid, args: &[Uuid]) -> Option<Uuid> {
    match aggregate_type_ids_const(args) {
        Some(args) => Some(combine(args, template_base)),
        None => None,
    }
}

/// Like [`specialized_template_postfix`], but requires at least one argument.
///
/// # Panics
///
/// Panics if `args` is empty.
const fn specialized_template_postfix_non_empty(template_base: Uuid, args: &[Uuid]) -> Uuid {
    match specialized_template_postfix(template_base, args) {
        Some(type_id) => type_id,
        None => Uuid::from_u128(0),
    }
}

const fn aggregate_type_ids_const(type_ids: &[Uuid]) -> Option<Uuid> {
    if type_ids.is_empty() {
        return None;
    }

    let mut index = 1;
    let mut acc = type_ids[0];
    while index < type_ids.len() {
        acc = combine(acc, type_ids[index]);
        index += 1;
    }

    Some(acc)
}

const fn sha1(bytes: &[u8]) -> [u8; 20] {
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

const fn sha1_padded_len(len: usize) -> usize {
    let rem = len % 64;
    if rem < 56 {
        len + (56 - rem) + 8
    } else {
        len + (64 - rem) + 56 + 8
    }
}

const fn sha1_padded_byte(bytes: &[u8], index: usize, total_len: usize) -> u8 {
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

const fn write_be_u32(bytes: &mut [u8; 20], offset: usize, value: u32) {
    let raw = value.to_be_bytes();
    bytes[offset] = raw[0];
    bytes[offset + 1] = raw[1];
    bytes[offset + 2] = raw[2];
    bytes[offset + 3] = raw[3];
}

const fn uuid_bytes(uuid: Uuid) -> [u8; 16] {
    uuid.as_u128().to_be_bytes()
}

const fn uuid_from_bytes(bytes: [u8; 16]) -> Uuid {
    Uuid::from_u128(u128::from_be_bytes(bytes))
}

const fn create_data_prefix<const N: usize>(bytes: &[u8; N], len: usize) -> Uuid {
    if len == 0 {
        return Uuid::from_u128(0);
    }

    let digest = sha1_prefix(bytes, len);
    let mut data = [0u8; 16];
    let mut index = 0;
    while index < data.len() {
        data[index] = digest[index];
        index += 1;
    }

    data[8] &= 0xbf;
    data[8] |= 0x80;
    data[6] &= 0x5f;
    data[6] |= 0x50;

    uuid_from_bytes(data)
}

const fn sha1_prefix<const N: usize>(bytes: &[u8; N], len: usize) -> [u8; 20] {
    let mut h0 = 0x6745_2301u32;
    let mut h1 = 0xefcd_ab89u32;
    let mut h2 = 0x98ba_dcfeu32;
    let mut h3 = 0x1032_5476u32;
    let mut h4 = 0xc3d2_e1f0u32;

    let total_len = sha1_padded_len(len);
    let mut chunk_start = 0;
    while chunk_start < total_len {
        let mut w = [0u32; 80];
        let mut word = 0;
        while word < 16 {
            let byte = chunk_start + word * 4;
            w[word] = ((sha1_prefix_padded_byte(bytes, len, byte, total_len) as u32) << 24)
                | ((sha1_prefix_padded_byte(bytes, len, byte + 1, total_len) as u32) << 16)
                | ((sha1_prefix_padded_byte(bytes, len, byte + 2, total_len) as u32) << 8)
                | sha1_prefix_padded_byte(bytes, len, byte + 3, total_len) as u32;
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

const fn sha1_prefix_padded_byte<const N: usize>(
    bytes: &[u8; N],
    len: usize,
    index: usize,
    total_len: usize,
) -> u8 {
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

const fn write_decimal<const N: usize>(
    buf: &mut [u8; N],
    mut offset: usize,
    value: usize,
) -> usize {
    let mut rev = [0u8; 20];
    let mut len = 0;
    let mut current = value;

    if current == 0 {
        buf[offset] = b'0';
        return offset + 1;
    }

    while current > 0 {
        rev[len] = b'0' + (current % 10).to_le_bytes()[0];
        current /= 10;
        len += 1;
    }

    while len > 0 {
        len -= 1;
        buf[offset] = rev[len];
        offset += 1;
    }

    offset
}

pub mod type_ids {
    //! Source-defined `AzCore` `TypeIds` used by generic folding helpers.

    use uuid::Uuid;

    pub const CHAR: Uuid = Uuid::from_u128(0x3AB0037F_AF8D_48CE_BCA0_A170D18B2C03);
    pub const SIGNED_CHAR: Uuid = Uuid::from_u128(0xCFD606FE_41B8_4744_B79F_8A6BD97713D8);
    pub const S8: Uuid = Uuid::from_u128(0x58422C0E_1E47_4854_98E6_34098F6FE12D);
    pub const SHORT: Uuid = Uuid::from_u128(0xB8A56D56_A10D_4DCE_9F63_405EE243DD3C);
    pub const INT: Uuid = Uuid::from_u128(0x72039442_EB38_4D42_A1AD_CB68F7E0EEF6);
    pub const LONG: Uuid = Uuid::from_u128(0x8F24B9AD_7C51_46CF_B2F8_277356957325);
    pub const S64: Uuid = Uuid::from_u128(0x70D8A282_A1EA_462D_9D04_51EDE81FAC2F);
    pub const U8: Uuid = Uuid::from_u128(0x72B9409A_7D1A_4831_9CFE_FCB3FADD3426);
    pub const U16: Uuid = Uuid::from_u128(0xECA0B403_C4F8_4B86_95FC_81688D046E40);
    pub const U32: Uuid = Uuid::from_u128(0x43DA906B_7DEF_4CA8_9790_854106D3F983);
    pub const ULONG: Uuid = Uuid::from_u128(0x5EC2D6F7_6859_400F_9215_C106F5B10E53);
    pub const U64: Uuid = Uuid::from_u128(0xD6597933_47CD_4FC8_B911_63F3E2B0993A);
    pub const FLOAT: Uuid = Uuid::from_u128(0xEA2C3E90_AFBE_44D4_A90D_FAAF79BAF93D);
    pub const DOUBLE: Uuid = Uuid::from_u128(0x110C4B14_11A8_4E9D_8638_5051013A56AC);
    pub const BOOL: Uuid = Uuid::from_u128(0xA0CA880C_AFE4_43CB_926C_59AC48496112);
    pub const AZ_UUID: Uuid = Uuid::from_u128(0xE152C105_A133_4D03_BBF8_3D4B2FBA3E2A);
    pub const AZ_ENTITY: Uuid = Uuid::from_u128(0x75651658_8663_478D_9090_2432DFCAFA44);
    pub const ENTITY_ID: Uuid = Uuid::from_u128(0x6383F1D3_BB27_4E6B_A49A_6409B2059EAA);
    pub const VOID: Uuid = Uuid::from_u128(0xC0F1AFAD_5CB3_450E_B0F5_ADB5D46B0E22);
    pub const CRC32: Uuid = Uuid::from_u128(0x9F4E062E_06A0_46D4_85DF_E0DA96467D3A);
    pub const PLATFORM_ID: Uuid = Uuid::from_u128(0x0635D08E_DDD2_48DE_A7AE_73CC563C57C3);
    pub const VECTOR_FLOAT: Uuid = Uuid::from_u128(0xEEA8B695_51EE_4717_B818_4070C6DA849D);
    pub const VECTOR2: Uuid = Uuid::from_u128(0x3D80F623_C85C_4741_90D0_E4E66164E6BF);
    pub const VECTOR3: Uuid = Uuid::from_u128(0x8379EB7D_01FA_4538_B64B_A6543B4BE73D);
    pub const VECTOR4: Uuid = Uuid::from_u128(0x0CE9FA36_1E3A_4C06_9254_B7C73A732053);
    pub const TRANSFORM: Uuid = Uuid::from_u128(0x5D9958E9_9F1E_4985_B532_FFFDE75FEDFD);
    pub const QUATERNION: Uuid = Uuid::from_u128(0x73103120_3DD3_4873_BAB3_9713FA2804FB);
    pub const COLOR: Uuid = Uuid::from_u128(0x7894072A_9050_4F0F_901B_34B1A0D29417);
    pub const COLORF: Uuid = Uuid::from_u128(0x63782551_A309_463B_A301_3A360800DF1E);
    pub const COLORB: Uuid = Uuid::from_u128(0x6F0CC2C0_0CC6_4DBF_9297_B043F270E6A4);
    pub const AABB: Uuid = Uuid::from_u128(0xA54C2B36_D5B8_46A1_A529_4EBDBD2450E7);
    pub const OBB: Uuid = Uuid::from_u128(0x004ABD25_CF14_4EB3_BD41_022C247C07FA);
    pub const PLANE: Uuid = Uuid::from_u128(0x847DD984_9DBF_4789_8E25_E0334402E8AD);
    pub const MATRIX3X3: Uuid = Uuid::from_u128(0x15A4332F_7C3F_4A58_AC35_50E1CE53FB9C);
    pub const MATRIX4X4: Uuid = Uuid::from_u128(0x157193C7_B673_4A2B_8B43_5681DCC3DEC3);
    pub const AZSTD_MONOSTATE: Uuid = Uuid::from_u128(0xB1E9136B_D77A_4643_BE8E_2ABDA246AE0E);
    pub const AZSTD_ALLOCATOR: Uuid = Uuid::from_u128(0xE9F5A3BE_2B3D_4C62_9E6B_4E00A13AB452);
    pub const AZ_AZSTD_ALLOC: Uuid = Uuid::from_u128(0x42D0AA1E_3C6C_440E_ABF8_435931150470);
    pub const AZ_OS_ALLOCATOR: Uuid = Uuid::from_u128(0x9F835EE3_F23C_454E_B4E3_011E2F3C8118);
    pub const AZ_SYSTEM_ALLOCATOR: Uuid = Uuid::from_u128(0x424C94D8_85CF_4E89_8CD6_AB5EC173E875);
    pub const AZSTD_LESS: Uuid = Uuid::from_u128(0x41B40AFC_68FD_4ED9_9EC7_BA9992802E1B);
    pub const AZSTD_LESS_EQUAL: Uuid = Uuid::from_u128(0x91CC0BDC_FC46_4617_A405_D914EF1C1902);
    pub const AZSTD_GREATER: Uuid = Uuid::from_u128(0x907F012A_7A4F_4B57_AC23_48DC08D0782E);
    pub const AZSTD_GREATER_EQUAL: Uuid = Uuid::from_u128(0xEB00488F_E20F_471A_B862_F1E3C39DDA1D);
    pub const AZSTD_EQUAL_TO: Uuid = Uuid::from_u128(0x4377BCED_F78C_4016_80BB_6AFACE6E5137);
    pub const AZSTD_HASH: Uuid = Uuid::from_u128(0xEFA74E54_BDFA_47BE_91A7_5A05DA0306D7);
    pub const AZSTD_CHAR_TRAITS: Uuid = Uuid::from_u128(0x9B018C0C_022E_4BA4_AE91_2C1E8592DBB2);
    pub const AZSTD_BASIC_STRING: Uuid = Uuid::from_u128(0xC26397ED_8F60_4DF6_8320_0D0C592DA3CD);
    pub const AZSTD_STRING: Uuid = Uuid::from_u128(0x03AAAB3F_5C47_5A66_9EBC_D5FA4DB353C9);
    pub const AZSTD_STRING_XML_ALIAS: Uuid =
        Uuid::from_u128(0xEF8FF807_DDEE_4EB0_B678_4CA3A2C490A4);
    pub const AZSTD_BASIC_STRING_VIEW: Uuid =
        Uuid::from_u128(0xD348D661_6BDE_4C0A_9540_FCEA4522D497);
    pub const AZSTD_PAIR: Uuid = Uuid::from_u128(0x919645C1_E464_482B_A69B_04AA688B6847);
    pub const AZSTD_VECTOR: Uuid = Uuid::from_u128(0xA60E3E61_1FF6_4982_B6B8_9E4350C4C679);
    pub const AZSTD_VECTOR_XML_ALIAS: Uuid =
        Uuid::from_u128(0x2BADE35A_6F1B_4698_B2BC_3373D010020C);
    pub const AZSTD_LIST: Uuid = Uuid::from_u128(0xE1E05843_BB02_4F43_B7DC_3ADB28DF42AC);
    pub const AZSTD_FORWARD_LIST: Uuid = Uuid::from_u128(0xD7E91EA3_326F_4019_87F0_6F45924B909A);
    pub const AZSTD_SET: Uuid = Uuid::from_u128(0x6C51837F_B0C9_40A3_8D52_2143341EDB07);
    pub const AZSTD_UNORDERED_SET: Uuid = Uuid::from_u128(0x8D60408E_DA65_4670_99A2_8ABB574625AE);
    pub const AZSTD_UNORDERED_MULTISET: Uuid =
        Uuid::from_u128(0xB5950921_7F70_4806_9C13_8C7DF841BB90);
    pub const AZSTD_MAP: Uuid = Uuid::from_u128(0xF8ECF58D_D33E_49DC_BF34_8FA499AC3AE1);
    pub const AZSTD_UNORDERED_MAP: Uuid = Uuid::from_u128(0x41171F6F_9E5E_4227_8420_289F1DD5D005);
    pub const AZSTD_UNORDERED_MULTIMAP: Uuid =
        Uuid::from_u128(0x9ED846FA_31C1_4133_B4F4_91DF9750BA96);
    pub const AZSTD_SHARED_PTR: Uuid = Uuid::from_u128(0xFE61C84E_149D_43FD_88BA_1C3DB7E548B4);
    pub const AZSTD_INTRUSIVE_PTR: Uuid = Uuid::from_u128(0x530F8502_309E_4EE1_9AEF_5C0456B1F502);
    pub const AZSTD_UNIQUE_PTR: Uuid = Uuid::from_u128(0xB55F90DA_C21E_4EB4_9857_87BE6529BA6D);
    pub const AZSTD_OPTIONAL: Uuid = Uuid::from_u128(0xAB8C50C0_23A7_4333_81CD_46F648938B1C);
    pub const AZSTD_FIXED_VECTOR: Uuid = Uuid::from_u128(0x74044B6F_E922_4FD7_915D_EFC5D1DC59AE);
    pub const AZSTD_FIXED_LIST: Uuid = Uuid::from_u128(0x508B9687_8410_4A73_AE0C_0BA15CF3F773);
    pub const AZSTD_FIXED_FORWARD_LIST: Uuid =
        Uuid::from_u128(0x0D9D2AB2_F0CC_4E30_A209_A33D78717649);
    pub const AZSTD_ARRAY: Uuid = Uuid::from_u128(0x911B2EA8_CCB1_4F0C_A535_540AD00173AE);
    pub const AZSTD_BITSET: Uuid = Uuid::from_u128(0x6BAE9836_EC49_466A_85F2_F4B1B70839FB);
    pub const AZSTD_TUPLE: Uuid = Uuid::from_u128(0xF99F9308_DC3E_4384_9341_89CBF1ABD51E);
    pub const AZSTD_RANGED_INT: Uuid = Uuid::from_u128(0xEAC2A157_5400_499D_81F1_8E8D979E96D8);
    pub const AZSTD_UNORDERED_FLAT_MAP: Uuid =
        Uuid::from_u128(0xAA6CB2BA_A6FA_43A3_B08C_4B6E0D751068);
    pub const VARIANT: Uuid = Uuid::from_u128(0x1E8BB1E5_410A_4367_8FAA_D43A4DE14D4B);
    pub const AZSTD_FUNCTION: Uuid = Uuid::from_u128(0xC9F9C644_CCC3_4F77_A792_F5B5DBCA746E);
    pub const AZ_DATA_ASSET: Uuid = Uuid::from_u128(0xC891BF19_B60C_45E2_BFD0_027D15DDC939);
    pub const AZ_DATA_ASSET_REFLECTION: Uuid =
        Uuid::from_u128(0x77A19D40_8731_4D3C_9041_1B43047366A4);
    pub const AZ_DATA_ASSET_DATA: Uuid = Uuid::from_u128(0xAF3F7D32_1536_422A_89F3_A11E1F5B5A9C);
    pub const AZ_DATA_ASSET_ID: Uuid = Uuid::from_u128(0x652ED536_3402_439B_AEBE_4A5DBC554085);
    pub const BYTE_STREAM: Uuid = Uuid::from_u128(0xADFD596B_7177_5519_9752_BC418FE42963);
    pub const AZ_INTERNAL_RVALUE_TO_LVALUE_WRAPPER: Uuid =
        Uuid::from_u128(0x2590807F_5748_4CD0_A475_83EF5FD216CF);
    pub const MB_REPLICATED_FIELD: Uuid = Uuid::from_u128(0x5C059EC7_44B0_4666_9FC9_674192338F39);
    pub const AMAZON_PERVASIVES_UID: Uuid = Uuid::from_u128(0xDFE50973_EA0B_4616_833A_B60B5E2E71DF);
}

/// Fold an `AZ_TYPE_INFO_AUTO` non-type template parameter.
///
/// `AzCore` converts numeric template values to decimal text and then
/// uses `Uuid::CreateName` over that text.
#[must_use]
pub const fn template_auto_type_id(value: usize) -> Uuid {
    let mut bytes = [0u8; 20];
    let len = write_decimal(&mut bytes, 0, value);
    create_data_prefix(&bytes, len)
}

/// Fold `AZStd::less<T>`.
#[inline]
#[must_use]
pub const fn azstd_less(value: Uuid) -> Uuid {
    specialized_template_postfix_non_empty(type_ids::AZSTD_LESS, &[value])
}

/// Fold `AZStd::hash<T>`.
#[inline]
#[must_use]
pub const fn azstd_hash(value: Uuid) -> Uuid {
    specialized_template_postfix_non_empty(type_ids::AZSTD_HASH, &[value])
}

/// Fold `AZStd::equal_to<T>`.
#[inline]
#[must_use]
pub const fn azstd_equal_to(value: Uuid) -> Uuid {
    specialized_template_postfix_non_empty(type_ids::AZSTD_EQUAL_TO, &[value])
}

/// Fold `AZStd::char_traits<T>`.
#[inline]
#[must_use]
pub const fn azstd_char_traits(element: Uuid) -> Uuid {
    specialized_template_postfix_non_empty(type_ids::AZSTD_CHAR_TRAITS, &[element])
}

/// Fold `AZ::AZStdAlloc<Allocator>`.
#[inline]
#[must_use]
pub const fn azstd_alloc(allocator: Uuid) -> Uuid {
    match specialized_template_prefix(type_ids::AZ_AZSTD_ALLOC, &[allocator]) {
        Some(type_id) => type_id,
        None => Uuid::from_u128(0),
    }
}

/// Fold `AZStd::basic_string<C, Traits, Allocator>`.
#[inline]
#[must_use]
pub const fn azstd_basic_string(char_type: Uuid, traits: Uuid, allocator: Uuid) -> Uuid {
    specialized_template_postfix_non_empty(
        type_ids::AZSTD_BASIC_STRING,
        &[char_type, traits, allocator],
    )
}

/// Fold canonical `AZStd::string`.
#[inline]
#[must_use]
pub const fn azstd_string() -> Uuid {
    azstd_basic_string(
        type_ids::CHAR,
        azstd_char_traits(type_ids::CHAR),
        type_ids::AZSTD_ALLOCATOR,
    )
}

/// Fold `AZStd::pair<K, V>`.
#[inline]
#[must_use]
pub const fn azstd_pair(first: Uuid, second: Uuid) -> Uuid {
    specialized_template_postfix_non_empty(type_ids::AZSTD_PAIR, &[first, second])
}

/// Fold `AZStd::vector<T, AZStd::allocator>`.
#[inline]
#[must_use]
pub const fn azstd_vector(element: Uuid) -> Uuid {
    azstd_vector_with_allocator(element, type_ids::AZSTD_ALLOCATOR)
}

/// Fold `AZStd::vector<T, Allocator>`.
#[inline]
#[must_use]
pub const fn azstd_vector_with_allocator(element: Uuid, allocator: Uuid) -> Uuid {
    specialized_template_postfix_non_empty(type_ids::AZSTD_VECTOR, &[element, allocator])
}

/// Fold `AZStd::list<T, AZStd::allocator>`.
#[inline]
#[must_use]
pub const fn azstd_list(element: Uuid) -> Uuid {
    specialized_template_postfix_non_empty(
        type_ids::AZSTD_LIST,
        &[element, type_ids::AZSTD_ALLOCATOR],
    )
}

/// Fold `AZStd::forward_list<T, AZStd::allocator>`.
#[inline]
#[must_use]
pub const fn azstd_forward_list(element: Uuid) -> Uuid {
    specialized_template_postfix_non_empty(
        type_ids::AZSTD_FORWARD_LIST,
        &[element, type_ids::AZSTD_ALLOCATOR],
    )
}

/// Fold `AZStd::set<K, AZStd::less<K>, AZStd::allocator>`.
#[inline]
#[must_use]
pub const fn azstd_set(key: Uuid) -> Uuid {
    specialized_template_postfix_non_empty(
        type_ids::AZSTD_SET,
        &[key, azstd_less(key), type_ids::AZSTD_ALLOCATOR],
    )
}

/// Fold `AZStd::unordered_set<K, AZStd::hash<K>, AZStd::equal_to<K>, AZStd::allocator>`.
#[inline]
#[must_use]
pub const fn azstd_unordered_set(key: Uuid) -> Uuid {
    specialized_template_postfix_non_empty(
        type_ids::AZSTD_UNORDERED_SET,
        &[
            key,
            azstd_hash(key),
            azstd_equal_to(key),
            type_ids::AZSTD_ALLOCATOR,
        ],
    )
}

/// Fold `AZStd::map<K, V, AZStd::less<K>, AZStd::allocator>`.
#[inline]
#[must_use]
pub const fn azstd_map(key: Uuid, value: Uuid) -> Uuid {
    specialized_template_postfix_non_empty(
        type_ids::AZSTD_MAP,
        &[key, value, azstd_less(key), type_ids::AZSTD_ALLOCATOR],
    )
}

/// Fold `AZStd::unordered_map<K, V, AZStd::hash<K>, AZStd::equal_to<K>, AZStd::allocator>`.
#[inline]
#[must_use]
pub const fn azstd_unordered_map(key: Uuid, value: Uuid) -> Uuid {
    specialized_template_postfix_non_empty(
        type_ids::AZSTD_UNORDERED_MAP,
        &[
            key,
            value,
            azstd_hash(key),
            azstd_equal_to(key),
            type_ids::AZSTD_ALLOCATOR,
        ],
    )
}

/// Fold `AZStd::unordered_flat_map<K, V, AZStd::hash<K>, AZStd::equal_to<K>, AZStd::allocator>`.
///
/// Uses the same operand order as `AZStd::unordered_map`, with the
/// flat-map base id.
#[inline]
#[must_use]
pub const fn azstd_unordered_flat_map(key: Uuid, value: Uuid) -> Uuid {
    specialized_template_postfix_non_empty(
        type_ids::AZSTD_UNORDERED_FLAT_MAP,
        &[
            key,
            value,
            azstd_hash(key),
            azstd_equal_to(key),
            type_ids::AZSTD_ALLOCATOR,
        ],
    )
}

/// Fold `AZStd::shared_ptr<T>`.
#[inline]
#[must_use]
pub const fn azstd_shared_ptr(element: Uuid) -> Uuid {
    specialized_template_postfix_non_empty(type_ids::AZSTD_SHARED_PTR, &[element])
}

/// Fold `AZStd::intrusive_ptr<T>`.
#[inline]
#[must_use]
pub const fn azstd_intrusive_ptr(element: Uuid) -> Uuid {
    specialized_template_postfix_non_empty(type_ids::AZSTD_INTRUSIVE_PTR, &[element])
}

/// Fold `AZStd::unique_ptr<T, Deleter>`.
///
/// Unique-pointer type IDs fold only the pointee type.
#[inline]
#[must_use]
pub const fn azstd_unique_ptr(element: Uuid) -> Uuid {
    specialized_template_postfix_non_empty(type_ids::AZSTD_UNIQUE_PTR, &[element])
}

/// Fold `AZStd::optional<T>`.
#[inline]
#[must_use]
pub const fn azstd_optional(element: Uuid) -> Uuid {
    specialized_template_postfix_non_empty(type_ids::AZSTD_OPTIONAL, &[element])
}

/// Fold `AZStd::fixed_vector<T, Capacity>`.
#[inline]
#[must_use]
pub const fn azstd_fixed_vector(element: Uuid, capacity: usize) -> Uuid {
    specialized_template_postfix_non_empty(
        type_ids::AZSTD_FIXED_VECTOR,
        &[element, template_auto_type_id(capacity)],
    )
}

/// Fold `AZStd::array<T, Size>`.
#[inline]
#[must_use]
pub const fn azstd_array(element: Uuid, size: usize) -> Uuid {
    specialized_template_postfix_non_empty(
        type_ids::AZSTD_ARRAY,
        &[element, template_auto_type_id(size)],
    )
}

/// Fold `AZStd::bitset<Bits>`.
#[inline]
#[must_use]
pub const fn azstd_bitset(bits: usize) -> Uuid {
    specialized_template_postfix_non_empty(type_ids::AZSTD_BITSET, &[template_auto_type_id(bits)])
}

/// Fold `AZStd::ranged_int<T, Min, Max>`.
///
/// New World's helper folds `T` with `CreateName("{Min}, {Max}")`, then appends
/// the ranged-int base id.
#[inline]
#[must_use]
pub const fn azstd_ranged_int(value: Uuid, min: usize, max: usize) -> Uuid {
    let mut bytes = [0u8; 42];
    let mut len = write_decimal(&mut bytes, 0, min);
    bytes[len] = b',';
    len += 1;
    bytes[len] = b' ';
    len += 1;
    len = write_decimal(&mut bytes, len, max);
    let range = create_data_prefix(&bytes, len);
    specialized_template_postfix_non_empty(type_ids::AZSTD_RANGED_INT, &[value, range])
}

/// Fold `AZStd::tuple<T...>`.
#[inline]
#[must_use]
pub const fn azstd_tuple(args: &[Uuid]) -> Option<Uuid> {
    match aggregate_type_ids_right(args) {
        Some(args) => Some(combine(type_ids::AZSTD_TUPLE, args)),
        None => None,
    }
}

/// Fold `AZ::Data::Asset<T>`.
#[inline]
#[must_use]
pub const fn az_data_asset(asset_type: Uuid) -> Uuid {
    match specialized_template_prefix(type_ids::AZ_DATA_ASSET, &[asset_type]) {
        Some(type_id) => type_id,
        None => Uuid::from_u128(0),
    }
}

/// Fold `AZ::Internal::RValueToLValueWrapper<T>`.
#[inline]
#[must_use]
pub const fn az_internal_rvalue_to_lvalue_wrapper(value: Uuid) -> Uuid {
    specialized_template_postfix_non_empty(type_ids::AZ_INTERNAL_RVALUE_TO_LVALUE_WRAPPER, &[value])
}

/// Fold `MB::ReplicatedField<T>`.
///
/// The template uses the embedded `AZ_TYPE_INFO` prefix form: base id
/// followed by the reflected value type id.
#[inline]
#[must_use]
pub const fn mb_replicated_field(value: Uuid) -> Uuid {
    match specialized_template_prefix(type_ids::MB_REPLICATED_FIELD, &[value]) {
        Some(type_id) => type_id,
        None => Uuid::from_u128(0),
    }
}

/// Fold `Amazon::Pervasives::UID<Bits>`.
///
/// The UID helper folds the bit count name before the UID base type.
#[inline]
#[must_use]
pub const fn amazon_pervasives_uid(bits: usize) -> Uuid {
    combine(template_auto_type_id(bits), type_ids::AMAZON_PERVASIVES_UID)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::uuid;

    #[test]
    fn create_name_matches_azcore_masking() {
        assert_eq!(
            create_name(b"hello"),
            uuid!("aaf4c61d-dcc5-58a2-9abe-de0f3b482cd9")
        );
        assert_eq!(create_name(b""), Uuid::nil());
    }

    #[test]
    fn combine_matches_uuid_operator_plus() {
        let s32 = type_ids::INT;
        let u8 = type_ids::U8;

        assert_eq!(
            combine(s32, u8),
            uuid!("2554130f-2bb8-5a25-8cc4-319329151f28")
        );
    }

    #[test]
    fn folds_azstd_default_template_arguments() {
        let u8 = type_ids::U8;
        let reflected_type = uuid!("323EF0E3-2B16-4D24-8DFB-D7107012BF21");

        assert_eq!(
            azstd_vector(u8),
            uuid!("adfd596b-7177-5519-9752-bc418fe42963")
        );
        assert_eq!(
            azstd_vector(reflected_type),
            uuid!("98ff2f25-6160-5d4c-b7c7-34b636703f59")
        );
    }

    #[test]
    fn folds_pair_and_wrappers() {
        let s32 = type_ids::INT;
        let u8 = type_ids::U8;
        let string = type_ids::AZSTD_STRING;

        assert_eq!(
            azstd_pair(s32, u8),
            uuid!("1f620a8f-a022-5620-88c9-cd9fbf7f0385")
        );
        assert_eq!(
            azstd_shared_ptr(string),
            uuid!("9d9b2586-e5ec-5509-8f94-09288552a680")
        );
        assert_eq!(
            azstd_unique_ptr(uuid!("95E1B61B-F826-4C94-9995-4D7587085DE9")),
            uuid!("a7499562-123d-5f98-b702-51f5b4649a0d")
        );
    }

    #[test]
    fn folds_templates_with_numeric_arguments() {
        let bool_type = type_ids::BOOL;
        let crc32 = type_ids::CRC32;

        assert_eq!(
            template_auto_type_id(32),
            uuid!("cb4e5208-b4cd-5726-8b20-8e49452ed6e8")
        );
        assert_eq!(
            azstd_array(bool_type, 32),
            uuid!("fa9eb196-34e6-57f1-a220-4f2cc93f8792")
        );
        assert_eq!(
            azstd_fixed_vector(crc32, 20),
            uuid!("4bbb5d83-c428-58e2-b045-24d4608383e1")
        );
        assert_eq!(
            azstd_bitset(2),
            uuid!("4852f7a3-95be-5cbf-b76f-031d6f334df9")
        );
    }

    #[test]
    fn folds_source_backed_special_cases() {
        assert_eq!(
            azstd_char_traits(type_ids::CHAR),
            uuid!("406e9b16-a89c-5289-b10e-17f338588559")
        );
        assert_eq!(
            azstd_string(),
            uuid!("03aaab3f-5c47-5a66-9ebc-d5fa4db353c9")
        );
        assert_eq!(type_ids::AZSTD_STRING, azstd_string());
        assert_eq!(type_ids::BYTE_STREAM, azstd_vector(type_ids::U8));
        assert_eq!(
            azstd_tuple(&[uuid!("31694A66-A3B2-49F0-A7B4-18D1B906CFBD"), type_ids::U64]),
            Some(uuid!("0c58dd7b-90db-5ad9-a24b-53ad03f6593a"))
        );
        assert_eq!(
            azstd_alloc(type_ids::AZ_OS_ALLOCATOR),
            uuid!("ee499aae-e5bd-59fa-85f2-babda6aa208e")
        );
        assert_eq!(
            azstd_vector_with_allocator(
                uuid!("C32D1E88-2B8B-432C-91BC-D0B4B135279D"),
                azstd_alloc(type_ids::AZ_OS_ALLOCATOR)
            ),
            uuid!("50b333e9-98cd-5ff6-871a-5c6cd54c83a1")
        );
        assert_eq!(
            azstd_tuple(&[
                uuid!("5D42C439-A859-4133-9032-88DE31048F2C"),
                azstd_string(),
                type_ids::INT
            ]),
            Some(uuid!("de1cb64d-ebc4-583e-af31-eb257b8ac677"))
        );
        assert_eq!(
            az_data_asset(uuid!("AF3F7D32-1536-422A-89F3-A11E1F5B5A9C")),
            uuid!("01db3319-83c9-55ad-a271-eb299466fe34")
        );
        assert_eq!(
            az_internal_rvalue_to_lvalue_wrapper(type_ids::S8),
            uuid!("842097ca-15c0-5c8d-a2d8-92ea8995c752")
        );
        assert_eq!(
            azstd_ranged_int(type_ids::U8, 0, 8),
            uuid!("255e32a3-024c-54a0-8d5c-6ba682b43192")
        );
        assert_eq!(
            azstd_unordered_flat_map(type_ids::CRC32, type_ids::FLOAT),
            uuid!("57c9c8da-f80a-56c2-9ade-16a19a8f6733")
        );
        assert_eq!(
            azstd_unordered_set(type_ids::AZ_DATA_ASSET_ID),
            uuid!("db1eb3e5-f953-53a7-b8f9-9121e6a77f85")
        );
        assert_eq!(
            mb_replicated_field(type_ids::INT),
            uuid!("44bc0c45-da18-5e2c-9d9d-943f964cb90c")
        );
        assert_eq!(
            amazon_pervasives_uid(128),
            uuid!("3485f20a-98c0-5315-876b-21bcd23a7bc0")
        );
    }
}
