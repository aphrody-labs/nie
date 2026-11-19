//! Azalee wiki: every page is generated from the PostgreSQL import of the game's VFS
//! (`vfs_entry`, `vfs_file`, `t2b_*`, `rdbn_<id>` schemas) and rendered through the shared
//! UI tree. The sitemap comes from the same table through `nie-sitemap`; images and icons come
//! from the VFS through the CDN path `/cdn/static/<vfs path>` (textures are decoded to PNG).

pub mod db;
pub mod i18n;
pub mod pages;
pub mod server;
