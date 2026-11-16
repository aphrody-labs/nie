---
name: bevy-remote
description: "Use when working with the `bevy_remote` crate of bevy: The Bevy Remote Protocol."
---

# bevy_remote

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_remote/` (manifest `third_party/bevy/crates/bevy_remote/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_remote
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

The Bevy Remote Protocol

## Crate docs (`third_party/bevy/crates/bevy_remote/src/lib.rs`)

An implementation of the Bevy Remote Protocol, to allow for remote control of a Bevy app.

Adding the [`RemotePlugin`] to your [`App`] will setup everything needed without
starting any transports. To start accepting remote connections you will need to
add a second plugin like the [`RemoteHttpPlugin`](http::RemoteHttpPlugin) to enable communication
over HTTP. These *remote clients* can inspect and alter the state of the
entity-component system.

The Bevy Remote Protocol is based on the JSON-RPC 2.0 protocol.

## Request objects

A typical client request might look like this:

```json
{
    "method": "world.get_components",
    "id": 0,
    "params": {
        "entity": "5v0",
        "components": [
            "bevy_transform::components::transform::Transform"
        ]
    }
}
```

The `id` and `method` fields are required. The `params` field may be omitted
for certain methods:

* `id` is arbitrary JSON data. The server completely ignores its contents,
  and the client may use it for any purpose. It will be copied via
  serialization and deserialization (so object property order, etc. can't be
  relied upon to be identical) and sent back to the client as part of the
  response.

* `method` is a string that specifies one of the possible [`BrpRequest`]
  variants: `world.query`, `world.get_components`, `world.insert_components`, etc. It's case-sensitive.

* `params` is parameter data specific to the request.

For more information, see the documentation for [`BrpRequest`].
[`BrpRequest`] is serialized to JSON via `serde`, so [the `serde`
documentation] may be useful to clarify the correspondence between the Rust
structure and the JSON format.

## Response objects

A response from the server to the client might look like this:

```json
{
    "jsonrpc": "2.0",
    "id": 0,
    "result": {
        "bevy_transform::components::transform::Transform": {
            "rotation": { "x": 0.0, "y": 0.0, "z": 0.0, "w": 1.0 },
            "scale": { "x": 1.0, "y": 1.0, "z": 1.0 },
            "translation": { "x": 0.0, "y": 0.5, "z": 0.0 }
        }

## Cargo features (from the manifest)

- `default` (enables 3)
- `http` (enables 5)
- `client` (enables 6)
- `bevy_asset` (enables 1)
- `bevy_debug_stepping` (enables 1)
- `bevy_render` (enables 2)

## Depends on (same ecosystem)

`bevy_annotations`, `bevy_app`, `bevy_asset`, `bevy_color`, `bevy_derive`, `bevy_dev_tools`, `bevy_diagnostic`, `bevy_ecs`, `bevy_input`, `bevy_log`, `bevy_math`, `bevy_platform`, `bevy_reflect`, `bevy_render`, `bevy_tasks`, `bevy_utils`, `bevy_window`

## Public items by file

- `third_party/bevy/crates/bevy_remote/src/builtin_methods.rs`: struct BrpGetComponentsParams, struct BrpGetResourcesParams, struct BrpQueryParams, struct BrpSpawnEntityParams, struct BrpDespawnEntityParams, struct BrpRemoveComponentsParams, struct BrpRemoveResourcesParams, struct BrpInsertComponentsParams, struct BrpInsertResourcesParams, struct BrpReparentEntitiesParams, struct BrpListComponentsParams, struct BrpMutateComponentsParams, struct BrpMutateResourcesParams, struct BrpTriggerEventParams, struct BrpWriteMessageParams, struct BrpObserveParams, struct BrpQuery, struct BrpQueryFilter, enum NamePathSegment, struct BrpJsonSchemaQueryFilter, struct JsonSchemaTypeLimit, struct BrpJsonSchemaResponse, struct BrpSpawnEntityResponse, enum BrpGetComponentsResponse, ...
- `third_party/bevy/crates/bevy_remote/src/client.rs`: struct BrpClient, enum BrpClientError
- `third_party/bevy/crates/bevy_remote/src/http.rs`: struct Headers, struct RemoteHttpPlugin, struct HostAddress, struct HostPort
- `third_party/bevy/crates/bevy_remote/src/input_methods.rs`: struct BrpClickParams, fn process_remote_click_request, struct BrpKeyParams, struct BrpTypeParams, fn process_remote_key_request, fn process_remote_type_request
- `third_party/bevy/crates/bevy_remote/src/inspection_methods.rs`: struct BrpInspectParams, struct BrpInspectComponentParams, struct BrpInspectComponentTypeParams, struct BrpInspectResourceParams, struct BrpInspectAllResourcesParams, struct BrpSummarizeParams, fn process_remote_inspect_request, fn process_remote_inspect_component_request, fn process_remote_inspect_component_type_request, fn process_remote_inspect_resource_request, fn process_remote_inspect_all_resources_request, fn process_remote_summarize_request, fn process_remote_component_metadata_request
- `third_party/bevy/crates/bevy_remote/src/lib.rs`: mod builtin_methods, mod client, mod http, mod input_methods, mod inspection_methods, mod schemas, mod stepping_methods, struct RemotePlugin, struct RemoteLast, enum RemoteSystems, enum RemoteMethodHandler, type RemoteInstantMethodSystemId, type RemoteWatchingMethodSystemId, type RemoteMethodSystemId, enum RemoteMethodKind, struct RemoteMethod, struct RemoteMethods, struct RemoteWatchingRequests, struct BrpRequest, struct BrpResponse, enum BrpPayload, struct BrpError, mod error_codes, type BrpResult, ...
- `third_party/bevy/crates/bevy_remote/src/schemas/index.rs`: struct SchemaRegistry, struct TypeSchema, struct SchemaField, struct SchemaVariant
- `third_party/bevy/crates/bevy_remote/src/schemas/json_schema.rs`: trait TypeRegistrySchemaReader, fn export_type, struct JsonSchemaBevyType, struct SchemaAnnotations, enum SchemaKind, enum SchemaType, struct ComponentMetadata, enum StorageKind, enum RelationshipKind, trait SchemaJsonType
- `third_party/bevy/crates/bevy_remote/src/schemas/mod.rs`: mod index, mod json_schema, mod open_rpc, struct SchemaTypesMetadata
- `third_party/bevy/crates/bevy_remote/src/schemas/open_rpc.rs`: struct OpenRpcDocument, struct InfoObject, struct ServerObject, struct MethodObject, struct Parameter
- `third_party/bevy/crates/bevy_remote/src/stepping_methods.rs`: struct BrpSteppingCursor, struct BrpSteppingStatusResponse

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
