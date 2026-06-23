pub(super) fn single_file_source() -> String {
    let mut source = String::new();
    push_module(&mut source, uuid_module_source());
    push_module(&mut source, rtti_module_source());
    push_module(&mut source, crc_module_source());
    push_module(&mut source, math_module_source());
    push_module(&mut source, asset_module_source());
    source
}

fn push_module(out: &mut String, source: &str) {
    for line in source.lines() {
        if line.trim_start().starts_with("import ") {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out.push('\n');
}

pub(super) fn uuid_module_source() -> &'static str {
    r#"
export class Uuid {
	static readonly NIL = new Uuid("00000000-0000-0000-0000-000000000000");

	private constructor(private readonly raw: string) {}

	static parse(value: string): Uuid {
		const normalized = value.replace(/^\{|\}$/g, "").toLowerCase();
		if (!/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(normalized)) {
			throw new Error(`invalid UUID: ${value}`);
		}
		return new Uuid(normalized);
	}

	static fromBytes(bytes: Uint8Array): Uuid {
		if (bytes.length < 16) {
			throw new Error("UUID byte arrays require at least 16 bytes");
		}

		const hex = Array.from(bytes.slice(0, 16), (byte) => byte.toString(16).padStart(2, "0"));
		return new Uuid(`${hex.slice(0, 4).join("")}-${hex.slice(4, 6).join("")}-${hex.slice(6, 8).join("")}-${hex.slice(8, 10).join("")}-${hex.slice(10, 16).join("")}`);
	}

	static async createData(bytes: Uint8Array): Promise<Uuid> {
		if (bytes.length === 0) {
			return Uuid.NIL;
		}

		const input = new ArrayBuffer(bytes.byteLength);
		new Uint8Array(input).set(bytes);
		const digest = new Uint8Array(await globalThis.crypto.subtle.digest("SHA-1", input));
		const data = digest.slice(0, 16);
		data[8] = (data[8] & 0xbf) | 0x80;
		data[6] = (data[6] & 0x5f) | 0x50;
		return Uuid.fromBytes(data);
	}

	static createName(name: string): Promise<Uuid> {
		return Uuid.createData(new TextEncoder().encode(name));
	}

	static combine(lhs: Uuid, rhs: Uuid): Promise<Uuid> {
		const bytes = new Uint8Array(32);
		bytes.set(lhs.toBytes(), 0);
		bytes.set(rhs.toBytes(), 16);
		return Uuid.createData(bytes);
	}

	static async aggregateTypeIds(typeIds: Iterable<Uuid>): Promise<Uuid | undefined> {
		let acc: Uuid | undefined;
		for (const typeId of typeIds) {
			acc = acc === undefined ? typeId : await Uuid.combine(acc, typeId);
		}
		return acc;
	}

	static async aggregateTypeIdsRight(typeIds: readonly Uuid[]): Promise<Uuid | undefined> {
		const [first, ...tailIds] = typeIds;
		if (first === undefined) {
			return undefined;
		}

		const tail = await Uuid.aggregateTypeIdsRight(tailIds);
		return tail !== undefined && !tail.isNil() ? Uuid.combine(first, tail) : first;
	}

	static async specializedTemplatePrefix(templateBase: Uuid, args: readonly Uuid[]): Promise<Uuid | undefined> {
		const aggregate = await Uuid.aggregateTypeIds(args);
		return aggregate === undefined ? undefined : Uuid.combine(templateBase, aggregate);
	}

	static async specializedTemplatePostfix(templateBase: Uuid, args: readonly Uuid[]): Promise<Uuid | undefined> {
		const aggregate = await Uuid.aggregateTypeIds(args);
		return aggregate === undefined ? undefined : Uuid.combine(aggregate, templateBase);
	}

	static templateAutoTypeId(value: number): Promise<Uuid> {
		return Uuid.createName(String(value));
	}

	toBytes(): Uint8Array {
		const hex = this.raw.replace(/-/g, "");
		const bytes = new Uint8Array(16);
		for (let index = 0; index < 16; index += 1) {
			bytes[index] = Number.parseInt(hex.slice(index * 2, index * 2 + 2), 16);
		}
		return bytes;
	}

	isNil(): boolean {
		return this.equals(Uuid.NIL);
	}

	equals(other: Uuid): boolean {
		return this.raw === other.raw;
	}

	toBracedUpperString(): string {
		return `{${this.raw.toUpperCase()}}`;
	}

	toString(): string {
		return this.raw;
	}
}

export const typeIds = {
	int: Uuid.parse("72039442-eb38-4d42-a1ad-cb68f7e0eef6"),
	u8: Uuid.parse("72b9409a-7d1a-4831-9cfe-fcb3fadd3426"),
} as const;
"#
}

pub(super) fn rtti_module_source() -> &'static str {
    r#"
import { Uuid } from "./uuid.js";

export abstract class AzRtti {
	abstract readonly azRtti: Rtti;
}

export class Rtti {
	constructor(
		readonly name: string,
		readonly typeId: Uuid,
	) {}

	static fromTypeId(name: string, typeId: string): Rtti {
		return new Rtti(name, Uuid.parse(typeId));
	}
}

export type RttiTarget = (abstract new (...args: never[]) => unknown) | object;

export interface RttiRegistration<T extends RttiTarget = RttiTarget> {
	readonly target: T;
	readonly rtti: Rtti;
}

export class RttiRegistry {
	private readonly byTypeId = new Map<string, RttiRegistration>();
	private readonly byTarget = new Map<RttiTarget, RttiRegistration>();

	register<T extends RttiTarget>(target: T, rtti: Rtti): RttiRegistration<T> {
		const typeId = rtti.typeId.toString();
		const existingById = this.byTypeId.get(typeId);
		if (existingById !== undefined) {
			if (existingById.target !== target || existingById.rtti.name !== rtti.name) {
				throw new Error(`AZ RTTI type id ${typeId} already registered for ${existingById.rtti.name}`);
			}
			return existingById as RttiRegistration<T>;
		}

		const existingByTarget = this.byTarget.get(target);
		if (existingByTarget !== undefined) {
			if (!existingByTarget.rtti.typeId.equals(rtti.typeId) || existingByTarget.rtti.name !== rtti.name) {
				throw new Error(`target already registered as ${existingByTarget.rtti.name} (${existingByTarget.rtti.typeId.toString()})`);
			}
			return existingByTarget as RttiRegistration<T>;
		}

		const registration = { target, rtti } satisfies RttiRegistration<T>;
		this.byTypeId.set(typeId, registration);
		this.byTarget.set(target, registration);
		return registration;
	}

	typeFor<T extends RttiTarget>(target: T): Rtti {
		const registration = this.byTarget.get(target);
		if (registration === undefined) {
			throw new Error("target is not registered with AZ RTTI");
		}
		return registration.rtti;
	}

	lookup(typeId: Uuid | string): RttiRegistration | undefined {
		const key = typeof typeId === "string" ? Uuid.parse(typeId).toString() : typeId.toString();
		return this.byTypeId.get(key);
	}
}

export const rttiRegistry = new RttiRegistry();

export function registerType<T extends RttiTarget>(target: T, rtti: Rtti): RttiRegistration<T> {
	return rttiRegistry.register(target, rtti);
}

export function rttiFor<T extends RttiTarget>(target: T): Rtti {
	return rttiRegistry.typeFor(target);
}
"#
}

