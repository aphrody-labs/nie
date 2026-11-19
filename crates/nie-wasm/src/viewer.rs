//! The WebGPU model viewer of the browser app: `nie-render3d`'s canvas presenter behind
//! `wasm-bindgen`. The page owns the animation loop and calls [`ModelViewer::render`] from
//! `requestAnimationFrame`.

use nie_render3d::web::WebViewer;
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

fn fail(error: impl std::fmt::Display) -> JsError {
    JsError::new(&format!("{error:#}"))
}

/// A GLB model drawn on a canvas with WebGPU.
#[wasm_bindgen]
pub struct ModelViewer {
    inner: WebViewer,
}

#[wasm_bindgen]
impl ModelViewer {
    /// Creates the device on `canvas`. Fails when WebGPU is not available in this context.
    pub async fn create(canvas: HtmlCanvasElement) -> Result<ModelViewer, JsError> {
        Ok(Self {
            inner: WebViewer::new(canvas).await.map_err(fail)?,
        })
    }

    /// The adapter the browser chose, as JSON.
    pub fn adapter(&self) -> String {
        let info = self.inner.adapter_info();
        serde_json::json!({
            "name": info.name,
            "backend": format!("{:?}", info.backend),
            "device_type": format!("{:?}", info.device_type),
            "format": format!("{:?}", self.inner.surface_format()),
        })
        .to_string()
    }

    /// Replaces the model with the GLB `bytes`.
    pub fn load_glb(&mut self, bytes: &[u8]) -> Result<(), JsError> {
        self.inner.load_glb(bytes).map_err(fail)
    }

    /// Sets the camera: radians and a distance in model radii.
    pub fn orbit(&mut self, yaw: f32, pitch: f32, distance: f32) -> Result<(), JsError> {
        self.inner.orbit(yaw, pitch, distance).map_err(fail)
    }

    /// Sets the backing store size in physical pixels.
    pub fn resize(&mut self, width: f64, height: f64) -> Result<(), JsError> {
        self.inner.resize(width, height).map_err(fail)
    }

    /// Draws one frame; `false` when the browser skipped it.
    pub fn render(&mut self) -> Result<bool, JsError> {
        self.inner.render().map_err(fail)
    }
}
