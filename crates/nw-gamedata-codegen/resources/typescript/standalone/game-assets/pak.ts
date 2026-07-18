import { existsSync } from "node:fs";
import { open, readdir, realpath, type FileHandle } from "node:fs/promises";
import { delimiter, dirname, isAbsolute, join, relative, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { inflateRawSync, inflateSync } from "node:zlib";

import { ASSET_CATALOG_PATH, type AssetCatalog, normalizeVirtualPath, parseRascCatalog } from "./catalog.js";

const ZIP_LOCAL_FILE_HEADER_SIGNATURE = 0x04034b50;
const ZIP_CENTRAL_DIRECTORY_SIGNATURE = 0x02014b50;
const ZIP_END_OF_CENTRAL_DIRECTORY_SIGNATURE = 0x06054b50;
const ZIP64_END_OF_CENTRAL_DIRECTORY_SIGNATURE = 0x06064b50;
const ZIP64_END_OF_CENTRAL_DIRECTORY_LOCATOR_SIGNATURE = 0x07064b50;
const ZIP_LOCAL_FILE_HEADER_FIXED_LENGTH = 30;
const ZIP_CENTRAL_DIRECTORY_FIXED_LENGTH = 46;
const ZIP_END_OF_CENTRAL_DIRECTORY_FIXED_LENGTH = 22;
const ZIP64_END_OF_CENTRAL_DIRECTORY_FIXED_LENGTH = 56;
const ZIP64_END_OF_CENTRAL_DIRECTORY_LOCATOR_LENGTH = 20;
const ZIP_MAX_COMMENT_LENGTH = 0xffff;
const ZIP64_U16_SENTINEL = 0xffff;
const ZIP64_U32_SENTINEL = 0xffffffff;
const ZIP64_EXTRA_FIELD_ID = 0x0001;
const AZCS_ZLIB = 0x73887d3a;
const AZCS_ZSTD = 0x72fd505e;
const OODLE_DLL_NAMES = ["oo2core_9_win64.dll", "oo2core_8_win64.dll"] as const;

const UTF8_DECODER = new TextDecoder();
const dynamicImport = (specifier: string): Promise<unknown> => import(specifier);

enum PakCompressionMethod {
  Stored = 0,
  Deflated = 8,
  Oodle = 15
}

interface PakEntryInfo {
  readonly name: string;
  readonly index: number;
  readonly compressedSize: number;
  readonly uncompressedSize: number;
  readonly compressionMethod: number;
  readonly localHeaderOffset: number;
}

export interface AssetLoaderOptions {
  readonly oodleLibrary?: string;
}

type ArchiveReadOptions = AssetLoaderOptions;

interface MountedPakArchive {
  readonly mountRoot: string;
  readonly archive: PakArchive;
}

interface PakEntryRef {
  readonly archive: PakArchive;
  readonly entry: PakEntryInfo;
}

interface CentralDirectoryPlan {
  readonly entryCount?: number;
  readonly centralDirectoryOffset: number;
  readonly centralDirectorySize: number;
}

interface PakArchiveSource {
  readonly bytes?: Uint8Array;
  readonly file?: FileHandle;
}

interface BunFfiModule {
  readonly dlopen: (
    path: string,
    symbols: Record<string, { readonly args: readonly unknown[]; readonly returns: unknown }>
  ) => { readonly symbols: Record<string, OodleDecompressor> };
  readonly FFIType: Record<string, unknown>;
}

type OodleDecompressor = (
  input: Uint8Array,
  inputLength: number,
  output: Uint8Array,
  outputLength: number,
  fuzzSafe: number,
  checkCrc: number,
  verbosity: number,
  decodeBufferBase: number,
  decodeBufferSize: number,
  fpCallback: number,
  callbackUserData: number,
  decoderMemory: number,
  decoderMemorySize: number,
  threadPhase: number
) => number;

interface LoadedOodle {
  readonly path: string;
  readonly decompress: OodleDecompressor;
}

let cachedOodleKey: string | undefined;
let cachedOodle: Promise<LoadedOodle> | undefined;

class PakArchive {
  readonly path: string;
  readonly entries: readonly PakEntryInfo[];

  private readonly source: PakArchiveSource;
  private readonly byName: ReadonlyMap<string, PakEntryInfo>;

  private constructor(path: string, entries: readonly PakEntryInfo[], source: PakArchiveSource) {
    this.path = path;
    this.entries = entries;
    this.source = source;
    const byName = new Map<string, PakEntryInfo>();
    for (const entry of entries) {
      byName.set(entry.name, entry);
      byName.set(normalizeArchivePath(entry.name), entry);
    }
    this.byName = byName;
  }

  static fromBytes(path: string, bytes: Uint8Array): PakArchive {
    const entries = parsePakEntries(path, bytes);
    return new PakArchive(path, entries, { bytes });
  }

  static async open(path: string): Promise<PakArchive> {
    const resolvedPath = await realpath(path);
    const file = await open(resolvedPath, "r");
    try {
      const entries = await parsePakEntriesFromFile(resolvedPath, file);
      return new PakArchive(resolvedPath, entries, { file });
    } catch (error) {
      try {
        await file.close();
      } catch (closeError) {
        throw new AggregateError([error, closeError], `failed to open pak archive ${resolvedPath} and close its file handle`);
      }
      throw error;
    }
  }

  async close(): Promise<void> {
    await this.source.file?.close();
  }

  entry(name: string): PakEntryInfo | undefined {
    return this.byName.get(name) ?? this.byName.get(normalizeArchivePath(name));
  }

  async read(name: string, options: ArchiveReadOptions = {}): Promise<Uint8Array> {
    const entry = this.entry(name);
    if (entry === undefined) {
      throw new Error(`pak entry not found in ${this.path}: ${name}`);
    }
    return this.readEntry(entry, options);
  }

  async readEntry(entry: PakEntryInfo, options: ArchiveReadOptions = {}): Promise<Uint8Array> {
    const localHeader = await this.readRange(entry.localHeaderOffset, ZIP_LOCAL_FILE_HEADER_FIXED_LENGTH, `${this.path}:${entry.name} local header`);
    const signature = readUint32(localHeader, 0, `${this.path}:${entry.name} local header signature`);
    if (signature !== ZIP_LOCAL_FILE_HEADER_SIGNATURE) {
      throw new Error(`corrupt local header for ${entry.name} in ${this.path}`);
    }
    const nameLength = readUint16(localHeader, 26, `${this.path}:${entry.name} local name length`);
    const extraLength = readUint16(localHeader, 28, `${this.path}:${entry.name} local extra length`);
    const compressedStart = checkedSum(
      entry.localHeaderOffset,
      ZIP_LOCAL_FILE_HEADER_FIXED_LENGTH + nameLength + extraLength,
      `${this.path}:${entry.name} compressed payload offset`
    );
    const compressed = await this.readRange(compressedStart, entry.compressedSize, `${this.path}:${entry.name} compressed payload`);
    return peelAzcs(await decompressPakEntry(entry, compressed, options));
  }

  private async readRange(offset: number, length: number, label: string): Promise<Uint8Array> {
    const { bytes, file } = this.source;
    if (bytes !== undefined) {
      assertRange(bytes, offset, length, label);
      return bytes.subarray(offset, offset + length);
    }
    if (file === undefined) {
      throw new Error(`pak archive ${this.path} is closed`);
    }
    return readFileRange(file, offset, length, label);
  }
}

class UnsupportedPakCompressionError extends Error {
  readonly entryName: string;
  readonly method: number;

  constructor(entryName: string, method: number) {
    super(`pak entry ${entryName} uses unsupported compression method ${method}`);
    this.name = "UnsupportedPakCompressionError";
    this.entryName = entryName;
    this.method = method;
  }
}

async function openPakArchive(path: string): Promise<PakArchive> {
  return PakArchive.open(path);
}

export class AssetLoader {
  readonly catalog: AssetCatalog;
  private readonly mountedArchives: readonly MountedPakArchive[];
  private readonly entriesByPath: ReadonlyMap<string, PakEntryRef>;
  private readonly options: ArchiveReadOptions;

  private constructor(catalog: AssetCatalog, mountedArchives: readonly MountedPakArchive[], entriesByPath: ReadonlyMap<string, PakEntryRef>, options: ArchiveReadOptions) {
    this.catalog = catalog;
    this.mountedArchives = mountedArchives;
    this.entriesByPath = entriesByPath;
    this.options = options;
  }

  static async open(assetRoot: string, options: AssetLoaderOptions = {}): Promise<AssetLoader> {
    const root = await realpath(assetRoot);
    const pakPaths = await canonicalPakPaths(await collectPakPaths(root));
    pakPaths.sort((left, right) => left.localeCompare(right));
    if (pakPaths.length === 0) {
      throw new Error(`no .pak files found under ${root}`);
    }

    const mountedArchives: MountedPakArchive[] = [];
    try {
      const entriesByPath = new Map<string, PakEntryRef>();
      const claimedPaths = new Set<string>();

      for (const pakPath of pakPaths) {
        const archive = await openPakArchive(pakPath);
        const mountRoot = pakMountRoot(root, pakPath);
        mountedArchives.push({ mountRoot, archive });

        for (const entry of archive.entries) {
          const path = normalizeVirtualPath(mountedEntryPath(mountRoot, entry.name));
          if (claimedPaths.has(path)) {
            continue;
          }
          claimedPaths.add(path);
          entriesByPath.set(path, { archive, entry });
        }
      }

      const catalog = await loadCatalogFromPaks(mountedArchives);
      return new AssetLoader(catalog, mountedArchives, entriesByPath, options);
    } catch (error) {
      try {
        await closeMountedArchives(mountedArchives);
      } catch (closeError) {
        throw new AggregateError([error, closeError], `failed to open asset loader and close opened pak archives`);
      }
      throw error;
    }
  }

  async close(): Promise<void> {
    await closeMountedArchives(this.mountedArchives);
  }

  async [Symbol.asyncDispose](): Promise<void> {
    await this.close();
  }

  async read(path: string): Promise<Uint8Array> {
    const located = this.entry(path);
    if (located === undefined) {
      throw new Error(`asset ${path} was not present in selected paks`);
    }
    return located.archive.readEntry(located.entry, this.options);
  }

  private entry(path: string): PakEntryRef | undefined {
    const normalized = normalizeVirtualPath(path);
    const exact = this.entriesByPath.get(normalized);
    if (exact !== undefined) {
      return exact;
    }
    for (const [candidate, located] of this.entriesByPath) {
      if (candidate.endsWith(`/${normalized}`)) {
        return located;
      }
    }
    return undefined;
  }

}

async function loadCatalogFromPaks(mountedArchives: readonly MountedPakArchive[]): Promise<AssetCatalog> {
  for (const mounted of mountedArchives) {
    const entry = mounted.archive.entry(ASSET_CATALOG_PATH);
    if (entry !== undefined) {
      return parseRascCatalog(await mounted.archive.readEntry(entry));
    }
  }
  throw new Error(`asset catalog ${ASSET_CATALOG_PATH} was not found in selected paks`);
}

async function collectPakPaths(root: string): Promise<string[]> {
  const out: string[] = [];
  await collectPakPathsInto(root, out);
  out.sort((left, right) => left.localeCompare(right));
  return out;
}

async function collectPakPathsInto(dir: string, out: string[]): Promise<void> {
  const entries = await readdir(dir, { withFileTypes: true });
  entries.sort((left, right) => left.name.localeCompare(right.name));
  for (const entry of entries) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) {
      await collectPakPathsInto(path, out);
      continue;
    }
    if (entry.isFile() && entry.name.toLowerCase().endsWith(".pak")) {
      out.push(path);
    }
  }
}

