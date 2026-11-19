//! Differential tests of the native string, sort and bit32 primitives against the real
//! PUC-Rio Lua 5.2.4 shipped by `mlua` (feature `vm`).
//!
//! Each test generates a few thousand deterministic cases (xorshift, no extra crates), runs them
//! through the real library and through `nie_lua::rt`, and compares byte for byte, errors
//! included (message text after the `defs:LINE:` position prefix).
#![cfg(all(
    feature = "vm",
    not(all(target_arch = "wasm32", target_os = "unknown"))
))]

use mlua::{Lua, MultiValue, Table, Value};
use nie_lua::rt::lstr::{self, Capture, FmtArg, ReplOut, Want};
use nie_lua::rt::{bit32, lsort};

// ---------------------------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------------------------

/// A Lua value as the tests see it.
#[derive(Clone, Debug)]
enum V {
    Nil,
    Bool(bool),
    Num(f64),
    Str(Vec<u8>),
}

impl V {
    fn same(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Nil, Self::Nil) => true,
            (Self::Bool(a), Self::Bool(b)) => a == b,
            (Self::Num(a), Self::Num(b)) => a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan()),
            (Self::Str(a), Self::Str(b)) => a == b,
            _ => false,
        }
    }
}

fn same_all(a: &[V], b: &[V]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.same(y))
}

fn s(text: &str) -> V {
    V::Str(text.as_bytes().to_vec())
}

fn show(v: &[V]) -> String {
    let parts: Vec<String> = v
        .iter()
        .map(|x| match x {
            V::Nil => "nil".to_owned(),
            V::Bool(b) => b.to_string(),
            V::Num(n) => format!("{n:?}"),
            V::Str(b) => format!("{:?}", String::from_utf8_lossy(b)),
        })
        .collect();
    format!("[{}]", parts.join(", "))
}

/// Latin-1 view of bytes, which is how the library reports non-ASCII bytes in messages.
fn latin1(b: &[u8]) -> String {
    b.iter().map(|&c| char::from(c)).collect()
}

/// Removes the `defs:LINE: ` position prefix `luaL_error` adds.
fn strip_position(msg: &[u8]) -> String {
    let text = latin1(msg);
    if let Some(rest) = text.strip_prefix("defs:")
        && let Some((line, tail)) = rest.split_once(": ")
        && line.bytes().all(|c| c.is_ascii_digit())
    {
        return tail.to_owned();
    }
    text
}

type Outcome = Result<Vec<V>, String>;

fn outcome_same(a: &Outcome, b: &Outcome) -> bool {
    match (a, b) {
        (Ok(x), Ok(y)) => same_all(x, y),
        (Err(x), Err(y)) => x == y,
        _ => false,
    }
}

fn show_outcome(o: &Outcome) -> String {
    match o {
        Ok(v) => show(v),
        Err(e) => format!("error {e:?}"),
    }
}

struct Oracle {
    lua: Lua,
    failures: Vec<String>,
    cases: usize,
    succeeded: usize,
}

