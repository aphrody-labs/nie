# Victory Road Virtual File System

Format and packing notes for the IEVR VFS, derived from `nie.exe` and the parsers in `crates/nie-formats`.
`nie vfs ls`, `nie vfs stats` and `nie vfs formats` are authoritative for what a given installation holds.

---

## 1. Overview

The VFS has two tiers:

1. **CRI CPK archives** (`data/packs/*.cpk`):
   - Indexed at game boot by `data/cpk_list.cfg.bin`.
   - XOR-encrypted per position with a key derived from the CRC32 of the archive file name (section 4.2).
   - Compressed with CRILAYLA (reverse-bitstream LZSS).
   - Cached in memory by an LRU bounded by `NIE_CPK_CACHE_BUDGET_GIB` (default 2 GiB).
2. **Loose file overlay**:
   - A file present in the loose directory tree overrides the CPK entry of the same path.

---

## 2. Binary formats

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

## 3. Packs and manifest

`data/cpk_list.cfg.bin` maps internal paths to pack files. It is a cfg.bin encrypted with the fixed Viola key
`0x1717E18E` (`VIOLA_FIXED_KEY`, `crates/nie-formats/src/cpk.rs`). The packs under `data/packs/` are named by hash
(for example `1d08dda99e8549431c6c7b459ed8deab.cpk`) and use the per-file-name key, not the Viola key: the Viola
key applied to a real pack gives noise.

Top-level logical directories are `data/common/*` and `data/dx11/*`; `nie vfs ls data` lists them.

---

## 4. Decompression & Cryptography Reference

### 4.1 CRILAYLA LZSS Decompression

- **Header**: 16 bytes starting with ASCII `CRILAYLA`.
- **Offset +8**: 32-bit unsigned LE integer specifying uncompressed payload size.
- **Offset +12**: 32-bit unsigned LE integer specifying compressed bitstream size.
- **Direction**: Bitstream is processed **backwards** starting from the final byte of the compressed stream towards offset `+16`.
- **History Buffer**: 256 bytes (`0x100`) circular dictionary.

### 4.2 CRI Position-Aware XOR Cipher

- `decrypt_block` (`crates/nie-formats/src/cpk.rs`) XORs the buffer one 4-byte word at a time. For the word at
  file offset `o`, `update_crc_state(o, key)` gives a CRC state and `compute_xor_byte(state, position)` the mask
  byte of each of the 4 positions. The cipher is its own inverse and works on any absolute file offset.
- For a pack the key is `key_from_filename(<base file name>)`: standard CRC32 (init and xorout `0xFFFFFFFF`) of the UTF-8 name.
