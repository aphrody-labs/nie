//! `bit32` library of PUC-Rio Lua 5.2.4 (`lbitlib.c`) on `u32`.
//!
//! Arguments arrive as `f64`, the only Lua number type, and are normalised exactly like the
//! stock build for x86 does:
//! - `luaL_checkunsigned` goes through the `LUA_IEEE754TRICK` (`n + 2^52 + 2^51`, low 32 bits of
//!   the result), which rounds to nearest and wraps modulo 2^32 for negative and large values;
//! - `luaL_checkint` truncates toward zero to a 64-bit integer (`cvttsd2si`, which yields
//!   `i64::MIN` out of range) and keeps its low 32 bits.
//!
//! Shift counts are masked to five bits like the `shl`/`shr` instructions do, which is what the
//! C code relies on for its out of range (undefined) cases.
//!
//! The two argument checks of `extract` and `replace` return the complete message
//! (`bad argument #N to 'extract' (field cannot be negative)`); the caller adds only the
//! `chunk:line:` position.

const NBITS: i32 = 32;

/// `luaL_checkunsigned`: the 32-bit value of a Lua number.
#[must_use]
pub fn to_unsigned(n: f64) -> u32 {
    (n + 6_755_399_441_055_744.0).to_bits() as u32
}

/// `luaL_checkint`: the 32-bit signed value of a Lua number.
#[must_use]
pub fn to_int(n: f64) -> i32 {
    let wide = if (-9_223_372_036_854_775_808.0..9_223_372_036_854_775_808.0).contains(&n) {
        n as i64
    } else {
        i64::MIN
    };
    wide as i32
}

/// Builds a number with `n` ones, `1 <= n <= 32`; larger counts wrap like the C shift does.
const fn mask(n: i32) -> u32 {
    !(0xFFFF_FFFE_u32.wrapping_shl((n - 1) as u32))
}

fn and_all(args: &[f64]) -> u32 {
    args.iter().fold(u32::MAX, |r, &a| r & to_unsigned(a))
}

/// `bit32.band(...)`.
#[must_use]
pub fn band(args: &[f64]) -> u32 {
    and_all(args)
}

/// `bit32.btest(...)`.
#[must_use]
pub fn btest(args: &[f64]) -> bool {
    and_all(args) != 0
}

/// `bit32.bor(...)`.
#[must_use]
pub fn bor(args: &[f64]) -> u32 {
    args.iter().fold(0, |r, &a| r | to_unsigned(a))
}

/// `bit32.bxor(...)`.
#[must_use]
pub fn bxor(args: &[f64]) -> u32 {
    args.iter().fold(0, |r, &a| r ^ to_unsigned(a))
}

/// `bit32.bnot(x)`.
#[must_use]
pub fn bnot(x: f64) -> u32 {
    !to_unsigned(x)
}

/// `b_shift`: logical shift, left for a positive count and right for a negative one.
fn shift(r: u32, i: i32) -> u32 {
    if i < 0 {
        // the compiler knows `-i` is positive, so `i32::MIN` compares as a huge count
        let i = i.wrapping_neg() as u32;
        if i >= NBITS as u32 { 0 } else { r >> i }
    } else if i >= NBITS {
        0
    } else {
        r << i
    }
}

/// `bit32.lshift(x, disp)`.
#[must_use]
pub fn lshift(x: f64, disp: f64) -> u32 {
    shift(to_unsigned(x), to_int(disp))
}

/// `bit32.rshift(x, disp)`.
#[must_use]
pub fn rshift(x: f64, disp: f64) -> u32 {
    shift(to_unsigned(x), to_int(disp).wrapping_neg())
}

/// `bit32.arshift(x, disp)`.
#[must_use]
pub fn arshift(x: f64, disp: f64) -> u32 {
    let r = to_unsigned(x);
    let i = to_int(disp);
    if i < 0 || r & (1 << (NBITS - 1)) == 0 {
        shift(r, i.wrapping_neg())
    } else if i >= NBITS {
        u32::MAX
    } else {
        (r >> i) | !(u32::MAX >> i)
    }
}

fn rot(x: f64, i: i32) -> u32 {
    let r = to_unsigned(x);
    let i = i & (NBITS - 1);
    if i == 0 { r } else { (r << i) | (r >> (NBITS - i)) }
}

/// `bit32.lrotate(x, disp)`.
#[must_use]
pub fn lrotate(x: f64, disp: f64) -> u32 {
    rot(x, to_int(disp))
}

/// `bit32.rrotate(x, disp)`.
#[must_use]
pub fn rrotate(x: f64, disp: f64) -> u32 {
    rot(x, to_int(disp).wrapping_neg())
}