pub(super) fn crc_module_source() -> &'static str {
    r#"
export class Crc32 {
	static readonly ZERO = new Crc32(0);

	private constructor(private readonly raw: number) {}

	static from(value: number): Crc32 {
		return new Crc32(value >>> 0);
	}

	static fromStringLower(value: string): Crc32 {
		return Crc32.fromBytesLower(new TextEncoder().encode(value));
	}

	static fromBytesLower(bytes: Uint8Array): Crc32 {
		return Crc32.fromBytes(bytes, true);
	}

	static fromBytes(bytes: Uint8Array, forceLowerCase = false): Crc32 {
		let crc = 0xffffffff;
		for (const byte of bytes) {
			const folded = forceLowerCase && byte >= 65 && byte <= 90 ? byte + 32 : byte;
			crc = Crc32.octet(crc, folded);
		}
		return Crc32.from((crc ^ 0xffffffff) >>> 0);
	}

	value(): number {
		return this.raw;
	}

	private static octet(currentCrc: number, byte: number): number {
		let tableValue = (currentCrc ^ byte) & 0xff;
		for (let bit = 0; bit < 8; bit += 1) {
			tableValue = tableValue & 1 ? 0xedb88320 ^ (tableValue >>> 1) : tableValue >>> 1;
		}
		return ((currentCrc >>> 8) ^ tableValue) >>> 0;
	}
}
"#
}

pub(super) fn collection_module_source() -> &'static str {
    r#"
