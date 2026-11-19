//! String library primitives of PUC-Rio Lua 5.2.4, in safe Rust over byte strings.
//!
//! Everything here is a transcription of the stock C code (`lstrlib.c`, `lobject.c`) or of the C
//! library calls it relies on (`printf`, `strtod` in the C locale, glibc flavour), so results
//! and error messages are byte-identical to the game's VM:
//!
//! - [`number_to_string`] / [`str_to_number`]: `lua_Number2str` (`%.14g`) and `luaO_str2d`;
//! - [`pat_find`], [`gmatch_step`], [`gsub`], [`expand_repl`]: the pattern matcher;
//! - [`format()`](format): `string.format`;
//! - [`sub`], [`rep`], [`byte`], [`lower`], [`upper`], [`reverse`], [`char_from`]: the simple
//!   functions with the 5.2 relative-position rules.
//!
//! Errors are plain `String`s holding what `luaL_error` would format, without the `chunk:line:`
//! position the caller prepends. [`format`] and [`char_from`] also validate arguments, so their
//! messages are complete (`bad argument #2 to 'format' (...)`).

use std::fmt::Write as _;

// ---------------------------------------------------------------------------------------------
// C library emulation
// ---------------------------------------------------------------------------------------------

/// `isspace` in the C locale (also `lisspace` of Lua): `\t \n \v \f \r` and the space.
const fn is_space(c: u8) -> bool {
    matches!(c, 9..=13 | b' ')
}

/// The conversion `(long long)n` compiles to on x86-64 (`cvttsd2si`): truncation, and
/// `i64::MIN` for NaN and out of range values.
fn cvt_i64(n: f64) -> i64 {
    if (-9_223_372_036_854_775_808.0..9_223_372_036_854_775_808.0).contains(&n) {
        n as i64
    } else {
        i64::MIN
    }
}

/// The conversion `(unsigned long long)n` compiles to on x86-64.
fn cvt_u64(n: f64) -> u64 {
    const TWO63: f64 = 9_223_372_036_854_775_808.0;
    if n < TWO63 {
        cvt_i64(n) as u64
    } else {
        (cvt_i64(n - TWO63) as u64) ^ (1 << 63)
    }
}

/// `ldexp`: `x * 2^e` with a single rounding (musl `scalbn`).
fn ldexp(mut x: f64, mut e: i32) -> f64 {
    const P1023: f64 = f64::from_bits(0x7fe0_0000_0000_0000);
    const PM969: f64 = f64::from_bits(0x0360_0000_0000_0000); // 2^-1022 * 2^53
    if e > 1023 {
        x *= P1023;
        e -= 1023;
        if e > 1023 {
            x *= P1023;
            e -= 1023;
            e = e.min(1023);
        }
    } else if e < -1022 {
        x *= PM969;
        e += 1022 - 53;
        if e < -1022 {
            x *= PM969;
            e += 1022 - 53;
            e = e.max(-1022);
        }
    }
    x * f64::from_bits(((0x3ff + i64::from(e)) as u64) << 52)
}

// ---------------------------------------------------------------------------------------------
// printf engine: %d %o %u %x %X %e %f %g %a %c %s with flags, width and precision
// ---------------------------------------------------------------------------------------------

/// A parsed `%[flags][width][.precision]` specification.
#[derive(Clone, Copy, Debug, Default)]
struct Spec {
    minus: bool,
    plus: bool,
    space: bool,
    hash: bool,
    zero: bool,
    width: usize,
    prec: Option<usize>,
}

/// Appends `sign`, `prefix` and `body` padded to the field width: zeros go between the prefix
/// and the digits when `zero_ok`, like glibc does for numbers.
fn pad_number(sign: &str, prefix: &str, body: &str, spec: &Spec, zero_ok: bool) -> Vec<u8> {
    let len = sign.len() + prefix.len() + body.len();
    let fill = spec.width.saturating_sub(len);
    let mut out = Vec::with_capacity(len + fill);
    let head = |out: &mut Vec<u8>| {
        out.extend_from_slice(sign.as_bytes());
        out.extend_from_slice(prefix.as_bytes());
    };
    if spec.minus {
        head(&mut out);
        out.extend_from_slice(body.as_bytes());
        out.resize(len + fill, b' ');
    } else if spec.zero && zero_ok {
        head(&mut out);
        out.resize(out.len() + fill, b'0');
        out.extend_from_slice(body.as_bytes());
    } else {
        out.resize(fill, b' ');
        head(&mut out);
        out.extend_from_slice(body.as_bytes());
    }
    out
}

/// Pads raw bytes (`%c`, `%s`) with spaces.
fn pad_bytes(body: &[u8], spec: &Spec) -> Vec<u8> {
    let fill = spec.width.saturating_sub(body.len());
    let mut out = Vec::with_capacity(body.len() + fill);
    if !spec.minus {
        out.resize(fill, b' ');
    }
    out.extend_from_slice(body);
    if spec.minus {
        out.resize(body.len() + fill, b' ');
    }
    out
}

/// `%d %i %u %o %x %X` of a magnitude and a sign.
fn fmt_int(neg: bool, mag: u64, conv: u8, spec: &Spec) -> Vec<u8> {
    let mut digits = match conv {
        b'o' => format!("{mag:o}"),
        b'x' => format!("{mag:x}"),
        b'X' => format!("{mag:X}"),
        _ => mag.to_string(),
    };
    if spec.prec == Some(0) && mag == 0 {
        digits.clear();
    }
    if let Some(p) = spec.prec
        && digits.len() < p
    {
        digits.insert_str(0, &"0".repeat(p - digits.len()));
    }
    let mut prefix = "";
    if spec.hash {
        match conv {
            b'o' if !digits.starts_with('0') => digits.insert(0, '0'),
            b'x' if mag != 0 => prefix = "0x",
            b'X' if mag != 0 => prefix = "0X",
            _ => {}
        }
    }
    let sign = match conv {
        b'd' | b'i' if neg => "-",
        b'd' | b'i' if spec.plus => "+",
        b'd' | b'i' if spec.space => " ",
        _ => "",
    };
    pad_number(sign, prefix, &digits, spec, spec.prec.is_none())
}

/// Decimal digits and exponent of `x >= 0` in scientific notation with `prec` fraction digits,
/// correctly rounded.
fn sci(x: f64, prec: usize) -> (String, i32) {
    let s = format!("{x:.prec$e}");
    let (mantissa, exp) = s.split_once('e').unwrap_or((&s, "0"));
    (mantissa.replace('.', ""), exp.parse().unwrap_or(0))
}

/// `%e` of `x >= 0`.
fn fmt_e(x: f64, prec: usize, upper: bool, hash: bool) -> String {
    let (digits, exp) = sci(x, prec);
    let mut out = String::with_capacity(prec + 8);
    out.push_str(&digits[..1]);
    if prec > 0 || hash {
        out.push('.');
    }
    out.push_str(&digits[1..]);
    out.push(if upper { 'E' } else { 'e' });
    out.push(if exp < 0 { '-' } else { '+' });
    let _ = write!(out, "{:02}", exp.unsigned_abs());
    out
}