async function canonicalPakPaths(paths: readonly string[]): Promise<string[]> {
  const out: string[] = [];
  for (const path of paths) {
    out.push(await realpath(path));
  }
  return out;
}

async function closeMountedArchives(mountedArchives: readonly MountedPakArchive[]): Promise<void> {
  const errors: unknown[] = [];
  for (const mounted of mountedArchives) {
    try {
      await mounted.archive.close();
    } catch (error) {
      errors.push(error);
    }
  }
  if (errors.length === 1) {
    throw errors[0];
  }
  if (errors.length > 1) {
    throw new AggregateError(errors, "failed to close pak archives");
  }
}

function pakMountRoot(assetRoot: string, pakPath: string): string {
  const relativePakPath = relative(assetRoot, pakPath);
  if (isOutsideRelativePath(relativePakPath)) {
    throw new Error(`pak path ${pakPath} is outside asset root ${assetRoot}`);
  }
  const relativePakDir = dirname(relativePakPath);
  if (relativePakDir === ".") {
    return "";
  }
  return normalizeVirtualPath(relativePakDir);
}

function isOutsideRelativePath(path: string): boolean {
  return path === ".." || path.startsWith(`..${sep}`) || isAbsolute(path);
}

function mountedEntryPath(mountRoot: string, entry: string): string {
  const normalizedEntry = entry.replaceAll("\\", "/").replace(/^\/+/, "");
  if (mountRoot.length === 0) {
    return normalizedEntry;
  }
  if (normalizedEntry.length === 0) {
    return mountRoot;
  }
  return `${mountRoot}/${normalizedEntry}`;
}

