//! Sprite sheets and film cards.

use nie_formats::sprite_sheet::SpriteSheet;
use nie_formats::usm;

use super::{Body, Built, Frames, Image};

/// Sprites kept per sheet.
const MAX_SPRITES: usize = 256;
/// Largest side of one sprite, in pixels.
const SPRITE_SIDE: u32 = 256;

/// Cuts every named region of an atlas out of the decoded texture.
pub(super) fn sprites(sheet: &SpriteSheet, width: u32, height: u32, rgba: &[u8]) -> Frames {
    let mut frames = Frames::default();
    for sprite in sheet.sprites.iter().take(MAX_SPRITES) {
        let (x, y) = (sprite.x.max(0) as u32, sprite.y.max(0) as u32);
        let (w, h) = (sprite.largeur.max(0) as u32, sprite.hauteur.max(0) as u32);
        if w == 0 || h == 0 || x + w > width || y + h > height {
            continue;
        }
        let mut crop = Vec::with_capacity(w as usize * h as usize * 4);
        for row in y..y + h {
            let start = (row as usize * width as usize + x as usize) * 4;
            crop.extend_from_slice(&rgba[start..start + w as usize * 4]);
        }
        let image = if w.max(h) > SPRITE_SIDE {
            nie_formats::image_out::reduire_rgba(&crop, w, h, SPRITE_SIDE)
                .ok()
                .map(|(width, height, rgba)| Image { width, height, rgba })
        } else {
            Some(Image {
                width: w,
                height: h,
                rgba: crop,
            })
        };
        if let Some(image) = image.filter(Image::is_valid) {
            frames.images.push(image);
            frames
                .labels
                .push(format!("{} ({w} x {h} at {x},{y})", sprite.nom));
        }
    }
    frames
}

/// The facts of a film, shown when frames cannot be decoded.
pub(super) fn film_card(path: &str, data: &[u8], why: &str) -> Built {
    let name = usm::nom_fichier_de(path);
    let mut meta = Vec::new();
    match usm::inspecter(data, name) {
        Ok(info) => {
            meta.push(("codec".to_owned(), info.codec.nom().to_owned()));
            let head = &info.entete;
            meta.push((
                "display".to_owned(),
                format!("{} x {}", head.largeur_affichee, head.hauteur_affichee),
            ));
            if let Some(fps) = head.images_par_seconde() {
                meta.push(("rate".to_owned(), format!("{fps:.3} fps")));
            }
            meta.push(("frames".to_owned(), info.images.to_string()));
            if let Some(seconds) = info.duree() {
                meta.push(("duration".to_owned(), format!("{seconds:.1} s")));
            }
            for track in &info.pistes {
                meta.push((
                    format!("audio {}", track.canal),
                    format!(
                        "{} {} Hz, {} ch, {} bytes",
                        track.codec.nom(),
                        track.frequence,
                        track.canaux,
                        track.taille
                    ),
                ));
            }
            if info.dechiffre {
                meta.push(("envelope".to_owned(), "CRI-encrypted, decrypted for the preview".to_owned()));
            }
        }
        Err(error) => meta.push(("usm".to_owned(), error.to_string())),
    }
    (Body::Message(why.to_owned()), meta, false)
}
