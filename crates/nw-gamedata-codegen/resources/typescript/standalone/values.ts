const UUID_PATTERN = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;

export class Uuid {
  static readonly NIL = new Uuid("00000000-0000-0000-0000-000000000000");

  private constructor(private readonly value: string) {}

  static parse(input: string): Uuid {
    const value = input.replace(/^\{|\}$/g, "").toLowerCase();
    if (!UUID_PATTERN.test(value)) {
      throw new TypeError(`invalid UUID: ${input}`);
    }
    return new Uuid(value);
  }

  static fromBytes(bytes: Uint8Array): Uuid {
    if (bytes.byteLength !== 16) {
      throw new RangeError(`UUIDs require exactly 16 bytes, got ${bytes.byteLength}`);
    }
    const hex = Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0"));
    return new Uuid(
      `${hex.slice(0, 4).join("")}-${hex.slice(4, 6).join("")}-${hex.slice(6, 8).join("")}-${hex.slice(8, 10).join("")}-${hex.slice(10).join("")}`
    );
  }

  equals(other: Uuid): boolean {
    return this.value === other.value;
  }

  isNil(): boolean {
    return this.equals(Uuid.NIL);
  }

  toString(): string {
    return this.value;
  }

  toJSON(): string {
    return this.value;
  }
}

declare const CRC32_BRAND: unique symbol;

export type Crc32 = number & { readonly [CRC32_BRAND]: "Crc32" };

export const Crc32 = {
  ZERO: 0 as Crc32,

  from(value: number): Crc32 {
    if (!Number.isInteger(value) || value < 0 || value > 0xffff_ffff) {
      throw new RangeError(`CRC32 must be an unsigned 32-bit integer, got ${value}`);
    }
    return value as Crc32;
  },

  fromStringLower(value: string): Crc32 {
    return Crc32.fromBytesLowercase(new TextEncoder().encode(value));
  },

  fromBytes(bytes: Uint8Array): Crc32 {
    return crc32(bytes, false);
  },

  fromBytesLowercase(bytes: Uint8Array): Crc32 {
    return crc32(bytes, true);
  },
} as const;

function crc32(bytes: Uint8Array, lowercaseAscii: boolean): Crc32 {
  let crc = 0xffff_ffff;
  for (const byte of bytes) {
    const value = lowercaseAscii && byte >= 65 && byte <= 90 ? byte + 32 : byte;
    crc ^= value;
    for (let bit = 0; bit < 8; bit += 1) {
      crc = crc & 1 ? 0xedb8_8320 ^ (crc >>> 1) : crc >>> 1;
    }
  }
  return Crc32.from((crc ^ 0xffff_ffff) >>> 0);
}

export class AssetId {
  constructor(
    readonly guid: Uuid,
    readonly subId: number
  ) {
    if (!Number.isInteger(subId) || subId < 0 || subId > 0xffff_ffff) {
      throw new RangeError(`asset sub ID must be an unsigned 32-bit integer, got ${subId}`);
    }
  }

  isNil(): boolean {
    return this.guid.isNil() && this.subId === 0;
  }
}

export type AssetType = Uuid;

export interface AssetReference {
  readonly id: AssetId;
  readonly assetType: AssetType;
  readonly hint: string;
}

export interface Vector3 {
  readonly x: number;
  readonly y: number;
  readonly z: number;
}

export const Vector3 = {
  ZERO: Object.freeze<Vector3>({ x: 0, y: 0, z: 0 }),
} as const;