function normalizeArchivePath(path: string): string {
  let normalized = path.replaceAll("\\", "/").trim().toLowerCase();
  while (normalized.startsWith("./")) {
    normalized = normalized.slice(2);
  }
  return normalized.replace(/^\/+/, "");
}

function parsePakEntries(path: string, bytes: Uint8Array): PakEntryInfo[] {
  const eocdOffset = findEndOfCentralDirectory(path, bytes, bytes.byteLength);
  const plan = centralDirectoryPlanFromEocd(path, bytes, eocdOffset, bytes.byteLength);
  assertRange(bytes, plan.centralDirectoryOffset, plan.centralDirectorySize, `${path} central directory`);
  return parseCentralDirectory(
    path,
    bytes.subarray(plan.centralDirectoryOffset, plan.centralDirectoryOffset + plan.centralDirectorySize),
    plan.entryCount
  );
}

async function parsePakEntriesFromFile(path: string, file: FileHandle): Promise<PakEntryInfo[]> {
  const stat = await file.stat();
  const archiveSize = safeNumber(BigInt(stat.size), `${path} size`);
  const tailLength = Math.min(archiveSize, ZIP_MAX_COMMENT_LENGTH + ZIP_END_OF_CENTRAL_DIRECTORY_FIXED_LENGTH);
  const tailOffset = archiveSize - tailLength;
  const tail = await readFileRange(file, tailOffset, tailLength, `${path} EOCD search tail`);
  const eocdTailOffset = findEndOfCentralDirectory(path, tail, archiveSize);
  const eocdOffset = tailOffset + eocdTailOffset;
  const plan = await centralDirectoryPlanFromFileEocd(path, file, tail, eocdTailOffset, eocdOffset, archiveSize);
  const centralDirectory = await readFileRange(file, plan.centralDirectoryOffset, plan.centralDirectorySize, `${path} central directory`);
  return parseCentralDirectory(path, centralDirectory, plan.entryCount);
}

