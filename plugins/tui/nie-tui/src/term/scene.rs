//! Turns a [`Frame`] of the terminal model into drawing calls on a [`Canvas`].
//!
//! The same scene feeds the sugarloaf window and the CPU screenshot, so what the evidence shows is
//! what the window draws. Coordinates are physical pixels from the top left of the window.

use std::sync::Arc;

use nie_explore::term::{Attrs, CursorStyle, Frame, Image, Rgb};

/// What a scene is drawn with.
pub trait Canvas {
    /// A filled rectangle, straight RGBA in `0.0..=1.0`.
    fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, rgba: [f32; 4]);
    /// Text whose first cell has its top left at `x`, `y`.
    fn text(&mut self, x: f32, y: f32, text: &str, color: [u8; 4], bold: bool, italic: bool);
    /// An image: its destination rectangle and the normalised source rectangle `[u0, v0, u1, v1]`.
    fn image(&mut self, image: &Arc<Image>, rect: [f32; 4], uv: [f32; 4], z_index: i32);
}

/// Where the grid sits and how big its cells are, in physical pixels.
#[derive(Clone, Copy, Debug)]
pub struct Layout {
    pub cell_w: f32,
    pub cell_h: f32,
    pub origin_x: f32,
    pub origin_y: f32,
}

/// Window-level state that changes how the cursor and the selection look.
#[derive(Clone, Copy, Debug)]
pub struct Look {
    pub focused: bool,
    pub cursor_visible: bool,
}

const SELECTION_BG: Rgb = [0x2f, 0x4f, 0x8f];
const SELECTION_FG: Rgb = [0xff, 0xff, 0xff];

fn rgba(c: Rgb) -> [f32; 4] {
    [f32::from(c[0]) / 255.0, f32::from(c[1]) / 255.0, f32::from(c[2]) / 255.0, 1.0]
}

fn rgba8(c: Rgb) -> [u8; 4] {
    [c[0], c[1], c[2], 255]
}

