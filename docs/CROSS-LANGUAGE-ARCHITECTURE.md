# Architecture: Pure TypeScript on Bun

> Technical design of the IECODE data stack. The Rust engines (`crates/nie-*`) and the .NET
> solution (`apps/iecode-dotnet`), together with their C-ABI bridges, were removed: Inagle is pure
> TypeScript running on Bun.

---

## 1. Architecture Overview

```
+---------------------------------------------------------------------------------+
|                       USER INTERFACES & WORKSPACE CONSUMERS                     |
|  - CLI: `inagle <command>`                                                      |
|  - Web Application: external web consumers (separate repositories)                 |
|  - Shared Components: `packages/ieverse`                                       |
+---------------------------------------+-----------------------------------------+
                                        |
+---------------------------------------v-----------------------------------------+
|                         INAGLE (`packages/inagle`, Bun)                        |
|   - Parsers: cfg.bin (RDBN), text, skills, items, teams, zukan                  |
|   - `src/nie-bridge`: Level-5 binary contracts in pure TypeScript             |
|   - `src/docs`: VFS catalog, nie.exe reverse-engineering map, OpenAPI, llms   |
+--------------------+-----------------------------------+------------------------+
                     |                                   |
+--------------------v-------------------+   +-----------v--------------------------+
|       DATA ADAPTERS                    |   |        PUBLIC CDN                    |
|   - SQLite mirror (offline / dev)      |   |   - Decoded textures, audio, video,  |
|   - PostgreSQL projections (durable)   |   |     models and raw CPK entries       |
+----------------------------------------+   +--------------------------------------+
```

---

## 2. Binary Contracts

`src/nie-bridge` implements the Level-5 binary contracts byte-exactly in TypeScript:
- **CRC32**: reflected IEEE 802.3 (`computeLevel5Crc32`) and the raw NIE accumulator (`computeNieCrc32`).
- **CPK**: filename-derived key (`deriveCpkKeyFromFilename`), Viola fixed key, position-aware XOR block decryption (`decryptCpkBlock`).
- **CRILAYLA**: reverse-bitstream LZSS decompression (`decompressCrilayla`).
- **Headers**: RDBN and G4TX probing and parsing (`probeFormat`, `parseRdbnHeader`, `parseG4txHeader`).
- **PRNG**: `PureCrand`, the MT19937 port of `lives::CRand`.

---

## 3. TypeScript Usage

```typescript
import { computeLevel5Crc32, decompressCrilayla, probeFormat } from "@rosegriffon/inagle";

// 1. Instant header probe
const probe = probeFormat(rawBuffer);
console.log("Detected format:", probe.kind);

// 2. CRILAYLA decompression
const decompressed = decompressCrilayla(compressedBytes);

// 3. Level-5 CRC32
const crc = computeLevel5Crc32("Focus");
```
