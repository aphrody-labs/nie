---
name: steam-local-inspect
description: This skill should be used when the user asks to "inspect my Steam install", "where is Steam installed", "list my Steam games", "find the install folder, compatdata prefix or shadercache of appid X", "which manifest or buildid is installed", "decode StateFlags", or "show the cached appinfo entry of an installed app". For layout and format explanations, steam-client-windows applies.
argument-hint: "[apps|paths|vdf|compat|env] [appid|filter|file]"
allowed-tools: Read
---

# Inspect the local Steam install

Read-only inspection of the Steam client on this machine (Windows, Linux, Flatpak, Snap, macOS) with the `steam` plugin MCP tools (`plugins/steam/servers/steam.ts`, pure TypeScript):

1. Locate Steam: `local_apps` returns the root, libraries and installed apps; `STEAM_PATH` overrides the lookup.
2. One app: `app_paths` (install dir, compatdata prefix, shadercache) and `read_vdf` on `<library>/steamapps/appmanifest_<appid>.acf` (full ACF).
3. PICS metadata cached by the client: `read_vdf` on `<root>/appcache/appinfo.vdf` with `app: <appid>` (v39 to v41).
4. Any other KeyValues file: `read_vdf` (text or binary detected).
5. Compat tools: `compat_tools`; launch environment: `launch_env`.

## Rules

- Never read or print `config/config.vdf`, `config/loginusers.vdf`,
  `userdata/<accountid>/config/local.vdf` or `ssfn*` files: they hold login tokens or machine
  auth. The `read_vdf` MCP tool refuses them (`isSecretFile` in `plugins/steam/servers/steam.ts`).
- Treat everything as read-only. Close Steam before editing any file it owns.
- To decode a depot manifest found by `paths`, use the offline manifest commands in
  [`../steam-community-tools/references/tools.md`](../steam-community-tools/references/tools.md).

## Worked example

The verified IEVR example (AppID 2799860) is in [`references/layout.md`](references/layout.md).

## Additional Resources

- `references/layout.md`: files the plugin reads, field meanings and StateFlags bits.
- Related skills: steam-client-windows (full layout, binary formats), steam-linux-proton
  (compatdata and prefixes), steam-content-cdn (manifests and chunks).

## Sources

- the upstream source (library discovery, StateFlag bit offsets)
- `xPaw/SteamAppInfo:README.md` (appinfo.vdf v39 to v41 layout)
- `ValveResourceFormat/ValveKeyValue` (binary KeyValues type bytes)
- the upstream source and the upstream source