export type FixedArray<T, Length extends number> = T[] & {
	readonly length: Length;
};

export type FixedVector<T, _Capacity extends number> = T[];

export type BitSet<Size extends number> = FixedArray<boolean, Size>;

export class FixedBytes<Length extends number> {
	constructor(
		readonly bytes: Uint8Array,
		readonly length: Length,
	) {
		if (bytes.length !== length) {
			throw new Error(`expected ${length} bytes, got ${bytes.length}`);
		}
	}
}
"#
}

pub(super) fn math_module_source() -> &'static str {
    r#"
export interface Vector2 {
	readonly x: number;
	readonly y: number;
}

export interface Vector3 {
	readonly x: number;
	readonly y: number;
	readonly z: number;
}

export interface Vector4 {
	readonly x: number;
	readonly y: number;
	readonly z: number;
	readonly w: number;
}

export interface Quaternion {
	readonly x: number;
	readonly y: number;
	readonly z: number;
	readonly w: number;
}

export interface Transform {
	readonly basisX: Vector3;
	readonly basisY: Vector3;
	readonly basisZ: Vector3;
	readonly translation: Vector3;
}

export interface Color {
	readonly r: number;
	readonly g: number;
	readonly b: number;
	readonly a: number;
}

export type ColorF = Color;

export interface ColorB {
	readonly r: number;
	readonly g: number;
	readonly b: number;
	readonly a: number;
}
"#
}

pub(super) fn asset_module_source() -> &'static str {
    r#"
import { Uuid } from "./uuid.js";

export class AssetId {
	constructor(
		readonly guid: Uuid,
		readonly subId: number,
	) {}

	static nil(): AssetId {
		return new AssetId(Uuid.NIL, 0);
	}

	static parse(value: string): AssetId {
		const separator = value.lastIndexOf(":");
		if (separator < 0) {
			throw new Error("missing ':' separator between guid and sub_id");
		}

		const guid = Uuid.parse(value.slice(0, separator).replace(/^\{|\}$/g, ""));
		const subId = Number.parseInt(value.slice(separator + 1), 16);
		return new AssetId(guid, subId);
	}

	isNil(): boolean {
		return this.subId === 0 && this.guid.isNil();
	}

	toString(): string {
		return `${this.guid.toBracedUpperString()}:${this.subId.toString(16)}`;
	}
}

export class Asset {
	constructor(
		readonly assetId: AssetId,
		readonly assetType: Uuid,
		readonly hint?: string,
	) {}

	static empty(): Asset {
		return new Asset(AssetId.nil(), Uuid.NIL);
	}

	static fromId(assetId: AssetId, assetType: Uuid): Asset {
		return new Asset(assetId, assetType);
	}

	static fromHint(hint: string): Asset {
		return new Asset(AssetId.nil(), Uuid.NIL, hint.trim());
	}

	isNil(): boolean {
		return this.assetId.isNil();
	}

	isEmpty(): boolean {
		return this.isNil() && this.assetType.isNil() && (this.hint?.trim() ?? "") === "";
	}
}
"#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn support_sources_keep_az_uuid_and_crc_behavior_in_wrappers() {
        let uuid = uuid_module_source();
        let crc = crc_module_source();

        assert!(uuid.contains("export class Uuid"));
        assert!(uuid.contains("data[8] = (data[8] & 0xbf) | 0x80"));
        assert!(uuid.contains("static async aggregateTypeIds"));
        assert!(uuid.contains("static async aggregateTypeIdsRight"));
        assert!(uuid.contains("static async specializedTemplatePrefix"));
        let rtti = rtti_module_source();
        assert!(rtti.contains("export abstract class AzRtti"));
        assert!(rtti.contains("export class Rtti"));
        assert!(rtti.contains("export class RttiRegistry"));
        assert!(rtti.contains("export function registerType"));
        assert!(rtti.contains("static fromTypeId(name: string, typeId: string): Rtti"));
        assert!(crc.contains("export class Crc32"));
        assert!(crc.contains("static fromStringLower(value: string): Crc32"));
        assert!(crc.contains("private static octet"));
    }

    #[test]
    fn single_file_support_strips_module_imports() {
        let source = single_file_source();

        assert!(source.contains("export class Uuid"));
        assert!(source.contains("export abstract class AzRtti"));
        assert!(source.contains("export class Rtti"));
        assert!(source.contains("export function registerType"));
        assert!(source.contains("export class AssetId"));
        assert!(!source.contains("import { Uuid } from"));
    }
}
