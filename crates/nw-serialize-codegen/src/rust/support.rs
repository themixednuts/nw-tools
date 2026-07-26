pub(super) fn rtti_module_source() -> &'static str {
    RUST_RTTI_MODULE
}

pub(super) fn uuid_module_source() -> &'static str {
    RUST_UUID_MODULE
}

pub(super) fn crc_module_source() -> &'static str {
    RUST_CRC_MODULE
}

pub(super) fn asset_module_source() -> &'static str {
    RUST_ASSET_MODULE
}

pub(super) fn single_file_source() -> String {
    let mut source = String::new();
    source.push_str("pub mod az {\n");
    push_module(&mut source, "rtti", rtti_module_source());
    push_module(&mut source, "uuid", uuid_module_source());
    push_module(&mut source, "crc", crc_module_source());
    push_module(&mut source, "asset", asset_module_source());
    source.push_str("}\n\n");
    source.push_str(
        "pub use crate::az::asset::Asset as AzAsset;\n\
         pub use crate::az::asset::AssetId as AzAssetId;\n\
         pub use crate::az::crc::Crc32 as AzCrc32;\n\
         pub use crate::az::rtti::AzRtti;\n\
         pub use crate::az::uuid::Uuid;\n\
         pub use crate::az::uuid::Uuid as AzUuid;\n",
    );
    source
}

fn push_module(out: &mut String, module_name: &str, module_source: &str) {
    out.push_str("\tpub mod ");
    out.push_str(module_name);
    out.push_str(" {\n");
    out.push_str(module_source);
    out.push_str("\n\t}\n");
}

const RUST_RTTI_MODULE: &str = r#"
use crate::az::uuid::Uuid;

pub trait AzRtti {
	const NAME: &'static str;
	const TYPE_ID: Uuid;
	const BASE_TYPE_IDS: &'static [Uuid] = &[];
}

macro_rules! az_rtti_scalar {
	($ty:ty, $name:literal, $uuid:literal) => {
		impl AzRtti for $ty {
			const NAME: &'static str = $name;
			const TYPE_ID: Uuid = Uuid::from_u128($uuid);
		}
	};
}

/// Standalone reflected `AZ::EntityId`.
///
/// Ordinary generated entity-id fields remain `u64` for compatibility. A
/// dynamic payload needs its distinct AZ type identity, however, so it uses
/// this transparent wrapper rather than incorrectly inheriting `u64`'s UUID.
#[derive(
	Debug,
	Clone,
	Copy,
	Default,
	Eq,
	Hash,
	Ord,
	PartialEq,
	PartialOrd,
	serde::Serialize,
	serde::Deserialize,
	bevy_reflect::Reflect,
)]
#[serde(transparent)]
#[repr(transparent)]
pub struct EntityId(pub u64);

// Reflected identities of the primitives, mirroring the `az_type_info!` block in
// `az-core/src/type_info.rs`. A dynamic sum resolves its payload's `$type`
// through this trait, so every observed keyed payload needs its identity here
// just as it does in the integrated backend.
az_rtti_scalar!(bool, "bool", 0xA0CA880C_AFE4_43CB_926C_59AC48496112);
az_rtti_scalar!(i8, "s8", 0x58422C0E_1E47_4854_98E6_34098F6FE12D);
az_rtti_scalar!(i16, "s16", 0xB8A56D56_A10D_4DCE_9F63_405EE243DD3C);
az_rtti_scalar!(i32, "s32", 0x72039442_EB38_4D42_A1AD_CB68F7E0EEF6);
az_rtti_scalar!(i64, "s64", 0x70D8A282_A1EA_462D_9D04_51EDE81FAC2F);
az_rtti_scalar!(u8, "u8", 0x72B9409A_7D1A_4831_9CFE_FCB3FADD3426);
az_rtti_scalar!(u16, "u16", 0xECA0B403_C4F8_4B86_95FC_81688D046E40);
az_rtti_scalar!(u32, "u32", 0x43DA906B_7DEF_4CA8_9790_854106D3F983);
az_rtti_scalar!(u64, "u64", 0xD6597933_47CD_4FC8_B911_63F3E2B0993A);
az_rtti_scalar!(f32, "float", 0xEA2C3E90_AFBE_44D4_A90D_FAAF79BAF93D);
az_rtti_scalar!(f64, "double", 0x110C4B14_11A8_4E9D_8638_5051013A56AC);
az_rtti_scalar!(EntityId, "AZ::EntityId", 0x6383F1D3_BB27_4E6B_A49A_6409B2059EAA);
az_rtti_scalar!(bevy_math::Vec2, "AZ::Vector2", 0x3D80F623_C85C_4741_90D0_E4E66164E6BF);
az_rtti_scalar!(bevy_math::Vec3, "AZ::Vector3", 0x8379EB7D_01FA_4538_B64B_A6543B4BE73D);
az_rtti_scalar!(
	bevy_color::LinearRgba,
	"AZ::Color",
	0x7894072A_9050_4F0F_901B_34B1A0D29417
);
"#;

