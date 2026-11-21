# bevy_image, KTX2 and Basis textures (Bevy 0.20.1)

Citations are `path:line` in the pinned checkout (bevyengine/bevy, tag v0.20.1, commit a0577b5); `IMG` = `crates/bevy_image/src`. Snippets are not compiled.

## Cargo features (what actually turns a loader on)

| Feature | Definition | Effect |
|---|---|---|
| `ktx2` | `crates/bevy_image/Cargo.toml` (`ktx2 = ["dep:ktx2"]`) | `.ktx2` loader (`IMG/lib.rs:36`, `IMG/image.rs:489`) |
| `zlib` | same file, `zlib = ["flate2"]` | KTX2 Zlib supercompression |
| `zstd` + `zstd_rust` or `zstd_c` | same file; `zstd` alone is a compile error (`IMG/lib.rs:12-16`) | KTX2 Zstandard supercompression; `zstd_rust` = ruzstd (safe), `zstd_c` = C zstd (faster) (`Cargo.toml:520-524`) |
| `basis-universal` | `Cargo.toml:456` | UASTC transcoding inside KTX2 and `.basis` files (`IMG/ktx2.rs:171`, `IMG/basis.rs`) |
| `dds`, `exr`, `hdr`, `png`, `jpeg`, `webp`, `qoi`, `bmp`, `gif`, `ico`, `ff`, `pnm`, `tga`, `tiff` | `Cargo.toml:473-515` | one loader format each (`IMG/image_loader.rs:21-48`) |
| `compressed_image_saver` / `compressed_image_saver_universal` | `Cargo.toml:459-464` | asset-processor saver, see below |

Defaults: root `default = ["2d","3d","ui","audio"]` (`Cargo.toml:136`). `common_api` enables `hdr` and `png` (`:202-217`); `3d_api` enables `ktx2` and `zstd_rust` (`:232-243`). Hence a default build loads png, hdr and ktx2 with Zstandard, but NOT jpeg, webp, dds, zlib-supercompressed KTX2, or UASTC/Basis (needs `basis-universal`). `ImageLoader::SUPPORTED_FORMATS` is the cfg-gated list (`IMG/image_loader.rs:19-48`).

## Loader, settings, formats

- `ImageLoader` is registered in `TexturePlugin::finish` with `CompressedImageFormats` read from the `CompressedImageFormatSupport` resource; if missing it warns and uses `NONE` (`crates/bevy_render/src/texture/mod.rs:47-61`). `RenderPlugin` fills that resource from `device.features()` (`crates/bevy_render/src/settings.rs:192-193`); the glTF plugin reads the same resource (`crates/bevy_gltf/src/lib.rs:287-297`). Headless/no-GPU app: insert `CompressedImageFormatSupport(CompressedImageFormats::NONE)` yourself.
- `CompressedImageFormats::from_features` maps wgpu `TEXTURE_COMPRESSION_ASTC` -> `ASTC_LDR`, `..._ASTC_HDR` -> `ASTC_HDR`, `..._BC` -> `BC`, `..._ETC2` -> `ETC2` (`IMG/image.rs:2431-2448`); `supports(format)` is true for every uncompressed format (`IMG/image.rs:2452-2490`).
- `ImageLoaderSettings` (`IMG/image_loader.rs:134-165`): `format` (`ImageFormatSetting::{FromExtension (default), Format, Guess}`, `:90-102`), `texture_format: Option<TextureFormat>` (override), `is_srgb` (default `true`, `:176`), `sampler`, `asset_usage`, `array_layout: Option<ImageArrayLayout>` (`RowCount|RowHeight|GridCount|GridSize`, `:105-131`), `expand_grayscale` (default true; false keeps R8/Rg8/R16/Rg16). Set per asset in `.meta` or with `load_builder().with_settings(|s: &mut ImageLoaderSettings| ..)` (pattern used in `crates/bevy_gltf/src/loader/mod.rs:2032-2036`).
- Extension -> format: `ktx2`, MIME `image/ktx2` (`IMG/image.rs:350,398,452,489`). `ImageFormat::Ktx2.as_image_crate_format()` is `None` (`:542`), so KTX2 never goes through the `image` crate.
- `Image::from_buffer(buffer, ImageType::{MimeType|Extension|Format}, CompressedImageFormats, is_srgb, ImageSampler, RenderAssetUsages)` dispatches to `ktx2_buffer_to_image` / `basis_buffer_to_image` / `dds_buffer_to_image` or the `image` crate (`IMG/image.rs:1589-1660`).