/// Draws the frame: cell backgrounds, images, cursor, then glyphs and decorations.
pub fn draw_frame(canvas: &mut dyn Canvas, frame: &Frame, lay: &Layout, look: Look) {
    let (cw, ch) = (lay.cell_w, lay.cell_h);
    let cell_rect = |col: usize, row: usize, span: usize| {
        (
            lay.origin_x + col as f32 * cw,
            lay.origin_y + row as f32 * ch,
            span as f32 * cw,
            ch,
        )
    };

    // Cell backgrounds, one rectangle per run of equal colour.
    for (row, cells) in frame.cells.iter().enumerate() {
        let mut col = 0;
        while col < cells.len() {
            let bg = if cells[col].selected { SELECTION_BG } else { cells[col].bg };
            let mut end = col + 1;
            while end < cells.len() && (if cells[end].selected { SELECTION_BG } else { cells[end].bg }) == bg {
                end += 1;
            }
            if bg != frame.default_bg {
                let (x, y, w, h) = cell_rect(col, row, end - col);
                canvas.rect(x, y, w, h, rgba(bg));
            }
            col = end;
        }
    }

    for p in &frame.placements {
        let r = [
            lay.origin_x + p.rect[0],
            lay.origin_y + p.rect[1],
            p.rect[2],
            p.rect[3],
        ];
        canvas.image(&p.image, r, p.uv, p.z_index);
    }

    // The cursor under the glyph: a filled block when focused, an outline otherwise.
    let cursor = frame.cursor;
    let show_cursor = cursor.style != CursorStyle::Hidden && frame.display_offset == 0 && look.cursor_visible;
    let mut block_cell = None;
    if show_cursor && cursor.row < frame.rows && cursor.col < frame.cols {
        let cell = &frame.cells[cursor.row][cursor.col];
        let span = usize::from(cell.width.max(1));
        let (x, y, w, h) = cell_rect(cursor.col, cursor.row, span);
        let thick = (ch / 8.0).round().max(1.0);
        match (cursor.style, look.focused) {
            (CursorStyle::Block, true) => {
                canvas.rect(x, y, w, h, rgba(cursor.color));
                block_cell = Some((cursor.row, cursor.col));
            }
            (CursorStyle::Block, false) => {
                let c = rgba(cursor.color);
                canvas.rect(x, y, w, thick, c);
                canvas.rect(x, y + h - thick, w, thick, c);
                canvas.rect(x, y, thick, h, c);
                canvas.rect(x + w - thick, y, thick, h, c);
            }
            (CursorStyle::Underline, _) => canvas.rect(x, y + h - thick, w, thick, rgba(cursor.color)),
            (CursorStyle::Beam, _) => canvas.rect(x, y, thick.min(w), h, rgba(cursor.color)),
            (CursorStyle::Hidden, _) => {}
        }
    }

    // Glyphs and decorations.
    let mut text = String::new();
    for (row, cells) in frame.cells.iter().enumerate() {
        for (col, cell) in cells.iter().enumerate() {
            if cell.width == 0 {
                continue;
            }
            let (x, y, w, h) = cell_rect(col, row, usize::from(cell.width));
            let mut fg = if cell.selected { SELECTION_FG } else { cell.fg };
            if block_cell == Some((row, col)) {
                fg = frame.default_bg;
            }
            if cell.ch != ' ' {
                text.clear();
                text.push(cell.ch);
                text.extend(cell.marks.iter());
                canvas.text(
                    x,
                    y,
                    &text,
                    rgba8(fg),
                    cell.attrs.has(Attrs::BOLD),
                    cell.attrs.has(Attrs::ITALIC),
                );
            }
            let thick = (ch / 16.0).round().max(1.0);
            if cell.attrs.underlined() {
                canvas.rect(x, y + h - thick * 2.0, w, thick, rgba(fg));
                if cell.attrs.has(Attrs::DOUBLE_UNDERLINE) {
                    canvas.rect(x, y + h - thick * 4.0, w, thick, rgba(fg));
                }
            }
            if cell.attrs.has(Attrs::STRIKEOUT) {
                canvas.rect(x, y + h / 2.0, w, thick, rgba(fg));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nie_explore::term::TermModel;

    #[derive(Default)]
    struct Recorder {
        rects: Vec<[f32; 4]>,
        texts: Vec<(f32, f32, String, [u8; 4])>,
        images: usize,
    }

    impl Canvas for Recorder {
        fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, _rgba: [f32; 4]) {
            self.rects.push([x, y, w, h]);
        }
        fn text(&mut self, x: f32, y: f32, text: &str, color: [u8; 4], _bold: bool, _italic: bool) {
            self.texts.push((x, y, text.to_string(), color));
        }
        fn image(&mut self, _image: &Arc<Image>, _rect: [f32; 4], _uv: [f32; 4], _z: i32) {
            self.images += 1;
        }
    }

    fn lay() -> Layout {
        Layout {
            cell_w: 10.0,
            cell_h: 20.0,
            origin_x: 0.0,
            origin_y: 0.0,
        }
    }

    #[test]
    fn glyphs_sit_on_the_cell_grid_and_backgrounds_merge_into_runs() {
        let mut m = TermModel::new(10, 3);
        m.feed(b"\x1b[44mab\x1b[0mc\x1b[?25l");
        let mut rec = Recorder::default();
        draw_frame(&mut rec, &m.frame(), &lay(), Look { focused: true, cursor_visible: true });
        assert_eq!(rec.rects, vec![[0.0, 0.0, 20.0, 20.0]], "one rectangle for the two blue cells");
        let texts: Vec<_> = rec.texts.iter().map(|t| (t.0, t.1, t.2.as_str())).collect();
        assert_eq!(texts, vec![(0.0, 0.0, "a"), (10.0, 0.0, "b"), (20.0, 0.0, "c")]);
    }

    #[test]
    fn block_cursor_recolours_its_glyph_and_hollows_when_unfocused() {
        let mut m = TermModel::new(10, 3);
        m.feed(b"x\x1b[H");
        let frame = m.frame();
        let mut focused = Recorder::default();
        draw_frame(&mut focused, &frame, &lay(), Look { focused: true, cursor_visible: true });
        assert_eq!(focused.rects, vec![[0.0, 0.0, 10.0, 20.0]], "filled block");
        assert_eq!(focused.texts[0].3, rgba8(frame.default_bg), "glyph under the block takes the background colour");
        let mut blurred = Recorder::default();
        draw_frame(&mut blurred, &frame, &lay(), Look { focused: false, cursor_visible: true });
        assert_eq!(blurred.rects.len(), 4, "outline of four edges");
        assert_eq!(blurred.texts[0].3, rgba8(frame.cells[0][0].fg));
    }

    #[test]
    fn images_are_forwarded_to_the_canvas() {
        let mut m = TermModel::new(20, 10);
        m.resize(20, 10, 10, 20);
        m.feed(b"\x1b_Gf=32,s=2,v=2,a=T;/wAA//8AAP//AAD//wAA/w==\x1b\\");
        let mut rec = Recorder::default();
        draw_frame(&mut rec, &m.frame(), &lay(), Look { focused: true, cursor_visible: false });
        assert_eq!(rec.images, 1);
    }
}
