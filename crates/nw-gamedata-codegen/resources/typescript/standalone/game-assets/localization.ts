import { XMLParser } from "fast-xml-parser";

export const localizationExtension = "xml";

export interface LocalizationBundle {
  readonly entries: ReadonlyMap<string, string>;
}

export function parseLocalizationXml(input: string | Uint8Array): LocalizationBundle {
  const text = typeof input === "string" ? input : new TextDecoder().decode(input);
  const parser = new XMLParser({
    attributeNamePrefix: "@",
    ignoreAttributes: false,
    textNodeName: "#text",
    trimValues: true
  });
  const parsed = parser.parse(text) as unknown;
  const entries = new Map<string, string>();
  collectLocalizationEntries(parsed, entries);
  return { entries };
}

export function isLocalizationPath(path: string): boolean {
  return path.toLowerCase().endsWith(`.${localizationExtension}`);
}

function collectLocalizationEntries(value: unknown, entries: Map<string, string>): void {
  if (Array.isArray(value)) {
    for (const item of value) {
      collectLocalizationEntries(item, entries);
    }
    return;
  }
  if (value === null || typeof value !== "object") {
    return;
  }

  const node = value as Record<string, unknown>;
  const key = localizationKey(node);
  const text = node["#text"];
  if (key !== undefined && typeof text === "string") {
    entries.set(key, text);
  }

  for (const child of Object.values(node)) {
    collectLocalizationEntries(child, entries);
  }
}

function localizationKey(node: Record<string, unknown>): string | undefined {
  for (const key of ["@key", "@Key", "@name", "@Name", "@id", "@Id"]) {
    const value = node[key];
    if (typeof value === "string") {
      return value;
    }
  }
  return undefined;
}