## KTX2 loader behavior (`IMG/ktx2.rs`)

- Supercompression (`:53-100`): none; `ZLIB` (feature `zlib`/`flate2`, `:56`); `Zstandard` (`:69` ruzstd, `:85` zstd C). Everything else, including **BasisLZ (ETC1S supercompression), is rejected** with "Unsupported supercompression scheme" (`:96-100`). So `toktx --encode etc1s` / `--bcmp` output does not load.
- Format discovery (`:396-410`): header `vkFormat` first (`ktx2_format_to_texture_format`, `:1205`, about 90 VkFormat arms), else first Basic DFD block (`ktx2_dfd_header_to_texture_format`, `:478`). Neither reads the DFD transfer function; sRGB vs linear comes only from the loader's `is_srgb` flag (`:33`, `:165`, `:229`, `:351-380`). A KTX2 declared `R8G8B8A8_SRGB` is loaded as `Rgba8UnormSrgb` only if `is_srgb`, else `Rgba8Unorm` (`:1224-1230`). Set `is_srgb: false` for normal, metallic-roughness and occlusion maps.
- Transcoding required (`TranscodeFormat`, `IMG/image.rs:2207-2218`): 3-channel RGB8 -> RGBA8 CPU expand (`ktx2.rs:146`); R8/RG8 sRGB -> linear CPU (`:112,129`); UASTC (DFD color model UASTC, `:1169-1185`, channel types Rgb/Rgba/Rrr/Rrrg/Rg) -> GPU format by device support, `get_transcoded_formats` (`:310-390`):
  - RGB/RGBA: `ASTC_LDR` -> ASTC 4x4 (srgb or unorm); else `BC` -> BC7; else `ETC2` -> ETC2 RGBA; else uncompressed RGBA8 (`:343-389`). ASTC is preferred over BC for speed and quality (`:343-345`).
  - R: BC4 / EAC R11 / R8; RG: BC5 / EAC RG11 / Rg8 (`:314-342`).
  - Transcode runs per mip, per layer, per face with `DecodeFlags::HIGH_QUALITY` and `has_alpha: false` (`:203,211`).
  - Without `basis-universal` the UASTC arm is compiled out and loading returns the `FormatRequiresTranscodingError` (`:240-241`).
