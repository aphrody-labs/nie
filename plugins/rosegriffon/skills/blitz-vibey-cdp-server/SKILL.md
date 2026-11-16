---
name: blitz-vibey-cdp-server
description: "Use when working with the `blitz-vibey-cdp-server` crate of blitz: Chrome DevTools Protocol server implementation."
---

# blitz-vibey-cdp-server

- Version: 0.3.0-beta.4 (workspace member of `third_party/blitz`, vendored commit `a84a3c51b562`)
- Source: `third_party/blitz/packages/blitz-vibey-cdp-server/` (manifest `third_party/blitz/packages/blitz-vibey-cdp-server/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/blitz/tree/main/packages/blitz-vibey-cdp-server
- License: [object Object]
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Chrome DevTools Protocol server implementation

## Crate docs (`third_party/blitz/packages/blitz-vibey-cdp-server/src/lib.rs`)

Chrome DevTools Protocol server implementation

Implements the server side of the [Chrome DevTools Protocol] (the DOM,
CSS and Overlay domains) allowing Chrome DevTools' Elements panel to
connect to and inspect Blitz documents.

Targets are advertised over HTTP at `/json/list` (and `/json/version`),
and each target is debuggable over a WebSocket at
`/devtools/page/{target_id}` speaking plain JSON CDP messages. A top-level
document's target id is its document id. A sub-document's target id is
that of the element hosting it (`{host_doc_id}.{host_node_id}`), so the
target follows the host as it navigates to new documents (e.g. a browser
tab or an iframe). With the `embedded-frontend` feature, the server also
serves a DevTools frontend (see [`frontend_url`]).

[Chrome DevTools Protocol]: https://chromedevtools.github.io/devtools-protocol/

## Cargo features (from the manifest)

- `default` (enables 1)
- `embedded-frontend` (enables 1): Embed a DevTools frontend (about 2 MB) and serve it, so that `frontend_url` is a local URL that browsers such as Chrome and Firefox can open. Without it, `frontend_url` is a `devtools://` URL to Chrom

## Depends on (same ecosystem)

`blitz-chrome-devtools-frontend`, `blitz-dom`, `blitz-traits`

## Public items by file

- `third_party/blitz/packages/blitz-vibey-cdp-server/src/documents.rs`: fn collect_document_ids, fn with_document_in, fn picking_document_id, fn find_picking_document
- `third_party/blitz/packages/blitz-vibey-cdp-server/src/frontend.rs`: fn frontend_url
- `third_party/blitz/packages/blitz-vibey-cdp-server/src/lib.rs`: mod documents, trait DocumentProvider, trait DevtoolsWaker, enum PickerEvent, struct CdpServer, fn target_id

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