const RUST_UUID_MODULE: &str = r#"
use bevy_reflect::Reflect;
use sha1::Digest;

use crate::az::rtti::AzRtti;

#[derive(Clone, Copy, Default, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize, Reflect)]
#[repr(transparent)]
pub struct Uuid(::uuid::Uuid);

impl Uuid {
	pub const NIL: Self = Self(::uuid::Uuid::nil());

	#[must_use]
	pub const fn nil() -> Self {
		Self::NIL
	}

	#[must_use]
	pub const fn from_u128(value: u128) -> Self {
		Self(::uuid::Uuid::from_u128(value))
	}

	#[must_use]
	pub const fn from_bytes(bytes: [u8; 16]) -> Self {
		Self(::uuid::Uuid::from_bytes(bytes))
	}

	/// Parses a UUID string into the AZ UUID wrapper.
	///
	/// # Errors
	///
	/// Returns the underlying `uuid` parse error when `value` is not a valid UUID.
	pub fn parse_str(value: &str) -> Result<Self, ::uuid::Error> {
		::uuid::Uuid::parse_str(value).map(Self)
	}

	#[must_use]
	pub const fn is_nil(self) -> bool {
		self.0.is_nil()
	}

	#[must_use]
	pub const fn as_inner(&self) -> &::uuid::Uuid {
		&self.0
	}

	#[must_use]
	pub const fn as_bytes(&self) -> &[u8; 16] {
		self.0.as_bytes()
	}

	#[must_use]
	pub fn braced_upper(self) -> String {
		self.0.as_braced().to_string().to_uppercase()
	}

	#[must_use]
	pub fn create_data(bytes: &[u8]) -> Self {
		if bytes.is_empty() {
			return Self::nil();
		}

		let mut hasher = sha1::Sha1::new();
		hasher.update(bytes);
		let digest = hasher.finalize();
		let mut data = [0_u8; 16];
		data.copy_from_slice(&digest[..16]);
		data[8] &= 0xBF;
		data[8] |= 0x80;
		data[6] &= 0x5F;
		data[6] |= 0x50;
		Self(::uuid::Uuid::from_bytes(data))
	}

	#[must_use]
	pub fn create_name(name: &[u8]) -> Self {
		Self::create_data(name)
	}

	#[must_use]
	pub fn combine(lhs: Self, rhs: Self) -> Self {
		let mut bytes = [0_u8; 32];
		bytes[..16].copy_from_slice(lhs.as_bytes());
		bytes[16..].copy_from_slice(rhs.as_bytes());
		Self::create_data(&bytes)
	}

	#[must_use]
	pub fn aggregate_type_ids(type_ids: impl IntoIterator<Item = Self>) -> Option<Self> {
		let mut iter = type_ids.into_iter();
		let mut acc = iter.next()?;
		for type_id in iter {
			acc = Self::combine(acc, type_id);
		}
		Some(acc)
	}

	#[must_use]
	pub fn aggregate_type_ids_right(type_ids: &[Self]) -> Option<Self> {
		let (first, tail) = type_ids.split_first()?;
		match Self::aggregate_type_ids_right(tail) {
			Some(tail) if !tail.is_nil() => Some(Self::combine(*first, tail)),
			_ => Some(*first),
		}
	}

