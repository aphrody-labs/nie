---
name: blitz-traits
description: "Use when working with the `blitz-traits` crate of blitz: Shared traits and types for Blitz."
---

# blitz-traits

- Version: 0.3.0-beta.4 (workspace member of `third_party/blitz`, vendored commit `a84a3c51b562`)
- Source: `third_party/blitz/packages/blitz-traits/` (manifest `third_party/blitz/packages/blitz-traits/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/blitz/tree/main/packages/blitz-traits
- License: [object Object]
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Shared traits and types for Blitz

## Crate docs (`third_party/blitz/packages/blitz-traits/src/lib.rs`)

Types and traits to enable interoperability between the other Blitz crates without
circular or unnecessary dependencies.

## Public items by file

- `third_party/blitz/packages/blitz-traits/src/devtools.rs`: struct DevtoolSettings, enum MutationRecord, struct NodeInserted, struct NodeRemoved, struct AttributeChanged, struct CharacterDataChanged, trait DevtoolsProvider
- `third_party/blitz/packages/blitz-traits/src/events.rs`: struct EventState, enum UiEvent, struct DomEvent, enum DomEventKind, enum DomEventData, struct HitResult, enum BlitzPointerId, struct PointerCoords, struct PointerDetails, struct Point, struct BlitzPointerEvent, enum BlitzWheelDelta, struct BlitzWheelEvent, struct BlitzScrollEvent, enum MouseEventButton, enum KeyState, struct BlitzKeyEvent, struct BlitzInputEvent, struct BlitzFocusEvent, enum BlitzImeEvent
- `third_party/blitz/packages/blitz-traits/src/lib.rs`: mod devtools, mod events, mod navigation, mod net, mod node_id, mod shell
- `third_party/blitz/packages/blitz-traits/src/navigation.rs`: trait NavigationProvider, struct DummyNavigationProvider, struct NavigationOptions
- `third_party/blitz/packages/blitz-traits/src/net.rs`: trait NetProvider, trait NetHandler, trait NetWaker, struct Request, enum Body, struct FormData, struct Entry, enum EntryValue, struct DummyNetProvider, struct AbortController, struct AbortSignal
- `third_party/blitz/packages/blitz-traits/src/node_id.rs`: struct NodeId
- `third_party/blitz/packages/blitz-traits/src/shell.rs`: struct ClipboardError, trait ShellProvider, struct DummyShellProvider, enum ColorScheme, struct Viewport, struct FileDialogFilter

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
