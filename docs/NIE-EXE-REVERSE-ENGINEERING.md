# nie.exe reverse engineering notes

`nie index`, `nie rtti`, `nie disasm`, `nie pdata` and `nie exe lint` are authoritative for the functions, classes
and layouts of `nie.exe` (Inazuma Eleven: Victory Road, PE32+, x86_64, image base `0x140000000`). This file
keeps only the constants and layouts that the parsers in this repository implement.

## Constants

| Constant | Value | Where |
| --- | --- | --- |
| `VIOLA_FIXED_KEY` | `0x1717E18E` | `crates/nie-formats/src/cpk.rs`: key of `cpk_list.cfg.bin` and packed cfg.bin. CPK files use a key derived from their file name (`docs/VFS-SPECIFICATION.md`). |
| CRC32 polynomial | `0xEDB88320` | RDBN hashes and the CPK file-name key |
| Lives save magic | `0x9DCE66C3` | `crates/nie-formats/src/lib.rs`: first u32 of a decrypted save container (`002AB8F4-*`) |
| Steam app id | `2799860` (`0x2AB8F4`) | prefix of the save file names |

## `font.cfg.bin` glyph record

Glyph metrics are T2B `CHR` entries with `variables = [font, base, codepoint, atlasX, atlasY, width, bearingX, advance, page]`;
`page` in 0..3 selects the R, G, B or A mask of the atlas (`crates/nie-formats/src/font.rs`).

## RDBN (`cfg.bin`) header

Little-endian, from `crates/nie-formats/src/cfgbin.rs`:

| Offset | Size | Field |
| --- | --- | --- |
| `0x00` | 4 | magic `RDBN` |
| `0x04` | 2 | `header_size` (usually `0x50`) |
| `0x06` | 4 | `version` (usually `0x64`) |
| `0x0A` | 2 | `data_offset` (x4 = absolute start of the data section) |
| `0x0C` | 4 | `data_size` |
| `0x24` | 2 + 2 | `type_offset`, `type_count` |
| `0x28` | 2 + 2 | `field_offset`, `field_count` |
| `0x2C` | 2 + 2 | `root_offset`, `root_count` |
| `0x30`..`0x38` | | string hash, string offsets, `hash_count`, value offset |
| `0x38` | 4 | `string_offset` |

Offsets from `0x24` on are relative to `data_offset`; the type, field and root tables use 32-byte entries.
