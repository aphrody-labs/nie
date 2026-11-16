// Steam helpers of the steam plugin: local install data (VDF, ACF, libraries, Proton), path map, SteamID forms,
// SteamDB/store links and the credential-file guard. Pure TypeScript, no native bridge.

import { existsSync, readdirSync, statSync } from "node:fs";
import { join, resolve } from "node:path";

export type Vdf = { [key: string]: string | Vdf };

// ---------------------------------------------------------------- local data (pure TypeScript)

const home = (): string => process.env.HOME ?? process.env.USERPROFILE ?? "";

/** Parses text KeyValues (VDF) into nested objects. */
export function parseTextVdf(src: string): Vdf {
  const tok = /"((?:[^"\\]|\\.)*)"|([{}])/g;
  const unq = (s: string) => s.replace(/\\(.)/g, (_, c) => (c === "n" ? "\n" : c === "t" ? "\t" : c));
  const stack: Vdf[] = [{}];
  let key: string | null = null;
  const clean = src.replace(/^\s*\/\/.*$/gm, "");
  for (const m of clean.matchAll(tok)) {
    if (m[2] === "{") {
      const child: Vdf = {};
      stack[stack.length - 1][key ?? ""] = child;
      stack.push(child);
      key = null;
    } else if (m[2] === "}") {
      stack.pop();
    } else if (key === null) key = unq(m[1]);
    else {
      stack[stack.length - 1][key] = unq(m[1]);
      key = null;
    }
  }
  return stack[0];
}

/** Parses binary KeyValues (shortcuts.vdf style: 0 object, 1 string, 2 int32, 7 uint64, 8 end). */
export function parseBinaryVdf(buf: Uint8Array): Vdf {
  const dv = new DataView(buf.buffer, buf.byteOffset, buf.byteLength);
  let p = 0;
  const cstr = (): string => {
    const s = p;
    while (p < buf.length && buf[p] !== 0) p++;
    return new TextDecoder().decode(buf.subarray(s, p++));
  };
  const obj = (): Vdf => {
    const o: Vdf = {};
    while (p < buf.length) {
      const t = buf[p++];
      if (t === 8) return o;
      const k = cstr();
      if (t === 0) o[k] = obj();
      else if (t === 1) o[k] = cstr();
      else if (t === 2) {
        o[k] = String(dv.getInt32(p, true));
        p += 4;
      } else if (t === 7) {
        o[k] = dv.getBigUint64(p, true).toString();
        p += 8;
      } else throw new Error(`unsupported binary VDF type ${t}`);
    }
    return o;
  };
  return obj();
}

/** Steam root: $STEAM_PATH, then the usual per-OS locations. */
export function findSteamRoot(): string | undefined {
  const c = [
    process.env.STEAM_PATH,
    "C:/Program Files (x86)/Steam",
    "C:/Program Files/Steam",
    join(home(), ".steam/steam"),
    join(home(), ".local/share/Steam"),
    join(home(), ".var/app/com.valvesoftware.Steam/.local/share/Steam"),
    join(home(), "snap/steam/common/.local/share/Steam"),
    join(home(), "Library/Application Support/Steam"),
  ];
  return c.find((p) => p && existsSync(join(p, "steamapps")));
}

export interface Library {
  path: string;
  label: string;
  apps: number[];
}

export async function listLibraries(root: string): Promise<Library[]> {
  const f = join(root, "steamapps", "libraryfolders.vdf");
  const parsed = existsSync(f) ? (parseTextVdf(await Bun.file(f).text()).libraryfolders as Vdf) : {};
  const libs = Object.values(parsed ?? {}).filter((v): v is Vdf => typeof v === "object").map((v) => ({
    path: String(v.path),
    label: String(v.label ?? ""),
    apps: Object.keys((v.apps as Vdf) ?? {}).map(Number),
  }));
  return libs.length ? libs : [{ path: root, label: "", apps: [] }];
}

export interface InstalledApp {
  appid: number;
  name: string;
  installdir: string;
  library: string;
  manifest: string;
  install_path: string;
  exists_on_disk: boolean;
  buildid: string;
  size_on_disk: number;
  state_flags: number;
  state: string[];
  last_updated: number;
  depots: { depot: string; manifest: string | null; size: string | null }[];
}

