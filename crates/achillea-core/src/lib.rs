//! Achillea rules, ported from `packages/achillea-core` (TypeScript). Pure functions, no I/O.
//! Numeric behaviour matches JavaScript (`Math.round` rounds half up), and the `tests/parity.rs`
//! test replays fixtures produced by the TypeScript implementation.

pub mod brackets;
pub mod clans;
pub mod elo;
pub mod league;
pub mod ranked;

/// JavaScript `Math.round`: halves round toward positive infinity.
#[must_use]
pub fn js_round(x: f64) -> f64 {
    (x + 0.5).floor()
}
