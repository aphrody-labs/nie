/**
 * Plomberie Bun FFI pour les bibliothèques natives à tampons possédés
 * (`{ data: *mut u8, len: usize, cap: usize }` libérés par un symbole `*_free_fields`) :
 * chargement paresseux avec diagnostic, chaînes C, lecture d'octets possédés.
 */
import {
  dlopen,
  suffix,
  toArrayBuffer,
  type FFIFunction,
  type Pointer,
} from "bun:ffi";
import { existsSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, resolve } from "node:path";

const encoder = new TextEncoder();
const decoder = new TextDecoder();

type OwnedFree = (data: bigint, len: bigint, cap: bigint) => void;

/** Copie NUL-terminée en UTF-8 de `text`. */
export function toCString(text: string): Uint8Array {
  const encoded = encoder.encode(text);
  const out = new Uint8Array(encoded.length + 1);
  out.set(encoded);
  return out;
}

/**
 * Lit les octets d'un slot rempli puis libère l'allocation native via `free(data, len, cap)`.
 * Renvoie null pour un slot vide ou non renseigné (l'allocation éventuelle est tout de même libérée).
 */
function withOwnedView<T>(
  slot: Uint8Array,
  free: OwnedFree,
  read: (view: Uint8Array) => T,
): T | null {
  const view = new DataView(slot.buffer, slot.byteOffset, slot.byteLength);
  const data = view.getBigUint64(0, true);
  const len = view.getBigUint64(8, true);
  const cap = view.getBigUint64(16, true);
  if (data === 0n || len === 0n) {
    if (data !== 0n) free(data, len, cap);
    return null;
  }
  try {
    return read(
      new Uint8Array(toArrayBuffer(Number(data) as unknown as Pointer, 0, Number(len))),
    );
  } finally {
    free(data, len, cap);
  }
}

export function readOwnedBytes(slot: Uint8Array, free: OwnedFree): Uint8Array | null {
  return withOwnedView(slot, free, (view) => view.slice());
}

export function readOwnedString(slot: Uint8Array, free: OwnedFree): string | null {
  return withOwnedView(slot, free, (view) => decoder.decode(view));
}

export function readOwnedJson<T = unknown>(slot: Uint8Array, free: OwnedFree): T | null {
  const text = readOwnedString(slot, free);
  if (!text) return null;
  try {
    return JSON.parse(text) as T;
  } catch {
    return null;
  }
}

/** Copie `len` octets à une adresse native puis libère l'allocation via `free(address)`. */
export function readNativeBytes(
  address: number | bigint,
  len: number,
  free: (address: number) => void,
): Uint8Array {
  const addr = Number(address);
  try {
    return new Uint8Array(toArrayBuffer(addr as unknown as Pointer, 0, len)).slice();
  } finally {
    free(addr);
  }
}

export interface NativeLibraryOptions<S extends Record<string, FFIFunction>> {
  /** Nom de base sans préfixe ni suffixe (`iecode` pour `libiecode.so`). */
  name: string;
  /** Noms de fichier exacts remplaçant `lib<name>.<suffix>`. */
  fileNames?: readonly string[];
  symbols: S;
  /** Fichier explicite, testé avant les variables d'environnement. */
  path?: string;
  /** Variables d'environnement désignant un fichier, testées dans l'ordre. */
  envPaths?: readonly string[];
  /** Dossiers parcourus après le dossier de l'exécutable et `~/.local/bin`. */
  searchDirs?: readonly string[];
  /** Indication de compilation ajoutée au diagnostic « introuvable ». */
  buildHint?: string;
}

/** Chargeur paresseux et diagnostiquable d'une bibliothèque native. */
export function nativeLibrary<S extends Record<string, FFIFunction>>(
  options: NativeLibraryOptions<S>,
) {
  const defaultFile =
    process.platform === "win32" ? `${options.name}.dll` : `lib${options.name}.${suffix}`;
  const files = options.fileNames?.length ? options.fileNames : [defaultFile];
  const file = files[0]!;
  let attempted = false;
  let symbols: ReturnType<typeof dlopen<S>>["symbols"] | null = null;
  let error: string | null = null;
  let path: string | null = null;
  let library: ReturnType<typeof dlopen<S>> | null = null;
  let closed = false;

  const resolvePath = (): string | null => {
    if (options.path && existsSync(options.path)) return options.path;
    for (const env of options.envPaths ?? []) {
      const value = process.env[env];
      if (value && existsSync(value)) return value;
    }
    const dirs = [
      dirname(process.execPath),
      resolve(homedir(), ".local/bin"),
      ...(options.searchDirs ?? []),
    ];
    return (
      dirs
        .flatMap((dir) => files.map((name) => resolve(dir, name)))
        .find((candidate) => existsSync(candidate)) ?? null
    );
  };
  const load = () => {
    if (attempted) return symbols;
    attempted = true;
    const found = resolvePath();
    if (!found) {
      error = `Native binary ${file} not found.${options.buildHint ? ` ${options.buildHint}` : ""}`;
      return null;
    }
    try {
      library = dlopen(found, options.symbols);
      symbols = library.symbols;
      path = found;
      error = null;
    } catch (cause) {
      error = `Failed to dlopen ${found}: ${cause instanceof Error ? cause.message : String(cause)}`;
      symbols = null;
    }
    return symbols;
  };
  return {
    resolvePath,
    load,
    isAvailable: () => load() !== null,
    loadError: () => (load(), error),
    libraryPath: () => (load(), path),
    get closed() {
      return closed;
    },
    /** Déchargement terminal et idempotent : ne plus utiliser les symboles renvoyés. */
    close() {
      if (closed) return;
      closed = true;
      attempted = true;
      symbols = null;
      error = `Native library ${file} is closed`;
      const owned = library;
      library = null;
      owned?.close();
    },
  };
}
