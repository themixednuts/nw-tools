import { type Vector3 } from "../values.js";

const STREAM_TAG_BINARY = 0;
const ELEMENT_HEADER = 1 << 3;
const HAS_VALUE = 1 << 4;
const EXTRA_SIZE_FIELD = 1 << 5;
const HAS_NAME = 1 << 6;
const HAS_VERSION = 1 << 7;
const VALUE_SIZE_MASK = 0x07;

const TEXT_DECODER = new TextDecoder();

export interface ObjectStream {
  readonly version: number;
  readonly elements: readonly ObjectStreamElement[];
}

export interface ObjectStreamElement {
  readonly flags: number;
  readonly nameCrc?: number;
  readonly version?: number;
  readonly typeId: string;
  readonly data: Uint8Array;
  readonly children: readonly ObjectStreamElement[];
}

export function parseObjectStream(bytes: Uint8Array): ObjectStream {
  const reader = new ObjectStreamReader(bytes);
  const tag = reader.u8();
  if (tag !== STREAM_TAG_BINARY) {
    throw new Error(`unsupported ObjectStream tag ${tag}`);
  }
  const version = reader.u32();
  if (version !== 2 && version !== 3) {
    throw new Error(`unsupported ObjectStream version ${version}`);
  }

  const roots: ObjectStreamElement[] = [];
  const stack: ObjectStreamElement[] = [];
  while (!reader.isDone()) {
    const flags = reader.u8();
    if (flags === 0) {
      if (stack.length === 0) {
        break;
      }
      stack.pop();
      continue;
    }
    if ((flags & ELEMENT_HEADER) === 0) {
      throw new Error(`invalid ObjectStream element flags ${flags}`);
    }

    const nameCrc = (flags & HAS_NAME) !== 0 ? reader.u32() : undefined;
    const elementVersion = (flags & HAS_VERSION) !== 0 ? reader.u8() : undefined;
    const typeId = reader.uuid();
    if (version === 2) {
      reader.uuid();
    }
    const dataSize = readDataSize(reader, flags);
    const data = reader.bytes(dataSize);
    const element: ObjectStreamElement = {
      flags,
      nameCrc,
      version: elementVersion,
      typeId,
      data,
      children: [],
    };

    const parent = stack.at(-1);
    if (parent === undefined) {
      roots.push(element);
    } else {
      (parent.children as ObjectStreamElement[]).push(element);
    }
    stack.push(element);
  }
  if (stack.length !== 0) {
    throw new Error("ObjectStream ended before all elements were closed");
  }
  return { version, elements: roots };
}

export function objectStreamString(element: ObjectStreamElement): string {
  return TEXT_DECODER.decode(element.data);
}

export function objectStreamBool(element: ObjectStreamElement): boolean {
  requireLength(element, 1);
  return element.data[0] !== 0;
}

export function objectStreamU8(element: ObjectStreamElement): number {
  requireLength(element, 1);
  return element.data[0];
}

export function objectStreamI32(element: ObjectStreamElement): number {
  requireLength(element, 4);
  return dataView(element).getInt32(0, false);
}

export function objectStreamU32(element: ObjectStreamElement): number {
  requireLength(element, 4);
  return dataView(element).getUint32(0, false);
}

export function objectStreamF32(element: ObjectStreamElement): number {
  requireLength(element, 4);
  return dataView(element).getFloat32(0, false);
}

export function objectStreamVec3(element: ObjectStreamElement): Vector3 {
  requireLength(element, 12);
  const view = dataView(element);
  return {
    x: view.getFloat32(0, false),
    y: view.getFloat32(4, false),
    z: view.getFloat32(8, false),
  };
}

export function singleObjectStreamRoot(
  stream: ObjectStream,
  expectedTypeId: string,
): ObjectStreamElement {
  if (stream.elements.length !== 1) {
    throw new Error(`expected one ObjectStream root, found ${stream.elements.length}`);
  }
  const root = stream.elements[0];
  requireObjectStreamType(root, expectedTypeId);
  return root;
}

export function requireObjectStreamType(
  element: ObjectStreamElement,
  expectedTypeId: string,
): void {
  if (element.typeId !== expectedTypeId.toLowerCase()) {
    throw new Error(`expected ObjectStream type ${expectedTypeId}, found ${element.typeId}`);
  }
}

export function childByNameCrc(
  element: ObjectStreamElement,
  nameCrc: number,
): ObjectStreamElement | undefined {
  return element.children.find((child) => child.nameCrc === nameCrc);
}

export function requiredChildByNameCrc(
  element: ObjectStreamElement,
  nameCrc: number,
): ObjectStreamElement {
  const child = childByNameCrc(element, nameCrc);
  if (child === undefined) {
    throw new Error(`ObjectStream element ${element.typeId} is missing field CRC ${nameCrc}`);
  }
  return child;
}

function readDataSize(reader: ObjectStreamReader, flags: number): number {
  if ((flags & HAS_VALUE) === 0) {
    return 0;
  }
  const inlineSize = flags & VALUE_SIZE_MASK;
  if ((flags & EXTRA_SIZE_FIELD) === 0) {
    return inlineSize;
  }
  switch (inlineSize) {
    case 1:
      return reader.u8();
    case 2:
      return reader.u16();
    case 4:
      return reader.u32();
    default:
      throw new Error(`unsupported ObjectStream value-size width ${inlineSize}`);
  }
}

function requireLength(element: ObjectStreamElement, expected: number): void {
  if (element.data.length !== expected) {
    throw new Error(
      `ObjectStream element ${element.typeId} has ${element.data.length} bytes, expected ${expected}`,
    );
  }
}

function dataView(element: ObjectStreamElement): DataView {
  return new DataView(
    element.data.buffer,
    element.data.byteOffset,
    element.data.byteLength,
  );
}

class ObjectStreamReader {
  private offset = 0;

  constructor(private readonly bytes_: Uint8Array) {}

  isDone(): boolean {
    return this.offset >= this.bytes_.length;
  }

  u8(): number {
    this.require(1);
    return this.bytes_[this.offset++];
  }

  u16(): number {
    this.require(2);
    const value = new DataView(this.bytes_.buffer, this.bytes_.byteOffset + this.offset, 2).getUint16(0, false);
    this.offset += 2;
    return value;
  }

  u32(): number {
    this.require(4);
    const value = new DataView(this.bytes_.buffer, this.bytes_.byteOffset + this.offset, 4).getUint32(0, false);
    this.offset += 4;
    return value;
  }

  uuid(): string {
    const bytes = this.bytes(16);
    const hex = Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0"));
    return `${hex.slice(0, 4).join("")}-${hex.slice(4, 6).join("")}-${hex.slice(6, 8).join("")}-${hex.slice(8, 10).join("")}-${hex.slice(10, 16).join("")}`;
  }

  bytes(length: number): Uint8Array {
    this.require(length);
    const bytes = this.bytes_.slice(this.offset, this.offset + length);
    this.offset += length;
    return bytes;
  }

  private require(length: number): void {
    if (this.offset + length > this.bytes_.length) {
      throw new Error("ObjectStream ended unexpectedly");
    }
  }
}