/// `%f` of `x >= 0`.
fn fmt_f(x: f64, prec: usize, hash: bool) -> String {
    let mut out = format!("{x:.prec$}");
    if prec == 0 && hash {
        out.push('.');
    }
    out
}

/// `%g` of `x >= 0`.
fn fmt_g(x: f64, prec: usize, upper: bool, hash: bool) -> String {
    let p = prec.max(1);
    let exp = if x == 0.0 { 0 } else { sci(x, p - 1).1 };
    let p_i = i32::try_from(p).unwrap_or(i32::MAX);
    if exp < -4 || exp >= p_i {
        let mut s = fmt_e(x, p - 1, upper, hash);
        if !hash {
            let at = s.find(['e', 'E']).unwrap_or(s.len());
            let mantissa = &s[..at];
            if mantissa.contains('.') {
                let kept = mantissa.trim_end_matches('0').trim_end_matches('.').len();
                s.replace_range(kept..at, "");
            }
        }
        s
    } else {
        let frac = usize::try_from(p_i - 1 - exp).unwrap_or(0);
        let mut s = format!("{x:.frac$}");
        if hash {
            if frac == 0 {
                s.push('.');
            }
        } else if s.contains('.') {
            let kept = s.trim_end_matches('0').trim_end_matches('.').len();
            s.truncate(kept);
        }
        s
    }
}

/// `%a` of `x >= 0` (glibc flavour), without the `0x` prefix.
fn fmt_a(x: f64, prec: Option<usize>, upper: bool, hash: bool) -> String {
    let bits = x.to_bits();
    let biased = ((bits >> 52) & 0x7ff) as i32;
    let mut frac = bits & ((1 << 52) - 1);
    let (mut lead, exp) = match (biased, frac) {
        (0, 0) => (0u64, 0),
        (0, _) => (0, -1022),
        _ => (1, biased - 1023),
    };
    let mut ndig = 13usize;
    match prec {
        None => {
            while ndig > 0 && frac & 0xf == 0 {
                frac >>= 4;
                ndig -= 1;
            }
        }
        Some(p) if p < 13 => {
            let shift = 4 * (13 - p);
            let rest = frac & ((1 << shift) - 1);
            let half = 1u64 << (shift - 1);
            frac >>= shift;
            let last_odd = if p == 0 { lead & 1 } else { frac & 1 } == 1;
            if rest > half || (rest == half && last_odd) {
                frac += 1;
                if p == 0 || frac >> (4 * p) != 0 {
                    frac &= if p == 0 { 0 } else { (1 << (4 * p)) - 1 };
                    lead += 1;
                }
            }
            ndig = p;
        }
        Some(_) => {}
    }
    let mut out = format!("{lead}");
    let shown = prec.map_or(ndig, |p| p.max(ndig));
    if shown > 0 || hash {
        out.push('.');
    }
    if ndig > 0 {
        let _ = write!(out, "{frac:0ndig$x}");
    }
    for _ in ndig..shown {
        out.push('0');
    }
    let _ = write!(out, "p{}{}", if exp < 0 { '-' } else { '+' }, exp.unsigned_abs());
    if upper {
        out.make_ascii_uppercase();
    }
    out
}

/// `%e %E %f %g %G %a %A` of any double.
fn fmt_float(conv: u8, x: f64, spec: &Spec) -> Vec<u8> {
    let upper = conv.is_ascii_uppercase();
    let sign = if x.is_sign_negative() {
        "-"
    } else if spec.plus {
        "+"
    } else if spec.space {
        " "
    } else {
        ""
    };
    if !x.is_finite() {
        let body = match (x.is_nan(), upper) {
            (true, false) => "nan",
            (true, true) => "NAN",
            (false, false) => "inf",
            (false, true) => "INF",
        };
        return pad_number(sign, "", body, spec, false);
    }
    let x = x.abs();
    let prec = spec.prec.unwrap_or(6);
    let (prefix, body) = match conv.to_ascii_lowercase() {
        b'f' => ("", fmt_f(x, prec, spec.hash)),
        b'e' => ("", fmt_e(x, prec, upper, spec.hash)),
        b'g' => ("", fmt_g(x, prec, upper, spec.hash)),
        _ => (
            if upper { "0X" } else { "0x" },
            fmt_a(x, spec.prec, upper, spec.hash),
        ),
    };
    pad_number(sign, prefix, &body, spec, true)
}

// ---------------------------------------------------------------------------------------------
// Number <-> string
// ---------------------------------------------------------------------------------------------

/// `lua_Number2str`: `printf("%.14g")` (glibc behaviour: `inf`, `-inf`, `nan`, `-nan`).
#[must_use]
pub fn number_to_string(n: f64) -> Vec<u8> {
    let spec = Spec::default();
    fmt_float(b'g', n, &Spec { prec: Some(14), ..spec })
}

/// `strtod` for decimal numbers in the C locale: the value and the end of the recognised prefix,
/// or `None` when nothing was recognised. `inf` and `nan` never reach it (`luaO_str2d` rejects
/// any `n`).
fn strtod_dec(s: &[u8]) -> Option<(f64, usize)> {
    let mut i = 0;
    while s.get(i).is_some_and(|&c| is_space(c)) {
        i += 1;
    }
    let start = i;
    if matches!(s.get(i), Some(b'+' | b'-')) {
        i += 1;
    }
    let int_start = i;
    while s.get(i).is_some_and(u8::is_ascii_digit) {
        i += 1;
    }
    let mut digits = i - int_start;
    if s.get(i) == Some(&b'.') {
        let frac_start = i + 1;
        let mut j = frac_start;
        while s.get(j).is_some_and(u8::is_ascii_digit) {
            j += 1;
        }
        digits += j - frac_start;
        if digits > 0 {
            i = j;
        }
    }
    if digits == 0 {
        return None;
    }
    if matches!(s.get(i), Some(b'e' | b'E')) {
        let mut j = i + 1;
        if matches!(s.get(j), Some(b'+' | b'-')) {
            j += 1;
        }
        if s.get(j).is_some_and(u8::is_ascii_digit) {
            while s.get(j).is_some_and(u8::is_ascii_digit) {
                j += 1;
            }
            i = j;
        }
    }
    let text = std::str::from_utf8(&s[start..i]).ok()?;
    text.parse::<f64>().ok().map(|v| (v, i))
}