	#[must_use]
	pub fn specialized_template_prefix(template_base: Self, args: &[Self]) -> Option<Self> {
		Self::aggregate_type_ids(args.iter().copied())
			.map(|args| Self::combine(template_base, args))
	}

	#[must_use]
	pub fn specialized_template_postfix(template_base: Self, args: &[Self]) -> Option<Self> {
		Self::aggregate_type_ids(args.iter().copied())
			.map(|args| Self::combine(args, template_base))
	}

	#[must_use]
	pub fn template_auto_type_id(value: usize) -> Self {
		Self::create_name(value.to_string().as_bytes())
	}
}

impl AzRtti for Uuid {
	const NAME: &'static str = "AZ::Uuid";
	const TYPE_ID: Self = Self::from_u128(0xE152C105_A133_4D03_BBF8_3D4B2FBA3E2A);
}

impl From<::uuid::Uuid> for Uuid {
	fn from(value: ::uuid::Uuid) -> Self {
		Self(value)
	}
}

impl From<Uuid> for ::uuid::Uuid {
	fn from(value: Uuid) -> Self {
		value.0
	}
}

impl AsRef<::uuid::Uuid> for Uuid {
	fn as_ref(&self) -> &::uuid::Uuid {
		&self.0
	}
}

impl core::fmt::Debug for Uuid {
	fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		core::fmt::Display::fmt(self, formatter)
	}
}

impl core::fmt::Display for Uuid {
	fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		core::fmt::Display::fmt(&self.0, formatter)
	}
}

impl core::str::FromStr for Uuid {
	type Err = ::uuid::Error;

	fn from_str(value: &str) -> Result<Self, Self::Err> {
		Self::parse_str(value)
	}
}

pub mod type_ids {
	use super::Uuid;

	pub const INT: Uuid = Uuid::from_u128(0x72039442_EB38_4D42_A1AD_CB68F7E0EEF6);
	pub const U8: Uuid = Uuid::from_u128(0x72B9409A_7D1A_4831_9CFE_FCB3FADD3426);
	pub const AZ_UUID: Uuid = Uuid::from_u128(0xE152C105_A133_4D03_BBF8_3D4B2FBA3E2A);
	pub const ENTITY_ID: Uuid = Uuid::from_u128(0x6383F1D3_BB27_4E6B_A49A_6409B2059EAA);
	pub const COMPONENT_ID: Uuid = Uuid::from_u128(0xD6597933_47CD_4FC8_B911_63F3E2B0993A);
	pub const COMPONENT_ID_VECTOR: Uuid = Uuid::from_u128(0xE7781CB0_E712_5E6A_948D_92FD4FE87F0D);
	pub const CRC32: Uuid = Uuid::from_u128(0x9F4E062E_06A0_46D4_85DF_E0DA96467D3A);
	pub const ASSET_ID: Uuid = Uuid::from_u128(0x652ED536_3402_439B_AEBE_4A5DBC554085);
}
"#;

const RUST_CRC_MODULE: &str = r#"
use bevy_reflect::Reflect;

use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, serde::Serialize, serde::Deserialize, Reflect)]
#[repr(transparent)]
pub struct Crc32(pub u32);

impl Crc32 {
	pub const ZERO: Self = Self(0);

	#[must_use]
	pub const fn new(value: u32) -> Self {
		Self(value)
	}

	#[must_use]
	pub const fn from_str_lower(value: &str) -> Self {
		Self(Self::crc32_lower(value.as_bytes()))
	}

	#[must_use]
	pub const fn from_bytes_lower(bytes: &[u8]) -> Self {
		Self(Self::crc32_lower(bytes))
	}

	#[must_use]
	pub const fn from_bytes(bytes: &[u8]) -> Self {
		Self(Self::crc32(bytes))
	}

	#[must_use]
	pub const fn value(self) -> u32 {
		self.0
	}

	#[must_use]
	pub const fn crc32(bytes: &[u8]) -> u32 {
		let mut crc = 0xFFFF_FFFF_u32;
		let mut index = 0;
		while index < bytes.len() {
			crc ^= bytes[index] as u32;
			let mut bit = 0;
			while bit < 8 {
				crc = if crc & 1 != 0 {
					0xEDB8_8320 ^ (crc >> 1)
				} else {
					crc >> 1
				};
				bit += 1;
			}
			index += 1;
		}
		crc ^ 0xFFFF_FFFF
	}

