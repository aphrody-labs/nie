---
name: dioxus-cli
description: "Use when working with the `dioxus-cli` crate of dioxus: CLI for building fullstack web, desktop, and mobile apps with a single codebase."
---

# dioxus-cli

- Version: 0.8.0-alpha.3 (workspace member of `third_party/dioxus`, vendored commit `525d4026c949`)
- Source: `third_party/dioxus/packages/dx/` (manifest `third_party/dioxus/packages/dx/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/dioxus/tree/main/packages/dx
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

CLI for building fullstack web, desktop, and mobile apps with a single codebase.

## README (`third_party/dioxus/packages/dx/README.md`)

📦✨ Dioxus CLI
  Tooling to supercharge Dioxus projects

The **dioxus-cli** (inspired by wasm-pack and webpack) is a tool for getting Dioxus projects up and running.
It handles building, bundling, development and publishing to simplify development.

## Installation

### Install the stable version (recommended)

```shell
cargo install dioxus-cli
```

### Install the latest development build through git

To get the latest bug fixes and features, you can install the development version from git.
However, this is not fully tested.
That means you're probably going to have more bugs despite having the latest bug fixes.

```shell
cargo install --git https://github.com/DioxusLabs/dioxus dioxus-cli
```

This will download the CLI from the master branch,
and install it in Cargo's global binary directory (`~/.cargo/bin/` by default).

### Install from local folder
Note: The CLI will fail to build projects in debug profile. This is currently under investigation.
```shell
cargo install --path .
```

## Get started

Use `dx new` to initialize a new Dioxus project.
It will be cloned from the [dioxus-template](https://github.com/DioxusLabs/dioxus-template) repository.

Alternatively, you can specify the template path:

```shell
dx new --template gh:dioxuslabs/dioxus-template
```

Run `dx --help` for a list of all the available commands.
Furthermore, you can run `dx  --help` to get help with a specific command.

## Dioxus config file

You can use the `Dioxus.toml` file for further configuration.
Some fields are mandatory, but the CLI tool will tell you which ones are missing.
You can create a `Dioxus.toml` with all fields already set using `dx config init project-name`,
or you can use this bare-bones template (only mandatory fields) to get started:

```toml
[application]
name = "project-name"
# Currently supported platforms: web, desktop
default_platform = "web"

# Optional: enable copying from a static directory (e.g. "public")
public_dir = "public"

[web.app]
title = "Hello"

[web.resource.dev]
```

## Cargo features (from the manifest)

- `default`
- `tokio-console` (enables 2)
- `no-downloads`
- `disable-telemetry`

## Depends on (same ecosystem)

`const-serialize`, `depinfo`, `dioxus-autofmt`, `dioxus-cli-config`, `dioxus-cli-telemetry`, `dioxus-component-manifest`, `dioxus-devtools-types`, `dioxus-html`, `dioxus-rsx`, `dioxus-rsx-hotreload`, `dioxus-rsx-rosetta`, `dioxus-vdom`, `dx-env`, `dx-wire-format`, `manganis`, `manganis-core`, `subsecond-types`, `wasm-split-cli`

## Public items by file

- `third_party/dioxus/packages/dx/src/build/apple.rs`: fn extract_swift_metadata_from_link_args, struct AppleWidgetSource, fn compile_apple_widget, fn create_framework_bundle
- `third_party/dioxus/packages/dx/src/build/context.rs`: struct BuildContext, type ProgressTx, type ProgressRx, struct BuildId, enum BuilderUpdate, struct BuildPhaseProfile
- `third_party/dioxus/packages/dx/src/build/patch.rs`: enum PatchError, struct HotpatchModuleCache, struct CachedSymbol, fn create_windows_jump_table, fn create_native_jump_table, fn create_wasm_jump_table, fn create_undefined_symbol_stub, fn prepare_wasm_base_module
- `third_party/dioxus/packages/dx/src/build/request.rs`: enum BuildMode, struct BuildArtifacts
- `third_party/dioxus/packages/dx/src/build/windows.rs`: enum IDI, enum VersionInfo, enum Properties
- `third_party/dioxus/packages/dx/src/cargo_toml.rs`: fn load_manifest_from_path
- `third_party/dioxus/packages/dx/src/check/issues.rs`: struct IssueReport, enum Issue
- `third_party/dioxus/packages/dx/src/check/metadata.rs`: struct HookInfo, enum ConditionalInfo, struct IfInfo, struct MatchInfo, enum AnyLoopInfo, struct ForInfo, struct WhileInfo, struct LoopInfo, struct ClosureInfo, struct AsyncInfo, struct ComponentInfo, struct FnInfo, struct Span, struct LineColumn
- `third_party/dioxus/packages/dx/src/check/mod.rs`: fn check_file
- `third_party/dioxus/packages/dx/src/cli/build.rs`: struct BuildArgs, struct BuildTargets
- `third_party/dioxus/packages/dx/src/cli/build_assets.rs`: struct BuildAssets
- `third_party/dioxus/packages/dx/src/cli/bundle.rs`: struct Bundle
- `third_party/dioxus/packages/dx/src/cli/component.rs`: enum ComponentCommand, struct ComponentArgs, struct RemoteComponentRegistry, struct ComponentRegistry
- `third_party/dioxus/packages/dx/src/cli/create.rs`: struct Create
- `third_party/dioxus/packages/dx/src/cli/hotpatch.rs`: struct HotpatchTip
- `third_party/dioxus/packages/dx/src/cli/init.rs`: struct Init
- `third_party/dioxus/packages/dx/src/cli/link.rs`: struct LinkAction, enum LinkerFlavor, fn get_actual_linker_args_excluding_program_name, fn handle_linker_arg_response_file
- `third_party/dioxus/packages/dx/src/cli/mod.rs`: enum BuildTools, mod styles
- `third_party/dioxus/packages/dx/src/cli/platform_override.rs`: struct CommandWithPlatformOverrides, struct ChainedCommand
- `third_party/dioxus/packages/dx/src/cli/translate.rs`: fn convert_html_to_formatted_rsx
- `third_party/dioxus/packages/dx/src/cli/update.rs`: fn log_if_cli_could_update
- `third_party/dioxus/packages/dx/src/cli/verbosity.rs`: struct Verbosity
- `third_party/dioxus/packages/dx/src/config/bundle.rs`: struct CustomSignCommandSettings
- `third_party/dioxus/packages/dx/src/config/dioxus_config.rs`: enum BundlePlatform
- `third_party/dioxus/packages/dx/src/config/inline_config.rs`: fn extract_inline_config, fn extract_inline_config_from_file, fn merge_toml_values, fn merge_with_inline_config
- `third_party/dioxus/packages/dx/src/config/manifest.rs`: struct PermissionsConfig, struct SimplePermission, struct DeepLinkConfig, struct BackgroundConfig, struct LocationPermission, enum LocationPrecision, struct StoragePermission, enum StorageAccess, struct RawPermission, struct IosConfig, struct WidgetExtensionConfig, struct IosDocumentType, struct IosTypeIdentifier, struct IosEntitlements, struct IosRawConfig, struct AndroidConfig, struct AndroidSigningConfig, struct AndroidIntentFilter, struct AndroidIntentData, struct AndroidQueries, struct AndroidQueryIntent, struct AndroidRawConfig, struct AndroidApplicationConfig, struct MacosConfig, ...
- `third_party/dioxus/packages/dx/src/config/manifest_mapper.rs`: struct AndroidPermissionEntry, struct PlistEntry, struct ManifestMapper
- `third_party/dioxus/packages/dx/src/error.rs`: fn log_stacktrace
- `third_party/dioxus/packages/dx/src/logging.rs`: fn verbosity_or_default, struct TraceController, struct TraceMsg, enum TraceContent, enum TraceSrc, struct PrettyUptime
- `third_party/dioxus/packages/dx/src/opt/folder.rs`: fn process_folder
- `third_party/dioxus/packages/dx/src/rustcwrapper.rs`: struct WorkspaceRustcArgs, struct RustcArgs, fn is_wrapping_rustc, fn run_rustc
- `third_party/dioxus/packages/dx/src/serve/output.rs`: struct Output
- `third_party/dioxus/packages/dx/src/wasm_opt.rs`: fn write_wasm, fn optimize, fn get_binary_path, fn installed_location
- `third_party/dioxus/packages/dx/src/workspace.rs`: struct Workspace

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
