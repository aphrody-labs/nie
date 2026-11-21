# Research protocol

nie is a research project on Inazuma Eleven: Victory Road, framed and documented with Level-5. Its
results must be reproducible by a third party from this repository and a legitimately owned
installation of the game. This file is the rule every document, experiment and change follows.

## What a result needs

1. **Question or hypothesis**, stated before the method.
2. **Method**: the exact commands, in the repository (a `nie` subcommand, a test, an example or a
   script under version control). A command that only exists in a chat or a scratch directory does
   not count.
3. **Environment fingerprint**, recorded with the result:
   - git commit of this repository (and of each sibling checkout involved);
   - Rust toolchain (`rust-toolchain.toml`) and `Cargo.lock` hash;
   - game: Steam app 2799860, `buildid` from `steamapps/appmanifest_2799860.acf`, size and hash of
     `nie.exe` and of `data/cpk_list.cfg.bin`;
   - host: name, kernel (`uname -r`), CPU model, memory, filesystem of the game directory.
4. **Data**: inputs are named by VFS path and buildid, never copied into the repository. Outputs
   are summarised in the document and identified by a content hash (BLAKE3 or SHA-256); bulky raw
   outputs stay on the build host with their hash recorded here.
5. **Result** with its measured numbers and units, and **limits**: what was not measured, what
   could invalidate it.
6. **Verification by a second path** where one exists (a second decoder, the original binary, an
   independent tool, the official source of a specification).

## Where things go

| Kind | Location |
|---|---|
| Experiment records | `docs/research/log/YYYY-MM-DD-<topic>.md` |
| Specifications we rely on, with official sources and versions | `docs/specs/` |
| Host and deployment changes (runbooks, exact commands, rollback) | `docs/research/log/` until the Rust deploy tool encodes them; then the tool is the reference |
| Questions for the Level-5 partner review (licensing, online services, anti-cheat) | listed, never decided by contributors |

## Rules

- No game asset in the repository; tests use synthetic fixtures.
- Versions are pinned (`Cargo.lock`, toolchain file, pinned git revisions) and read from
  their official source when chosen.
- A change to a production host is described here, with its verification output and its rollback,
  in the same commit series as the change.
- Numbers come from a command run in the recorded environment; estimates are labelled as such.
