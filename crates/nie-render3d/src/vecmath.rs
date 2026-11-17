//! Helpers vectoriels 3D partagés par [`render`](crate::render) et [`scene`](crate::scene).
//!
//! POD `[f32; 3]` + math **scalaire** (pas de SIMD/FMA) pour le déterminisme du rendu CPU.
//! L'implémentation vit dans [`softraster`](crate::softraster), le rastériseur CPU déterministe ;
//! ce module n'en garde que les noms internes.

pub(crate) use crate::softraster::{V3, cross, dot, normv, sub};