/// `lua_strx2number`: hexadecimal integers and floats with an optional binary exponent.
fn strtod_hex(s: &[u8]) -> Option<(f64, usize)> {
    fn hexval(c: u8) -> f64 {
        f64::from(char::from(c).to_digit(16).unwrap_or(0))
    }
    let at = |i: usize| s.get(i).copied().unwrap_or(0);
    let mut i = 0;
    while is_space(at(i)) {
        i += 1;
    }
    let neg = at(i) == b'-';
    if matches!(at(i), b'-' | b'+') {
        i += 1;
    }
    if !(at(i) == b'0' && matches!(at(i + 1), b'x' | b'X')) {
        return None;
    }
    i += 2;
    let mut r = 0.0f64;
    let mut int_digits = 0;
    while at(i).is_ascii_hexdigit() {
        r = r * 16.0 + hexval(at(i));
        int_digits += 1;
        i += 1;
    }
    let mut frac_digits = 0i32;
    if at(i) == b'.' {
        i += 1;
        while at(i).is_ascii_hexdigit() {
            r = r * 16.0 + hexval(at(i));
            frac_digits += 1;
            i += 1;
        }
    }
    if int_digits == 0 && frac_digits == 0 {
        return None;
    }
    let mut e = frac_digits.wrapping_mul(-4);
    let mut end = i;
    if matches!(at(i), b'p' | b'P') {
        i += 1;
        let neg_exp = at(i) == b'-';
        if matches!(at(i), b'-' | b'+') {
            i += 1;
        }
        if at(i).is_ascii_digit() {
            let mut exp = 0i32;
            while at(i).is_ascii_digit() {
                exp = exp.wrapping_mul(10).wrapping_add(i32::from(at(i) - b'0'));
                i += 1;
            }
            if neg_exp {
                exp = exp.wrapping_neg();
            }
            e = e.wrapping_add(exp);
            end = i;
        }
    }
    if neg {
        r = -r;
    }
    Some((ldexp(r, e), end))
}

/// `luaO_str2d`: the number a Lua string converts to (`tonumber(s)`, arithmetic coercion).
///
/// Accepts surrounding whitespace, decimal numbers and hexadecimal integers or floats; rejects
/// `inf`, `nan`, trailing garbage, embedded NULs and the empty string.
#[must_use]
pub fn str_to_number(s: &[u8]) -> Option<f64> {
    if s.iter().any(|&c| matches!(c, 0 | b'n' | b'N')) {
        return None;
    }
    let (value, mut end) = if s.iter().any(|&c| matches!(c, b'x' | b'X')) {
        strtod_hex(s)?
    } else {
        strtod_dec(s)?
    };
    while s.get(end).is_some_and(|&c| is_space(c)) {
        end += 1;
    }
    (end == s.len()).then_some(value)
}

// ---------------------------------------------------------------------------------------------
// Pattern matching
// ---------------------------------------------------------------------------------------------

/// Maximum number of captures of a pattern.
const MAX_CAPTURES: usize = 32;
/// Maximum recursion depth of the matcher (`MAXCCALLS`).
const MAX_CCALLS: i32 = 200;
const CAP_UNFINISHED: isize = -1;
const CAP_POSITION: isize = -2;

/// One capture of a successful match.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Capture<'a> {
    /// A substring capture `(...)`.
    Str(&'a [u8]),
    /// A position capture `()`, 1-based like Lua.
    Position(usize),
    /// A capture left open by a malformed pattern. [`pat_find`] and [`gmatch_step`] raise
    /// `unfinished capture` instead; [`gsub`] hands it to the replacement because PUC raises
    /// only when the replacement actually reads it.
    Unfinished,
}

/// A match: `s[start..end]` plus the explicit captures (empty when the pattern has none).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found<'a> {
    /// 0-based start of the match.
    pub start: usize,
    /// 0-based end of the match, exclusive.
    pub end: usize,
    /// The explicit captures only.
    pub captures: Vec<Capture<'a>>,
}

/// The text of the whole match.
#[must_use]
pub fn whole<'a>(s: &'a [u8], f: &Found<'_>) -> &'a [u8] {
    &s[f.start..f.end]
}

/// Result of one step of the matcher.
enum Flow {
    Done(Option<usize>),
    Goto(usize, usize),
    Default,
}

struct Matcher<'s, 'p> {
    src: &'s [u8],
    pat: &'p [u8],
    depth: i32,
    level: usize,
    cap: [(usize, isize); MAX_CAPTURES],
}

/// `match_class` of `lstrlib.c`.
fn match_class(c: u8, cl: u8) -> bool {
    let res = match cl.to_ascii_lowercase() {
        b'a' => c.is_ascii_alphabetic(),
        b'c' => c.is_ascii_control(),
        b'd' => c.is_ascii_digit(),
        b'g' => c.is_ascii_graphic(),
        b'l' => c.is_ascii_lowercase(),
        b'p' => c.is_ascii_punctuation(),
        b's' => is_space(c),
        b'u' => c.is_ascii_uppercase(),
        b'w' => c.is_ascii_alphanumeric(),
        b'x' => c.is_ascii_hexdigit(),
        b'z' => c == 0,
        _ => return cl == c,
    };
    if cl.is_ascii_lowercase() { res } else { !res }
}

impl<'s, 'p> Matcher<'s, 'p> {
    fn new(src: &'s [u8], pat: &'p [u8]) -> Self {
        Self {
            src,
            pat,
            depth: MAX_CCALLS,
            level: 0,
            cap: [(0, 0); MAX_CAPTURES],
        }
    }

    /// Pattern byte, with the terminating NUL of a C string past the end.
    fn p(&self, i: usize) -> u8 {
        self.pat.get(i).copied().unwrap_or(0)
    }

    /// Subject byte, with the terminating NUL of a C string past the end.
    fn s(&self, i: usize) -> u8 {
        self.src.get(i).copied().unwrap_or(0)
    }

    fn check_capture(&self, l: u8) -> Result<usize, String> {
        let l = i32::from(l) - i32::from(b'1');
        match usize::try_from(l) {
            Ok(idx) if idx < self.level && self.cap[idx].1 != CAP_UNFINISHED => Ok(idx),
            _ => Err(format!("invalid capture index %{}", l + 1)),
        }
    }

    fn capture_to_close(&self) -> Result<usize, String> {
        (0..self.level)
            .rev()
            .find(|&l| self.cap[l].1 == CAP_UNFINISHED)
            .ok_or_else(|| "invalid pattern capture".to_owned())
    }

    fn classend(&self, mut p: usize) -> Result<usize, String> {
        let c = self.p(p);
        p += 1;
        match c {
            b'%' => {
                if p == self.pat.len() {
                    return Err("malformed pattern (ends with '%')".to_owned());
                }
                Ok(p + 1)
            }
            b'[' => {
                if self.p(p) == b'^' {
                    p += 1;
                }
                loop {
                    if p == self.pat.len() {
                        return Err("malformed pattern (missing ']')".to_owned());
                    }
                    let ch = self.p(p);
                    p += 1;
                    if ch == b'%' && p < self.pat.len() {
                        p += 1;
                    }
                    if self.p(p) == b']' {
                        break;
                    }
                }
                Ok(p + 1)
            }
            _ => Ok(p),
        }
    }

