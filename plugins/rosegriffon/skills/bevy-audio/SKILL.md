---
name: bevy-audio
description: "Use when working with the `bevy_audio` crate of bevy: Provides audio functionality for Bevy Engine."
---

# bevy_audio

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_audio/` (manifest `third_party/bevy/crates/bevy_audio/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_audio
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Provides audio functionality for Bevy Engine

## README (`third_party/bevy/crates/bevy_audio/README.md`)

# Bevy Audio

## Cargo features (from the manifest)

- `aac` (enables 1)
- `flac` (enables 1)
- `mp3` (enables 1)
- `mp4` (enables 1)
- `vorbis` (enables 1)
- `wav` (enables 1)
- `symphonia-flac` (enables 1)
- `symphonia-vorbis` (enables 1)
- `symphonia-wav` (enables 1)

## Depends on (same ecosystem)

`bevy_app`, `bevy_asset`, `bevy_ecs`, `bevy_math`, `bevy_reflect`, `bevy_transform`

## Public items by file

- `third_party/bevy/crates/bevy_audio/src/audio.rs`: enum PlaybackMode, struct PlaybackSettings, struct SpatialListener, struct SpatialScale, struct DefaultSpatialScale, struct AudioPlayer
- `third_party/bevy/crates/bevy_audio/src/audio_output.rs`: struct PlaybackDespawnMarker, struct PlaybackRemoveMarker
- `third_party/bevy/crates/bevy_audio/src/audio_source.rs`: struct AudioSource, struct AudioLoader, trait Decodable, trait AddAudioSource
- `third_party/bevy/crates/bevy_audio/src/lib.rs`: mod prelude, struct AudioPlugin
- `third_party/bevy/crates/bevy_audio/src/pitch.rs`: struct Pitch
- `third_party/bevy/crates/bevy_audio/src/sinks.rs`: trait AudioSinkPlayback, struct AudioSink, struct SpatialAudioSink
- `third_party/bevy/crates/bevy_audio/src/volume.rs`: struct GlobalVolume, enum Volume

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
