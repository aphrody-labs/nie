//! Film previews: the first seconds of an H.264 USM, decoded with OpenH264.

use nie_formats::usm::{self, CodecVideo};
use openh264::decoder::Decoder;
use openh264::formats::YUVSource;

use super::{Body, Built, Context, Frames, Image, PreviewRequest, sheet};

/// Access units fed to the decoder.
const MAX_UNITS: usize = 120;
/// Frames kept for the animation.
const MAX_FRAMES: usize = 40;
/// Longest side of a kept frame.
const FRAME_SIDE: u32 = 360;

pub(super) fn film(ctx: &Context<'_>, request: &PreviewRequest, data: &[u8]) -> Built {
    let name = usm::nom_fichier_de(&request.path);
    let film = match usm::demuxer_nomme(data, name) {
        Ok(film) => film,
        Err(error) => return sheet::film_card(&request.path, data, &format!("usm: {error}")),
    };
    if film.codec != CodecVideo::H264 {
        let why = format!(
            "{} frames are not decoded by the explorer, only H.264",
            film.codec.nom()
        );
        return sheet::film_card(&request.path, data, &why);
    }
    let (_, mut meta, _) = sheet::film_card(&request.path, data, "");
    let mut decoder = match Decoder::new() {
        Ok(decoder) => decoder,
        Err(error) => {
            meta.push(("decoder".to_owned(), error.to_string()));
            return (Body::Message("OpenH264 failed to start".into()), meta, false);
        }
    };
    let crop = (
        film.entete.largeur_affichee as usize,
        film.entete.hauteur_affichee as usize,
    );
    let rate = film.entete.images_par_seconde().unwrap_or(24.0);
    let mut decoded: Vec<Image> = Vec::new();
    let mut seen = 0usize;
    let stride = (MAX_UNITS / MAX_FRAMES).max(1);
    for unit in film.images.iter().take(MAX_UNITS) {
        if (ctx.cancelled)() {
            return (Body::Message("cancelled".into()), meta, false);
        }
        let Ok(Some(yuv)) = decoder.decode(unit) else {
            continue;
        };
        seen += 1;
        if !(seen - 1).is_multiple_of(stride) {
            continue;
        }
        let (w, h) = yuv.dimensions();
        let mut rgba = vec![0u8; w * h * 4];
        yuv.write_rgba8(&mut rgba);
        let (w, h, rgba) = crop_to(w, h, rgba, crop);
        if let Ok((w, h, rgba)) =
            nie_formats::image_out::reduire_rgba(&rgba, w as u32, h as u32, FRAME_SIDE)
        {
            decoded.push(Image {
                width: w,
                height: h,
                rgba,
            });
        }
    }
    meta.push((
        "decoded".to_owned(),
        format!("{seen} frame(s) of the first {MAX_UNITS} units, {} kept", decoded.len()),
    ));
    if decoded.is_empty() {
        return (Body::Message("OpenH264 produced no frame".into()), meta, false);
    }
    let labels = (0..decoded.len())
        .map(|i| format!("{:.2} s", (i * stride) as f64 / rate))
        .collect();
    let fps = (rate / stride as f64).round().max(1.0) as u32;
    (
        Body::Frames(Frames {
            images: decoded,
            labels,
            fps,
        }),
        meta,
        false,
    )
}

/// Crops the coded picture (a multiple of 16) to the display size declared by the container.
fn crop_to(w: usize, h: usize, rgba: Vec<u8>, crop: (usize, usize)) -> (usize, usize, Vec<u8>) {
    let (cw, ch) = (crop.0.min(w), crop.1.min(h));
    if cw == 0 || ch == 0 || (cw == w && ch == h) {
        return (w, h, rgba);
    }
    let mut out = Vec::with_capacity(cw * ch * 4);
    for row in 0..ch {
        out.extend_from_slice(&rgba[row * w * 4..(row * w + cw) * 4]);
    }
    (cw, ch, out)
}
