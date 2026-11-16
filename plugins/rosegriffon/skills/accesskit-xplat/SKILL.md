---
name: accesskit-xplat
description: "Use when working with the `accesskit_xplat` crate of blitz: AccessKit UI accessibility infrastructure: cross-platform adapter."
---

# accesskit_xplat

- Version: 0.2.1 (workspace member of `third_party/blitz`, vendored commit `a84a3c51b562`)
- Source: `third_party/blitz/packages/accesskit_xplat/` (manifest `third_party/blitz/packages/accesskit_xplat/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/blitz/tree/main/packages/accesskit_xplat
- License: Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

AccessKit UI accessibility infrastructure: cross-platform adapter

## README (`third_party/blitz/packages/accesskit_xplat/README.md`)

# accesskit_xplat

Cross-platform [AccessKit](github.com/AccessKit/accesskit) adapter similar to [accesskit_winit](https://docs.rs/accesskit_winit), but without depending on Winit.
This allows this crate to be used with any version of Winit without needing a version of crate that matches the version of Winit
that you are using (this is particularly helpful for beta or git versions of Winit that don't usually get an accesskit_winit release).

**WARNING:** The AccessKit developers [have noted](https://github.com/AccessKit/accesskit/pull/656#issuecomment-3706315306) that the approaches used by both this crates and accesskit_winit may not be the
optimal way to implement AccessKit, so use this crate at your own risk. But if you would otherwise be using accesskit_winit then
this crate has no additional caveats (except that it requires little bit of extra boilerplate code.)

## Example usage

Based on Winit `0.31.0-beta.2`'s `Window` and `WindowEvent` types but could be adapted for other versions of Winit.

```rust
use accesskit::Rect;
use accesskit_xplat::{Adapter, EventHandler, WindowEvent as AccessKitEvent};
use raw_window_handle::HasWindowHandle;
use std::sync::Arc;
use winit_core::{
    event::WindowEvent,
    window::{Window, WindowId},
};

/// State of the accessibility node tree and platform adapter.
pub struct AccessibilityState {
    adapter: Adapter,
}

struct Handler {
    window_id: WindowId,
    // Whatever else you like here. Perhaps EventLoopProxy and/or a channel sender.
}
impl EventHandler for Handler {
    fn handle_accesskit_event(&self, event: AccessKitEvent) {
        // Your own custom event handling code
    }
}

impl AccessibilityState {
    pub fn new(window: &dyn Window) -> Self {
        let window_id = window.id();
        Self {
            adapter: Adapter::with_combined_handler(
                // On Android, pass `&android_activity::AndroidApp` when creating the `Adapter`
                #[cfg(target_os = "android")]
                &crate::current_android_app(),
                // On all other platforms, pass `RawWindowHandle` when creating the `Adapter`
                #[cfg(not(target_os = "android"))]
                window.window_handle().unwrap().as_raw(),
                Arc::new(Handler { window_id }),
            ),
        }
    }

    /// Allows reacting to window events.
    ///
    /// This must be called whenever a new window event is received
    /// and before it is handled by the application.
    pub fn process_window_event(&mut self, window: &dyn Window, event: &WindowEvent) {
        match event {
            WindowEvent::Focused(is_focused) => {
                self.adapter.set_focus(*is_focused);
            }
            WindowEvent::Moved(_) | WindowEvent::SurfaceResized(_) => {
                let outer_position: (_, _) = window
                    .outer_position()
                    .unwrap_or_default()
                    .cast::()
                    .into();

## Cargo features (from the manifest)

- `default` (enables 2)
- `async-io` (enables 1)
- `tokio` (enables 1)

## Public items by file

- `third_party/blitz/packages/accesskit_xplat/src/lib.rs`: enum WindowEvent, trait EventHandler, struct Adapter
- `third_party/blitz/packages/accesskit_xplat/src/platform_impl/android.rs`: struct Adapter
- `third_party/blitz/packages/accesskit_xplat/src/platform_impl/macos.rs`: struct Adapter
- `third_party/blitz/packages/accesskit_xplat/src/platform_impl/null.rs`: struct Adapter
- `third_party/blitz/packages/accesskit_xplat/src/platform_impl/unix.rs`: struct Adapter
- `third_party/blitz/packages/accesskit_xplat/src/platform_impl/windows.rs`: struct Adapter

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
