//! The scene render path, generic over the smithay renderer.

use std::time::Duration;

use anyhow::{Result, anyhow};
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::element::solid::SolidColorRenderElement;
use smithay::backend::renderer::{ImportAll, Renderer, Texture};
use smithay::desktop::layer_map_for_output;
use smithay::desktop::space::render_output;
use smithay::desktop::utils::OutputPresentationFeedback;
use smithay::reexports::wayland_protocols::wp::presentation_time::server::wp_presentation_feedback::Kind;
use smithay::utils::{Monotonic, Physical, Rectangle};
use smithay::wayland::presentation::Refresh;

use crate::state::State;

/// Desktop background behind every surface.
pub const CLEAR_COLOR: [f32; 4] = [0.07, 0.08, 0.10, 1.0];

/// Draw the output into `framebuffer`; returns the damage, `None` when nothing changed.
pub fn render_scene<R>(
    state: &State,
    renderer: &mut R,
    framebuffer: &mut R::Framebuffer<'_>,
    tracker: &mut OutputDamageTracker,
    age: usize,
) -> Result<Option<Vec<Rectangle<i32, Physical>>>>
where
    R: Renderer + ImportAll,
    R::TextureId: Clone + Texture + 'static,
{
    let result = render_output::<_, SolidColorRenderElement, _, _>(
        &state.output,
        renderer,
        framebuffer,
        1.0,
        age,
        [&state.space],
        &[],
        tracker,
        CLEAR_COLOR,
    )
    .map_err(|e| anyhow!("render: {e:?}"))?;
    result.sync.wait().map_err(|e| anyhow!("render sync: {e:?}"))?;
    Ok(result.damage.cloned())
}

/// After a frame reached the output: frame callbacks and presentation feedback to every surface.
pub fn finish_frame(state: &mut State) {
    let output = state.output.clone();
    let time = state.clock.now();
    let mut feedback = OutputPresentationFeedback::new(&output);

    for window in state.space.elements() {
        window.send_frame(&output, time, Some(Duration::ZERO), |_, _| Some(output.clone()));
        window.take_presentation_feedback(&mut feedback, |_, _| Some(output.clone()), |_, _| Kind::empty());
    }
    for layer in layer_map_for_output(&output).layers() {
        layer.send_frame(&output, time, Some(Duration::ZERO), |_, _| Some(output.clone()));
        layer.take_presentation_feedback(&mut feedback, |_, _| Some(output.clone()), |_, _| Kind::empty());
    }

    let refresh = output
        .current_mode()
        .filter(|mode| mode.refresh > 0)
        .map_or(Refresh::Unknown, |mode| {
            Refresh::fixed(Duration::from_secs_f64(1000.0 / f64::from(mode.refresh)))
        });
    feedback.presented::<_, Monotonic>(time, refresh, state.frame_seq, Kind::empty());
    state.frame_seq += 1;
    state.needs_redraw = false;
}