function centralDirectoryPlanFromEocd(path: string, bytes: Uint8Array, eocdOffset: number, archiveSize: number): CentralDirectoryPlan {
  const diskNumber = readUint16(bytes, eocdOffset + 4, `${path} EOCD disk number`);
  const centralDirectoryDisk = readUint16(bytes, eocdOffset + 6, `${path} EOCD central-directory disk`);
  const entryCountOnDisk = readUint16(bytes, eocdOffset + 8, `${path} EOCD disk entry count`);
  const entryCount = readUint16(bytes, eocdOffset + 10, `${path} EOCD entry count`);
  const centralDirectorySize = readUint32(bytes, eocdOffset + 12, `${path} EOCD central-directory size`);
  const centralDirectoryOffset = readUint32(bytes, eocdOffset + 16, `${path} EOCD central-directory offset`);
  if (centralDirectorySize !== ZIP64_U32_SENTINEL && centralDirectoryOffset !== ZIP64_U32_SENTINEL) {
    validateCentralDirectoryLocation(path, diskNumber, centralDirectoryDisk);
    if (entryCountOnDisk !== ZIP64_U16_SENTINEL && entryCount !== ZIP64_U16_SENTINEL) {
      validateCentralDirectoryEntryCount(path, entryCountOnDisk, entryCount);
      return { entryCount, centralDirectoryOffset, centralDirectorySize };
    }
    return { centralDirectoryOffset, centralDirectorySize };
  }

  const locatorOffset = eocdOffset - ZIP64_END_OF_CENTRAL_DIRECTORY_LOCATOR_LENGTH;
  assertRange(bytes, locatorOffset, ZIP64_END_OF_CENTRAL_DIRECTORY_LOCATOR_LENGTH, `${path} ZIP64 EOCD locator`);
  const locator = bytes.subarray(locatorOffset, locatorOffset + ZIP64_END_OF_CENTRAL_DIRECTORY_LOCATOR_LENGTH);
  return centralDirectoryPlanFromZip64(path, bytes, locator, archiveSize);
}