const STATE_BITS: [number, string][] = [[4, "fully installed"], [2, "update required"], [1024, "update running"], [512, "update paused"], [256, "update started"], [2048, "uninstalling"]];

export async function listApps(root: string, filter?: string): Promise<InstalledApp[]> {
  const out: InstalledApp[] = [];
  for (const lib of await listLibraries(root)) {
    const dir = join(lib.path, "steamapps");
    if (!existsSync(dir)) continue;
    for (const name of readdirSync(dir).filter((n) => /^appmanifest_\d+\.acf$/.test(n))) {
      const a = parseTextVdf(await Bun.file(join(dir, name)).text()).AppState as Vdf | undefined;
      if (!a) continue;
      const flags = Number(a.StateFlags ?? 0);
      const install_path = join(dir, "common", String(a.installdir ?? ""));
      const depots = Object.entries((a.InstalledDepots as Vdf) ?? {}).map(([depot, d]) => ({
        depot,
        manifest: typeof d === "object" ? String(d.manifest ?? "") || null : null,
        size: typeof d === "object" ? String(d.size ?? "") || null : null,
      }));
      out.push({
        appid: Number(a.appid), name: String(a.name ?? ""), installdir: String(a.installdir ?? ""),
        library: lib.path, manifest: join(dir, name), install_path, exists_on_disk: existsSync(install_path),
        buildid: String(a.buildid ?? ""), size_on_disk: Number(a.SizeOnDisk ?? 0), state_flags: flags,
        state: STATE_BITS.filter(([b]) => flags & b).map(([, s]) => s), last_updated: Number(a.LastUpdated ?? 0), depots,
      });
    }
  }
  const q = filter?.toLowerCase();
  return out.filter((a) => !q || a.name.toLowerCase().includes(q) || String(a.appid) === q).sort((x, y) => x.appid - y.appid);
}

/** Reads appinfo.vdf (v39 inline keys, v40 and v41 string table). Returns the app entry as JSON, or the id list. */
export function parseAppInfo(buf: Uint8Array, app?: number): unknown {
  const dv = new DataView(buf.buffer, buf.byteOffset, buf.byteLength);
  const magic = dv.getUint32(0, true);
  const ver = (magic & 0xff) - 0x27 + 39;
  if (magic >>> 8 !== 0x075644) throw new Error("not an appinfo.vdf");
  let p = 8;
  const strings: string[] = [];
  if (ver >= 41) {
    let t = Number(dv.getBigInt64(p, true));
    p += 8;
    const n = dv.getUint32(t, true);
    t += 4;
    for (let i = 0; i < n; i++) {
      let e = t;
      while (buf[e] !== 0) e++;
      strings.push(new TextDecoder().decode(buf.subarray(t, e)));
      t = e + 1;
    }
  }
  const walk = (start: number, end: number): [Vdf, number] => {
    let q = start;
    const key = (): string => {
      if (ver >= 40) return strings[dv.getUint32((q += 4) - 4, true)] ?? "";
      let e = q;
      while (buf[e] !== 0) e++;
      const s = new TextDecoder().decode(buf.subarray(q, e));
      q = e + 1;
      return s;
    };
    const str = (): string => {
      let e = q;
      while (buf[e] !== 0) e++;
      const s = new TextDecoder().decode(buf.subarray(q, e));
      q = e + 1;
      return s;
    };
    const obj = (): Vdf => {
      const o: Vdf = {};
      while (q < end) {
        const t = buf[q++];
        if (t === 8) return o;
        const k = key();
        if (t === 0) o[k] = obj();
        else if (t === 1) o[k] = str();
        else if (t === 2 || t === 4 || t === 6) { o[k] = String(dv.getInt32(q, true)); q += 4; }
        else if (t === 3) { o[k] = String(dv.getFloat32(q, true)); q += 4; }
        else if (t === 7) { o[k] = dv.getBigUint64(q, true).toString(); q += 8; }
        else if (t === 10) { o[k] = dv.getBigInt64(q, true).toString(); q += 8; }
        else throw new Error(`unsupported appinfo type ${t}`);
      }
      return o;
    };
    return [obj(), q];
  };
  const ids: number[] = [];
  while (p + 8 <= buf.length) {
    const id = dv.getUint32(p, true);
    if (id === 0) break;
    const size = dv.getUint32(p + 4, true);
    const body = p + 8;
    if (app === undefined) ids.push(id);
    else if (id === app) {
      const head = 4 + 4 + 8 + 20 + 4 + (ver >= 40 ? 20 : 0);
      const [data] = walk(body + head, body + size);
      return { appid: id, last_updated: dv.getUint32(body + 4, true), change_number: dv.getUint32(body + 36, true), data };
    }
    p = body + size;
  }
  if (app !== undefined) throw new Error(`app ${app} not found in appinfo.vdf`);
  return { version: ver, apps: ids };
}

