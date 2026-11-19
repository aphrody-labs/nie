//! The headless backend: smithay's CPU `PixmanRenderer` into an offscreen image.

use std::fs::File;
use std::io::BufWriter;
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::pixman::PixmanRenderer;
use smithay::backend::renderer::{Bind, ExportMem, ImportDma, Offscreen};
use smithay::utils::{Rectangle, Size};

use crate::render::{finish_frame, render_scene};
use crate::state::{Compositor, Config};

/// Memory order R, G, B, A (the little-endian `Abgr8888` word), the layout PNG expects.
const FORMAT: Fourcc = Fourcc::Abgr8888;

/// The whole output as RGBA, 8 bits per channel.
#[derive(Debug, Clone)]
pub struct Screenshot {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl Screenshot {
    /// The RGBA value at output pixel (`x`, `y`).
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let at = ((y * self.width + x) * 4) as usize;
        [self.rgba[at], self.rgba[at + 1], self.rgba[at + 2], self.rgba[at + 3]]
    }

    /// Save as an 8-bit RGBA PNG.
    pub fn write_png(&self, path: &Path) -> Result<()> {
        let file = File::create(path).with_context(|| format!("create {}", path.display()))?;
        let mut encoder = png::Encoder::new(BufWriter::new(file), self.width, self.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().context("png header")?;
        writer.write_image_data(&self.rgba).context("png data")?;
        writer.finish().context("png finish")
    }
}

/// A compositor with no GPU, no DRM and no display: it composites into memory.
pub struct Headless {
    comp: Compositor,
    renderer: PixmanRenderer,
    image: pixman::Image<'static, 'static>,
    tracker: OutputDamageTracker,
    size: Size<i32, smithay::utils::Buffer>,
}

impl Headless {
    pub fn new(config: &Config) -> Result<Self> {
        let mut comp = Compositor::new(config)?;
        let mut renderer = PixmanRenderer::new().map_err(|e| anyhow!("pixman renderer: {e:?}"))?;
        comp.state.enable_dmabuf(renderer.dmabuf_formats());
        let size = Size::from(config.size);
        let image = renderer.create_buffer(FORMAT, size).map_err(|e| anyhow!("offscreen buffer: {e:?}"))?;
        let tracker = OutputDamageTracker::from_output(&comp.state.output);
        Ok(Self { comp, renderer, image, tracker, size })
    }

    /// The compositor: socket path, state and event loop.
    pub fn compositor(&mut self) -> &mut Compositor {
        &mut self.comp
    }

    /// Path of the Wayland socket clients connect to.
    pub fn socket_path(&self) -> &Path {
        self.comp.socket_path()
    }

    /// Serve clients for at most `timeout`, then draw when something changed.
    pub fn dispatch(&mut self, timeout: Duration) -> Result<()> {
        self.comp.dispatch(Some(timeout))?;
        if self.comp.state.needs_redraw {
            self.render()?;
        }
        Ok(())
    }

    /// Draw the scene into the offscreen image and release the frame callbacks.
    pub fn render(&mut self) -> Result<()> {
        {
            let mut target = self.renderer.bind(&mut self.image).map_err(|e| anyhow!("bind: {e:?}"))?;
            render_scene(&self.comp.state, &mut self.renderer, &mut target, &mut self.tracker, 0)?;
        }
        finish_frame(&mut self.comp.state);
        self.comp.flush()
    }

    /// Draw now and read the whole output back.
    pub fn screenshot(&mut self) -> Result<Screenshot> {
        self.render()?;
        let target = self.renderer.bind(&mut self.image).map_err(|e| anyhow!("bind: {e:?}"))?;
        let mapping = self
            .renderer
            .copy_framebuffer(&target, Rectangle::from_size(self.size), FORMAT)
            .map_err(|e| anyhow!("copy framebuffer: {e:?}"))?;
        let rgba = self.renderer.map_texture(&mapping).map_err(|e| anyhow!("map: {e:?}"))?.to_vec();
        Ok(Screenshot { width: self.size.w as u32, height: self.size.h as u32, rgba })
    }
}