async function centralDirectoryPlanFromFileEocd(
  path: string,
  file: FileHandle,
  eocdBytes: Uint8Array,
  eocdOffsetInBytes: number,
  eocdOffset: number,
  archiveSize: number
): Promise<CentralDirectoryPlan> {
  const diskNumber = readUint16(eocdBytes, eocdOffsetInBytes + 4, `${path} EOCD disk number`);
  const centralDirectoryDisk = readUint16(eocdBytes, eocdOffsetInBytes + 6, `${path} EOCD central-directory disk`);
  const entryCountOnDisk = readUint16(eocdBytes, eocdOffsetInBytes + 8, `${path} EOCD disk entry count`);
  const entryCount = readUint16(eocdBytes, eocdOffsetInBytes + 10, `${path} EOCD entry count`);
  const centralDirectorySize = readUint32(eocdBytes, eocdOffsetInBytes + 12, `${path} EOCD central-directory size`);
  const centralDirectoryOffset = readUint32(eocdBytes, eocdOffsetInBytes + 16, `${path} EOCD central-directory offset`);
  if (centralDirectorySize !== ZIP64_U32_SENTINEL && centralDirectoryOffset !== ZIP64_U32_SENTINEL) {
    validateCentralDirectoryLocation(path, diskNumber, centralDirectoryDisk);
    if (entryCountOnDisk !== ZIP64_U16_SENTINEL && entryCount !== ZIP64_U16_SENTINEL) {
      validateCentralDirectoryEntryCount(path, entryCountOnDisk, entryCount);
      return { entryCount, centralDirectoryOffset, centralDirectorySize };
    }
    return { centralDirectoryOffset, centralDirectorySize };
  }

  const locatorOffset = eocdOffset - ZIP64_END_OF_CENTRAL_DIRECTORY_LOCATOR_LENGTH;
  const locator = await readFileRange(file, locatorOffset, ZIP64_END_OF_CENTRAL_DIRECTORY_LOCATOR_LENGTH, `${path} ZIP64 EOCD locator`);
  const zip64EocdOffset = safeNumber(readUint64(locator, 8, `${path} ZIP64 EOCD offset`), `${path} ZIP64 EOCD offset`);
  const zip64Eocd = await readFileRange(file, zip64EocdOffset, ZIP64_END_OF_CENTRAL_DIRECTORY_FIXED_LENGTH, `${path} ZIP64 EOCD`);
  return centralDirectoryPlanFromZip64(path, zip64Eocd, locator, archiveSize);
}

function centralDirectoryPlanFromZip64(path: string, zip64EocdSource: Uint8Array, locator: Uint8Array, archiveSize: number): CentralDirectoryPlan {
  const locatorSignature = readUint32(locator, 0, `${path} ZIP64 EOCD locator signature`);
  if (locatorSignature !== ZIP64_END_OF_CENTRAL_DIRECTORY_LOCATOR_SIGNATURE) {
    throw new Error(`ZIP64 EOCD locator not found in ${path}`);
  }
  const locatorDisk = readUint32(locator, 4, `${path} ZIP64 EOCD locator disk`);
  const totalDisks = readUint32(locator, 16, `${path} ZIP64 EOCD locator total disks`);
  if (locatorDisk !== 0 || totalDisks !== 1) {
    throw new Error(`multi-disk ZIP64 paks are unsupported: ${path}`);
  }
  const zip64EocdOffset = safeNumber(readUint64(locator, 8, `${path} ZIP64 EOCD offset`), `${path} ZIP64 EOCD offset`);
  const zip64Eocd = zip64EocdSource.byteLength === ZIP64_END_OF_CENTRAL_DIRECTORY_FIXED_LENGTH ? zip64EocdSource : zip64EocdSource.subarray(zip64EocdOffset);
  assertRange(zip64Eocd, 0, ZIP64_END_OF_CENTRAL_DIRECTORY_FIXED_LENGTH, `${path} ZIP64 EOCD`);
  const signature = readUint32(zip64Eocd, 0, `${path} ZIP64 EOCD signature`);
  if (signature !== ZIP64_END_OF_CENTRAL_DIRECTORY_SIGNATURE) {
    throw new Error(`ZIP64 EOCD record not found in ${path}`);
  }
  const diskNumber = readUint32(zip64Eocd, 16, `${path} ZIP64 EOCD disk number`);
  const centralDirectoryDisk = readUint32(zip64Eocd, 20, `${path} ZIP64 EOCD central-directory disk`);
  if (diskNumber !== 0 || centralDirectoryDisk !== 0) {
    throw new Error(`multi-disk ZIP64 paks are unsupported: ${path}`);
  }
  const entryCountOnDisk = safeNumber(readUint64(zip64Eocd, 24, `${path} ZIP64 EOCD disk entry count`), `${path} ZIP64 EOCD disk entry count`);
  const entryCount = safeNumber(readUint64(zip64Eocd, 32, `${path} ZIP64 EOCD entry count`), `${path} ZIP64 EOCD entry count`);
  if (entryCountOnDisk !== entryCount) {
    throw new Error(`split ZIP64 central-directory entry count in ${path}`);
  }
  const centralDirectorySize = safeNumber(readUint64(zip64Eocd, 40, `${path} ZIP64 EOCD central-directory size`), `${path} ZIP64 EOCD central-directory size`);
  const centralDirectoryOffset = safeNumber(readUint64(zip64Eocd, 48, `${path} ZIP64 EOCD central-directory offset`), `${path} ZIP64 EOCD central-directory offset`);
  if (centralDirectoryOffset + centralDirectorySize > archiveSize) {
    throw new Error(`ZIP64 central-directory range exceeds archive size in ${path}`);
  }
  return { entryCount, centralDirectoryOffset, centralDirectorySize };
}

