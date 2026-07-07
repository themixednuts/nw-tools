import { readdir, readFile } from "node:fs/promises";
import { join, relative } from "node:path";

import { normalizeVirtualPath } from "./catalog.js";
import { type DatasheetAsset, isDatasheetBytes, isDatasheetPath } from "./datasheet.js";

export async function loadLooseDatasheets(root: string): Promise<DatasheetAsset[]> {
  const assets: DatasheetAsset[] = [];
  await collectLooseDatasheets(root, root, assets);
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
    assets.push({
      path: normalizeVirtualPath(relative(root, path)),
      bytes
    });
  }
}
