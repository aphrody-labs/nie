//! Developer commands: `lint`, `fmt`, `test`, `compile` and `check`.
//!
//! Each runs through cargo on this machine, and the heavy steps (clippy, tests, coverage, release and
//! cross builds) go to the build host of `deploy/build.toml` (`NIE_BUILD_HOST` and `NIE_BUILD_DIR`
//! override it) when it answers; `--local` and `--remote` force the choice. On the build host itself
//! everything runs in place. The coverage lints that read the game (`nie vfs lint`, `nie exe lint`) are
//! commands of the `nie` CLI, not of this crate.

pub mod check;
pub mod compile;
pub mod fmt;
pub mod host;
pub mod lint;
pub mod remote;
pub mod step;
pub mod test;
