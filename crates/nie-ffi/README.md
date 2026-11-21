# nie-ffi

Native C ABI for Inazuma Eleven reverse-engineering and game-data tooling. The active library
uses workspace implementations from `nie-explore`, `nie-data`,
`nie-formats` and `nie-save`.

## Current build profile

The default profile exports native format inspection and decode, CFG.bin and typed menu
settings, CPK/CRI-LAYLA, G4TX, local VFS, font and texture helpers, IEVR save inspection, shared
game-source discovery, native domain queries over mounted VFS data.
The capability report describes this profile at runtime. Emulator, WASM, and WGPU integrations
are excluded.
