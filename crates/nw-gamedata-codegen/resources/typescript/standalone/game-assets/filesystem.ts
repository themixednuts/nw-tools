import { readdir, readFile, realpath } from "node:fs/promises";
import { isAbsolute, join, relative, sep } from "node:path";

import { normalizeVirtualPath } from "./catalog.js";
import { type DatasheetAsset, isDatasheetBytes, isDatasheetPath } from "./datasheet.js";

export async function loadLooseDatasheets(root: string): Promise<DatasheetAsset[]> {
  const resolvedRoot = await realpath(root);
  const assets: DatasheetAsset[] = [];
  await collectLooseDatasheets(resolvedRoot, resolvedRoot, assets);
  return assets;
}

async function collectLooseDatasheets(root: string, dir: string, assets: DatasheetAsset[]): Promise<void> {
  const entries = await readdir(dir, { withFileTypes: true });
  entries.sort((left, right) => left.name.localeCompare(right.name));
  for (const entry of entries) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) {
      await collectLooseDatasheets(root, path, assets);
      continue;
    }
    if (!entry.isFile() || !isDatasheetPath(path)) {
      continue;
    }
    const bytes = await readFile(path);
    if (!isDatasheetBytes(bytes)) {
      continue;
    }
    const relativePath = relative(root, path);
    if (isOutsideRelativePath(relativePath)) {
      throw new Error(`datasheet path ${path} is outside root ${root}`);
    }
    assets.push({
      path: normalizeVirtualPath(relativePath),
      bytes
    });
  }
}

function isOutsideRelativePath(path: string): boolean {
  return path === ".." || path.startsWith(`..${sep}`) || isAbsolute(path);
}
