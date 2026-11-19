//! 1280x720 turntable of a synthetic textured sphere on the nie-render3d wgpu renderer.
//!
//! Reports the first-frame cost (model build, GPU upload, first draw) and the per-frame CPU wall
//! time (submit to GPU idle) as p50/p99, plus the GPU pass time from timestamp queries when the
//! `gpu-timing` feature is on and the adapter supports `TIMESTAMP_QUERY`. The pass criterion is
//! p99 <= 8.33 ms (120 fps).

use std::time::Instant;

use anyhow::{Context, Result};
use nie_gpu::wgpu;
use nie_gpu::{DeviceLimits, DeviceRequest, request_adapter, request_device};
use nie_render3d::glb::{Model, Primitive, Texture};
use nie_render3d::gpu::{Camera, GpuRenderer};
use serde_json::{Value, json};

use crate::{Args, percentile};

const BUDGET_MS: f64 = 1000.0 / 120.0;

/// UV sphere of `stacks` x `slices` quads with one `side`x`side` RGBA atlas.
fn sphere_model(stacks: u32, slices: u32, side: u32) -> Model {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uv = Vec::new();
    for i in 0..=stacks {
        let v = i as f32 / stacks as f32;
        let phi = v * std::f32::consts::PI;
        for j in 0..=slices {
            let u = j as f32 / slices as f32;
            let theta = u * std::f32::consts::TAU;
            let n = [phi.sin() * theta.cos(), phi.cos(), phi.sin() * theta.sin()];
            positions.push(n);
            normals.push(n);
            uv.push([u, v]);
        }
    }
    let mut indices = Vec::new();
    let row = slices + 1;
    for i in 0..stacks {
        for j in 0..slices {
            let a = i * row + j;
            let b = a + row;
            indices.extend_from_slice(&[a, b, a + 1, a + 1, b, b + 1]);
        }
    }
    let mut rgba = Vec::with_capacity((side * side * 4) as usize);
    for y in 0..side {
        for x in 0..side {
            let check = ((x / 32) + (y / 32)) % 2 == 0;
            let base = if check { 200u8 } else { 60u8 };
            rgba.extend_from_slice(&[base, (x * 255 / side) as u8, (y * 255 / side) as u8, 255]);
        }
    }
    Model {
        primitives: vec![Primitive { positions, normals, uv, indices, texture: Some(0) }],
        textures: vec![Texture { width: side, height: side, rgba }],
    }
}

fn renderer(timestamps: bool) -> Result<(GpuRenderer, bool)> {
    let instance = nie_gpu::create_instance(wgpu::InstanceDescriptor::new_without_display_handle());
    let options = |fallback| wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: fallback,
        apply_limit_buckets: false,
    };
    let adapter = match pollster::block_on(request_adapter(&instance, &options(false))) {
        Ok(adapter) => adapter,
        Err(_) => pollster::block_on(request_adapter(&instance, &options(true)))
            .context("no wgpu adapter")?,
    };
    let features = if timestamps {
        adapter.features() & wgpu::Features::TIMESTAMP_QUERY
    } else {
        wgpu::Features::empty()
    };
    let device = pollster::block_on(request_device(
        &adapter,
        DeviceRequest::new(Some("nie-perf frame"), DeviceLimits::DownlevelWithAdapterResolution)
            .with_features(features),
    ))
    .context("wgpu device")?;
    let enabled = device.device.features().contains(wgpu::Features::TIMESTAMP_QUERY);
    Ok((GpuRenderer::from_device(adapter.get_info(), device.device, device.queue), enabled))
}

fn stats(mut sample: Vec<f64>) -> Value {
    sample.sort_by(f64::total_cmp);
    let mean = sample.iter().sum::<f64>() / sample.len().max(1) as f64;
    json!({
        "n": sample.len(),
        "p50": percentile(&sample, 50.0),
        "p99": percentile(&sample, 99.0),
        "max": sample.last().copied().unwrap_or(f64::NAN),
        "mean": mean,
    })
}

pub fn run(args: &Args) -> Result<Value> {
    let frames = args.number("frames", 600usize)?;
    let warmup = args.number("warmup", 30usize)?;
    let width = args.number("width", 1280u32)?;
    let height = args.number("height", 720u32)?;
    let side = args.number("atlas", 1024u32)?;
    let timestamps = args.string("timestamps").unwrap_or("on") != "off";

    let t_build = Instant::now();
    let model = sphere_model(128, 256, side);
    let build_ms = t_build.elapsed().as_secs_f64() * 1e3;
    let triangles: usize = model.primitives.iter().map(|p| p.indices.len() / 3).sum();

    let t_device = Instant::now();
    let (mut renderer, timing_enabled) = renderer(timestamps)?;
    let device_ms = t_device.elapsed().as_secs_f64() * 1e3;

    let t_upload = Instant::now();
    let gpu_model = renderer.try_upload(&model)?;
    renderer.device().poll(wgpu::PollType::wait_indefinitely()).context("poll")?;
    let upload_ms = t_upload.elapsed().as_secs_f64() * 1e3;

    let mut camera = Camera::default();
    let t_first = Instant::now();
    renderer.render_to_texture(&gpu_model, camera, width, height)?;
    renderer.device().poll(wgpu::PollType::wait_indefinitely()).context("poll")?;
    let first_draw_ms = t_first.elapsed().as_secs_f64() * 1e3;

    let mut cpu = Vec::with_capacity(frames);
    #[cfg(feature = "gpu-timing")]
    let mut gpu = Vec::with_capacity(frames);
    for i in 0..warmup + frames {
        camera.yaw = std::f32::consts::TAU * (i % frames.max(1)) as f32 / frames.max(1) as f32;
        let t = Instant::now();
        renderer.render_to_texture(&gpu_model, camera, width, height)?;
        renderer.device().poll(wgpu::PollType::wait_indefinitely()).context("poll")?;
        let ms = t.elapsed().as_secs_f64() * 1e3;
        #[cfg(feature = "gpu-timing")]
        let pass = renderer.gpu_pass_ms();
        if i >= warmup {
            cpu.push(ms);
            #[cfg(feature = "gpu-timing")]
            if let Some(pass) = pass {
                gpu.push(pass);
            }
        }
    }

    let info = renderer.adapter_info();
    #[cfg(feature = "gpu-timing")]
    let gpu_stats = if gpu.is_empty() { Value::Null } else { stats(gpu) };
    #[cfg(not(feature = "gpu-timing"))]
    let gpu_stats = Value::Null;
    let cpu_stats = stats(cpu);
    let pass_basis = if gpu_stats.is_null() { &cpu_stats } else { &gpu_stats };
    let p99 = pass_basis["p99"].as_f64().unwrap_or(f64::NAN);
    Ok(json!({
        "cmd": "frame",
        "adapter": {"name": info.name, "backend": format!("{:?}", info.backend), "type": format!("{:?}", info.device_type)},
        "resolution": [width, height],
        "triangles": triangles,
        "atlas": side,
        "timestamps_requested": timestamps,
        "timestamps_active": timing_enabled && cfg!(feature = "gpu-timing"),
        "first_frame_ms": {
            "model_build": build_ms, "device": device_ms, "upload": upload_ms,
            "first_draw": first_draw_ms, "model_to_pixels": build_ms + upload_ms + first_draw_ms,
        },
        "cpu_frame_ms": cpu_stats,
        "gpu_pass_ms": gpu_stats,
        "budget_ms": BUDGET_MS,
        "pass_basis": if gpu_stats.is_null() { "cpu_frame" } else { "gpu_pass" },
        "pass": p99 <= BUDGET_MS,
    }))
}