    /// `matchbracketclass`: `p` is the `[`, `ec` the closing `]`.
    fn match_bracket(&self, c: u8, mut p: usize, ec: usize) -> bool {
        let mut sig = true;
        if self.p(p + 1) == b'^' {
            sig = false;
            p += 1;
        }
        loop {
            p += 1;
            if p >= ec {
                return !sig;
            }
            if self.p(p) == b'%' {
                p += 1;
                if match_class(c, self.p(p)) {
                    return sig;
                }
            } else if self.p(p + 1) == b'-' && p + 2 < ec {
                p += 2;
                if self.p(p - 2) <= c && c <= self.p(p) {
                    return sig;
                }
            } else if self.p(p) == c {
                return sig;
            }
        }
    }

    fn singlematch(&self, s: usize, p: usize, ep: usize) -> bool {
        if s >= self.src.len() {
            return false;
        }
        let c = self.src[s];
        match self.p(p) {
            b'.' => true,
            b'%' => match_class(c, self.p(p + 1)),
            b'[' => self.match_bracket(c, p, ep - 1),
            pc => pc == c,
        }
    }

    fn matchbalance(&self, mut s: usize, p: usize) -> Result<Option<usize>, String> {
        if p + 1 >= self.pat.len() {
            return Err("malformed pattern (missing arguments to '%b')".to_owned());
        }
        if self.s(s) != self.p(p) {
            return Ok(None);
        }
        let (b, e) = (self.p(p), self.p(p + 1));
        let mut cont = 1;
        loop {
            s += 1;
            if s >= self.src.len() {
                return Ok(None);
            }
            let c = self.src[s];
            if c == e {
                cont -= 1;
                if cont == 0 {
                    return Ok(Some(s + 1));
                }
            } else if c == b {
                cont += 1;
            }
        }
    }

    fn max_expand(&mut self, s: usize, p: usize, ep: usize) -> Result<Option<usize>, String> {
        let mut i = 0;
        while self.singlematch(s + i, p, ep) {
            i += 1;
        }
        loop {
            if let Some(r) = self.do_match(s + i, ep + 1)? {
                return Ok(Some(r));
            }
            if i == 0 {
                return Ok(None);
            }
            i -= 1;
        }
    }

    fn min_expand(&mut self, mut s: usize, p: usize, ep: usize) -> Result<Option<usize>, String> {
        loop {
            if let Some(r) = self.do_match(s, ep + 1)? {
                return Ok(Some(r));
            }
            if self.singlematch(s, p, ep) {
                s += 1;
            } else {
                return Ok(None);
            }
        }
    }

    fn start_capture(&mut self, s: usize, p: usize, what: isize) -> Result<Option<usize>, String> {
        let level = self.level;
        if level >= MAX_CAPTURES {
            return Err("too many captures".to_owned());
        }
        self.cap[level] = (s, what);
        self.level = level + 1;
        let res = self.do_match(s, p)?;
        if res.is_none() {
            self.level -= 1;
        }
        Ok(res)
    }

    fn end_capture(&mut self, s: usize, p: usize) -> Result<Option<usize>, String> {
        let l = self.capture_to_close()?;
        self.cap[l].1 = (s - self.cap[l].0) as isize;
        let res = self.do_match(s, p)?;
        if res.is_none() {
            self.cap[l].1 = CAP_UNFINISHED;
        }
        Ok(res)
    }

    fn match_capture(&self, s: usize, l: u8) -> Result<Option<usize>, String> {
        let l = self.check_capture(l)?;
        let (init, len) = self.cap[l];
        let Ok(len) = usize::try_from(len) else {
            return Ok(None); // a position capture never matches text
        };
        let rest = self.src.len().saturating_sub(s);
        if rest >= len && self.src[init..init + len] == self.src[s..s + len] {
            Ok(Some(s + len))
        } else {
            Ok(None)
        }
    }

    /// `match` of `lstrlib.c`: tail calls are the `continue`s of the loop.
    fn do_match(&mut self, mut s: usize, mut p: usize) -> Result<Option<usize>, String> {
        if self.depth == 0 {
            return Err("pattern too complex".to_owned());
        }
        self.depth -= 1;
        let result = loop {
            if p == self.pat.len() {
                break Some(s);
            }
            let flow = match self.p(p) {
                b'(' => {
                    let r = if self.p(p + 1) == b')' {
                        self.start_capture(s, p + 2, CAP_POSITION)?
                    } else {
                        self.start_capture(s, p + 1, CAP_UNFINISHED)?
                    };
                    Flow::Done(r)
                }
                b')' => Flow::Done(self.end_capture(s, p + 1)?),
                b'$' if p + 1 == self.pat.len() => {
                    Flow::Done((s == self.src.len()).then_some(s))
                }
                b'%' => match self.p(p + 1) {
                    b'b' => match self.matchbalance(s, p + 2)? {
                        Some(ns) => Flow::Goto(ns, p + 4),
                        None => Flow::Done(None),
                    },
                    b'f' => {
                        let p = p + 2;
                        if self.p(p) != b'[' {
                            return Err("missing '[' after '%f' in pattern".to_owned());
                        }
                        let ep = self.classend(p)?;
                        let previous = if s == 0 { 0 } else { self.src[s - 1] };
                        if !self.match_bracket(previous, p, ep - 1)
                            && self.match_bracket(self.s(s), p, ep - 1)
                        {
                            Flow::Goto(s, ep)
                        } else {
                            Flow::Done(None)
                        }
                    }
                    d @ b'0'..=b'9' => match self.match_capture(s, d)? {
                        Some(ns) => Flow::Goto(ns, p + 2),
                        None => Flow::Done(None),
                    },
                    _ => Flow::Default,
                },
                _ => Flow::Default,
            };
            match flow {
                Flow::Done(r) => break r,
                Flow::Goto(ns, np) => {
                    s = ns;
                    p = np;
                }
                Flow::Default => {
                    let ep = self.classend(p)?;
                    let suffix = self.p(ep);
                    if !self.singlematch(s, p, ep) {
                        if matches!(suffix, b'*' | b'?' | b'-') {
                            p = ep + 1;
                        } else {
                            break None;
                        }
                    } else {
                        match suffix {
                            b'?' => {
                                if let Some(r) = self.do_match(s + 1, ep + 1)? {
                                    break Some(r);
                                }
                                p = ep + 1;
                            }
                            b'+' => break self.max_expand(s + 1, p, ep)?,
                            b'*' => break self.max_expand(s, p, ep)?,
                            b'-' => break self.min_expand(s, p, ep)?,
                            _ => {
                                s += 1;
                                p = ep;
                            }
                        }
                    }
                }
            }
        };
        self.depth += 1;
        Ok(result)
    }

    /// The explicit captures; `strict` raises `unfinished capture` like `push_onecapture`.
    fn captures(&self, strict: bool) -> Result<Vec<Capture<'s>>, String> {
        self.cap[..self.level]
            .iter()
            .map(|&(init, len)| match len {
                CAP_UNFINISHED if strict => Err("unfinished capture".to_owned()),
                CAP_UNFINISHED => Ok(Capture::Unfinished),
                CAP_POSITION => Ok(Capture::Position(init + 1)),
                l => Ok(Capture::Str(&self.src[init..init + l as usize])),
            })
            .collect()
    }
}

