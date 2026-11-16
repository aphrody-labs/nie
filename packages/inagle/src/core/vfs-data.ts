import { ptr } from "bun:ffi";
import { readOwnedJson } from "../ffi/runtime.js";
import {
  NieVfsHandle,
  freeNieBytes,
  getNativeLib,
  locateGameSource,
} from "../ffi/nie.js";
import type { ConfigDocument } from "./config-contract.js";

let selectedRoot: string | undefined;
let mounted: NieVfsHandle | undefined;
let entries: string[] | undefined;
const documents = new Map<string, ConfigDocument>();
const configLookupCache = new Map<string, string | null>();
export function configureGameVfs(gameDir?: string): void {
  const root = gameDir || undefined;
  if (root === selectedRoot) return;
  mounted?.close();
  mounted = undefined;
  entries = undefined;
  documents.clear();
  configLookupCache.clear();
  selectedRoot = root;
}

export function gameDataDirectory(): string {
  return locateGameSource(selectedRoot).source;
}

export function gameVfs(): NieVfsHandle {
  return (mounted ??= new NieVfsHandle(gameDataDirectory()));
}

export function gameVfsPaths(): string[] {
  if (!entries) {
    const vfs = gameVfs();
    entries = [];
    const count = Number(vfs.count());
    for (let offset = 0; offset < count; offset += 4096) {
      entries.push(
        ...vfs.listRange(offset, 4096).map((entry) => String(entry.path)),
      );
    }
  }
  return entries;
}

export function findVfsConfig(category: string, prefix: string): string | null {
  const cacheKey = `${category}\0${prefix}`;
  if (configLookupCache.has(cacheKey)) {
    return configLookupCache.get(cacheKey)!;
  }
  const base = `data/common/gamedata/${category}/`;
  const fullPrefix = base + prefix;
  let latest: string | null = null;
  for (const path of gameVfsPaths()) {
    if (
      path.startsWith(fullPrefix) &&
      path.endsWith(".cfg.bin") &&
      (latest === null || path > latest)
    ) {
      latest = path;
    }
  }
  const resolved = latest ? latest.slice(base.length) + ".json" : null;
  configLookupCache.set(cacheKey, resolved);
  return resolved;
}

export function readVfsConfig(path: string): ConfigDocument | null {
  const normalized = path.replaceAll("\\", "/");
  const common = normalized.indexOf("/common/");
  const internal =
    common >= 0
      ? `data${normalized.slice(common)}`.replace(/\.json$/, "")
      : normalized.replace(/\.json$/, "");
  if (documents.has(internal)) return documents.get(internal)!;
  const bytes = gameVfs().read(internal);
  if (!bytes) return null;
  const lib = getNativeLib();
  if (!lib)
    throw new Error("Native libiecode is required for direct VFS decoding.");
  const output = new Uint8Array(24);
  lib.nie_cfgbin_typed_json_out(ptr(bytes), bytes.length, ptr(output));
  const document = readOwnedJson<ConfigDocument>(output, freeNieBytes(lib));
  if (!document) return null;
  documents.set(internal, document);
  return document;
}
