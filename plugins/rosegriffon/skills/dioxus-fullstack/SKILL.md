---
name: dioxus-fullstack
description: "Use when working with the `dioxus-fullstack` crate of dioxus: Library for fetching resources from servers in Dioxus apps."
---

# dioxus-fullstack

- Version: 0.8.0-alpha.3 (workspace member of `third_party/dioxus`, vendored commit `525d4026c949`)
- Source: `third_party/dioxus/packages/fullstack/` (manifest `third_party/dioxus/packages/fullstack/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/dioxus/tree/main/packages/fullstack
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Library for fetching resources from servers in Dioxus apps.

## README (`third_party/dioxus/packages/fullstack/README.md`)

# dioxus-fullstack

- wraps dioxus-server
- exposes rpc system

## Cargo features (from the manifest)

- `default` (enables 1)
- `web` (enables 1)
- `native`
- `server` (enables 11)
- `postcard` (enables 1)
- `msgpack` (enables 1)
- `ws` (enables 2)

## Depends on (same ecosystem)

`dioxus-asset-resolver`, `dioxus-fullstack-core`, `dioxus-fullstack-macro`, `dioxus-hooks`, `dioxus-html`, `dioxus-signals`, `dioxus-vdom`, `dx-env`

## Public items by file

- `third_party/dioxus/packages/fullstack/src/client.rs`: fn install_default_crypto_provider, type ClientResult, struct ClientRequest, struct ClientResponse, fn set_server_url, fn get_server_url, fn clear_request_headers, fn set_request_headers, fn get_request_headers, trait ClientResponseDriver
- `third_party/dioxus/packages/fullstack/src/encoding.rs`: trait Encoding, struct JsonEncoding, struct CborEncoding, struct PostcardEncoding, struct MsgPackEncoding
- `third_party/dioxus/packages/fullstack/src/lazy.rs`: struct Lazy
- `third_party/dioxus/packages/fullstack/src/lib.rs`: mod magic, mod request, mod encoding, mod lazy, mod payloads
- `third_party/dioxus/packages/fullstack/src/magic.rs`: struct ServerFnEncoder, struct ServerFnDecoder, enum RestEndpointPayload, struct ErrorPayload, fn reqwest_response_to_serverfn_err, fn reqwest_error_to_request_error, mod req_to, mod req_from
- `third_party/dioxus/packages/fullstack/src/payloads/cbor.rs`: struct Cbor, enum CborRejection
- `third_party/dioxus/packages/fullstack/src/payloads/files.rs`: struct FileStream
- `third_party/dioxus/packages/fullstack/src/payloads/header.rs`: struct SetHeader
- `third_party/dioxus/packages/fullstack/src/payloads/msgpack.rs`: struct MsgPack, struct MsgPackRaw, enum MsgPackRejection
- `third_party/dioxus/packages/fullstack/src/payloads/multipart.rs`: struct MultipartFormData
- `third_party/dioxus/packages/fullstack/src/payloads/postcard.rs`: struct Postcard, enum PostcardRejection
- `third_party/dioxus/packages/fullstack/src/payloads/query.rs`: struct Query
- `third_party/dioxus/packages/fullstack/src/payloads/sse.rs`: struct ServerEvents, struct ServerSentEvent
- `third_party/dioxus/packages/fullstack/src/payloads/stream.rs`: type TextStream, type ByteStream, type JsonStream, type CborStream, type ChunkedByteStream, type ChunkedTextStream, struct Streaming, enum StreamingError, fn encode_stream_frame, fn decode_stream_frame
- `third_party/dioxus/packages/fullstack/src/payloads/text.rs`: struct Text
- `third_party/dioxus/packages/fullstack/src/payloads/websocket.rs`: fn use_websocket, struct UseWebsocket, enum WebsocketState, struct Websocket, struct WebSocketOptions, struct UpgradingWebsocket, struct TypedWebsocket, enum WebsocketError, enum Message, enum CloseCode
- `third_party/dioxus/packages/fullstack/src/request.rs`: trait IntoRequest, trait FromResponse, trait FromResponseParts, struct ServerFnRequest, trait AssertIsResult, fn assert_is_result, trait AssertCanEncode, struct CantEncode, struct EncodeIsVerified, trait AssertCanDecode, struct CantDecode, struct DecodeIsVerified, fn assert_can_encode, fn assert_can_decode

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