/// Splits the `^` anchor off a pattern like `str_find_aux` and `str_gsub` do.
fn split_anchor(pat: &[u8]) -> (bool, &[u8]) {
    match pat.split_first() {
        Some((b'^', rest)) => (true, rest),
        _ => (false, pat),
    }
}

/// `string.find` / `string.match` core: the first match of `pat` in `s` starting the scan at
/// the 0-based `init` (a `^` anchors it there). `Ok(None)` also when `init > s.len()`.
///
/// # Errors
///
/// Malformed pattern, `pattern too complex`, `too many captures`, `unfinished capture`, ...
pub fn pat_find<'a>(s: &'a [u8], pat: &[u8], init: usize) -> Result<Option<Found<'a>>, String> {
    if init > s.len() {
        return Ok(None);
    }
    let (anchor, pat) = split_anchor(pat);
    let mut m = Matcher::new(s, pat);
    let mut s1 = init;
    loop {
        m.level = 0;
        if let Some(end) = m.do_match(s1, 0)? {
            return Ok(Some(Found {
                start: s1,
                end,
                captures: m.captures(true)?,
            }));
        }
        let more = s1 < s.len();
        s1 += 1;
        if !more || anchor {
            return Ok(None);
        }
    }
}

/// One step of `string.gmatch` (`gmatch_aux`): the next match at or after the 0-based `start`
/// and the `start` of the following step. `^` is an ordinary character here, as in Lua 5.2.
///
/// # Errors
///
/// Same as [`pat_find`].
pub fn gmatch_step<'a>(
    s: &'a [u8],
    pat: &[u8],
    start: usize,
) -> Result<Option<(Found<'a>, usize)>, String> {
    let mut m = Matcher::new(s, pat);
    for src in start..=s.len() {
        m.level = 0;
        if let Some(end) = m.do_match(src, 0)? {
            let next = if end == src { end + 1 } else { end };
            let found = Found {
                start: src,
                end,
                captures: m.captures(true)?,
            };
            return Ok(Some((found, next)));
        }
    }
    Ok(None)
}

/// `lmemfind`: the first occurrence of `pat` in `s[init..]` as `(start, end)`.
#[must_use]
pub fn find_plain(s: &[u8], pat: &[u8], init: usize) -> Option<(usize, usize)> {
    let hay = s.get(init..)?;
    if pat.is_empty() {
        return Some((init, init));
    }
    hay.windows(pat.len())
        .position(|w| w == pat)
        .map(|at| (init + at, init + at + pat.len()))
}

/// Whether `pat` has pattern special characters (`nospecials` negated).
#[must_use]
pub fn has_specials(pat: &[u8]) -> bool {
    pat.iter().any(|c| b"^$*+?.([%-".contains(c))
}

/// What a `gsub` replacement decided for one match.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReplOut {
    /// `nil`/`false` from a function or table: keep the original text.
    Keep,
    /// The replacement text.
    Bytes(Vec<u8>),
}

/// The replacement callback of [`gsub`]: the whole match and its captures.
pub type ReplFn<'a> = dyn FnMut(&[u8], &[Capture<'_>]) -> Result<ReplOut, String> + 'a;

/// `string.gsub` core. For each match (at most `max_n`, default `s.len() + 1`) `repl` receives
/// the whole match and the captures (the whole match as the only capture when the pattern has
/// none, like `get_onecapture(0)`) and returns the replacement.
///
/// Returns the new string and the number of matches.
///
/// # Errors
///
/// Pattern errors, or the first error returned by `repl`.
pub fn gsub(
    s: &[u8],
    pat: &[u8],
    max_n: Option<usize>,
    repl: &mut ReplFn<'_>,
) -> Result<(Vec<u8>, usize), String> {
    let max_s = max_n.unwrap_or(s.len() + 1);
    let (anchor, pat) = split_anchor(pat);
    let mut m = Matcher::new(s, pat);
    let mut out = Vec::with_capacity(s.len());
    let (mut n, mut src) = (0, 0);
    while n < max_s {
        m.level = 0;
        let end = m.do_match(src, 0)?;
        if let Some(e) = end {
            n += 1;
            let text = &s[src..e];
            let mut caps = m.captures(false)?;
            if caps.is_empty() {
                caps.push(Capture::Str(text));
            }
            match repl(text, &caps)? {
                ReplOut::Keep => out.extend_from_slice(text),
                ReplOut::Bytes(b) => out.extend_from_slice(&b),
            }
        }
        match end {
            Some(e) if e > src => src = e,
            _ if src < s.len() => {
                out.push(s[src]);
                src += 1;
            }
            _ => break,
        }
        if anchor {
            break;
        }
    }
    out.extend_from_slice(&s[src..]);
    Ok((out, n))
}

