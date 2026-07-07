export const ASSET_CATALOG_PATH = "assetcatalog.catalog" as const;
export const RASC_SIGNATURE = "RASC" as const;

export interface AssetId {
  readonly guid: string;
  readonly subId: number;
}

export interface AssetType {
  readonly guid: string;
}

export interface AssetCatalogEntry {
  readonly assetId: AssetId;
  readonly assetType: AssetType;
  readonly relativePath: string;
  readonly sizeBytes: number;
}

export interface AssetCatalog {
  readonly version: number;
  readonly entries: readonly AssetCatalogEntry[];
}

export function isAssetCatalogPath(path: string): boolean {
  return path.split(/[\\/]/).at(-1)?.toLowerCase() === ASSET_CATALOG_PATH;
}

export function parseRascCatalog(bytes: Uint8Array): AssetCatalog {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  if (bytes.byteLength < 40) {
    throw new Error(`RASC input too small: ${bytes.byteLength} bytes`);
  }
  const signature = String.fromCharCode(bytes[0] ?? 0, bytes[1] ?? 0, bytes[2] ?? 0, bytes[3] ?? 0);
  if (signature !== RASC_SIGNATURE) {
    throw new Error(`invalid RASC signature ${signature}`);
  }
  const version = view.getUint32(4, true);
  const fileSize = Number(view.getBigUint64(8, true));
  const guidOffset = view.getUint32(16, true);
  const assetTypeOffset = view.getUint32(20, true);
  const dirOffset = view.getUint32(24, true);
  const fileNameOffset = view.getUint32(28, true);
  const endSentinel = view.getUint32(32, true);
  const numEntries = view.getUint32(36, true);
  if (fileSize >>> 0 !== endSentinel) {
    throw new Error(`RASC size sentinel mismatch: ${fileSize} vs ${endSentinel}`);
  }
  const entries: AssetCatalogEntry[] = [];
  for (let index = 0; index < numEntries; index += 1) {
    const entryOffset = 40 + index * 40;
    const guidIndex = view.getUint32(entryOffset, true);
    const subId = view.getUint32(entryOffset + 4, true);
    const assetTypeIndex = view.getUint32(entryOffset + 16, true);
    const sizeBytes = view.getUint32(entryOffset + 24, true);
    const dirStringOffset = view.getUint32(entryOffset + 32, true);
    const fileStringOffset = view.getUint32(entryOffset + 36, true);
    const directory = readNullTerminatedUtf8(bytes, dirOffset + dirStringOffset);
    const fileName = readNullTerminatedUtf8(bytes, fileNameOffset + fileStringOffset);
    entries.push({
      assetId: {
        guid: formatUuid(bytes, guidOffset + guidIndex * 16),
        subId
      },
      assetType: {
        guid: formatUuid(bytes, assetTypeOffset + assetTypeIndex * 16)
      },
      relativePath: normalizeVirtualPath(directory.length === 0 ? fileName : `${directory}/${fileName}`),
      sizeBytes
    });
  }
  return { version, entries };
}

function readNullTerminatedUtf8(bytes: Uint8Array, offset: number): string {
  let end = offset;
  while (end < bytes.byteLength && bytes[end] !== 0) {
    end += 1;
  }
  return new TextDecoder().decode(bytes.subarray(offset, end));
}

function formatUuid(bytes: Uint8Array, offset: number): string {
  const hex = Array.from(bytes.subarray(offset, offset + 16), (byte) => byte.toString(16).padStart(2, "0"));
  return `${hex.slice(0, 4).join("")}-${hex.slice(4, 6).join("")}-${hex.slice(6, 8).join("")}-${hex.slice(8, 10).join("")}-${hex.slice(10, 16).join("")}`;
}

export function normalizeVirtualPath(path: string): string {
  return path
    .replaceAll("\\", "/")
    .replace(/\/+/g, "/")
    .replace(/^\/+|\/+$/g, "")
    .toLowerCase();
}