export type VdfFormat = "auto" | "text" | "binary" | "appinfo";

/** Parses a KeyValues file to JSON (text, binary, or appinfo.vdf with an optional app). */
export function readVdf(path: string, format: VdfFormat = "auto", app?: number): unknown {
  if (isSecretFile(path)) throw new Error(`${path} holds credentials; not read`);
  const bytes = new Uint8Array(require("node:fs").readFileSync(path));
  if (format === "appinfo" || (format === "auto" && /appinfo\.vdf$/i.test(path))) return parseAppInfo(bytes, app);
  const text = format === "text" || (format === "auto" && bytes[0] === 0x22);
  return text ? parseTextVdf(new TextDecoder().decode(bytes)) : parseBinaryVdf(bytes);
}

export interface CompatTool {
  name: string;
  display_name: string | null;
  path: string;
  kind: string;
  source: string;
  version: string | null;
}

/** Proton and compatibilitytools.d entries (GE, CachyOS, umu). */
export function compatTools(root: string): CompatTool[] {
  const out: CompatTool[] = [];
  const scan = (dir: string, source: string, test: (n: string) => boolean) => {
    if (!existsSync(dir)) return;
    for (const n of readdirSync(dir).filter(test)) {
      const p = join(dir, n);
      if (!statSync(p).isDirectory()) continue;
      const vf = join(p, "compatibilitytool.vdf");
      let display: string | null = null;
      if (existsSync(vf)) {
        const t = (parseTextVdf(require("node:fs").readFileSync(vf, "utf8")).compatibilitytools as Vdf | undefined)?.compat_tools as Vdf | undefined;
        const first = t && (Object.values(t)[0] as Vdf | undefined);
        display = first ? String(first.display_name ?? "") || null : null;
      }
      const ver = existsSync(join(p, "version")) ? require("node:fs").readFileSync(join(p, "version"), "utf8").trim() : null;
      out.push({ name: n, display_name: display, path: p, kind: /proton/i.test(n) ? "proton" : /runtime|sniper|soldier/i.test(n) ? "runtime" : "other", source, version: ver });
    }
  };
  scan(join(root, "steamapps", "common"), "steam", (n) => /^(Proton|SteamLinuxRuntime)/i.test(n));
  scan(join(root, "compatibilitytools.d"), "compatibilitytools.d", () => true);
  return out;
}

export interface LaunchEnv {
  app: number;
  prefix: string;
  install_path: string;
  env: Record<string, string>;
  wrapper: string[];
  launch_options: string;
}

export function launchEnv(
  app: number,
  o: { root?: string; prefix?: string; installPath?: string; cacheRoot?: string; gamemode?: boolean } = {},
): LaunchEnv {
  const root = o.root ?? findSteamRoot() ?? "";
  const prefix = o.prefix ?? join(root, "steamapps", "compatdata", String(app));
  const env: Record<string, string> = {
    SteamAppId: String(app), SteamGameId: String(app), STEAM_COMPAT_APP_ID: String(app),
    STEAM_COMPAT_DATA_PATH: prefix, STEAM_COMPAT_CLIENT_INSTALL_PATH: root, WINEPREFIX: join(prefix, "pfx"),
  };
  if (o.installPath) env.STEAM_COMPAT_INSTALL_PATH = o.installPath;
  if (o.cacheRoot) env.DXVK_STATE_CACHE_PATH = env.VKD3D_SHADER_CACHE_PATH = o.cacheRoot;
  return { app, prefix, install_path: o.installPath ?? "", env, wrapper: o.gamemode ? ["gamemoderun"] : [], launch_options: o.gamemode ? "gamemoderun %command%" : "%command%" };
}

