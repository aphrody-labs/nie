# nie-ocgen: 3D generation chain for an original character

An original character (OC) exists as drawings and text. The game only knows `cfg.bin` documents and
`20_EDIT` meshes. This crate links the two in steps; each step writes a file a person can read.

No image is generated. Each colour is the dominant group, in Oklab, of a rectangle a person placed on
one of the author's sheets; each slot hash is copied from a document the game ships.

## Steps

| Step | Input | Output |
| --- | --- | --- |
| [`sources`](src/sources.rs) | dumps of `data/common/`, `data/oc/<slug>/source/` | editor tables, decoded sheets |
| [`palette`](src/palette.rs) | sheets and declared regions | measured colours with their provenance |
| [`morphology`](src/morphology.rs) | the shipped `mdl_editpreview_avatar_*` documents | a starting model |
| [`recipe`](src/recipe.rs) | starting model, measures, bindings | a `CHARA_EDIT_PARAM` document |
| [`plan`](src/plan.rs) | the document and the checkout | what is still missing to assemble a mesh |

## What the crate asserts

- The emitted document is in the game's format. [`param`](src/param.rs) reads the shipped `CHARA_EDIT_PARAM`
  documents (the `mdl_edit_avatar*` presets, the `mdl_editpreview_avatar*` morphologies, the `edit_parameter`
  save template) and re-emits them byte-identically; `tests/format_roundtrip.rs` checks it. The module never
  invents a slot hash.
- Every change is justified: the run report lists, for each written value, the sheet, the rectangle, the share
  of the dominant group and the number of pixels kept.

## What it does not assert

That a character renders, resembles its sheets, or loads in the game. No mesh is assembled here: assembly needs
the `20_EDIT` archives, and [`plan`](src/plan.rs) reports their absence instead of substituting anything.

## Binding a role to a slot

The colour slots are hashes whose preimages this repository has not recovered, so they cannot be named. The
binding is declared in the recipe, and `nie-ocgen slots` prints the evidence to declare it from: per slot, the
number of rows, the number of documents and sample colours.

## Background rejection

The background is measured on the corners of each sheet and rejected as a radius around that colour. A generic
"light and unsaturated is paper" rule discarded pale hair that sits close to the paper colour.

## Tagged JSON to file

`nie_formats::cfgbin::encode_tagged_t2b` is the inverse of `t2b_to_tagged_json`. `nie-ocgen` writes a `cfg.bin`
only after reading its own bytes back: a file this repository cannot decode identically is never written.

- `nie_explore::bridge::json_to_t2b_entries` inverts `t2b_to_json`, not the tagged form; it would write
  `CHARA_EDIT_PARAM_0` in the key table instead of `CHARA_EDIT_PARAM`.
- A T2B container is recognised by its name (`_BEG`, `_BEGIN`, `PTREE`); a parent named otherwise is refused
  instead of losing its children.

A file produced here re-reads; it is not proven loadable. `encode_t2b` does not yet reproduce every game file
byte for byte.

## Portrait icons

No lock blocks this asset. What distinguishes the two 256x256 sub-textures of the reference in game is not
measured, so each slot gets its own declared framing. The container is re-parsed and each sub-texture is
re-decoded to PNG; a valid structure does not prove the right pixels are in it.

The default payload is uncompressed BGRA8. BC7 (the game's format) is behind the `bc7` feature: `intel_tex_2`
ships its ISPC kernels for MSVC only, and the link fails on `windows-gnu`.

## Commands

```bash
nie-ocgen morphologies              # the starting documents, their body type and traits
nie-ocgen slots                     # the colour slot survey, the evidence for bindings
nie-ocgen measure astro-lor         # the colours, nothing generated
nie-ocgen run astro-lor             # the whole chain, output in var/ocgen/ (JSON and cfg.bin)
nie-ocgen icons astro-lor           # the portrait G4TX containers and their re-decoded PNG
nie-ocgen encode <dir> --out X      # every tagged *.cfg.bin.json to cfg.bin
```

`nie oc <subcommand>` is the canonical surface and `nie ocgen <subcommand>` is an alias; both call
[`cli::run`](src/cli.rs). The logic stays in the library.

Profiles and sheets are read from the `--data` root (default `data`, not tracked), for example
`<data>/oc/<slug>/game/complete-profile.json`.