function validateCentralDirectoryLocation(path: string, diskNumber: number, centralDirectoryDisk: number): void {
  if (diskNumber !== 0 || centralDirectoryDisk !== 0) {
    throw new Error(`multi-disk zip paks are unsupported: ${path}`);
  }
}

function validateCentralDirectoryEntryCount(path: string, entryCountOnDisk: number, entryCount: number): void {
  if (entryCountOnDisk !== entryCount) {
    throw new Error(`split central-directory entry count in ${path}`);
  }
}

function parseCentralDirectory(path: string, centralDirectory: Uint8Array, entryCount: number | undefined): PakEntryInfo[] {
  let cursor = 0;
  const entries: PakEntryInfo[] = [];
  for (let index = 0; entryCount === undefined ? cursor < centralDirectory.byteLength : index < entryCount; index += 1) {
    assertRange(centralDirectory, cursor, ZIP_CENTRAL_DIRECTORY_FIXED_LENGTH, `${path} central-directory header ${index}`);
    const signature = readUint32(centralDirectory, cursor, `${path} central-directory signature`);
    if (signature !== ZIP_CENTRAL_DIRECTORY_SIGNATURE) {
      throw new Error(`corrupt central-directory header ${index} in ${path}`);
    }
    const compressionMethod = readUint16(centralDirectory, cursor + 10, `${path} compression method`);
    let compressedSize = readUint32(centralDirectory, cursor + 20, `${path} compressed size`);
    let uncompressedSize = readUint32(centralDirectory, cursor + 24, `${path} uncompressed size`);
    const nameLength = readUint16(centralDirectory, cursor + 28, `${path} file name length`);
    const extraLength = readUint16(centralDirectory, cursor + 30, `${path} extra length`);
    const commentLength = readUint16(centralDirectory, cursor + 32, `${path} comment length`);
    let localHeaderOffset = readUint32(centralDirectory, cursor + 42, `${path} local header offset`);
    const nameOffset = cursor + ZIP_CENTRAL_DIRECTORY_FIXED_LENGTH;
    const extraOffset = nameOffset + nameLength;
    const recordLength = ZIP_CENTRAL_DIRECTORY_FIXED_LENGTH + nameLength + extraLength + commentLength;
    assertRange(centralDirectory, cursor, recordLength, `${path} central-directory record ${index}`);
    if (compressedSize === ZIP64_U32_SENTINEL || uncompressedSize === ZIP64_U32_SENTINEL || localHeaderOffset === ZIP64_U32_SENTINEL) {
      const zip64 = zip64EntryValues(
        path,
        index,
        centralDirectory.subarray(extraOffset, extraOffset + extraLength),
        compressedSize === ZIP64_U32_SENTINEL,
        uncompressedSize === ZIP64_U32_SENTINEL,
        localHeaderOffset === ZIP64_U32_SENTINEL
      );
      compressedSize = zip64.compressedSize ?? compressedSize;
      uncompressedSize = zip64.uncompressedSize ?? uncompressedSize;
      localHeaderOffset = zip64.localHeaderOffset ?? localHeaderOffset;
    }
    const name = UTF8_DECODER.decode(centralDirectory.subarray(nameOffset, nameOffset + nameLength));
    entries.push({
      name,
      index,
      compressedSize,
      uncompressedSize,
      compressionMethod,
      localHeaderOffset
    });
    cursor += recordLength;
  }
  if (cursor !== centralDirectory.byteLength) {
    throw new Error(`central-directory cursor mismatch in ${path}: ${cursor} vs ${centralDirectory.byteLength}`);
  }
  return entries;
}

function zip64EntryValues(
  path: string,
  index: number,
  extra: Uint8Array,
  needsCompressedSize: boolean,
  needsUncompressedSize: boolean,
  needsLocalHeaderOffset: boolean
): { compressedSize?: number; uncompressedSize?: number; localHeaderOffset?: number } {
  let cursor = 0;
  while (cursor + 4 <= extra.byteLength) {
    const headerId = readUint16(extra, cursor, `${path} ZIP64 extra ${index} header id`);
    const dataSize = readUint16(extra, cursor + 2, `${path} ZIP64 extra ${index} data size`);
    const dataOffset = cursor + 4;
    assertRange(extra, dataOffset, dataSize, `${path} ZIP64 extra ${index} data`);
    if (headerId === ZIP64_EXTRA_FIELD_ID) {
      let valueOffset = dataOffset;
      let uncompressedSize: number | undefined;
      let compressedSize: number | undefined;
      let localHeaderOffset: number | undefined;
      if (needsUncompressedSize) {
        uncompressedSize = safeNumber(readUint64(extra, valueOffset, `${path} ZIP64 uncompressed size`), `${path} ZIP64 uncompressed size`);
        valueOffset += 8;
      }
      if (needsCompressedSize) {
        compressedSize = safeNumber(readUint64(extra, valueOffset, `${path} ZIP64 compressed size`), `${path} ZIP64 compressed size`);
        valueOffset += 8;
      }
      if (needsLocalHeaderOffset) {
        localHeaderOffset = safeNumber(readUint64(extra, valueOffset, `${path} ZIP64 local header offset`), `${path} ZIP64 local header offset`);
      }
      return { compressedSize, uncompressedSize, localHeaderOffset };
    }
    cursor = dataOffset + dataSize;
  }
  throw new Error(`ZIP64 extra field missing for ${path} entry ${index}`);
}

