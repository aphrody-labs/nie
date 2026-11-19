//! Specs sync: for each official specification nie relies on, record where it comes from, which
//! version it is, a hash of the fetched content and the few facts nie depends on (limits, magic
//! numbers, ABI tables), extracted by regular expressions from the official page. Prose is never
//! copied: the generated documents hold short facts and links. A fact whose pattern stops
//! matching fails the sync, so a rewritten upstream page is noticed instead of silently changing.
//!
//! Host specs (kernel, CPU, memory, virtualization, filesystems, GPUs) are collected live, over
//! ssh for the fleet and locally for the PC.

pub mod config;
pub mod fetch;
pub mod hosts;
pub mod publish;
pub mod sync;
