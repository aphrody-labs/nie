---
name: bevy-asset
description: "Use when working with the `bevy_asset` crate of bevy: Provides asset functionality for Bevy Engine."
---

# bevy_asset

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_asset/` (manifest `third_party/bevy/crates/bevy_asset/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_asset
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Provides asset functionality for Bevy Engine

## README (`third_party/bevy/crates/bevy_asset/README.md`)

# Bevy Asset

## Cargo features (from the manifest)

- `file_watcher` (enables 3)
- `embedded_watcher` (enables 1)
- `multi_threaded` (enables 1)
- `http` (enables 2)
- `https` (enables 4)
- `web_asset_cache`
- `asset_processor`
- `watch`
- `trace`

## Depends on (same ecosystem)

`bevy_app`, `bevy_asset_macros`, `bevy_diagnostic`, `bevy_ecs`, `bevy_platform`, `bevy_reflect`, `bevy_tasks`, `bevy_utils`

## Public items by file

- `third_party/bevy/crates/bevy_asset/src/asset_changed.rs`: struct AssetChanged, struct AssetChangedFetch, struct AssetChangedState
- `third_party/bevy/crates/bevy_asset/src/assets.rs`: struct AssetIndex, struct LoadedUntypedAsset, struct Assets, struct AssetMut, struct AssetsMutIterator, enum InvalidGenerationError
- `third_party/bevy/crates/bevy_asset/src/common_loaders/mod.rs`: mod ron
- `third_party/bevy/crates/bevy_asset/src/common_loaders/ron.rs`: struct RonLoader, struct TypedRonLoader, struct RonSaverSettings, struct RonSaver, struct TypedRonSaver, enum RonDeserializeError, enum ReflectedRonDeserializeError, enum RonSerializeError, enum ReflectedRonSerializeError
- `third_party/bevy/crates/bevy_asset/src/direct_access_ext.rs`: trait DirectAssetAccessExt
- `third_party/bevy/crates/bevy_asset/src/event.rs`: struct AssetLoadFailedEvent, struct UntypedAssetLoadFailedEvent, enum AssetEvent
- `third_party/bevy/crates/bevy_asset/src/folder.rs`: struct LoadedFolder
- `third_party/bevy/crates/bevy_asset/src/handle.rs`: struct AssetHandleProvider, struct StrongHandle, enum Handle, enum HandleTemplate, struct ArcMutexValue, fn asset_value, enum UntypedHandle, enum UntypedAssetConversionError, macro uuid_handle!
- `third_party/bevy/crates/bevy_asset/src/id.rs`: enum AssetId, enum UntypedAssetId, enum UntypedAssetIdConversionError
- `third_party/bevy/crates/bevy_asset/src/io/android.rs`: struct AndroidAssetReader
- `third_party/bevy/crates/bevy_asset/src/io/embedded/embedded_watcher.rs`: struct EmbeddedWatcher
- `third_party/bevy/crates/bevy_asset/src/io/embedded/mod.rs`: struct EmbeddedAssetRegistry, trait GetAssetServer, fn _embedded_asset_path, fn watched_path, macro load_embedded_asset!, macro embedded_path!, macro embedded_asset!, macro load_internal_asset!, macro load_internal_binary_asset!
- `third_party/bevy/crates/bevy_asset/src/io/file/file_watcher.rs`: struct FileWatcher
- `third_party/bevy/crates/bevy_asset/src/io/file/mod.rs`: struct FileAssetReader, struct FileAssetWriter
- `third_party/bevy/crates/bevy_asset/src/io/gated.rs`: struct GatedReader, struct GateOpener
- `third_party/bevy/crates/bevy_asset/src/io/memory.rs`: struct Dir, struct DirStream, struct MemoryAssetReader, struct MemoryAssetWriter, struct Data, enum Value
- `third_party/bevy/crates/bevy_asset/src/io/mod.rs`: mod android, mod embedded, mod file, mod memory, mod processor_gated, mod wasm, mod web, mod gated, enum AssetReaderError, trait Reader, trait SeekableReader, struct ReaderNotSeekableError, trait AssetReaderFuture, trait AssetReader, trait ErasedAssetReader, type Writer, type PathStream, enum AssetWriterError, trait AssetWriter, trait ErasedAssetWriter, enum AssetSourceEvent, trait AssetWatcher, struct VecReader, struct SliceReader
- `third_party/bevy/crates/bevy_asset/src/io/processor_gated.rs`: struct TransactionLockedReader
- `third_party/bevy/crates/bevy_asset/src/io/source.rs`: enum AssetSourceId, struct AssetSourceBuilder, struct AssetSourceBuilders, struct AssetSource, struct AssetSources, struct MissingAssetSourceError, struct MissingAssetWriterError, struct MissingProcessedAssetReaderError, struct MissingProcessedAssetWriterError
- `third_party/bevy/crates/bevy_asset/src/io/wasm.rs`: struct HttpWasmAssetReader
- `third_party/bevy/crates/bevy_asset/src/io/web.rs`: struct WebAssetPlugin, enum WebAssetReader
- `third_party/bevy/crates/bevy_asset/src/lib.rs`: mod asset_changed, mod io, mod meta, mod processor, mod saver, mod transformer, mod prelude, mod common_loaders, struct AssetPlugin, enum UnapprovedPathMode, enum AssetMode, enum AssetMetaCheck, trait Asset, trait AsAssetId, trait VisitAssetDependencies, trait AssetApp, struct AssetTrackingSystems, struct AssetEventSystems
- `third_party/bevy/crates/bevy_asset/src/loader.rs`: trait AssetLoader, trait ErasedAssetLoader, struct LoadedAsset, struct ErasedLoadedAsset, enum LoadDirectError, enum DeserializeMetaError, struct LoadContext, enum ReadAssetError, enum ReadAssetBytesError
- `third_party/bevy/crates/bevy_asset/src/loader_builders.rs`: struct NestedLoadBuilder
- `third_party/bevy/crates/bevy_asset/src/meta.rs`: type MetaTransform, struct AssetMeta, enum AssetAction, struct ProcessedInfo, struct ProcessDependencyInfo, struct AssetMetaMinimal, enum AssetActionMinimal, struct ProcessedInfoMinimal, trait AssetMetaDyn, trait Settings, type AssetHash
- `third_party/bevy/crates/bevy_asset/src/path.rs`: struct AssetPath, enum ParseAssetPathError
- `third_party/bevy/crates/bevy_asset/src/processor/log.rs`: enum LogEntry, trait ProcessorTransactionLogFactory, trait ProcessorTransactionLog, struct FileTransactionLogFactory, enum ReadLogError, enum ValidateLogError, enum LogEntryError
- `third_party/bevy/crates/bevy_asset/src/processor/mod.rs`: struct AssetProcessor, struct AssetProcessorData, enum ProcessResult, enum ProcessStatus, struct ProcessorAssetInfos, enum ProcessorState, enum InitializeError, enum SetTransactionLogFactoryError, enum GetProcessorError
- `third_party/bevy/crates/bevy_asset/src/processor/process.rs`: trait Process, struct LoadTransformAndSave, struct LoadTransformAndSaveSettings, enum ProcessError, trait ErasedProcessor, enum MetaTypePathKind, struct ProcessContext
- `third_party/bevy/crates/bevy_asset/src/reflect.rs`: struct ReflectAsset, struct ReflectHandle, struct HandleSerializeProcessor, enum EphemeralHandleBehavior, trait LoadFromPath, struct HandleDeserializeProcessor, enum HandleReference, struct TypedHandleReference
- `third_party/bevy/crates/bevy_asset/src/saver.rs`: trait AssetSaver, trait ErasedAssetSaver, struct SavedAsset, struct ErasedSavedAsset, struct SavedAssetBuilder, fn save_using_saver, enum SaveAssetError
- `third_party/bevy/crates/bevy_asset/src/server/info.rs`: struct MissingHandleProviderError
- `third_party/bevy/crates/bevy_asset/src/server/mod.rs`: struct AssetServer, enum AssetServerMode, struct LoadBuilder, fn handle_internal_asset_events, fn publish_asset_server_diagnostics, enum LoadState, enum DependencyLoadState, enum RecursiveDependencyLoadState, struct RequestedHandleTypeMismatchError, enum AssetLoadError, struct AssetLoaderError, struct AddAsyncError, struct MissingAssetLoaderForExtensionError, struct MissingAssetLoaderForTypeNameError, struct MissingAssetLoaderForTypeIdError, enum WaitForAssetError, enum WriteDefaultMetaError
- `third_party/bevy/crates/bevy_asset/src/transformer.rs`: trait AssetTransformer, struct TransformedAsset, struct TransformedSubAsset, struct IdentityAssetTransformer

## Examples in the source tree

- `third_party/bevy/examples/3d/anti_aliasing.rs`
- `third_party/bevy/examples/3d/solari.rs`
- `third_party/bevy/examples/3d/solari_reflections.rs`

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