/// `add_s`: expands the `%0`-`%9` and `%%` escapes of a `gsub` replacement string.
///
/// `caps` are the captures as [`gsub`] hands them out (`%1` is the whole match when it is empty).
///
/// # Errors
///
/// `invalid use of '%' in replacement string`, `invalid capture index`, `unfinished capture`.
pub fn expand_repl(repl: &[u8], whole: &[u8], caps: &[Capture<'_>]) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(repl.len());
    let mut i = 0;
    while i < repl.len() {
        let c = repl[i];
        i += 1;
        if c != b'%' {
            out.push(c);
            continue;
        }
        let d = repl.get(i).copied().unwrap_or(0);
        i += 1;
        match d {
            b'0' => out.extend_from_slice(whole),
            b'1'..=b'9' => {
                let idx = usize::from(d - b'1');
                match caps.get(idx) {
                    Some(Capture::Str(b)) => out.extend_from_slice(b),
                    Some(Capture::Position(p)) => out.extend(number_to_string(*p as f64)),
                    Some(Capture::Unfinished) => return Err("unfinished capture".to_owned()),
                    None if idx == 0 && caps.is_empty() => out.extend_from_slice(whole),
                    None => return Err("invalid capture index".to_owned()),
                }
            }
            b'%' => out.push(b'%'),
            _ => return Err("invalid use of '%' in replacement string".to_owned()),
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------------------------
// string.format
// ---------------------------------------------------------------------------------------------

/// What `string.format` wants from the host for one argument.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Want {
    /// Only whether the argument exists: the host answers `Err("bad argument #N to 'format'
    /// (no value)")` when it does not, `Ok` of anything otherwise. Asked first for every item,
    /// like `luaL_argerror(L, arg, "no value")` precedes the option scan in PUC.
    Present,
    /// `luaL_checknumber`.
    Number,
    /// `luaL_checklstring` (numbers convert to their string).
    Str,
    /// `luaL_tolstring` (any value, honouring `__tostring`).
    Any,
}

/// An argument handed to `string.format`.
#[derive(Clone, Debug, PartialEq)]
pub enum FmtArg {
    /// A number.
    Num(f64),
    /// A string.
    Str(Vec<u8>),
}

type ArgFn<'a> = dyn FnMut(usize, Want) -> Result<FmtArg, String> + 'a;

fn arg_number(arg: &mut ArgFn<'_>, idx: usize) -> Result<f64, String> {
    match arg(idx, Want::Number)? {
        FmtArg::Num(n) => Ok(n),
        FmtArg::Str(s) => str_to_number(&s).ok_or_else(|| {
            format!(
                "bad argument #{} to 'format' (number expected, got string)",
                idx + 2
            )
        }),
    }
}

fn arg_bytes(arg: &mut ArgFn<'_>, idx: usize, want: Want) -> Result<Vec<u8>, String> {
    match arg(idx, want)? {
        FmtArg::Str(s) => Ok(s),
        FmtArg::Num(n) => Ok(number_to_string(n)),
    }
}

/// `scanformat`: parses the flags, width and precision at `fmt[start..]`; returns the spec and
/// the index of the conversion character.
fn scan_format(fmt: &[u8], start: usize) -> Result<(Spec, usize), String> {
    let at = |i: usize| fmt.get(i).copied().unwrap_or(0);
    let digit = |i: usize| at(i).is_ascii_digit();
    let mut spec = Spec::default();
    let mut p = start;
    while at(p) != 0 && b"-+ #0".contains(&at(p)) {
        match at(p) {
            b'-' => spec.minus = true,
            b'+' => spec.plus = true,
            b' ' => spec.space = true,
            b'#' => spec.hash = true,
            _ => spec.zero = true,
        }
        p += 1;
    }
    if p - start >= 6 {
        return Err("invalid format (repeated flags)".to_owned());
    }
    for _ in 0..2 {
        if digit(p) {
            spec.width = spec.width * 10 + usize::from(at(p) - b'0');
            p += 1;
        }
    }
    if at(p) == b'.' {
        p += 1;
        let mut prec = 0;
        for _ in 0..2 {
            if digit(p) {
                prec = prec * 10 + usize::from(at(p) - b'0');
                p += 1;
            }
        }
        spec.prec = Some(prec);
    }
    if digit(p) {
        return Err("invalid format (width or precision too long)".to_owned());
    }
    Ok((spec, p))
}

/// `addquoted`: the `%q` conversion.
fn add_quoted(out: &mut Vec<u8>, s: &[u8]) {
    out.push(b'"');
    for (i, &c) in s.iter().enumerate() {
        match c {
            b'"' | b'\\' | b'\n' => {
                out.push(b'\\');
                out.push(c);
            }
            0 | 1..=31 | 127 => {
                let next_digit = s.get(i + 1).is_some_and(u8::is_ascii_digit);
                let text = if next_digit {
                    format!("\\{c:03}")
                } else {
                    format!("\\{c}")
                };
                out.extend_from_slice(text.as_bytes());
            }
            _ => out.push(c),
        }
    }
    out.push(b'"');
}

/// `string.format(fmt, ...)`, with the arguments fetched lazily through `arg(index, want)`
/// (`index` is 0-based among the values after `fmt`).
///
/// Conversions are those of Lua 5.2.4: `c d i o u x X e E f g G a A q s %` (not `F`).
/// Integer conversions follow `lua_Number` to integer semantics (`not a number in proper
/// range`), `%s` keeps strings of 100 bytes or more untouched unless a precision is given and
/// stops at an embedded NUL otherwise, as the C `sprintf` does.
///
/// # Errors
///
/// `invalid option '%y' to 'format'`, `invalid format (...)`, the errors of `arg`, and the
/// complete `bad argument #N to 'format' (...)` messages for out of range numbers.
pub fn format(fmt: &[u8], arg: &mut ArgFn<'_>) -> Result<Vec<u8>, String> {
    let at = |i: usize| fmt.get(i).copied().unwrap_or(0);
    let mut out = Vec::with_capacity(fmt.len());
    let mut i = 0;
    let mut idx = 0;
    while i < fmt.len() {
        if fmt[i] != b'%' {
            out.push(fmt[i]);
            i += 1;
            continue;
        }
        i += 1;
        if at(i) == b'%' {
            out.push(b'%');
            i += 1;
            continue;
        }
        arg(idx, Want::Present)?;
        let (spec, conv_at) = scan_format(fmt, i)?;
        let conv = at(conv_at);
        i = conv_at + 1;
        let argn = idx + 2;
        match conv {
            b'c' => {
                let n = arg_number(arg, idx)?;
                let c = (cvt_i64(n) as i32) as u8;
                out.extend(pad_bytes(&[c], &spec));
            }
            b'd' | b'i' => {
                let n = arg_number(arg, idx)?;
                let ni = cvt_i64(n);
                let diff = n - ni as f64;
                if !(-1.0 < diff && diff < 1.0) {
                    return Err(format!(
                        "bad argument #{argn} to 'format' (not a number in proper range)"
                    ));
                }
                out.extend(fmt_int(ni < 0, ni.unsigned_abs(), conv, &spec));
            }
            b'o' | b'u' | b'x' | b'X' => {
                let n = arg_number(arg, idx)?;
                let ni = cvt_u64(n);
                let diff = n - ni as f64;
                if !(-1.0 < diff && diff < 1.0) {
                    return Err(format!(
                        "bad argument #{argn} to 'format' (not a non-negative number in proper range)"
                    ));
                }
                out.extend(fmt_int(false, ni, conv, &spec));
            }
            b'e' | b'E' | b'f' | b'g' | b'G' | b'a' | b'A' => {
                let n = arg_number(arg, idx)?;
                out.extend(fmt_float(conv, n, &spec));
            }
            b'q' => add_quoted(&mut out, &arg_bytes(arg, idx, Want::Str)?),
            b's' => {
                let s = arg_bytes(arg, idx, Want::Any)?;
                if spec.prec.is_none() && s.len() >= 100 {
                    out.extend_from_slice(&s);
                } else {
                    let cut = s.iter().position(|&c| c == 0).unwrap_or(s.len());
                    let cut = spec.prec.map_or(cut, |p| cut.min(p));
                    out.extend(pad_bytes(&s[..cut], &spec));
                }
            }
            _ => {
                return Err(format!(
                    "invalid option '%{}' to 'format'",
                    char::from(conv)
                ));
            }
        }
        idx += 1;
    }
    Ok(out)
}

// ---------------------------------------------------------------------------------------------
// Simple functions with the 5.2 position rules
// ---------------------------------------------------------------------------------------------

/// `luaL_checkinteger` on a Lua number (`lua_Integer` is a 64-bit `ptrdiff_t`).
fn to_integer(n: f64) -> i64 {
    cvt_i64(n)
}

/// `luaL_checkint`: the low 32 bits of [`to_integer`].
fn to_int(n: f64) -> i32 {
    to_integer(n) as i32
}

/// `posrelat`: translates a relative string position (negative counts from the end).
fn posrelat(pos: i64, len: usize) -> usize {
    if pos >= 0 {
        pos as usize
    } else if pos.unsigned_abs() > len as u64 {
        0
    } else {
        len - pos.unsigned_abs() as usize + 1
    }
}

/// `string.sub(s, i, j)` (pass `j = -1` when absent).
#[must_use]
pub fn sub(s: &[u8], i: f64, j: f64) -> &[u8] {
    let start = posrelat(to_integer(i), s.len()).max(1);
    let end = posrelat(to_integer(j), s.len()).min(s.len());
    if start <= end { &s[start - 1..end] } else { &[] }
}

/// `string.rep(s, n [, sep])`.
///
/// # Errors
///
/// `resulting string too large` (and `not enough memory` when the allocation fails).
pub fn rep(s: &[u8], n: f64, sep: Option<&[u8]>) -> Result<Vec<u8>, String> {
    const MAXSIZE: usize = usize::MAX >> 1;
    let sep = sep.unwrap_or(&[]);
    let n = to_int(n);
    if n <= 0 {
        return Ok(Vec::new());
    }
    let n = n as usize;
    if s.len().checked_add(sep.len()).is_none_or(|unit| unit >= MAXSIZE / n) {
        return Err("resulting string too large".to_owned());
    }
    let total = n * s.len() + (n - 1) * sep.len();
    if total == 0 {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    out.try_reserve_exact(total)
        .map_err(|_| "not enough memory".to_owned())?;
    for _ in 1..n {
        out.extend_from_slice(s);
        out.extend_from_slice(sep);
    }
    out.extend_from_slice(s);
    Ok(out)
}

/// `string.byte(s, i, j)`: pass `i = 1` and `j = i` when absent.
///
/// # Errors
///
/// `string slice too long` (also stands for the `luaL_checkstack` overflow).
pub fn byte(s: &[u8], i: f64, j: f64) -> Result<Vec<f64>, String> {
    let posi = posrelat(to_integer(i), s.len()).max(1);
    let pose = posrelat(to_integer(j), s.len()).min(s.len());
    if posi > pose {
        return Ok(Vec::new());
    }
    let n = (pose - posi + 1) as u32 as i32;
    if (posi as i64) + i64::from(n) <= pose as i64 {
        return Err("string slice too long".to_owned());
    }
    // LUAI_MAXSTACK is 1_000_000 slots; the exact margin depends on the caller's frame
    if n > 999_950 {
        return Err("stack overflow (string slice too long)".to_owned());
    }
    Ok(s[posi - 1..pose].iter().map(|&c| f64::from(c)).collect())
}

/// `string.lower`.
#[must_use]
pub fn lower(s: &[u8]) -> Vec<u8> {
    s.to_ascii_lowercase()
}

/// `string.upper`.
#[must_use]
pub fn upper(s: &[u8]) -> Vec<u8> {
    s.to_ascii_uppercase()
}

/// `string.reverse`.
#[must_use]
pub fn reverse(s: &[u8]) -> Vec<u8> {
    s.iter().rev().copied().collect()
}

/// `string.char(...)`.
///
/// # Errors
///
/// The complete `bad argument #N to 'char' (value out of range)` message.
pub fn char_from(codes: &[f64]) -> Result<Vec<u8>, String> {
    codes
        .iter()
        .enumerate()
        .map(|(i, &c)| {
            u8::try_from(to_int(c)).map_err(|_| {
                format!("bad argument #{} to 'char' (value out of range)", i + 1)
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ns(n: f64) -> String {
        String::from_utf8(number_to_string(n)).unwrap()
    }

    fn fmt(f: &str, args: &[FmtArg]) -> Result<String, String> {
        let r = format(f.as_bytes(), &mut |i, _| {
            args.get(i)
                .cloned()
                .ok_or_else(|| format!("bad argument #{} to 'format' (no value)", i + 2))
        })?;
        Ok(String::from_utf8_lossy(&r).into_owned())
    }

    fn num(n: f64) -> FmtArg {
        FmtArg::Num(n)
    }

    #[test]
    fn tostring_matches_printf_14g() {
        assert_eq!(ns(0.0), "0");
        assert_eq!(ns(-0.0), "-0");
        assert_eq!(ns(1.0), "1");
        assert_eq!(ns(0.1), "0.1");
        assert_eq!(ns(1e15), "1e+15");
        assert_eq!(ns(123_456_789_012_345.0), "1.2345678901234e+14");
        assert_eq!(ns(12_345_678_901_234.0), "12345678901234");
        assert_eq!(ns(1e-5), "1e-05");
        assert_eq!(ns(0.0001), "0.0001");
        assert_eq!(ns(1e100), "1e+100");
        assert_eq!(ns(f64::INFINITY), "inf");
        assert_eq!(ns(f64::NEG_INFINITY), "-inf");
        assert_eq!(ns(f64::NAN.copysign(1.0)), "nan");
        assert_eq!(ns(f64::NAN.copysign(-1.0)), "-nan");
        assert_eq!(ns(1.234_567_890_123_456_7), "1.2345678901235");
        assert_eq!(ns(2f64.powi(53)), "9.007199254741e+15");
    }

    #[test]
    fn tonumber_rules() {
        assert_eq!(str_to_number(b"  10  "), Some(10.0));
        assert_eq!(str_to_number(b"0x10"), Some(16.0));
        assert_eq!(str_to_number(b"-0x10"), Some(-16.0));
        assert_eq!(str_to_number(b"0x1p4"), Some(16.0));
        assert_eq!(str_to_number(b"0x.8"), Some(0.5));
        assert_eq!(str_to_number(b"1e2"), Some(100.0));
        assert_eq!(str_to_number(b".5"), Some(0.5));
        assert_eq!(str_to_number(b"5."), Some(5.0));
        for bad in [&b""[..], b" ", b"1e", b"inf", b"nan", b"0x", b"1 2", b"1\0", b"- 1", b"1f"] {
            assert_eq!(str_to_number(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn find_and_match() {
        let f = pat_find(b"hello world", b"o w", 0).unwrap().unwrap();
        assert_eq!((f.start, f.end), (4, 7));
        let f = pat_find(b"key=val", b"(%w+)=(%w+)", 0).unwrap().unwrap();
        assert_eq!(f.captures, [Capture::Str(b"key"), Capture::Str(b"val")]);
        let f = pat_find(b"abc", b"()b()", 0).unwrap().unwrap();
        assert_eq!(f.captures, [Capture::Position(2), Capture::Position(3)]);
        assert!(pat_find(b"abc", b"^b", 0).unwrap().is_none());
        assert!(pat_find(b"abc", b"^b", 1).unwrap().is_some());
        let f = pat_find(b"f(a(b)c)d", b"%b()", 0).unwrap().unwrap();
        assert_eq!(whole(b"f(a(b)c)d", &f), b"(a(b)c)");
        let f = pat_find(b"THE (quick) fox", b"%f[%a]%a+", 4).unwrap().unwrap();
        assert_eq!((f.start, f.end), (5, 10));
        assert!(pat_find(b"abab", b"(ab)%1", 0).unwrap().is_some());
    }

    #[test]
    fn pattern_errors() {
        let err = |p: &[u8]| pat_find(b"abc", p, 0).unwrap_err();
        assert_eq!(err(b"%"), "malformed pattern (ends with '%')");
        assert_eq!(err(b"[a"), "malformed pattern (missing ']')");
        assert_eq!(err(b")"), "invalid pattern capture");
        assert_eq!(err(b"(a)%2"), "invalid capture index %2");
        assert_eq!(err(b"%f"), "missing '[' after '%f' in pattern");
        assert_eq!(err(b"(a"), "unfinished capture");
        assert_eq!(err(b"%b"), "malformed pattern (missing arguments to '%b')");
        let many = "(a)".repeat(33);
        assert_eq!(
            pat_find(&b"a".repeat(40), many.as_bytes(), 0).unwrap_err(),
            "too many captures"
        );
        let deep = "a?".repeat(250);
        assert_eq!(
            pat_find(&b"a".repeat(250), deep.as_bytes(), 0).unwrap_err(),
            "pattern too complex"
        );
    }

    #[test]
    fn gmatch_and_gsub() {
        let mut next = 0;
        let mut words = Vec::new();
        while let Some((f, n)) = gmatch_step(b"one two  three", b"%a+", next).unwrap() {
            words.push(whole(b"one two  three", &f).to_vec());
            next = n;
        }
        assert_eq!(words, [b"one".to_vec(), b"two".to_vec(), b"three".to_vec()]);

        let (out, n) = gsub(b"abc", b"", None, &mut |_, _| Ok(ReplOut::Bytes(b"-".to_vec()))).unwrap();
        assert_eq!((out.as_slice(), n), (&b"-a-b-c-"[..], 4));
        let (out, n) = gsub(b"hello world", b"o", Some(1), &mut |w, c| {
            Ok(ReplOut::Bytes(expand_repl(b"[%0%1]", w, c)?))
        })
        .unwrap();
        assert_eq!((out.as_slice(), n), (&b"hell[oo] world"[..], 1));
        let (out, _) = gsub(b"abc", b"^a", None, &mut |_, _| Ok(ReplOut::Keep)).unwrap();
        assert_eq!(out, b"abc");
        assert_eq!(
            expand_repl(b"%", b"x", &[]).unwrap_err(),
            "invalid use of '%' in replacement string"
        );
        assert_eq!(expand_repl(b"%2", b"x", &[]).unwrap_err(), "invalid capture index");
    }

    #[test]
    fn format_conversions() {
        assert_eq!(fmt("%5.2f|", &[num(3.15159)]).unwrap(), " 3.15|");
        assert_eq!(fmt("%-8s|", &[FmtArg::Str(b"ab".to_vec())]).unwrap(), "ab      |");
        assert_eq!(fmt("%+d", &[num(5.0)]).unwrap(), "+5");
        assert_eq!(fmt("%#x", &[num(255.0)]).unwrap(), "0xff");
        assert_eq!(fmt("%08.3f", &[num(-3.15159)]).unwrap(), "-003.152");
        assert_eq!(fmt("%g %g %g", &[num(100000.0), num(1e6), num(0.0001)]).unwrap(), "100000 1e+06 0.0001");
        assert_eq!(fmt("%e", &[num(12345.678)]).unwrap(), "1.234568e+04");
        assert_eq!(fmt("%.3s", &[FmtArg::Str(b"abcdef".to_vec())]).unwrap(), "abc");
        assert_eq!(fmt("%c%c", &[num(72.0), num(105.0)]).unwrap(), "Hi");
        assert_eq!(fmt("%q", &[FmtArg::Str(b"a\n\"b\\\x011".to_vec())]).unwrap(), "\"a\\\n\\\"b\\\\\\0011\"");
        assert_eq!(fmt("%a", &[num(1.0)]).unwrap(), "0x1p+0");
        assert_eq!(fmt("%a", &[num(0.5)]).unwrap(), "0x1p-1");
        assert_eq!(fmt("%d%%", &[num(7.0)]).unwrap(), "7%");
        assert_eq!(fmt("%5.1f|%-6d|%x", &[num(f64::INFINITY), num(-3.0), num(0.0)]).unwrap(), "  inf|-3    |0");
    }

    #[test]
    fn format_errors() {
        assert_eq!(fmt("%y", &[num(1.0)]).unwrap_err(), "invalid option '%y' to 'format'");
        assert_eq!(fmt("%d", &[]).unwrap_err(), "bad argument #2 to 'format' (no value)");
        assert_eq!(
            fmt("%d", &[num(1.5)]).unwrap(),
            "1"
        );
        assert_eq!(
            fmt("%d", &[num(1e300)]).unwrap_err(),
            "bad argument #2 to 'format' (not a number in proper range)"
        );
        assert_eq!(
            fmt("%x", &[num(-1.0)]).unwrap_err(),
            "bad argument #2 to 'format' (not a non-negative number in proper range)"
        );
        assert_eq!(fmt("%123d", &[num(1.0)]).unwrap_err(), "invalid format (width or precision too long)");
        assert_eq!(fmt("%-----+d", &[num(1.0)]).unwrap_err(), "invalid format (repeated flags)");
    }

    #[test]
    fn position_helpers() {
        assert_eq!(sub(b"hello", 2.0, 4.0), b"ell");
        assert_eq!(sub(b"hello", -3.0, -1.0), b"llo");
        assert_eq!(sub(b"hello", 0.0, 100.0), b"hello");
        assert_eq!(sub(b"hello", 4.0, 2.0), b"");
        assert_eq!(rep(b"ab", 3.0, Some(b",")).unwrap(), b"ab,ab,ab");
        assert_eq!(rep(b"ab", 0.0, None).unwrap(), b"");
        assert_eq!(byte(b"abc", 1.0, 3.0).unwrap(), [97.0, 98.0, 99.0]);
        assert_eq!(byte(b"abc", 5.0, 5.0).unwrap(), Vec::<f64>::new());
        assert_eq!(char_from(&[72.0, 105.0]).unwrap(), b"Hi");
        assert_eq!(
            char_from(&[65.0, 256.0]).unwrap_err(),
            "bad argument #2 to 'char' (value out of range)"
        );
        assert_eq!(upper(b"aZ\xe9"), b"AZ\xe9");
        assert_eq!(lower(b"aZ"), b"az");
        assert_eq!(reverse(b"abc"), b"cba");
    }

    #[test]
    fn plain_find() {
        assert_eq!(find_plain(b"abcabc", b"ca", 0), Some((2, 4)));
        assert_eq!(find_plain(b"abcabc", b"abc", 1), Some((3, 6)));
        assert_eq!(find_plain(b"abc", b"", 3), Some((3, 3)));
        assert_eq!(find_plain(b"abc", b"x", 0), None);
        assert!(has_specials(b"a.b"));
        assert!(!has_specials(b"a]b"));
    }
}