const DEFS: &str = r#"
function P(f, ...) return table.pack(pcall(f, ...)) end
function W_tostring(x) return (tostring(x)) end
function W_tonumber(x) return (tonumber(x)) end
function W_fmt(...) return (string.format(...)) end
function W_find(...) local t = table.pack(string.find(...)) return table.unpack(t, 1, t.n) end
function W_match(...) local t = table.pack(string.match(...)) return table.unpack(t, 1, t.n) end
function W_gmatch(s, p)
  local f = string.gmatch(s, p)
  local res = {}
  while #res < 2000 do
    local t = table.pack(f())
    if t.n == 0 then break end
    res[#res + 1] = t.n
    for i = 1, t.n do res[#res + 1] = t[i] end
  end
  return table.unpack(res)
end
local function rf1(...) return '<' .. table.concat({...}, '|') .. '>' end
local function rf2(a) if #tostring(a) % 2 == 0 then return false end return a .. '!' end
local tbl = {a = '[A]', b = false, ab = 12, [1] = 'one', [2] = 'two'}
function W_gsub(s, p, kind, r, n)
  local repl
  if kind == 0 then repl = r elseif kind == 1 then repl = rf1 elseif kind == 2 then repl = rf2 else repl = tbl end
  local a, b = string.gsub(s, p, repl, n)
  return a, b
end
function W_rep(s, n, sep) return (string.rep(s, n, sep)) end
function W_sub(s, i, j) return (string.sub(s, i, j)) end
function W_byte(s, i, j) local t = table.pack(string.byte(s, i, j)) return table.unpack(t, 1, t.n) end
function W_char(...) return (string.char(...)) end
function W_lower(s) return (string.lower(s)) end
function W_upper(s) return (string.upper(s)) end
function W_reverse(s) return (string.reverse(s)) end
function W_sort(t, kind)
  local log, n = {}, 0
  local function cmp(a, b)
    n = n + 1
    log[#log + 1] = a
    log[#log + 1] = b
    if kind == 0 then return a < b
    elseif kind == 1 then return a <= b
    elseif kind == 2 then return a > b
    elseif kind == 3 then return true
    elseif kind == 4 then return false
    else return math.floor(n * 2654435761 % 4294967296 / 128) % 2 == 1 end
  end
  local ok, err = pcall(table.sort, t, cmp)
  return ok, err, log, t
end
"#;

const BIT_FUNCS: [&str; 12] = [
    "band", "bor", "bxor", "bnot", "btest", "lshift", "rshift", "arshift", "lrotate", "rrotate",
    "extract", "replace",
];

impl Oracle {
    fn new() -> Self {
        let lua = Lua::new();
        let mut defs = DEFS.to_owned();
        for name in BIT_FUNCS {
            defs.push_str(&format!("function W_{name}(...) return (bit32.{name}(...)) end\n"));
        }
        lua.load(&defs).set_name("=defs").exec().expect("load Lua definitions");
        Self { lua, failures: Vec::new(), cases: 0, succeeded: 0 }
    }

    fn to_value(&self, v: &V) -> Value {
        match v {
            V::Nil => Value::Nil,
            V::Bool(b) => Value::Boolean(*b),
            V::Num(n) => Value::Number(*n),
            V::Str(b) => Value::String(self.lua.create_string(b).expect("create string")),
        }
    }

    fn from_value(v: &Value) -> V {
        match v {
            Value::Nil => V::Nil,
            Value::Boolean(b) => V::Bool(*b),
            Value::Number(n) => V::Num(*n),
            Value::Integer(i) => V::Num(*i as f64),
            Value::String(st) => V::Str(st.as_bytes().to_vec()),
            other => panic!("unexpected Lua value {other:?}"),
        }
    }

    /// Calls the global Lua function `name` under `pcall` with `args`.
    fn call(&self, name: &str, args: &[V]) -> Outcome {
        let mut all = vec![Value::Function(self.lua.globals().get(name).expect("global"))];
        all.extend(args.iter().map(|a| self.to_value(a)));
        let p: mlua::Function = self.lua.globals().get("P").expect("P");
        let pack: Table = p.call(all.into_iter().collect::<MultiValue>()).expect("P call");
        let n: usize = pack.get("n").expect("n");
        let vals: Vec<V> = (1..=n)
            .map(|i| Self::from_value(&pack.get::<Value>(i).expect("slot")))
            .collect();
        match vals.first() {
            Some(V::Bool(true)) => Ok(vals[1..].to_vec()),
            Some(V::Bool(false)) => match vals.get(1) {
                Some(V::Str(m)) => Err(strip_position(m)),
                other => panic!("non-string error {other:?}"),
            },
            other => panic!("bad pcall result {other:?}"),
        }
    }

    fn check(&mut self, what: &str, input: &str, lua: &Outcome, mine: &Outcome) {
        self.cases += 1;
        self.succeeded += usize::from(lua.is_ok());
        if !outcome_same(lua, mine) {
            self.failures.push(format!(
                "{what} {input}\n    lua : {}\n    mine: {}",
                show_outcome(lua),
                show_outcome(mine)
            ));
        }
    }

    fn finish(self, what: &str) {
        for f in self.failures.iter().take(12) {
            eprintln!("{f}");
        }
        assert!(
            self.failures.is_empty(),
            "{what}: {} of {} cases differ (first ones above)",
            self.failures.len(),
            self.cases
        );
        eprintln!(
            "{what}: {} cases identical ({} without error)",
            self.cases, self.succeeded
        );
    }
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        ((self.next() >> 11) % n as u64) as usize
    }

    fn chance(&mut self, num: usize, den: usize) -> bool {
        self.below(den) < num
    }

    fn pick<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        &xs[self.below(xs.len())]
    }
}

const EDGE: &[f64] = &[
    0.0, -0.0, 1.0, -1.0, 0.5, -0.5, 1.5, 2.5, -2.5, 0.1, 0.2, 0.3, 255.0, 256.0, 65535.0, 1e14,
    999_999_999_999_999.0, 1e15, 1e16, 123_456_789_012_345.0, 9_007_199_254_740_992.0,
    9_223_372_036_854_775_807.0, 9_223_372_036_854_775_808.0, -9_223_372_036_854_775_808.0,
    18_446_744_073_709_551_615.0, 18_446_744_073_709_551_616.0, 4_294_967_295.0, 4_294_967_296.0,
    4_294_967_297.0, 2_147_483_648.0, -2_147_483_649.0, 1e300, -1e300, 1e-300, 5e-324, 1e-5,
    0.0001, 99_999.5, 123_456.789, 0.000_123_456, 100_000.0, 1_000_000.0, 12_345.678,
    f64::MAX, f64::MIN_POSITIVE, f64::INFINITY, f64::NEG_INFINITY, f64::NAN, -f64::NAN,
    std::f64::consts::PI, std::f64::consts::E, 1e21, 1e22, 0.999_999_999_999_995,
];

fn random_number(rng: &mut Rng) -> f64 {
    match rng.below(10) {
        0..=3 => *rng.pick(EDGE),
        4..=5 => rng.below(200) as f64 - 100.0,
        6 => (rng.next() as i64 >> rng.below(60)) as f64,
        7 => {
            let mantissa = (rng.next() >> 11) as f64;
            mantissa * 10f64.powi(rng.below(40) as i32 - 30)
        }
        8 => f64::from_bits(rng.next()),
        _ => (rng.below(2_000_000) as f64 - 1_000_000.0) / 8.0,
    }
}

fn random_bytes(rng: &mut Rng, alphabet: &[u8], max_len: usize) -> Vec<u8> {
    (0..rng.below(max_len + 1)).map(|_| *rng.pick(alphabet)).collect()
}

fn bits_f(n: f64) -> String {
    format!("{n:e}/{:016x}", n.to_bits())
}

// ---------------------------------------------------------------------------------------------
// tostring / tonumber
// ---------------------------------------------------------------------------------------------

#[test]
fn tostring_matches_lua() {
    let mut o = Oracle::new();
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let mut values: Vec<f64> = EDGE.to_vec();
    values.extend((0..20_000).map(|_| random_number(&mut rng)));
    values.extend((0..2_000).map(|i| f64::from(i) * 1.1));
    values.extend((-30..60).map(|e| 10f64.powi(e)));
    for n in values {
        let lua = o.call("W_tostring", &[V::Num(n)]);
        let mine: Outcome = Ok(vec![V::Str(lstr::number_to_string(n))]);
        o.check("tostring", &bits_f(n), &lua, &mine);
    }
    o.finish("tostring");
}

fn gen_number_text(rng: &mut Rng) -> Vec<u8> {
    const WS: &[u8] = b" \t\n\x0b\x0c\r";
    let mut t = Vec::new();
    let ws = |rng: &mut Rng, t: &mut Vec<u8>| {
        for _ in 0..rng.below(3) {
            t.push(*rng.pick(WS));
        }
    };
    ws(rng, &mut t);
    if rng.chance(1, 4) {
        t.push(*rng.pick(b"+-"));
    }
    match rng.below(6) {
        0 => {
            // hexadecimal, at most 13 digits so glibc and lua_strx2number round alike
            t.extend_from_slice(if rng.chance(1, 2) { b"0x" } else { b"0X" });
            let digits = b"0123456789abcdefABCDEF";
            let int_len = rng.below(7);
            for _ in 0..int_len {
                t.push(*rng.pick(digits));
            }
            if rng.chance(1, 2) {
                t.push(b'.');
                for _ in 0..rng.below(7) {
                    t.push(*rng.pick(digits));
                }
            }
            if rng.chance(1, 2) {
                t.push(*rng.pick(b"pP"));
                if rng.chance(1, 2) {
                    t.push(*rng.pick(b"+-"));
                }
                for _ in 0..rng.below(4) {
                    t.push(*rng.pick(b"0123456789"));
                }
            }
        }
        1 => {
            // arbitrary junk from the number alphabet
            t.extend(random_bytes(rng, b"0123456789.eE+-xXpP aAfFnN\0\t", 12));
        }
        _ => {
            for _ in 0..rng.below(8) {
                t.push(*rng.pick(b"0123456789"));
            }
            if rng.chance(1, 2) {
                t.push(b'.');
                for _ in 0..rng.below(20) {
                    t.push(*rng.pick(b"0123456789"));
                }
            }
            if rng.chance(1, 3) {
                t.push(*rng.pick(b"eE"));
                if rng.chance(1, 2) {
                    t.push(*rng.pick(b"+-"));
                }
                for _ in 0..rng.below(5) {
                    t.push(*rng.pick(b"0123456789"));
                }
            }
        }
    }
    ws(rng, &mut t);
    if rng.chance(1, 8) {
        t.extend(random_bytes(rng, b"xyz1 .e", 2));
    }
    t
}

#[test]
fn tonumber_matches_lua() {
    let mut o = Oracle::new();
    let mut rng = Rng(0xD1B5_4A32_D192_ED03);
    let fixed: &[&[u8]] = &[
        b"", b" ", b"0", b"-0", b"+0", b"0x", b"0x0", b"0xA", b"0Xa.8P1", b"1e309", b"-1e309",
        b"1e-400", b"inf", b"-inf", b"nan", b"infinity", b"1e", b"1e+", b".", b"-.", b".5", b"5.",
        b"0x.p1", b"0x1p", b"0x1p+", b"1 2", b" 12 ", b"1\0", b"\x00", b"0x1.8", b"0e0",
        b"123456789012345678901234567890", b"0.1e1", b"00012", b"1_0", b"0b1",
        b"4.9406564584124654e-324", b"1.7976931348623157e308", b"1.7976931348623159e308",
        b"2.2250738585072011e-308", b"9007199254740993", b"0x1P-1074", b"0x1p-1075",
    ];
    let mut inputs: Vec<Vec<u8>> = fixed.iter().map(|b| b.to_vec()).collect();
    inputs.extend((0..30_000).map(|_| gen_number_text(&mut rng)));
    for text in inputs {
        let lua = o.call("W_tonumber", &[V::Str(text.clone())]);
        let lua = lua.map(|v| match v.first() {
            Some(V::Num(n)) => vec![V::Num(*n)],
            _ => vec![V::Nil],
        });
        let mine: Outcome = Ok(vec![lstr::str_to_number(&text).map_or(V::Nil, V::Num)]);
        o.check("tonumber", &format!("{:?}", latin1(&text)), &lua, &mine);
    }
    o.finish("tonumber");
}

// ---------------------------------------------------------------------------------------------
// Pattern matching
// ---------------------------------------------------------------------------------------------

fn gen_subject(rng: &mut Rng) -> Vec<u8> {
    random_bytes(rng, b"aabbc ()123_-xX\n\0[]%.", 18)
}

fn gen_pattern(rng: &mut Rng) -> Vec<u8> {
    const TOKENS: &[&[u8]] = &[
        b"a", b"b", b"c", b"x", b" ", b"%a", b"%d", b"%s", b"%w", b"%W", b"%p", b"%l", b"%u",
        b"%x", b"%c", b"%g", b"%S", b".", b"[abc]", b"[^a]", b"[a-c]", b"[%d_]", b"[]]", b"[^]]",
        b"[a-]", b"[%a-z]", b"%b()", b"%bxy", b"%f[%w]", b"%f[%W]", b"%f[a]", b"%1", b"%2",
        b"()", b"(", b")", b"%%", b"%.", b"%(", b"%z", b"-", b"[%", b"%b", b"%f", b"[", b"%0",
        b"\0", b"1", b"2", b"_", b"[a-%%]", b"[%]]",
    ];
    const QUANT: &[&[u8]] = &[b"*", b"+", b"-", b"?"];
    let mut p = Vec::new();
    if rng.chance(1, 8) {
        p.push(b'^');
    }
    for _ in 0..1 + rng.below(6) {
        p.extend_from_slice(rng.pick(TOKENS));
        if rng.chance(1, 3) {
            p.extend_from_slice(rng.pick(QUANT));
        }
    }
    if rng.chance(1, 8) {
        p.push(b'$');
    }
    if rng.chance(1, 4) {
        // wrap some of it in captures
        let cut = rng.below(p.len() + 1);
        p.insert(cut, b'(');
        let cut2 = cut + 1 + rng.below(p.len() - cut);
        p.insert(cut2, b')');
    }
    p
}

fn gen_deep_pattern(rng: &mut Rng) -> (Vec<u8>, Vec<u8>) {
    let tok: &[u8] = rng.pick(&[&b"a?"[..], b"a*b?", b"(a)", b"a-", b".?", b"()"]);
    let reps = 20 + rng.below(260);
    let pat = tok.repeat(reps);
    let subject = vec![b'a'; rng.below(300)];
    (subject, pat)
}

fn posrel(pos: i64, len: i64) -> i64 {
    if pos >= 0 {
        pos
    } else if -pos > len {
        0
    } else {
        len + pos + 1
    }
}

fn cap_to_v(c: &Capture<'_>) -> Result<V, String> {
    match c {
        Capture::Str(b) => Ok(V::Str(b.to_vec())),
        Capture::Position(p) => Ok(V::Num(*p as f64)),
        Capture::Unfinished => Err("unfinished capture".to_owned()),
    }
}

fn model_find(s: &[u8], pat: &[u8], init: Option<f64>, plain: bool, find: bool) -> Outcome {
    let ls = s.len() as i64;
    let mut init_i = init.map_or(1, |i| posrel(i as i64, ls));
    if init_i < 1 {
        init_i = 1;
    } else if init_i > ls + 1 {
        return Ok(vec![V::Nil]);
    }
    let from = (init_i - 1) as usize;
    if find && (plain || !lstr::has_specials(pat)) {
        return Ok(match lstr::find_plain(s, pat, from) {
            Some((a, b)) => vec![V::Num((a + 1) as f64), V::Num(b as f64)],
            None => vec![V::Nil],
        });
    }
    match lstr::pat_find(s, pat, from)? {
        None => Ok(vec![V::Nil]),
        Some(f) => {
            let caps: Vec<V> = f.captures.iter().map(|c| cap_to_v(c).expect("strict")).collect();
            if find {
                let mut out = vec![V::Num((f.start + 1) as f64), V::Num(f.end as f64)];
                out.extend(caps);
                Ok(out)
            } else if caps.is_empty() {
                Ok(vec![V::Str(lstr::whole(s, &f).to_vec())])
            } else {
                Ok(caps)
            }
        }
    }
}

fn model_gmatch(s: &[u8], pat: &[u8]) -> Outcome {
    let mut res: Vec<V> = Vec::new();
    let mut next = 0;
    while res.len() < 2000 {
        let Some((f, n)) = lstr::gmatch_step(s, pat, next)? else { break };
        next = n;
        let caps: Vec<V> = if f.captures.is_empty() {
            vec![V::Str(lstr::whole(s, &f).to_vec())]
        } else {
            f.captures.iter().map(|c| cap_to_v(c).expect("strict")).collect()
        };
        res.push(V::Num(caps.len() as f64));
        res.extend(caps);
    }
    Ok(res)
}

fn first_cap_text(caps: &[Capture<'_>]) -> Result<Vec<u8>, String> {
    match caps.first() {
        Some(Capture::Str(b)) => Ok(b.to_vec()),
        Some(Capture::Position(p)) => Ok(lstr::number_to_string(*p as f64)),
        _ => Err("unfinished capture".to_owned()),
    }
}

fn model_gsub(s: &[u8], pat: &[u8], kind: usize, repl: &[u8], n: Option<usize>) -> Outcome {
    let mut f = |whole: &[u8], caps: &[Capture<'_>]| -> Result<ReplOut, String> {
        match kind {
            0 => Ok(ReplOut::Bytes(lstr::expand_repl(repl, whole, caps)?)),
            1 => {
                let mut out = b"<".to_vec();
                for (i, c) in caps.iter().enumerate() {
                    if i > 0 {
                        out.push(b'|');
                    }
                    match c {
                        Capture::Str(b) => out.extend_from_slice(b),
                        Capture::Position(p) => out.extend(lstr::number_to_string(*p as f64)),
                        Capture::Unfinished => return Err("unfinished capture".to_owned()),
                    }
                }
                out.push(b'>');
                Ok(ReplOut::Bytes(out))
            }
            2 => {
                for c in caps {
                    if matches!(c, Capture::Unfinished) {
                        return Err("unfinished capture".to_owned());
                    }
                }
                let a = first_cap_text(caps)?;
                if a.len() % 2 == 0 {
                    Ok(ReplOut::Keep)
                } else {
                    let mut out = a;
                    out.push(b'!');
                    Ok(ReplOut::Bytes(out))
                }
            }
            _ => {
                let key = match caps.first() {
                    Some(Capture::Str(b)) => Some(b.to_vec()),
                    Some(Capture::Position(p)) => Some(format!("#{p}").into_bytes()),
                    _ => return Err("unfinished capture".to_owned()),
                };
                Ok(match key.as_deref() {
                    Some(b"a") => ReplOut::Bytes(b"[A]".to_vec()),
                    Some(b"ab") => ReplOut::Bytes(b"12".to_vec()),
                    Some(b"#1") => ReplOut::Bytes(b"one".to_vec()),
                    Some(b"#2") => ReplOut::Bytes(b"two".to_vec()),
                    _ => ReplOut::Keep,
                })
            }
        }
    };
    let (out, count) = lstr::gsub(s, pat, n, &mut f)?;
    Ok(vec![V::Str(out), V::Num(count as f64)])
}

#[test]
fn find_match_gmatch_match_lua() {
    let mut o = Oracle::new();
    let mut rng = Rng(0x2545_F491_4F6C_DD1D);
    let inits: &[Option<f64>] = &[None, None, Some(1.0), Some(2.0), Some(-2.0), Some(0.0), Some(100.0), Some(-100.0), Some(5.0)];
    let mut cases: Vec<(Vec<u8>, Vec<u8>)> = (0..40_000).map(|_| (gen_subject(&mut rng), gen_pattern(&mut rng))).collect();
    cases.extend((0..300).map(|_| gen_deep_pattern(&mut rng)));
    for (subject, pat) in cases {
        let init = *rng.pick(inits);
        let plain = rng.chance(1, 8);
        let id = format!("s={:?} p={:?} init={init:?} plain={plain}", latin1(&subject), latin1(&pat));
        let args = |plain_arg: bool| {
            let mut a = vec![V::Str(subject.clone()), V::Str(pat.clone())];
            a.push(init.map_or(V::Nil, V::Num));
            if plain_arg {
                a.push(V::Bool(true));
            }
            a
        };
        let lua = o.call("W_find", &args(plain));
        o.check("find", &id, &lua, &model_find(&subject, &pat, init, plain, true));
        let lua = o.call("W_match", &args(false));
        o.check("match", &id, &lua, &model_find(&subject, &pat, init, false, false));
        let lua = o.call("W_gmatch", &[V::Str(subject.clone()), V::Str(pat.clone())]);
        o.check("gmatch", &id, &lua, &model_gmatch(&subject, &pat));
    }
    o.finish("find/match/gmatch");
}

#[test]
fn gsub_matches_lua() {
    let mut o = Oracle::new();
    let mut rng = Rng(0x1234_5678_9ABC_DEF1);
    let repls: &[&[u8]] = &[
        b"", b"x", b"%0", b"%1", b"%2", b"[%0]", b"%%", b"%", b"%a", b"%1%1", b"<%1|%2>", b"%9",
        b"a%0b", b"%%%0",
    ];
    let limits: &[Option<usize>] = &[None, None, None, Some(0), Some(1), Some(2), Some(5), Some(100)];
    let mut cases: Vec<(Vec<u8>, Vec<u8>)> = (0..30_000).map(|_| (gen_subject(&mut rng), gen_pattern(&mut rng))).collect();
    cases.extend((0..200).map(|_| gen_deep_pattern(&mut rng)));
    for (subject, pat) in cases {
        let kind = rng.below(4);
        let repl = rng.pick(repls).to_vec();
        let n = *rng.pick(limits);
        let id = format!("s={:?} p={:?} kind={kind} repl={:?} n={n:?}", latin1(&subject), latin1(&pat), latin1(&repl));
        let args = vec![
            V::Str(subject.clone()),
            V::Str(pat.clone()),
            V::Num(kind as f64),
            V::Str(repl.clone()),
            n.map_or(V::Nil, |n| V::Num(n as f64)),
        ];
        let lua = o.call("W_gsub", &args);
        o.check("gsub", &id, &lua, &model_gsub(&subject, &pat, kind, &repl, n));
    }
    o.finish("gsub");
}

// ---------------------------------------------------------------------------------------------
// string.format
// ---------------------------------------------------------------------------------------------

fn gen_format(rng: &mut Rng) -> Vec<u8> {
    let mut f = Vec::new();
    for _ in 0..1 + rng.below(3) {
        if rng.chance(1, 3) {
            f.extend(random_bytes(rng, b"ab :|", 3));
        }
        f.push(b'%');
        let flags = if rng.chance(1, 40) { 6 } else { rng.below(4) };
        for _ in 0..flags {
            f.push(*rng.pick(b"-+ #0"));
        }
        match rng.below(8) {
            0..=2 => {}
            3..=5 => f.extend(rng.below(100).to_string().bytes()),
            6 => f.extend((100 + rng.below(900)).to_string().bytes()),
            _ => f.push(b'1'),
        }
        if rng.chance(1, 2) {
            f.push(b'.');
            match rng.below(6) {
                0 => {}
                1..=3 => f.extend(rng.below(20).to_string().bytes()),
                4 => f.extend(rng.below(100).to_string().bytes()),
                _ => f.extend((100 + rng.below(900)).to_string().bytes()),
            }
        }
        f.push(*rng.pick(b"cdiuoxXeEgGaAfsqsddffgxFyhlpn"));
    }
    if rng.chance(1, 30) {
        f.push(b'%');
    }
    f
}

fn gen_fmt_arg(rng: &mut Rng) -> V {
    match rng.below(10) {
        0..=6 => V::Num(random_number(rng)),
        7 => V::Str(random_bytes(rng, b"ab 01\0\n\"\\\x01\x7f\xe9", 6)),
        8 => V::Str(b"x".repeat(95 + rng.below(12))),
        _ => V::Str(format!("{}", random_number(rng)).into_bytes()),
    }
}

fn model_format(fmt: &[u8], args: &[V]) -> Outcome {
    let r = lstr::format(fmt, &mut |idx, want| {
        let n = idx + 2;
        let Some(a) = args.get(idx) else {
            return Err(format!("bad argument #{n} to 'format' (no value)"));
        };
        match (want, a) {
            (Want::Present, _) => Ok(FmtArg::Num(0.0)),
            (Want::Number, V::Num(x)) => Ok(FmtArg::Num(*x)),
            (Want::Number, V::Str(b)) => lstr::str_to_number(b).map(FmtArg::Num).ok_or_else(|| {
                format!("bad argument #{n} to 'format' (number expected, got string)")
            }),
            (_, V::Num(x)) => Ok(FmtArg::Num(*x)),
            (_, V::Str(b)) => Ok(FmtArg::Str(b.clone())),
            _ => unreachable!("only numbers and strings are generated"),
        }
    });
    r.map(|b| vec![V::Str(b)])
}

#[test]
fn format_matches_lua() {
    let mut o = Oracle::new();
    let mut rng = Rng(0xA5A5_5A5A_DEAD_BEEF);
    let mut fixed: Vec<(Vec<u8>, Vec<V>)> = Vec::new();
    for fmt in [
        "%5.2f", "%-8s|", "%+d", "%#x", "%08.3f", "%g", "%e", "%G", "%E", "%.0f", "%.0e", "%#.0f",
        "%#g", "%10.4g", "%-10.3e|", "%+.3e", "% d", "%05d", "%-5d|", "%.3d", "%+5.3d", "%#o",
        "%x", "%X", "%#X", "%u", "%c", "%5c|", "%-5c|", "%q", "%s", "%.0s", "%10s", "%.20g", "%.99f",
        "%99.99f", "%a", "%A", "%.3a", "%.0a", "%10a", "%-12a|", "%020a", "%i", "%o", "%.14g",
        "%5s", "%05s", "%05c", "%y", "%", "%5", "%.", "%%", "%d%%",
    ] {
        for &n in EDGE {
            fixed.push((fmt.as_bytes().to_vec(), vec![V::Num(n)]));
        }
    }
    let mut cases = fixed;
    for _ in 0..40_000 {
        let fmt = gen_format(&mut rng);
        let args: Vec<V> = (0..4).map(|_| gen_fmt_arg(&mut rng)).collect();
        cases.push((fmt, args));
    }
    for (fmt, args) in cases {
        let mut all = vec![V::Str(fmt.clone())];
        all.extend(args.iter().cloned());
        let lua = o.call("W_fmt", &all);
        o.check("format", &format!("fmt={:?} args={}", latin1(&fmt), show(&args)), &lua, &model_format(&fmt, &args));
    }
    o.finish("format");
}

// ---------------------------------------------------------------------------------------------
// Simple string functions
// ---------------------------------------------------------------------------------------------

fn position_number(rng: &mut Rng) -> f64 {
    match rng.below(9) {
        0..=4 => rng.below(31) as f64 - 15.0,
        5 => *rng.pick(&[1e10, -1e10, 9.3e18, -9.3e18, f64::NAN, f64::INFINITY, f64::NEG_INFINITY]),
        6 => rng.below(20) as f64 / 4.0 - 2.5,
        _ => random_number(rng),
    }
}

#[test]
fn simple_functions_match_lua() {
    let mut o = Oracle::new();
    let mut rng = Rng(0xFEED_FACE_CAFE_BEEF);
    for _ in 0..20_000 {
        let subject = random_bytes(&mut rng, b"abcXYZ019 \0\xe9\xff\n", 12);
        let sv = V::Str(subject.clone());
        let id = format!("s={:?}", latin1(&subject));

        let i = position_number(&mut rng);
        let j = rng.chance(2, 3).then(|| position_number(&mut rng));
        let lua = o.call("W_sub", &[sv.clone(), V::Num(i), j.map_or(V::Nil, V::Num)]);
        let mine = Ok(vec![V::Str(lstr::sub(&subject, i, j.unwrap_or(-1.0)).to_vec())]);
        o.check("sub", &format!("{id} i={i:?} j={j:?}"), &lua, &mine);

        let i = rng.chance(3, 4).then(|| position_number(&mut rng));
        let j = rng.chance(1, 2).then(|| position_number(&mut rng));
        let lua = o.call("W_byte", &[sv.clone(), i.map_or(V::Nil, V::Num), j.map_or(V::Nil, V::Num)]);
        let ii = i.unwrap_or(1.0);
        let mine = lstr::byte(&subject, ii, j.unwrap_or(ii)).map(|v| v.into_iter().map(V::Num).collect());
        o.check("byte", &format!("{id} i={i:?} j={j:?}"), &lua, &mine);

        let n = *rng.pick(&[-1.0, 0.0, 1.0, 2.0, 3.0, 10.0, 40.0, 4_294_967_299.0, 2_147_483_648.0, f64::NAN, 2.5]);
        let sep = rng.chance(1, 2).then(|| random_bytes(&mut rng, b",;\0x", 3));
        let lua = o.call("W_rep", &[sv.clone(), V::Num(n), sep.clone().map_or(V::Nil, V::Str)]);
        let mine = lstr::rep(&subject, n, sep.as_deref()).map(|b| vec![V::Str(b)]);
        o.check("rep", &format!("{id} n={n:?} sep={sep:?}"), &lua, &mine);

        for (name, f) in [
            ("W_lower", lstr::lower as fn(&[u8]) -> Vec<u8>),
            ("W_upper", lstr::upper),
            ("W_reverse", lstr::reverse),
        ] {
            let lua = o.call(name, std::slice::from_ref(&sv));
            o.check(name, &id, &lua, &Ok(vec![V::Str(f(&subject))]));
        }

        let codes: Vec<f64> = (0..rng.below(5))
            .map(|_| match rng.below(4) {
                0 => *rng.pick(&[-1.0, 256.0, 4_294_967_361.0, f64::NAN, 1e10, 65.5, 0.0, 255.0]),
                _ => rng.below(256) as f64,
            })
            .collect();
        let args: Vec<V> = codes.iter().map(|&c| V::Num(c)).collect();
        let lua = o.call("W_char", &args);
        let mine = lstr::char_from(&codes).map(|b| vec![V::Str(b)]);
        o.check("char", &format!("{codes:?}"), &lua, &mine);
    }
    o.finish("sub/byte/rep/lower/upper/reverse/char");
}

// ---------------------------------------------------------------------------------------------
// table.sort
// ---------------------------------------------------------------------------------------------

fn model_sort(mut v: Vec<f64>, kind: usize) -> Outcome {
    let mut log: Vec<f64> = Vec::new();
    let mut n: u64 = 0;
    let result = lsort::sort(
        &mut v,
        &mut |a, b| {
            n += 1;
            log.push(*a);
            log.push(*b);
            Ok(match kind {
                0 => a < b,
                1 => a <= b,
                2 => a > b,
                3 => true,
                4 => false,
                _ => ((n * 2_654_435_761) % 4_294_967_296 / 128) % 2 == 1,
            })
        },
        || "invalid order function for sorting".to_owned(),
    );
    let mut out = match &result {
        Ok(()) => vec![V::Bool(true), V::Nil],
        Err(e) => vec![V::Bool(false), s(e)],
    };
    out.push(V::Num(log.len() as f64));
    out.extend(log.into_iter().map(V::Num));
    out.extend(v.into_iter().map(V::Num));
    Ok(out)
}

#[test]
fn sort_traces_match_lua() {
    let mut o = Oracle::new();
    let mut rng = Rng(0x0DDB_1A5E_5BAD_5EED);
    for case in 0..6_000 {
        let len = match case % 10 {
            0 => rng.below(4),
            9 => 100 + rng.below(150),
            _ => rng.below(40),
        };
        let spread = *rng.pick(&[2usize, 5, 1000]);
        let v: Vec<f64> = (0..len).map(|_| rng.below(spread) as f64).collect();
        let kind = if case % 3 == 0 { rng.below(6) } else { 0 };
        let arr = {
            let t = o.lua.create_table().expect("table");
            for (i, x) in v.iter().enumerate() {
                t.raw_set(i + 1, *x).expect("set");
            }
            t
        };
        let f: mlua::Function = o.lua.globals().get("W_sort").expect("W_sort");
        let (ok, err, log, sorted): (bool, Option<mlua::LuaString>, Table, Table) =
            f.call((arr, kind as f64)).expect("sort call");
        let mut lua = vec![
            V::Bool(ok),
            err.map_or(V::Nil, |m| V::Str(strip_position(&m.as_bytes()).into_bytes())),
        ];
        let log: Vec<f64> = log.sequence_values().collect::<Result<_, _>>().expect("log");
        lua.push(V::Num(log.len() as f64));
        lua.extend(log.into_iter().map(V::Num));
        let sorted: Vec<f64> = sorted.sequence_values().collect::<Result<_, _>>().expect("table");
        lua.extend(sorted.into_iter().map(V::Num));
        let lua = Ok(lua);
        o.check("sort", &format!("kind={kind} v={v:?}"), &lua, &model_sort(v.clone(), kind));
    }
    o.finish("table.sort");
}

// ---------------------------------------------------------------------------------------------
// bit32
// ---------------------------------------------------------------------------------------------

fn bit_number(rng: &mut Rng) -> f64 {
    match rng.below(8) {
        0..=2 => rng.below(40) as f64 - 8.0,
        3 => f64::from(rng.next() as u32),
        4 => *rng.pick(&[
            -1.0, 4_294_967_295.0, 4_294_967_296.0, 4_294_967_297.0, -4_294_967_295.0,
            2_147_483_648.0, 2_147_483_647.0, 0.5, 1.5, 2.5, -0.5, 1e10, -1e10, 1e15, 2f64.powi(51),
            2f64.powi(52), 2f64.powi(53), 1e19, -1e19, f64::NAN, f64::INFINITY, f64::NEG_INFINITY,
        ]),
        5 => (rng.next() as i64 >> rng.below(40)) as f64,
        _ => random_number(rng),
    }
}

fn model_bit(name: &str, a: &[f64]) -> Outcome {
    let num = |x: u32| Ok(vec![V::Num(f64::from(x))]);
    let w = |i: usize| a.get(i).copied();
    match name {
        "band" => num(bit32::band(a)),
        "bor" => num(bit32::bor(a)),
        "bxor" => num(bit32::bxor(a)),
        "btest" => Ok(vec![V::Bool(bit32::btest(a))]),
        "bnot" => num(bit32::bnot(a[0])),
        "lshift" => num(bit32::lshift(a[0], a[1])),
        "rshift" => num(bit32::rshift(a[0], a[1])),
        "arshift" => num(bit32::arshift(a[0], a[1])),
        "lrotate" => num(bit32::lrotate(a[0], a[1])),
        "rrotate" => num(bit32::rrotate(a[0], a[1])),
        "extract" => bit32::extract(a[0], a[1], w(2)).and_then(num),
        _ => bit32::replace(a[0], a[1], a[2], w(3)).and_then(num),
    }
}

#[test]
fn bit32_matches_lua() {
    let mut o = Oracle::new();
    let mut rng = Rng(0xB17B_17B1_7B17_B17B);
    for _ in 0..40_000 {
        let name = *rng.pick(&BIT_FUNCS);
        let count = match name {
            "band" | "bor" | "bxor" | "btest" => rng.below(5),
            "bnot" => 1,
            "extract" => 2 + rng.below(2),
            "replace" => 3 + rng.below(2),
            _ => 2,
        };
        let a: Vec<f64> = (0..count)
            .map(|i| {
                if (name == "extract" && i > 0) || (name == "replace" && i > 1) {
                    match rng.below(4) {
                        0 => rng.below(34) as f64 - 2.0,
                        1 => rng.below(8) as f64,
                        2 => rng.below(32) as f64,
                        _ => bit_number(&mut rng),
                    }
                } else {
                    bit_number(&mut rng)
                }
            })
            .collect();
        let args: Vec<V> = a.iter().map(|&x| V::Num(x)).collect();
        let lua = o.call(&format!("W_{name}"), &args);
        let id = format!("{name}({})", a.iter().map(|&x| bits_f(x)).collect::<Vec<_>>().join(", "));
        o.check("bit32", &id, &lua, &model_bit(name, &a));
    }
    o.finish("bit32");
}