function findEndOfCentralDirectory(path: string, bytes: Uint8Array, archiveSize: number): number {
  const minOffset = Math.max(0, bytes.byteLength - ZIP_MAX_COMMENT_LENGTH - ZIP_END_OF_CENTRAL_DIRECTORY_FIXED_LENGTH);
  const archiveTailOffset = archiveSize - bytes.byteLength;
  for (let offset = bytes.byteLength - ZIP_END_OF_CENTRAL_DIRECTORY_FIXED_LENGTH; offset >= minOffset; offset -= 1) {
    if (readUint32(bytes, offset, `${path} EOCD probe`) !== ZIP_END_OF_CENTRAL_DIRECTORY_SIGNATURE) {
      continue;
    }
    const commentLength = readUint16(bytes, offset + 20, `${path} EOCD comment length`);
    if (archiveTailOffset + offset + ZIP_END_OF_CENTRAL_DIRECTORY_FIXED_LENGTH + commentLength === archiveSize) {
      return offset;
    }
  }
  throw new Error(`zip end-of-central-directory not found in ${path}`);
}

async function decompressPakEntry(entry: PakEntryInfo, compressed: Uint8Array, options: ArchiveReadOptions): Promise<Uint8Array> {
  switch (entry.compressionMethod) {
    case PakCompressionMethod.Stored:
      return compressed;
    case PakCompressionMethod.Deflated:
      return compressed[0] === 0x78 && compressed[1] === 0xda ? inflateSync(compressed) : inflateRawSync(compressed);
    case PakCompressionMethod.Oodle:
      return decompressOodle(entry, compressed, options);
    default:
      throw new UnsupportedPakCompressionError(entry.name, entry.compressionMethod);
  }
}

async function decompressOodle(entry: PakEntryInfo, compressed: Uint8Array, options: ArchiveReadOptions): Promise<Uint8Array> {
  const loaded = await loadOodle(options.oodleLibrary);
  const output = new Uint8Array(entry.uncompressedSize);
  const decoded = loaded.decompress(
    compressed,
    compressed.byteLength,
    output,
    output.byteLength,
    1,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    3
  );
  if (!Number.isSafeInteger(decoded) || decoded <= 0 || decoded > output.byteLength) {
    throw new Error(`Oodle failed to decompress ${entry.name} from ${loaded.path}: returned ${decoded}`);
  }
  return output.subarray(0, decoded);
}

async function loadOodle(explicitDllPath?: string): Promise<LoadedOodle> {
  const key = explicitDllPath ?? "";
  if (cachedOodleKey !== key) {
    cachedOodleKey = key;
    cachedOodle = loadOodleUncached(explicitDllPath);
  }
  return cachedOodle as Promise<LoadedOodle>;
}

async function loadOodleUncached(explicitDllPath?: string): Promise<LoadedOodle> {
  const ffi = (await dynamicImport("bun:ffi")) as BunFfiModule;
  const { FFIType } = ffi;
  let lastError: unknown;
  for (const candidate of oodleCandidatePaths(explicitDllPath)) {
    if (!isBareDllName(candidate) && !existsSync(candidate)) {
      continue;
    }
    try {
      const library = ffi.dlopen(candidate, {
        OodleLZ_Decompress: {
          args: [
            FFIType.ptr,
            FFIType.i32,
            FFIType.ptr,
            FFIType.i32,
            FFIType.u32,
            FFIType.u32,
            FFIType.u32,
            FFIType.ptr,
            FFIType.isize,
            FFIType.ptr,
            FFIType.ptr,
            FFIType.ptr,
            FFIType.isize,
            FFIType.u32
          ],
          returns: FFIType.i32
        }
      });
      return {
        path: candidate,
        decompress: library.symbols.OodleLZ_Decompress
      };
    } catch (error) {
      lastError = error;
    }
  }
  throw new Error(`Oodle library not found. Set OODLE_DLL or pass oodleLibrary. Last error: ${lastError instanceof Error ? lastError.message : String(lastError)}`);
}

