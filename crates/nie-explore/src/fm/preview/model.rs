//! Model previews: a turntable of a G4MD mesh, rendered by the CPU rasteriser of `nie-render3d`.

use nie_formats::{g4md, g4mg};
use nie_render3d::{glb, render};

use super::{Body, Built, Context, Frames, Image, PreviewRequest, hex};

/// Pictures in one turn.
const STEPS: usize = 24;
/// Side of a picture, in pixels.
const SIDE: u32 = 320;

fn normals(positions: &[[f32; 3]], indices: &[u32]) -> Vec<[f32; 3]> {
    let mut sums = vec![[0f32; 3]; positions.len()];
    for tri in indices.as_chunks::<3>().0 {
        let [a, b, c] = [tri[0] as usize, tri[1] as usize, tri[2] as usize];
        let (Some(pa), Some(pb), Some(pc)) = (positions.get(a), positions.get(b), positions.get(c))
        else {
            continue;
        };
        let u = [pb[0] - pa[0], pb[1] - pa[1], pb[2] - pa[2]];
        let v = [pc[0] - pa[0], pc[1] - pa[1], pc[2] - pa[2]];
        let n = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        for i in [a, b, c] {
            for (s, x) in sums[i].iter_mut().zip(n) {
                *s += x;
            }
        }
    }
    for n in &mut sums {
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        if len > 0.0 {
            n.iter_mut().for_each(|x| *x /= len);
        } else {
            *n = [0.0, 1.0, 0.0];
        }
    }
    sums
}

pub(super) fn turntable(ctx: &Context<'_>, request: &PreviewRequest, data: &[u8]) -> Built {
    let parsed = match g4md::parse(data) {
        Ok(parsed) => parsed,
        Err(error) => return hex::message_with_hex(data, &format!("g4md: {error}")),
    };
    let geometry_path = request.path.replace(".g4md", ".g4mg");
    let mut meta = vec![
        ("submeshes".to_owned(), parsed.submeshes.len().to_string()),
        ("geometry".to_owned(), geometry_path.clone()),
    ];
    let Ok(geometry_bytes) = ctx.vfs.read(&geometry_path) else {
        return (Body::Message("the .g4mg geometry file is missing".into()), meta, false);
    };
    let geometry = g4mg::extract_geometry(&geometry_bytes, &parsed);
    let mut model = glb::Model {
        primitives: Vec::new(),
        textures: Vec::new(),
    };
    for sub in &geometry {
        if parsed.lod_level_at(sub.index).is_some() || sub.positions.is_empty() {
            continue;
        }
        let positions: Vec<[f32; 3]> = sub.positions.iter().map(|p| [p.x, p.y, p.z]).collect();
        let normals_in: Vec<[f32; 3]> = if sub.normals.len() == positions.len() {
            sub.normals.iter().map(|n| [n.x, n.y, n.z]).collect()
        } else {
            normals(&positions, &sub.indices)
        };
        let uv = if sub.uv0.len() == positions.len() {
            sub.uv0.iter().map(|t| [t.u, t.v]).collect()
        } else {
            vec![[0.0, 0.0]; positions.len()]
        };
        model.primitives.push(glb::Primitive {
            positions,
            normals: normals_in,
            uv,
            indices: sub.indices.clone(),
            texture: None,
        });
    }
    let triangles: usize = model.primitives.iter().map(|p| p.indices.len() / 3).sum();
    meta.push(("triangles".to_owned(), triangles.to_string()));
    if model.primitives.is_empty() {
        return (Body::Message("the model has no base-level geometry".into()), meta, false);
    }
    let mut frames = Frames::default();
    for step in 0..STEPS {
        if (ctx.cancelled)() {
            return (Body::Message("cancelled".into()), meta, false);
        }
        let angle = std::f32::consts::TAU * step as f32 / STEPS as f32;
        let rgba = render::render(&model, angle, SIDE, SIDE);
        let image = Image {
            width: SIDE,
            height: SIDE,
            rgba,
        };
        if image.is_valid() {
            frames.images.push(image);
            frames
                .labels
                .push(format!("{:.0} degrees", 360.0 * step as f32 / STEPS as f32));
        }
    }
    frames.fps = 12;
    meta.push(("renderer".to_owned(), "CPU z-buffer (nie-render3d)".to_owned()));
    (Body::Frames(frames), meta, false)
}