- ETC1S color model returns `Etc1s` (`:1154`); the "transcode" is a TODO pass-through that only succeeds when the device supports ETC2 (`:226-241`); unreachable for real BasisLZ files because of the supercompression rejection above.
- PVRTC and PVRTC2 are unsupported (`:1161-1168`).
- Layout: image data is concatenated per mip with `TextureDataOrder::MipMajor` (`:264`); `mip_level_count = header.level_count` (`:278`) (no generation at load); `face_count==6` -> `Cube`/`CubeArray` view, `layer_count>1` -> `D2Array`, `depth>1` -> `D3` (`:287-303`). Block-compressed dimensions must be multiples of the block size because of wgpu issue 7677 (`:262-267`, https://github.com/gfx-rs/wgpu/issues/7677): author textures with power-of-two or multiple-of-4 sizes.
- Reference loader tests: `IMG/ktx2.rs:1559-1588`.

## Mipmaps

- Images from png/jpg start with `mip_level_count: 1` (`IMG/image.rs:1157,1284`); `ImageSamplerDescriptor::mipmap_filter` has no effect without mips.
- Mips come from: (a) the file (KTX2/Basis/DDS carry them); (b) the asset processor `CompressedImageSaver` (`generate_mipmaps` default true, `IMG/compressed_image_saver/mod.rs:97-101,188`); (c) GPU compute downsampling in the render world: `MipGenerationJobs::add(phase, image)` plus `generate_mips_for_phase` (`crates/bevy_core_pipeline/src/mip_generation/mod.rs:223-260,414`; example `examples/2d/dynamic_mip_generation.rs`); (d) CPU plugin `examples/large_scenes/mipmap_generator` (README says it is slow and meant for prototyping, `README.md:19-29`).
- Pipeline conclusion: for our GLB pipeline ship pre-mipped KTX2.

## Samplers

- `ImagePlugin::default()` = linear filters (`IMG/image.rs:178-183`); `ImagePlugin::default_nearest()` (`:209`) for pixel art; `with_default_sampler(desc)` (`:198`). `ImageSampler::{Default, Descriptor}` (`:697`).
- `ImageSamplerDescriptor` fields: address modes u/v/w (default `ClampToEdge`), mag/min/mipmap filter, `lod_min_clamp` 0, `lod_max_clamp` 32, `anisotropy_clamp` 1, `border_color`, `compare` (`:854-897`). Helpers `linear()`, `nearest()`, `set_address_mode`, `set_anisotropic_filter(n)` (forces Linear, required by wgpu) (`:903-950`).
- glTF textures get their sampler from the glTF sampler object unless `GltfLoaderSettings.override_sampler` (`crates/bevy_gltf/src/loader/mod.rs:1211-1215`).

## Array, cube and 3D textures

- KTX2 faces/layers/depth map as above; DDS handles the same via `IMG/dds.rs`.
- From a vertical strip or grid: `ImageLoaderSettings.array_layout` or `Image::reinterpret_stacked_2d_as_array` (`IMG/image.rs:1416`) / `create_stacked_array_from_2d_grid` (`:1454`).
- Only six-face cubemaps are accepted (`TextureError::IncompleteCubemap`, `:2330`).

## Conversions and CPU access

- `Image::try_into_dynamic()` (`IMG/image_texture_conversion.rs:180-233`) supports exactly: R8Unorm, Rg8Unorm, Rgba8UnormSrgb, Bgra8Unorm/UnormSrgb (swizzled), Rgba16Float, Rgba32Float. Linear `Rgba8Unorm` and every compressed format return `IntoDynamicImageError::UnsupportedFormat` (`:228`). Render offscreen to an `*Srgb` target (see `render.md` recipe) before saving PNG.
- `Image::convert` only handles R8Unorm, Rg8Unorm, Rgba8UnormSrgb (`IMG/image.rs:1548-1556`). `get_color_at/set_color_at` need uncompressed data (`:1804-1874`); `Image::is_compressed` (`:1665`).
- Atlases: `TextureAtlasLayout`, `TextureAtlas`, `TextureAtlasBuilder`, `DynamicTextureAtlasBuilder` live in this crate (`IMG/texture_atlas.rs`, `texture_atlas_builder.rs`, `dynamic_texture_atlas_builder.rs`); details in `sprite-text-ui.md`.

## Compressed image saver (asset processor)

- `compressed_image_saver` (needs `ctt`, `ktx2`, `zstd`, C++ compiler) writes BC1-7/BC4-6 style KTX2: R8 -> BC4, Rg8 -> BC5, HDR -> BC6H, RGBA8 -> BC7 (sRGB aware); `BEVY_COMPRESSED_IMAGE_SAVER_ASTC=<block>` switches to ASTC (`IMG/compressed_image_saver/mod.rs:34-96`, Cargo features `Cargo.toml` of bevy_image).
- `compressed_image_saver_universal` writes UASTC `.basis` (`IMG/compressed_image_saver/universal.rs:17-40,70-80`), loadable only with `basis-universal`.
- `ImagePlugin.default_compressed_image_processor_extensions` defaults to png, jpeg, jpg (`IMG/image.rs:187`) and registers the processor when an `AssetProcessor` exists (`:243-254`).
- `CompressedImageSaverSettings` defaults: `is_normal_map=false` (must be true for normal maps, linear input required, `mod.rs:150-188`), `input_alpha_mode=Straight`, `output_alpha_mode=Premultiplied` (materials must then use premultiplied alpha), `generate_mipmaps=true` (`:150-190`).
- Example: `examples/asset/compressed_image_saver.rs` (requires `compressed_image_saver`, `asset_processor`, `jpeg`, `Cargo.toml:2144`; `AssetPlugin { mode: AssetMode::Processed }`, example `:14-21`). Asset processing itself: `asset.md`.

## Not supported (verified)

- BasisLZ/ETC1S supercompression: `IMG/ktx2.rs:96-100`. PVRTC/PVRTC2: `:1161-1168`. KTX2 transfer function ignored: no `transfer` token in `IMG/ktx2.rs`. HDR block-compressed targets: BC6H/ASTC-HDR transcoding is absent (`:339-341` comment). KHR_texture_basisu glTF syntax: `crates/bevy_gltf/src/lib.rs:122,129`, see `gltf.md`.

## Recipes for our GLB pipeline

KTX2 textures. Produce with toktx 4.4.2 (flags checked with `toktx --help` locally and on fr1, both `v4.4.2`):

```sh
# color (sRGB): UASTC + Zstd, full mip chain, Bevy transcodes to ASTC/BC7/ETC2 at load
toktx --t2 --encode uastc --uastc_quality 2 --zcmp 19 --genmipmap --assign_oetf srgb albedo.ktx2 albedo.png
# data maps (normal, ORM): linear; no --normal_mode (it packs XY into RGB=X, A=Y, which StandardMaterial does not decode)
toktx --t2 --encode uastc --uastc_quality 2 --zcmp 19 --genmipmap --assign_oetf linear normal.ktx2 normal.png
# GPU-native ASTC (loads directly only on devices with ASTC_LDR)
toktx --t2 --encode astc --astc_blk_d 6x6 --genmipmap albedo_astc.ktx2 albedo.png
```

`--zcmp` is allowed with every encoder except ETC1S/BasisLZ, and `--bcmp`/`--encode etc1s` outputs do not load in Bevy (toktx help text; `IMG/ktx2.rs:96-100`). Bevy ignores the file's OETF, so linear maps must be loaded with `is_srgb: false` (glTF loader does this for normal, MR and occlusion textures through `linear_textures`, `crates/bevy_gltf/src/loader/mod.rs:1210`).

Cargo (UASTC needs `basis-universal`; ktx2 + zstd_rust already come with the default 3d profile):

```toml
bevy = { version = "=0.20.1", default-features = false, features = ["3d", "basis-universal"] }  # not compiled
```

Inspect what Bevy produced (not compiled):

```rust
fn report_ktx2(mut events: MessageReader<AssetEvent<Image>>, images: Res<Assets<Image>>) {
    for event in events.read() {
        if let AssetEvent::LoadedWithDependencies { id } = event {
            if let Some(image) = images.get(*id) {
                info!("{id}: {:?} mips={} size={:?} compressed={}",
                    image.texture_descriptor.format,
                    image.texture_descriptor.mip_level_count,
                    image.texture_descriptor.size,
                    image.is_compressed());
            }
        }
    }
}
```

`MessageReader<AssetEvent<Image>>` matches `examples/3d/tonemapping.rs:232`. Expected formats: UASTC color on NVIDIA/AMD desktop (`BC`) -> `Bc7RgbaUnormSrgb`; Apple/Android (`ASTC_LDR`) -> `Astc{B4x4,UnormSrgb}`; no compression feature -> `Rgba8UnormSrgb` (`IMG/ktx2.rs:343-389`).

glTF path: Bevy does not parse `KHR_texture_basisu` (`crates/bevy_gltf/src/lib.rs:122,129`). The workable route is a plain image entry whose `uri` ends in `.ktx2` (loaded by extension through `ImageLoader`, `crates/bevy_gltf/src/loader/mod.rs:1252-1263`) or whose bufferView `mimeType` is `image/ktx2` (`:1218-1233`); the repo's own README documents the mimeType replacement trick (`examples/large_scenes/mipmap_generator/README.md:31`). Details and extension verdicts: `gltf.md`.
