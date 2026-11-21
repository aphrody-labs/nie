# nie-render3d

Deterministic CPU rendering and inspection of reconstructed IEVR GLB models with the in-crate scalar rasterizer (`softraster`). Optional features: `gpu` (wgpu viewport), `webgpu` (canvas), `webgl` (WebGL 2 fallback), `bevy`.

```sh
cargo check -p nie-render3d
```

The `cli` feature (default) writes local images and video.
