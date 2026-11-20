//! The browser entry point: the explorer over `nie serve`, drawn by gpui_web (WebGPU, WebGL fallback).

use std::borrow::Cow;

use gpui::{App, AppContext as _, Bounds, WindowBounds, WindowOptions, px, size};
use nie_ui::explorer::Explorer;
use nie_ui::theme::{Preferences, Theme};
use nie_ui::Settings;
use wasm_bindgen::prelude::wasm_bindgen;

use crate::http_explorer::HttpBackend;

const FONTS: [&[u8]; 4] = [
    include_bytes!("../../../../aphrody/assets/fonts/ibm-plex-sans/IBMPlexSans-Regular.ttf"),
    include_bytes!("../../../../aphrody/assets/fonts/ibm-plex-sans/IBMPlexSans-SemiBold.ttf"),
    include_bytes!("../../../../aphrody/assets/fonts/lilex/Lilex-Regular.ttf"),
    include_bytes!("../../../../aphrody/assets/fonts/lilex/Lilex-Bold.ttf"),
];

fn preferences() -> Preferences {
    let search = web_sys::window().and_then(|w| w.location().search().ok()).unwrap_or_default();
    let has = |k: &str| search.trim_start_matches('?').split('&').any(|p| p == k);
    Preferences {
        high_contrast: has("contrast=high"),
        scale: search
            .trim_start_matches('?')
            .split('&')
            .find_map(|p| p.strip_prefix("scale="))
            .and_then(|v| v.parse().ok())
            .unwrap_or(1.0),
        reduce_motion: has("motion=reduce"),
    }
}

/// `?backend=webgl` or `?backend=webgpu` picks the graphics API; WebGPU with a WebGL fallback otherwise.
fn backend() -> gpui_platform::WebBackendPreference {
    let search = web_sys::window().and_then(|w| w.location().search().ok()).unwrap_or_default();
    match search.trim_start_matches('?').split('&').find_map(|p| p.strip_prefix("backend=")) {
        Some("webgl") => gpui_platform::WebBackendPreference::WebGl,
        Some("webgpu") => gpui_platform::WebBackendPreference::WebGpu,
        _ => gpui_platform::WebBackendPreference::Auto,
    }
}

/// Starts the explorer in the page. Called by the generated JavaScript when the module loads.
#[wasm_bindgen(start)]
pub fn start() {
    gpui_platform::web_init();
    use gpui::Platform as _;
    let platform = std::rc::Rc::new(gpui_web::WebPlatform::new_with_backend(false, backend()));
    let fetch = std::sync::Arc::new(gpui_web::FetchHttpClient::new(platform.foreground_executor().to_send()));
    let app = gpui::Application::with_platform(platform).with_http_client(fetch);
    let handle = app.run_embedded(|cx: &mut App| {
        if let Err(error) = cx.text_system().add_fonts(FONTS.iter().map(|f| Cow::Borrowed(*f)).collect()) {
            web_sys::console::error_1(&format!("failed to load fonts: {error:#}").into());
            return;
        }
        // The palette comes from the game's FONT_COLOR table through `nie serve`; until it is
        // fetched the contrast floor is shown.
        Settings::install(cx, Theme::bootstrap(preferences()));
        let backend = HttpBackend::new(cx.http_client(), cx.foreground_executor().clone(), "");
        let bounds = Bounds::centered(None, size(px(1280.0), px(760.0)), cx);
        let opened = cx.open_window(
            WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() },
            |window, cx| {
                let view = cx.new(|cx| Explorer::new(backend, cx));
                let focus = view.read(cx).focus_handle().clone();
                window.focus(&focus, cx);
                view
            },
        );
        if let Err(error) = opened {
            web_sys::console::error_1(&format!("cannot open the window: {error:#}").into());
        }
        cx.activate(true);
    });
    std::mem::forget(handle);
}
