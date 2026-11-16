# Target architecture (decided 2026-11-16)

## Products and hosts

| Product | Language | Host |
| --- | --- | --- |
| Rose Griffon site, APIs, auth, Rose Griffon Discord bot | TypeScript on Bun | rosegriffon.fr |
| Achillea (ranked, clans, tournaments, leagues, Ultimate Team) and its Discord bot | Rust (Dioxus + Bevy, WASM/WebGPU) | its own subdomain |
| Azalee (wiki) | Rust, merged with inagle and nie | its own subdomain |
| nie (engine, VFS, studio, multiplayer) | Rust | its own subdomain |

- inagle, nie and azalee merge into one Rust workspace area (game data, VFS, formats, wiki).
- Everything generic and not tied to IEVR moves to Bun, as global owner packages at the
  monorepo root (`packages/*`), with no product name in them.

## One shared contract between the two worlds

A single Rust <-> Bun FFI layer (C ABI, `bun:ffi` on the Bun side) is the only bridge. Shared once,
generated from one source:

- OpenAPI contract (complete), C ABI headers, JSON Schemas, specs
- design system and themes (tokens), DB schema and migrations, types, auth model, file formats

Rule: one crate or one package per concern; every duplicate between crates/ and packages/ is merged.
For each dependency the upstream docs are read before it is used.

## `rgui`: UI migration tool

A tool that reads rg-ui (Base UI, shadcn, Tailwind CSS) and emits the Rust equivalents:
pure Bevy UI, pure Taffy layouts, pure Dioxus components, the native shell, and GPUI / GPUI component
kits. Assets of the monorepo end up either as SVG in code or as objects on R2.

Status of the targets (2026-11): Dioxus 0.7 stable; dioxus-native 0.8.0-alpha.3 (Blitz 0.3 beta,
renderer still experimental, desktop/mobile stable path still uses a WebView); Taffy is shared by
Zed, Bevy UI and Blitz.

## Self-hosting: one binary per product

Replace Vercel / Cloudflare-as-host / nginx / Docker / Coolify with self-hostable binaries:

| Layer today | Replacement | Why |
| --- | --- | --- |
| nginx (host + stack), Docker compose | a Rust edge built on Pingora (Cloudflare's proxy library) inside the product binary | same language and async runtime as the apps; ACME handled in code (as Tako does) |
| Quick standalone proxy if needed | Caddy 2.11.7 (Go, automatic HTTPS, single binary) | lowest operational risk |
| Hot-reconfigurable proxy | Sozu (Rust, control socket, live config without restart; HTTP/1 only per Ferron's benchmarks) | candidates only |
| Docker / Coolify deploys | `bun build --compile` (Bun side) and `cargo build --release` LTO binaries, run by systemd, deployed by `infra/ops` | no container runtime needed |
| Cloudflare | stays in front (DNS, cache, R2, Workers) but optional: the binary serves without it | |

`apps/rosegriffon` ("one self-hostable executable") is the Bun-side starting point.

## Branches

- `main`: frozen at 311e9e8b.
- `bun`: TypeScript to pure Bun 1.4.5 and the global Bun packages.
- `rust`: Achillea, Azalee, nie port to Dioxus + Bevy, the merged Rust game area, the FFI contract.