// ---------------------------------------------------------------- path map

const exists = (p: string): string | null => (existsSync(p) ? p : null);

/** Map an appid to every local path Steam uses for it. */
export async function appPaths(
  root: string,
  appid: number,
  accountId?: number,
) {
  const apps = await listApps(root);
  const app = apps.find((a) => a.appid === appid);
  const lib = app?.library ?? root;
  const sa = join(lib, "steamapps");
  const depotManifests = existsSync(join(root, "depotcache"))
    ? readdirSync(join(root, "depotcache"))
        .filter((f) => app?.depots.some((d) => f.startsWith(`${d.depot}_`)))
        .map((f) => join(root, "depotcache", f))
    : [];
  const userdata = join(root, "userdata");
  const users = existsSync(userdata)
    ? readdirSync(userdata).filter((d) => /^\d+$/.test(d))
    : [];
  const userDirs = (accountId ? [String(accountId)] : users).map((u) => ({
    accountId: Number(u),
    appDir: exists(join(userdata, u, String(appid))),
    remote: exists(join(userdata, u, String(appid), "remote")),
    // Any userdata/<accountid>/<n>/remote/<appid>/screenshots directory present on disk
    // (the client stores screenshots as <gameid>/screenshots/, per steamclient64.dll strings).
    screenshots: findScreenshots(join(userdata, u), appid),
  }));
  return {
    appid,
    name: app?.name ?? null,
    installed: Boolean(app),
    library: lib,
    appmanifest: app?.manifest ?? null,
    installDir: app ? exists(app.install_path) : null,
    downloading: exists(join(sa, "downloading", String(appid))),
    shadercache: exists(join(sa, "shadercache", String(appid))),
    workshop: exists(join(sa, "workshop", "content", String(appid))),
    workshopManifest: exists(join(sa, "workshop", `appworkshop_${appid}.acf`)),
    compatdata: exists(join(sa, "compatdata", String(appid))),
    protonPrefix: exists(join(sa, "compatdata", String(appid), "pfx")),
    depotManifests,
    librarycache: exists(join(root, "appcache", "librarycache", String(appid))),
    users: userDirs,
    state: app?.state ?? [],
  };
}

function findScreenshots(userDir: string, appid: number): string[] {
  if (!existsSync(userDir)) return [];
  return readdirSync(userDir)
    .map((d) => join(userDir, d, "remote", String(appid), "screenshots"))
    .filter((p) => existsSync(p));
}

export function dirSize(path: string, limit = 200_000): number {
  let total = 0;
  let seen = 0;
  const walk = (p: string): void => {
    for (const e of readdirSync(p, { withFileTypes: true })) {
      if (++seen > limit) return;
      const f = join(p, e.name);
      if (e.isDirectory()) walk(f);
      else total += statSync(f).size;
    }
  };
  walk(path);
  return total;
}

// ---------------------------------------------------------------- SteamID

/** Base of individual SteamID64 values (universe 1, type 1, instance 1). */
export const STEAMID64_BASE = 76561197960265728n;
// Letters per EAccountType index, as SteamKit SteamID.cs AccountTypeChars; unknown types render as "i".
const TYPE_LETTERS = ["I", "U", "M", "G", "A", "P", "C", "g", "T", "i", "a"];

export interface SteamIdForms {
  steamid64: string;
  accountId: number;
  universe: number;
  accountType: number;
  instance: number;
  steam2: string;
  steam3: string;
  profileUrl: string;
}

