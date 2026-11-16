---
name: bevy-image
description: "Use when working with the `bevy_image` crate of bevy: Provides image types for Bevy Engine."
---

# bevy_image

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_image/` (manifest `third_party/bevy/crates/bevy_image/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_image
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Provides image types for Bevy Engine

## Crate docs (`third_party/bevy/crates/bevy_image/src/lib.rs`)

The Bevy game engine's GPU-oriented image type.

## Cargo features (from the manifest)

- `default` (enables 1)
- `bevy_reflect` (enables 1): bevy_reflect can't optional as it's needed for TypePath this feature only control reflection in bevy_image
- `basis-universal` (enables 1): Image formats
- `bmp` (enables 1)
- `dds` (enables 1)
- `exr` (enables 1)
- `ff` (enables 1)
- `gif` (enables 1)
- `hdr` (enables 1)
- `ktx2` (enables 1)
- `ico` (enables 1)
- `jpeg` (enables 1)
- `png` (enables 1)
- `pnm` (enables 1)
- `qoi` (enables 1)
- `tga` (enables 1)
- `tiff` (enables 1)
- `webp` (enables 1)
- `serialize` (enables 3)
- `zlib` (enables 1): For ktx2 supercompression
- `zstd`: A marker feature indicating zstd support is required for a particular feature. A backend must be chosen by enabling either the "zstd_rust" or the "zstd_c" feature.
- `zstd_rust` (enables 2): Pure-rust zstd implementation (safer)
- `zstd_c` (enables 2): Binding to zstd C implementation (faster)
- `compressed_image_saver_universal` (enables 1): Texture compression asset processor (cross-platform, transcodes to any GPU format at load time)
- `compressed_image_saver` (enables 3): Texture compression asset processor (BCn for desktop, ASTC for mobile via env var)
- `equirectangular_cubemap` (enables 1): Equirectangular (lat-long) panorama to cubemap conversion

## Depends on (same ecosystem)

`bevy_app`, `bevy_asset`, `bevy_color`, `bevy_ecs`, `bevy_math`, `bevy_platform`, `bevy_reflect`, `bevy_utils`

## Public items by file

- `third_party/bevy/crates/bevy_image/src/basis.rs`: fn basis_buffer_to_image, fn get_transcoded_formats
- `third_party/bevy/crates/bevy_image/src/compressed_image_saver/ctt.rs`: struct CompressedImageSaverCtt
- `third_party/bevy/crates/bevy_image/src/compressed_image_saver/ctt_helpers.rs`: fn parse_astc_env_var, fn choose_ctt_compressed_format, fn ctt_format, fn bevy_to_ctt_alpha_mode
- `third_party/bevy/crates/bevy_image/src/compressed_image_saver/mod.rs`: struct CompressedImageSaver, struct CompressedImageSaverSettings, enum ImageCompressorAlphaMode, enum CompressedImageSaverError
- `third_party/bevy/crates/bevy_image/src/compressed_image_saver/universal.rs`: struct CompressedImageSaverUniversal
- `third_party/bevy/crates/bevy_image/src/dds.rs`: fn dds_buffer_to_image, fn dds_format_to_texture_format
- `third_party/bevy/crates/bevy_image/src/dynamic_texture_atlas_builder.rs`: enum DynamicTextureAtlasBuilderError, struct DynamicTextureAtlasBuilder
- `third_party/bevy/crates/bevy_image/src/equirectangular.rs`: enum EquirectangularToCubemapError, fn equirectangular_uv
- `third_party/bevy/crates/bevy_image/src/exr_texture_loader.rs`: struct ExrTextureLoader, struct ExrTextureLoaderSettings, enum ExrTextureLoaderError
- `third_party/bevy/crates/bevy_image/src/hdr_texture_loader.rs`: struct HdrTextureLoader, struct HdrTextureLoaderSettings, enum HdrTextureLoaderError
- `third_party/bevy/crates/bevy_image/src/image.rs`: trait TextureSrgbViewFormats, struct ImagePlugin, enum ImageFormat, trait ToExtents, struct Image, enum ImageSampler, enum ImageAddressMode, enum ImageFilterMode, enum ImageCompareFunction, enum ImageSamplerBorderColor, struct ImageSamplerDescriptor, enum TextureChannelLayout, enum TranscodeFormat, enum TextureReinterpretationError, enum TextureAccessError, enum TextureError, enum ImageType, trait TextureFormatPixelInfo, struct CompressedImageFormatSupport
- `third_party/bevy/crates/bevy_image/src/image_loader.rs`: struct ImageLoader, enum ImageFormatSetting, enum ImageArrayLayout, struct ImageLoaderSettings, enum ImageLoaderError, struct FileTextureError
- `third_party/bevy/crates/bevy_image/src/image_texture_conversion.rs`: enum IntoDynamicImageError
- `third_party/bevy/crates/bevy_image/src/ktx2.rs`: fn ktx2_buffer_to_image, fn get_transcoded_formats, fn ktx2_get_texture_format, fn ktx2_dfd_header_to_texture_format, fn ktx2_format_to_texture_format
- `third_party/bevy/crates/bevy_image/src/lib.rs`: mod prelude
- `third_party/bevy/crates/bevy_image/src/saver.rs`: struct ImageSaver, struct ImageSaverSettings, enum SaveImageFormatSetting, enum SaveImageError
- `third_party/bevy/crates/bevy_image/src/serialized_image.rs`: struct SerializedImage
- `third_party/bevy/crates/bevy_image/src/texture_atlas.rs`: struct TextureAtlasPlugin, struct TextureAtlasSources, struct TextureAtlasLayout, struct TextureAtlas
- `third_party/bevy/crates/bevy_image/src/texture_atlas_builder.rs`: enum TextureAtlasBuilderError, struct TextureAtlasBuilder, type TextureAtlasBuilderResult

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