	#[must_use]
	pub const fn crc32_lower(bytes: &[u8]) -> u32 {
		let mut crc = 0xFFFF_FFFF_u32;
		let mut index = 0;
		while index < bytes.len() {
			let byte = bytes[index];
			let folded = if byte >= b'A' && byte <= b'Z' {
				byte + 32
			} else {
				byte
			};
			crc ^= folded as u32;
			let mut bit = 0;
			while bit < 8 {
				crc = if crc & 1 != 0 {
					0xEDB8_8320 ^ (crc >> 1)
				} else {
					crc >> 1
				};
				bit += 1;
			}
			index += 1;
		}
		crc ^ 0xFFFF_FFFF
	}
}

impl From<&str> for Crc32 {
	fn from(value: &str) -> Self {
		Self::from_str_lower(value)
	}
}

impl From<u32> for Crc32 {
	fn from(value: u32) -> Self {
		Self(value)
	}
}

impl From<Crc32> for u32 {
	fn from(value: Crc32) -> Self {
		value.0
	}
}

impl AzRtti for Crc32 {
	const NAME: &'static str = "AZ::Crc32";
	const TYPE_ID: Uuid = Uuid::from_u128(0x9F4E062E_06A0_46D4_85DF_E0DA96467D3A);
}
"#;

const RUST_ASSET_MODULE: &str = r#"
use bevy_reflect::Reflect;

use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid;

#[derive(Clone, Copy, Default, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize, Reflect)]
pub struct AssetId {
	pub guid: Uuid,
	pub sub_id: u32,
}

impl AssetId {
	#[must_use]
	pub const fn new(guid: Uuid, sub_id: u32) -> Self {
		Self { guid, sub_id }
	}

	#[must_use]
	pub const fn nil() -> Self {
		Self {
			guid: Uuid::nil(),
			sub_id: 0,
		}
	}

	#[must_use]
	pub const fn is_nil(&self) -> bool {
		self.sub_id == 0 && self.guid.is_nil()
	}
}

impl AsRef<Self> for AssetId {
	fn as_ref(&self) -> &Self {
		self
	}
}

impl From<Uuid> for AssetId {
	fn from(guid: Uuid) -> Self {
		Self { guid, sub_id: 0 }
	}
}

impl From<(Uuid, u32)> for AssetId {
	fn from((guid, sub_id): (Uuid, u32)) -> Self {
		Self { guid, sub_id }
	}
}

impl core::fmt::Debug for AssetId {
	fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		formatter
			.debug_struct("AssetId")
			.field("guid", &format_args!("{}", self.guid))
			.field("sub_id", &format_args!("{:#x}", self.sub_id))
			.finish()
	}
}

impl core::fmt::Display for AssetId {
	fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		write!(formatter, "{}:{:x}", self.guid.braced_upper(), self.sub_id)
	}
}

#[derive(Debug)]
pub enum AssetIdParseError {
	MissingSeparator,
	BadGuid(::uuid::Error),
	BadSubId(core::num::ParseIntError),
}

impl core::fmt::Display for AssetIdParseError {
	fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		match self {
			Self::MissingSeparator => {
				formatter.write_str("missing ':' separator between guid and sub_id")
			}
			Self::BadGuid(error) => write!(formatter, "invalid guid: {error}"),
			Self::BadSubId(error) => write!(formatter, "invalid sub_id: {error}"),
		}
	}
}

impl std::error::Error for AssetIdParseError {
	fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
		match self {
			Self::MissingSeparator => None,
			Self::BadGuid(error) => Some(error),
			Self::BadSubId(error) => Some(error),
		}
	}
}

impl core::str::FromStr for AssetId {
	type Err = AssetIdParseError;

	fn from_str(value: &str) -> Result<Self, Self::Err> {
		let (guid_part, sub_id_part) = value
			.rsplit_once(':')
			.ok_or(AssetIdParseError::MissingSeparator)?;
		let guid = Uuid::parse_str(guid_part.trim_start_matches('{').trim_end_matches('}'))
			.map_err(AssetIdParseError::BadGuid)?;
		let sub_id = u32::from_str_radix(sub_id_part, 16).map_err(AssetIdParseError::BadSubId)?;
		Ok(Self { guid, sub_id })
	}
}

