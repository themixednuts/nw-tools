package types

import "github.com/google/uuid"

type UUID = uuid.UUID

type AssetID struct {
	GUID  UUID
	SubID uint32
}

type AssetReference struct {
	ID        AssetID
	AssetType UUID
	Hint      string
}

type Vector3 struct {
	X float32
	Y float32
	Z float32
}

type CRC32 uint32

// StoreProductType mirrors the reflected New World StoreProductType enum.
type StoreProductType uint8

const (
	StoreProductTypeInvalid StoreProductType = iota
	StoreProductTypeApparelSkin
	StoreProductTypeApparelSkinSet
	StoreProductTypeBundle
	StoreProductTypeCampskin
	StoreProductTypeEmote
	StoreProductTypeEmotePermit
	StoreProductTypeGuildCrestPack
	StoreProductTypeHousePet
	StoreProductTypeHousingItem
	StoreProductTypeHousingSet
	StoreProductTypeInstrumentSkinDrum
	StoreProductTypeInstrumentSkinFlute
	StoreProductTypeInstrumentSkinGuitar
	StoreProductTypeInstrumentSkinMandolin
	StoreProductTypeInstrumentSkinUprightBass
	StoreProductTypeItemDyePack
	StoreProductTypeLoadout
	StoreProductTypeMarksOfFortune
	StoreProductTypeMount
	StoreProductTypeMountAttachment
	StoreProductTypeMountDye
	StoreProductTypeMountBear
	StoreProductTypeMountHorse
	StoreProductTypeMountLion
	StoreProductTypeMountTurkey
	StoreProductTypeMountWolf
	StoreProductTypeService
	StoreProductTypeTitle
	StoreProductTypeToken
	StoreProductTypeTokenSingle
	StoreProductTypeTokenPack
	StoreProductTypeToolSkin
	StoreProductTypeToolSkinSet
	StoreProductTypeWeaponSkinBlunderbass
	StoreProductTypeWeaponSkinBow
	StoreProductTypeWeaponSkinFireStaff
	StoreProductTypeWeaponSkinFlail
	StoreProductTypeWeaponSkinGreatAxe
	StoreProductTypeWeaponSkinGreatsword
	StoreProductTypeWeaponSkinHatchet
	StoreProductTypeWeaponSkinIceGauntlet
	StoreProductTypeWeaponSkinKiteshield
	StoreProductTypeWeaponSkinLifeStaff
	StoreProductTypeWeaponSkinMusket
	StoreProductTypeWeaponSkinRapier
	StoreProductTypeWeaponSkinShield
	StoreProductTypeWeaponSkinSpear
	StoreProductTypeWeaponSkinSword
	StoreProductTypeWeaponSkinVoidGauntlet
	StoreProductTypeWeaponSkinWarhammer
)

const ZeroCRC32 CRC32 = 0

func CRC32FromStringLower(value string) CRC32 {
	return CRC32FromBytesLowercase([]byte(value))
}

func CRC32FromBytes(bytes []byte) CRC32 {
	return crc32(bytes, false)
}

func CRC32FromBytesLowercase(bytes []byte) CRC32 {
	return crc32(bytes, true)
}

func crc32(bytes []byte, lowercaseASCII bool) CRC32 {
	crc := uint32(0xffff_ffff)
	for _, value := range bytes {
		if lowercaseASCII && value >= 'A' && value <= 'Z' {
			value += 'a' - 'A'
		}
		crc ^= uint32(value)
		for range 8 {
			if crc&1 != 0 {
				crc = 0xedb8_8320 ^ (crc >> 1)
			} else {
				crc >>= 1
			}
		}
	}
	return CRC32(crc ^ 0xffff_ffff)
}

func (value CRC32) Value() uint32 {
	return uint32(value)
}