/// `fieldargs`: validates `field` (argument `farg`) and `width` (argument `farg + 1`).
fn fieldargs(name: &str, farg: usize, field: f64, width: Option<f64>) -> Result<(i32, i32), String> {
    let f = to_int(field);
    let w = width.map_or(1, to_int);
    if f < 0 {
        return Err(format!("bad argument #{farg} to '{name}' (field cannot be negative)"));
    }
    if w <= 0 {
        return Err(format!(
            "bad argument #{} to '{name}' (width must be positive)",
            farg + 1
        ));
    }
    if f.wrapping_add(w) > NBITS {
        return Err("trying to access non-existent bits".to_owned());
    }
    Ok((f, w))
}

/// `bit32.extract(x, field [, width])`.
///
/// # Errors
///
/// `field cannot be negative`, `width must be positive` and `trying to access non-existent bits`.
pub fn extract(x: f64, field: f64, width: Option<f64>) -> Result<u32, String> {
    let r = to_unsigned(x);
    let (f, w) = fieldargs("extract", 2, field, width)?;
    Ok(r.wrapping_shr(f as u32) & mask(w))
}

/// `bit32.replace(x, v, field [, width])`.
///
/// # Errors
///
/// Same as [`extract`], reported for arguments 3 and 4 of `replace`.
pub fn replace(x: f64, v: f64, field: f64, width: Option<f64>) -> Result<u32, String> {
    let r = to_unsigned(x);
    let v = to_unsigned(v);
    let (f, w) = fieldargs("replace", 3, field, width)?;
    let m = mask(w);
    let v = v & m;
    Ok((r & !m.wrapping_shl(f as u32)) | v.wrapping_shl(f as u32))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argument_normalisation_wraps() {
        assert_eq!(to_unsigned(-1.0), u32::MAX);
        assert_eq!(to_unsigned(4_294_967_296.0), 0);
        assert_eq!(to_unsigned(4_294_967_297.0), 1);
        assert_eq!(to_unsigned(2.5), 2);
        assert_eq!(to_unsigned(3.5), 4);
        assert_eq!(to_unsigned(f64::NAN), 0);
        assert_eq!(to_int(f64::NAN), 0);
        assert_eq!(to_int(-1.9), -1);
    }

    #[test]
    fn logic_and_shifts() {
        assert_eq!(band(&[]), u32::MAX);
        assert_eq!(bor(&[]), 0);
        assert_eq!(band(&[0xff.into(), 0x0f.into()]), 0x0f);
        assert_eq!(bxor(&[3.0, 5.0]), 6);
        assert_eq!(bnot(0.0), u32::MAX);
        assert!(btest(&[1.0, 3.0]));
        assert!(!btest(&[1.0, 2.0]));
        assert_eq!(lshift(1.0, 31.0), 0x8000_0000);
        assert_eq!(lshift(1.0, 32.0), 0);
        assert_eq!(lshift(0x8000_0000_u32.into(), -31.0), 1);
        assert_eq!(rshift(0x8000_0000_u32.into(), 31.0), 1);
        assert_eq!(arshift(0x8000_0000_u32.into(), 4.0), 0xf800_0000);
        assert_eq!(arshift(0x8000_0000_u32.into(), 40.0), u32::MAX);
        assert_eq!(arshift(0x4000_0000_u32.into(), 4.0), 0x0400_0000);
        assert_eq!(arshift(1.0, -4.0), 16);
        assert_eq!(lrotate(0x8000_0001_u32.into(), 1.0), 3);
        assert_eq!(rrotate(3.0, 1.0), 0x8000_0001);
        assert_eq!(lrotate(5.0, 32.0), 5);
    }

    #[test]
    fn fields() {
        assert_eq!(extract(f64::from(0xabcd_u32), 4.0, Some(8.0)), Ok(0xbc));
        assert_eq!(extract(f64::from(u32::MAX), 0.0, Some(32.0)), Ok(u32::MAX));
        assert_eq!(replace(0.0, 0xf.into(), 4.0, Some(4.0)), Ok(0xf0));
        assert_eq!(replace(f64::from(u32::MAX), 0.0, 0.0, Some(32.0)), Ok(0));
        assert_eq!(
            extract(1.0, -1.0, None),
            Err("bad argument #2 to 'extract' (field cannot be negative)".to_owned())
        );
        assert_eq!(
            extract(1.0, 0.0, Some(0.0)),
            Err("bad argument #3 to 'extract' (width must be positive)".to_owned())
        );
        assert_eq!(
            replace(1.0, 1.0, 31.0, Some(2.0)),
            Err("trying to access non-existent bits".to_owned())
        );
    }
}
