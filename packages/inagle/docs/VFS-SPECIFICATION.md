# Victory Road Virtual File System (VFS) Specification

> Comprehensive, authoritative technical specification for the Inazuma Eleven: Victory Road Virtual File System.
> Grounded in `aphrody-nie-formats`, `IECODE.Core.CriFs`, and reverse engineering of `nie.exe`.

---

## 1. Architectural Overview

The Victory Road VFS combines two distinct mounting tiers:

1. **Mounted CRI CPK Archives** (`data/packs/*.cpk`):
   - High-density packages indexed at game boot via `cpk_list.cfg.bin`.
   - Protected with CRI position-dependent XOR cipher and Viola key (`0x1717E18E`).
   - Compressed with reverse-bitstream CRILAYLA LZSS.
   - Cached in memory using an LRU cache budget (default 16 GiB on desktop).
2. **Loose File Dump Overlay**:
   - For fast modding, testing, and tool ingestion, the VFS inspects loose files on disk.
   - When a requested file path exists in the loose directory tree, it overrides the CPK archive entry.

---

## 2. Authoritative Binary Formats Catalog (28 Formats)

| Magic (Hex)  | ASCII   | Extension   | Category      | Endianness | Encrypted | Compressed | Description                                                                                    |
| ------------ | ------- | ----------- | ------------- | ---------- | --------- | ---------- | ---------------------------------------------------------------------------------------------- |
| `0x58543447` | `G4TX`  | `.g4tx`     | **texture**   | Little     | No        | Yes        | Level-5 Texture Container (main DDS BCn textures + atlas sprite subregions)                    |
| `0x444D3447` | `G4MD`  | `.g4md`     | **model**     | Little     | No        | Yes        | Level-5 3D Model Data (meshes, bone indices, vertex buffers, materials)                        |
| `0x474D3447` | `G4MG`  | `.g4mg`     | **model**     | Little     | No        | Yes        | Level-5 Model Group (collection of submeshes and LOD levels)                                   |
| `0x544D3447` | `G4MT`  | `.g4mt`     | **model**     | Little     | No        | No         | Level-5 Material Properties (shaders, blend modes, diffuse/specular colors)                    |
| `0x4B533447` | `G4SK`  | `.g4sk`     | **model**     | Little     | No        | No         | Level-5 Skeleton & Bone Hierarchy (quaternions, bind poses, transform trees)                   |
| `0x4B503447` | `G4PK`  | `.g4pk`     | **container** | Little     | No        | Yes        | Level-5 Package Container (multi-resource bundle for characters and stadiums)                  |
| `0x41523447` | `G4RA`  | `.g4ra`     | **container** | Little     | No        | Yes        | Level-5 Resource Archive (compact asset bundle indexed in nie.exe)                             |
| `0x20494741` | `AGI `  | `.agi`      | **model**     | Little     | No        | Yes        | Level-5 Animation Graphic Interface (timeline tracks, animation nodes)                         |
| `0x424A424F` | `OBJB`  | `.objb`     | **config**    | Little     | No        | Yes        | Level-5 Object Binary (scene layout descriptors, actor placements)                             |
| `0x4C435850` | `PXCL`  | `.pxcl`     | **system**    | Little     | No        | No         | Level-5 Pixel Collision & Physics Hull (hitboxes, boundary colliders)                          |
| `0x4E424452` | `RDBN`  | `.cfg.bin`  | **config**    | Little     | Yes       | Yes        | Level-5 Relational Database Binary (T2B structured binary data sheets)                         |
| `0x5346454D` | `MEVS`  | `.mevbin`   | **script**    | Little     | No        | Yes        | Level-5 Motion Event Script (choreographed cutscenes, camera paths, audio cues)                |
| `0x4D433447` | `G4CM`  | `.g4cm`     | **script**    | Little     | No        | No         | Level-5 Camera Track & Spline Path (cinematic parameters, FOV transitions)                     |
| `0x204B5043` | `CPK `  | `.cpk`      | **container** | Big        | Yes       | Yes        | CRI Middleware File Package (TOC, ITOC, ETOC, @UTF directory descriptors)                      |
| `0x32534641` | `AFS2`  | `.afs2`     | **audio**     | Little     | Yes       | No         | Criware Audio Archive (AWB wavebank containers)                                                |
| `0x00465455` | `@UTF`  | `.utf`      | **container** | Big        | Yes       | No         | CRI Universal Table Format (schema, column flags, string pools, row vectors)                   |
| `0x20534444` | `DDS `  | `.dds`      | **texture**   | Little     | No        | No         | DirectDraw Surface (raw BC1/BC2/BC3/BC4/BC5/BC7 compressed texture payload)                    |
| `0x474E5089` | `PNG`   | `.png`      | **texture**   | Big        | No        | Yes        | Portable Network Graphics (decoded atlas glyphs, UI elements, web exports)                     |
| `0x46464952` | `RIFF`  | `.wav`      | **audio**     | Little     | No        | No         | Resource Interchange File Format (PCM and ADPCM audio tracks)                                  |
| `0x4B435058` | `XPCK`  | `.xpck`     | **container** | Little     | No        | Yes        | Level-5 XPCK Package (legacy DS/3DS/Switch container format)                                   |
| `0x41534658` | `XFSA`  | `.xfsa`     | **container** | Little     | No        | Yes        | Level-5 XFSA Archive (file system archive with hash-addressed records)                         |
| `0x4159414C` | `LAYL`  | `.crilayla` | **container** | Little     | No        | Yes        | CRI Layla Reverse-Bitstream LZSS Compressed Container (16-byte header + bitstream + 0x100 raw) |
| `0x46424341` | `@ACB`  | `.acb`      | **audio**     | Big        | Yes       | No         | Criware Audio Cue Sheet (cue mappings, sound triggers, DSP effects)                            |
| `0x20534D43` | `CRID`  | `.usm`      | **video**     | Big        | Yes       | No         | CRI Movie 2 Container (video stream, multi-track audio, subtitle packets)                      |
| `0x564E3447` | `G4NV`  | `.g4nv`     | **system**    | Little     | No        | No         | Level-5 Navigation Mesh (stadium and field locomotion polygons)                                |
| `0x004C4F43` | `COL\0` | `.col`      | **system**    | Little     | No        | No         | Level-5 Collision mesh data                                                                    |
| `0x534C4F43` | `COLS`  | `.cols`     | **system**    | Little     | No        | No         | Level-5 Collision Set (stadium boundaries, goal posts, ball bouncing volumes)                  |
| `0x9DCE66C3` | `LIVE`  | `.live`     | **system**    | Little     | Yes       | No         | Level-5 Lives Save Container (002AB8F4-* CRC32-keyed XOR stream cipher, 0xEEFF blobs)          |

