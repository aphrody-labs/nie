//! Pictures: G4TX textures and atlases, DDS and PNG files.

use nie_formats::{g4tx, g4tx_decode, image_out, planche, sprite_sheet};

use super::{Body, Built, Context, Frames, Image, MAX_SIDE, PreviewRequest, sheet};

/// Reduces a decoded picture so the largest side is at most [`MAX_SIDE`].
pub(super) fn fit(width: u32, height: u32, rgba: Vec<u8>) -> Option<Image> {
    let (width, height, rgba) = if width.max(height) > MAX_SIDE {
        image_out::reduire_rgba(&rgba, width, height, MAX_SIDE).ok()?
    } else {
        (width, height, rgba)
    };
    let image = Image {
        width,
        height,
        rgba,
    };
    image.is_valid().then_some(image)
}

fn message(text: impl Into<String>) -> Built {
    (Body::Message(text.into()), Vec::new(), false)
}

/// A G4TX container: the texture selected by `request.index`, or its sprites when it is an atlas.
pub(super) fn texture(ctx: &Context<'_>, request: &PreviewRequest, data: &[u8]) -> Built {
    let parsed = match g4tx::parse(data) {
        Ok(parsed) => parsed,
        Err(error) => return message(format!("g4tx: {error}")),
    };
    if parsed.textures.is_empty() {
        return message("g4tx: the container holds no texture");
    }
    let count = parsed.textures.len();
    let index = request.index.rem_euclid(i32::try_from(count).unwrap_or(1)) as usize;
    let tex = &parsed.textures[index];
    let mut meta = vec![
        ("texture".to_owned(), format!("{} ({} of {count})", tex.name, index + 1)),
        ("dimensions".to_owned(), format!("{} x {}", tex.width, tex.height)),
        (
            "storage".to_owned(),
            if tex.is_dds { "DDS payload" } else { "raw payload" }.to_owned(),
        ),
    ];
    if (ctx.cancelled)() {
        return (Body::Message("cancelled".into()), meta, false);
    }
    let Some((w, h, rgba)) = g4tx_decode::decode_texture_rgba(data, tex) else {
        meta.push(("decoder".to_owned(), "no decoder for this pixel format".to_owned()));
        return (
            Body::Message(format!("cannot decode `{}`", tex.name)),
            meta,
            false,
        );
    };
    if let Some(fiche) = planche::analyser(data).into_iter().find(|f| f.nom == tex.name) {
        meta.push(("planche".to_owned(), format!("{} / {:?}", fiche.role.nom(), fiche.convention)));
        if let Some(mask) = fiche.nom_masque {
            meta.push(("mask".to_owned(), mask));
        }
    }
    if let Some(sheet) = sprite_sheet::depuis_g4tx(&parsed, index)
        && !sheet.is_empty()
    {
        meta.push(("sprites".to_owned(), sheet.len().to_string()));
        let frames: Frames = sheet::sprites(&sheet, w, h, &rgba);
        if !frames.images.is_empty() {
            return (Body::Frames(frames), meta, false);
        }
    }
    match fit(w, h, rgba) {
        Some(image) => (Body::Image(image), meta, false),
        None => (Body::Message("decoded picture has an invalid size".into()), meta, false),
    }
}

/// A DDS or PNG file.
pub(super) fn picture(data: &[u8]) -> Built {
    if data.starts_with(b"DDS ") {
        return match g4tx_decode::decode_dds_to_rgba(data).and_then(|(w, h, p)| fit(w, h, p)) {
            Some(image) => (
                Body::Image(image),
                vec![("format".to_owned(), "DDS".to_owned())],
                false,
            ),
            None => message("dds: no decoder for this pixel format"),
        };
    }
    if data.starts_with(&[0x89, b'P', b'N', b'G']) {
        let mut decoder = png::Decoder::new(std::io::Cursor::new(data));
        decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::ALPHA);
        let Ok(mut reader) = decoder.read_info() else {
            return message("png: unreadable header");
        };
        let Some(size) = reader.output_buffer_size() else {
            return message("png: size overflow");
        };
        let mut pixels = vec![0; size];
        let Ok(info) = reader.next_frame(&mut pixels) else {
            return message("png: unreadable data");
        };
        pixels.truncate(info.buffer_size());
        let rgba = match (info.color_type, info.bit_depth) {
            (png::ColorType::Rgba, png::BitDepth::Eight) => pixels,
            (png::ColorType::Rgb, png::BitDepth::Eight) => pixels
                .as_chunks::<3>()
                .0
                .iter()
                .flat_map(|p| [p[0], p[1], p[2], 255])
                .collect(),
            (png::ColorType::Grayscale, png::BitDepth::Eight) => {
                pixels.iter().flat_map(|g| [*g, *g, *g, 255]).collect()
            }
            (png::ColorType::GrayscaleAlpha, png::BitDepth::Eight) => pixels
                .as_chunks::<2>()
                .0
                .iter()
                .flat_map(|p| [p[0], p[0], p[0], p[1]])
                .collect(),
            (color, depth) => return message(format!("png: {color:?} {depth:?} is not decoded")),
        };
        return match fit(info.width, info.height, rgba) {
            Some(image) => (
                Body::Image(image),
                vec![("format".to_owned(), "PNG".to_owned())],
                false,
            ),
            None => message("png: invalid size"),
        };
    }
    message("webp: the explorer has no WebP decoder")
}