function oodleCandidatePaths(explicitDllPath?: string): string[] {
  const candidates: string[] = [];
  const add = (value: string | undefined): void => {
    if (value === undefined || value.trim().length === 0) {
      return;
    }
    const trimmed = value.trim();
    if (trimmed.toLowerCase().endsWith(".dll")) {
      candidates.push(trimmed);
      return;
    }
    for (const name of OODLE_DLL_NAMES) {
      candidates.push(join(trimmed, name));
    }
  };

  add(explicitDllPath);
  for (const value of packageLocalOodleDirs()) {
    add(value);
  }
  add(process.env.OODLE_DLL);
  for (const value of (process.env.OODLE_PATH ?? "").split(delimiter)) {
    add(value);
  }
  for (const value of (process.env.PATH ?? "").split(delimiter)) {
    add(value);
  }
  candidates.push(...OODLE_DLL_NAMES);

  return [...new Set(candidates)];
}

function packageLocalOodleDirs(): string[] {
  const moduleDir = dirname(fileURLToPath(import.meta.url));
  return [
    moduleDir,
    join(moduleDir, "bin"),
    join(moduleDir, "..", "bin"),
    join(moduleDir, "..", "..", "bin"),
    join(process.cwd(), "bin")
  ];
}

function isBareDllName(path: string): boolean {
  return !path.includes("/") && !path.includes("\\") && !path.includes(":");
}

function peelAzcs(bytes: Uint8Array): Uint8Array {
  if (bytes.byteLength < 16 || bytes[0] !== 0x41 || bytes[1] !== 0x5a || bytes[2] !== 0x43 || bytes[3] !== 0x53) {
    return bytes;
  }
  const compressorId = readUint32BigEndian(bytes, 4, "AZCS compressor id");
  switch (compressorId) {
    case AZCS_ZLIB:
      assertRange(bytes, 20, bytes.byteLength - 20, "AZCS zlib payload");
      return inflateSync(bytes.subarray(20));
    case AZCS_ZSTD:
      throw new Error("AZCS ZSTD payloads are unsupported by this TypeScript package");
    default:
      throw new Error(`unsupported AZCS compressor id ${compressorId.toString(16)}`);
  }
}

async function readFileRange(file: FileHandle, offset: number, length: number, label: string): Promise<Uint8Array> {
  if (!Number.isSafeInteger(offset) || !Number.isSafeInteger(length) || offset < 0 || length < 0) {
    throw new Error(`${label} out of bounds`);
  }
  const bytes = new Uint8Array(length);
  let read = 0;
  while (read < length) {
    const result = await file.read(bytes, read, length - read, offset + read);
    if (result.bytesRead === 0) {
      throw new Error(`${label} ended after ${read} of ${length} bytes`);
    }
    read += result.bytesRead;
  }
  return bytes;
}

function readUint16(bytes: Uint8Array, offset: number, label: string): number {
  assertRange(bytes, offset, 2, label);
  return new DataView(bytes.buffer, bytes.byteOffset + offset, 2).getUint16(0, true);
}

function readUint32(bytes: Uint8Array, offset: number, label: string): number {
  assertRange(bytes, offset, 4, label);
  return new DataView(bytes.buffer, bytes.byteOffset + offset, 4).getUint32(0, true);
}

function readUint32BigEndian(bytes: Uint8Array, offset: number, label: string): number {
  assertRange(bytes, offset, 4, label);
  return new DataView(bytes.buffer, bytes.byteOffset + offset, 4).getUint32(0, false);
}

function readUint64(bytes: Uint8Array, offset: number, label: string): bigint {
  assertRange(bytes, offset, 8, label);
  return new DataView(bytes.buffer, bytes.byteOffset + offset, 8).getBigUint64(0, true);
}

function safeNumber(value: bigint, label: string): number {
  if (value > BigInt(Number.MAX_SAFE_INTEGER)) {
    throw new Error(`${label} exceeds JavaScript safe integer range`);
  }
  return Number(value);
}

function checkedSum(left: number, right: number, label: string): number {
  const sum = left + right;
  if (!Number.isSafeInteger(sum)) {
    throw new Error(`${label} exceeds JavaScript safe integer range`);
  }
  return sum;
}

function assertRange(bytes: Uint8Array, offset: number, length: number, label: string): void {
  if (!Number.isSafeInteger(offset) || !Number.isSafeInteger(length) || offset < 0 || length < 0 || offset + length > bytes.byteLength) {
    throw new Error(`${label} out of bounds`);
  }
}