---

## 3. CPK Package Mapping & Priority Rules

| Archive Name       | Wildcard Mask      | Key Derivation   | Priority | CRILAYLA | Role / Contents                                                                                       |
| ------------------ | ------------------ | ---------------- | -------- | -------- | ----------------------------------------------------------------------------------------------------- |
| `common.cpk`       | `*common*.cpk`     | `filename-crc32` | 100      | Yes      | Core game database, character tables (chara_base), item configs, skill definitions, and growth curves |
| `menu.cpk`         | `*menu*.cpk`       | `filename-crc32` | 90       | Yes      | UI layouts, T2B menu settings, 200_icon sprites, portrait textures, and font atlases                  |
| `model.cpk`        | `*model*.cpk`      | `filename-crc32` | 80       | Yes      | 3D player models (G4MD), team kits, stadium geometries, and animation tracks (AGI)                    |
| `dx11.cpk`         | `*dx11*.cpk`       | `filename-crc32` | 70       | Yes      | DirectX 11 compiled shaders, material descriptors, and GPU texture streams                            |
| `sound_asset.cpk`  | `*sound*.cpk`      | `filename-crc32` | 60       | No       | HCA voice lines, character dialogue, match commentary, and crowd atmosphere                           |
| `movie.cpk`        | `*movie*.cpk`      | `filename-crc32` | 50       | No       | Full-motion cutscenes, anime openings, and special moves (Hissatsu) USM video sequences               |
| `event.cpk`        | `*event*.cpk`      | `filename-crc32` | 40       | Yes      | Story mode cutscenes, MEVBIN choreography scripts, and chapter dialogue triggers                      |
| `cpk_list.cfg.bin` | `cpk_list.cfg.bin` | `viola-fixed`    | 1000     | No       | Master VFS manifest mapping internal paths to CPK pack files. Encrypted with Viola key 0x1717E18E     |

---

## 4. Top-Level Directory Topology

| Logical Directory | Dominant Format | Est. Files | Subsystem Role                                                                                                       | Sample Entries                                                                           |
| ----------------- | --------------- | ---------- | -------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------- |
| `data/common/`    | `.cfg.bin`      | ~1420      | Authoritative relational database configs (RDBN format) powering all game rules, player statistics, and drop tables. | `data/common/chara/chara_base.cfg.bin`, `data/common/item/item_config.cfg.bin`           |
| `data/menu/`      | `.g4tx`         | ~3866      | User interface assets, HUD compositions, sprite sheets, font glyphs, and localized texture cards.                    | `data/menu/200_icon/icon_chara_c01000010.g4tx`, `data/menu/font/font_menu.g4tx`          |
| `data/model/`     | `.g4md`         | ~12400     | 3D skinned meshes, bone palettes, skeleton hierarchies, material definitions, and animation bindings.                | `data/model/chara/c01000010/c01000010.g4md`, `data/model/chara/c01000010/c01000010.g4sk` |
| `data/event/`     | `.mevbin`       | ~5173      | Cinematic cutscenes, camera choreography, dialogue synchronizers, and story progression state trees.                 | `data/event/ch01/ev_01_001.mevbin`, `data/event/ch01/ev_01_001.cfg.bin`                  |
| `data/audio/`     | `.acb`          | ~850       | Criware CRI Atom Craft audio cue sheets, HCA stream banks, music loops, and synchronized voice tracks.               | `data/audio/bgm/bgm_match_01.acb`, `data/audio/voice/v_c01000010.acb`                    |
| `data/saves/`     | `.live`         | ~10        | Player career saves, team rosters, gacha inventory, and trophy progress encrypted with CRC32 filename cipher.        | `data/saves/002AB8F4-SYSTEMLIVE`, `data/saves/002AB8F4-USERDATALIVE`                     |

---

## 5. Decompression & Cryptography Reference

### 5.1 CRILAYLA LZSS Decompression

- **Header**: 16 bytes starting with ASCII `CRILAYLA`.
- **Offset +8**: 32-bit unsigned LE integer specifying uncompressed payload size.
- **Offset +12**: 32-bit unsigned LE integer specifying compressed bitstream size.
- **Direction**: Bitstream is processed **backwards** starting from the final byte of the compressed stream towards offset `+16`.
- **History Buffer**: 256 bytes (`0x100`) circular dictionary.

### 5.2 CRI Position-Aware XOR Cipher

- Plaintext blocks are transformed in 4-byte quadwords:
  ```c
  uint32_t quad_mask = key ^ ((offset * 0x41C64E6D + 0x3039) & 0xFFFFFFFF);
  plaintext[i] = ciphertext[i] ^ ((uint8_t*)&quad_mask)[i % 4];
  ```
- Key is generated via reflected CRC32 (`0xEDB88320`) of the archive filename string.
