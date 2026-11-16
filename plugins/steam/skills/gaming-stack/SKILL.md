---
name: gaming-stack
description: Use when asked how to run a Windows game through Proton or Wine on Linux, Vulkan or DirectX layers of a host, headless steamcmd on a build host, or the launch environment of a Steam app (Inazuma Eleven: Victory Road for nie).
argument-hint: "[proton|layers|dx|steamcmd|run] [appid|exe]"
allowed-tools: Bash, Read
---

# Gaming stack

Local Steam data comes from the `steam` plugin MCP (`plugins/steam/servers/steam.ts`, pure TypeScript): `local_apps`, `compat_tools`, `launch_env`, `app_paths`, `read_vdf`.

## Procedure

1. Host view: call `local_apps` and `compat_tools`; Vulkan with `vulkaninfo --summary`.
2. Headless Proton on Linux (build host): official `steamcmd` bootstrap, `+app_update <proton app> validate`, then `<proton>/proton waitforexitandrun <exe>` with the env from `launch_env` (`umu-run` when installed).
3. On Windows the game starts natively.

## Rules

- Valve binaries (Steam client, steamcmd, Proton, Steamworks SDK) are downloaded at run time from the official source, never committed.
- Credential files (`config.vdf`, `loginusers.vdf`, `local.vdf`, `ssfn*`) are never read.
- Builds run on the build host (fr1), not on prd.

## Sources

Upstream: `ValveSoftware/Proton`, `ValveSoftware/steam-runtime`, `Open-Wine-Components/umu-launcher`, `KhronosGroup/Vulkan-Loader` docs.