export function decodeSteamId64(id: bigint): SteamIdForms {
  const accountId = Number(id & 0xffffffffn);
  const instance = Number((id >> 32n) & 0xfffffn);
  const accountType = Number((id >> 52n) & 0xfn);
  const universe = Number((id >> 56n) & 0xffn);
  const letter = TYPE_LETTERS[accountType] ?? "i";
  return {
    steamid64: id.toString(),
    accountId,
    universe,
    accountType,
    instance,
    // STEAM_X uses 0 for public on legacy engines; 1 on newer (Orange Box+). Report 1.
    steam2: `STEAM_${universe}:${accountId & 1}:${accountId >>> 1}`,
    steam3: `[${letter}:${universe}:${accountId}${accountType === 1 && instance !== 1 ? `:${instance}` : ""}]`,
    profileUrl: `https://steamcommunity.com/profiles/${id}`,
  };
}

/** Parse SteamID64, STEAM_X:Y:Z, [U:1:N], a bare account id or a /profiles/ URL.
 * Returns null for vanity names (resolve them over the network). */
export function parseSteamId(input: string): SteamIdForms | null {
  const s = input.trim();
  let m = s.match(/steamcommunity\.com\/profiles\/(\d{17})/);
  if (m) return decodeSteamId64(BigInt(m[1]!));
  if (/^\d{17}$/.test(s)) return decodeSteamId64(BigInt(s));
  m = s.match(/^STEAM_([0-5]):([01]):(\d+)$/i);
  if (m) {
    const acc = BigInt(Number(m[3]) * 2 + Number(m[2]));
    return decodeSteamId64(STEAMID64_BASE + acc);
  }
  m = s.match(/^\[?U:1:(\d+)\]?$/i);
  if (m) return decodeSteamId64(STEAMID64_BASE + BigInt(m[1]!));
  if (/^\d{1,10}$/.test(s) && Number(s) < 2 ** 32)
    return decodeSteamId64(STEAMID64_BASE + BigInt(s));
  return null;
}

/** Extract a vanity name from a /id/<name> URL or bare name. */
export function vanityOf(input: string): string | null {
  const m = input
    .trim()
    .match(/(?:steamcommunity\.com\/id\/)?([A-Za-z0-9_-]{2,32})\/?$/);
  return m ? m[1]! : null;
}

// ---------------------------------------------------------------- links

export function steamdbLinks(
  kind: string,
  id: string | number,
): Record<string, string> {
  const b = "https://steamdb.info";
  switch (kind) {
    case "app":
      return {
        app: `${b}/app/${id}/`,
        info: `${b}/app/${id}/info/`,
        depots: `${b}/app/${id}/depots/`,
        history: `${b}/app/${id}/history/`,
        patchnotes: `${b}/app/${id}/patchnotes/`,
        charts: `${b}/app/${id}/charts/`,
        packages: `${b}/app/${id}/subs/`,
        store: `https://store.steampowered.com/app/${id}/`,
        community: `https://steamcommunity.com/app/${id}`,
      };
    case "depot":
      return {
        depot: `${b}/depot/${id}/`,
        manifests: `${b}/depot/${id}/manifests/`,
      };
    case "sub":
    case "package":
      return {
        sub: `${b}/sub/${id}/`,
        store: `https://store.steampowered.com/sub/${id}/`,
      };
    case "user":
    case "calculator":
      return {
        calculator: `${b}/calculator/${id}/`,
        profile: `https://steamcommunity.com/profiles/${id}`,
      };
    default:
      throw new Error(`unknown kind ${kind} (app, depot, sub, user)`);
  }
}

// ---------------------------------------------------------------- safety

/** Files that hold login tokens or machine auth: config/config.vdf, config/loginusers.vdf,
 * userdata/<id>/config/local.vdf (cached connect tokens) and ssfn* sentry files. */
export function isSecretFile(path: string): boolean {
  const raw = path.replace(/\\/g, "/").split("/").pop()!.toLowerCase();
  if (raw.includes(":")) return true; // alternate data stream
  const base = raw.replace(/[. ]+$/, ""); // Windows ignores trailing dots and spaces
  return (
    base === "config.vdf" ||
    base === "loginusers.vdf" ||
    base === "local.vdf" ||
    base.startsWith("ssfn")
  );
}

export function isUnder(path: string, dir: string): boolean {
  const norm = (p: string): string =>
    resolve(p).replace(/\\/g, "/").toLowerCase().replace(/\/+$/, "");
  const p = norm(path);
  const d = norm(dir);
  return p === d || p.startsWith(`${d}/`);
}