impl AzRtti for AssetId {
	const NAME: &'static str = "AZ::Data::AssetId";
	const TYPE_ID: Uuid = Uuid::from_u128(0x652ED536_3402_439B_AEBE_4A5DBC554085);
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, Reflect)]
pub struct Asset {
	#[reflect(ignore)]
	pub asset_id: AssetId,
	#[reflect(ignore)]
	pub asset_type: Uuid,
	pub hint: Option<String>,
}

impl Asset {
	#[must_use]
	pub fn new(asset_id: AssetId, asset_type: Uuid, hint: Option<impl Into<String>>) -> Self {
		Self {
			asset_id,
			asset_type,
			hint: hint.map(Into::into),
		}
	}

	#[must_use]
	pub const fn empty() -> Self {
		Self {
			asset_id: AssetId::nil(),
			asset_type: Uuid::nil(),
			hint: None,
		}
	}

	#[must_use]
	pub const fn from_id(asset_id: AssetId, asset_type: Uuid) -> Self {
		Self {
			asset_id,
			asset_type,
			hint: None,
		}
	}

	#[must_use]
	pub fn from_hint(hint: impl Into<String>) -> Self {
		Self {
			hint: Some(hint.into()),
			..Self::empty()
		}
	}

	#[must_use]
	pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
		self.hint = Some(hint.into());
		self
	}

	#[must_use]
	pub fn hint(&self) -> Option<&str> {
		self.hint.as_deref().map(str::trim).filter(|hint| !hint.is_empty())
	}

	#[must_use]
	pub const fn is_nil(&self) -> bool {
		self.asset_id.is_nil()
	}

	#[must_use]
	pub fn is_empty(&self) -> bool {
		self.is_nil() && self.asset_type.is_nil() && self.hint().is_none()
	}
}

impl Default for Asset {
	fn default() -> Self {
		Self::empty()
	}
}

impl AzRtti for Asset {
	const NAME: &'static str = "AZ::Data::Asset";
	const TYPE_ID: Uuid = Uuid::from_u128(0xC891BF19_B60C_45E2_BFD0_027D15DDC939);
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn support_sources_keep_az_uuid_and_crc_behavior_in_wrappers() {
        let uuid = uuid_module_source();
        let crc = crc_module_source();

        assert!(uuid.contains("pub struct Uuid(::uuid::Uuid);"));
        assert!(uuid.contains("data[8] &= 0xBF;"));
        assert!(uuid.contains("pub fn aggregate_type_ids"));
        assert!(uuid.contains("pub fn aggregate_type_ids_right"));
        assert!(uuid.contains("pub fn specialized_template_prefix"));
        assert!(uuid.contains("0xE152C105_A133_4D03_BBF8_3D4B2FBA3E2A"));
        assert!(!uuid.contains("0xE152_C105_A133_4D03_BBF8_3D4B_2FBA_3E2A"));
        assert!(crc.contains("pub struct Crc32(pub u32);"));
        assert!(crc.contains("pub const fn from_str_lower"));
        assert!(crc.contains("0xEDB8_8320"));
    }

    #[test]
    fn standalone_rtti_covers_dynamic_payload_identities() {
        let rtti = rtti_module_source();

        assert!(rtti.contains("pub struct EntityId(pub u64);"));
        for type_id in [
            "0x6383F1D3_BB27_4E6B_A49A_6409B2059EAA",
            "0x3D80F623_C85C_4741_90D0_E4E66164E6BF",
            "0x8379EB7D_01FA_4538_B64B_A6543B4BE73D",
            "0x7894072A_9050_4F0F_901B_34B1A0D29417",
        ] {
            assert!(rtti.contains(type_id), "missing dynamic identity {type_id}");
        }
    }

    #[test]
    fn single_file_support_wraps_modules_under_az() {
        let source = single_file_source();

        assert!(source.contains("pub mod az"));
        assert!(source.contains("pub mod uuid"));
        assert!(source.contains("pub use crate::az::uuid::Uuid as AzUuid;"));
    }
}
