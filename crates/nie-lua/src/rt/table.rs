//! Lua tables: a dense array part plus an insertion-ordered hash part.
//!
//! The hash part keeps removed keys as tombstones (value `nil`) so that `next` keeps working
//! when a script assigns `nil` to fields during a traversal, as Lua allows.

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

use super::value::{Value, tag};

/// Fast non-cryptographic hasher for `(tag, bits)` keys.
#[derive(Default)]
pub struct KeyHasher(u64);

impl Hasher for KeyHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.0 = (self.0.rotate_left(5) ^ u64::from(*b)).wrapping_mul(0x517c_c1b7_2722_0a95);
        }
    }

    fn write_u64(&mut self, i: u64) {
        self.0 = (self.0.rotate_left(5) ^ i).wrapping_mul(0x517c_c1b7_2722_0a95);
    }
}

type Index = HashMap<(u64, u64), u32, BuildHasherDefault<KeyHasher>>;

/// A Lua table.
pub struct Table {
    /// Values for keys `1..=arr.len()`.
    pub arr: Vec<Value>,
    entries: Vec<(Value, Value)>,
    index: Index,
    tombstones: usize,
    /// Metatable, or null.
    pub meta: *mut Table,
}

/// Normalizes a key for hashing: `-0.0` and `0.0` are the same key.
#[inline]
fn hkey(k: Value) -> (u64, u64) {
    if k.tag == tag::NUM && k.as_num() == 0.0 {
        (tag::NUM, 0)
    } else {
        (k.tag, k.bits)
    }
}

/// `Some(i)` when the number is a positive integer representable as an array index.
#[inline]
fn array_index(k: Value) -> Option<usize> {
    if k.tag != tag::NUM {
        return None;
    }
    let n = k.as_num();
    // `as` saturates; the round trip rejects fractions, NaN and out-of-range values.
    let i = n as i64;
    (i >= 1 && i as f64 == n && i < (1 << 40)).then_some(i as usize)
}

impl Table {
    /// A table with pre-sized parts.
    #[must_use]
    pub fn new(narr: usize, nhash: usize) -> Self {
        Self {
            arr: Vec::with_capacity(narr),
            entries: Vec::with_capacity(nhash),
            index: Index::with_capacity_and_hasher(nhash, BuildHasherDefault::default()),
            tombstones: 0,
            meta: std::ptr::null_mut(),
        }
    }

    /// Raw get.
    #[inline]
    #[must_use]
    pub fn get(&self, key: Value) -> Value {
        if let Some(i) = array_index(key) {
            if let Some(v) = self.arr.get(i - 1) {
                return *v;
            }
        }
        if self.entries.is_empty() {
            return Value::NIL;
        }
        match self.index.get(&hkey(key)) {
            Some(&slot) => self.entries[slot as usize].1,
            None => Value::NIL,
        }
    }

    /// Raw get with a string key given by pointer.
    #[inline]
    #[must_use]
    pub fn get_str(&self, key: Value) -> Value {
        if self.entries.is_empty() {
            return Value::NIL;
        }
        match self.index.get(&(key.tag, key.bits)) {
            Some(&slot) => self.entries[slot as usize].1,
            None => Value::NIL,
        }
    }

    /// Raw get with an integer key.
    #[inline]
    #[must_use]
    pub fn get_int(&self, i: i64) -> Value {
        self.get(Value::num(i as f64))
    }

    /// Raw set. The caller has rejected `nil` and NaN keys.
    pub fn set(&mut self, key: Value, val: Value) {
        if let Some(i) = array_index(key) {
            let len = self.arr.len();
            if i <= len {
                self.arr[i - 1] = val;
                if val.is_nil() && i == len {
                    self.trim();
                }
                return;
            }
            if i == len + 1 && self.index_is_clear_of_ints(i) {
                if !val.is_nil() {
                    self.arr.push(val);
                    self.migrate();
                }
                return;
            }
            if i == len + 1 {
                // A hash entry already holds this key (tombstone or value): fall through.
            }
        }
        self.set_hash(key, val);
    }

    /// Fast check used before appending: when the hash part has no live entry for `i`.
    fn index_is_clear_of_ints(&self, i: usize) -> bool {
        if self.entries.is_empty() {
            return true;
        }
        match self.index.get(&(tag::NUM, (i as f64).to_bits())) {
            Some(&slot) => {
                let e = &self.entries[slot as usize];
                e.1.is_nil()
            }
            None => true,
        }
    }

    fn set_hash(&mut self, key: Value, val: Value) {
        let hk = hkey(key);
        match self.index.get(&hk) {
            Some(&slot) => {
                let e = &mut self.entries[slot as usize];
                match (e.1.is_nil(), val.is_nil()) {
                    (true, false) => self.tombstones -= 1,
                    (false, true) => self.tombstones += 1,
                    _ => {}
                }
                e.1 = val;
            }
            None => {
                if val.is_nil() {
                    return;
                }
                if self.tombstones > 16 && self.tombstones * 2 > self.entries.len() {
                    self.compact();
                }
                let k = if hk.0 == tag::NUM {
                    Value {
                        tag: tag::NUM,
                        bits: hk.1,
                    }
                } else {
                    key
                };
                self.index.insert(hk, self.entries.len() as u32);
                self.entries.push((k, val));
            }
        }
    }

    fn compact(&mut self) {
        self.entries.retain(|e| !e.1.is_nil());
        self.tombstones = 0;
        self.index.clear();
        for (slot, e) in self.entries.iter().enumerate() {
            self.index.insert(hkey(e.0), slot as u32);
        }
    }

    /// Drops trailing nils from the array part.
    fn trim(&mut self) {
        while self.arr.last().is_some_and(|v| v.is_nil()) {
            self.arr.pop();
        }
    }

    /// Moves keys `len+1, len+2, ...` from the hash part into the array part.
    fn migrate(&mut self) {
        if self.entries.len() == self.tombstones {
            return;
        }
        loop {
            let next = (self.arr.len() + 1) as f64;
            let Some(&slot) = self.index.get(&(tag::NUM, next.to_bits())) else {
                return;
            };
            let v = self.entries[slot as usize].1;
            if v.is_nil() {
                return;
            }
            self.entries[slot as usize].1 = Value::NIL;
            self.tombstones += 1;
            self.arr.push(v);
        }
    }

    /// The border used by `#`: the array length (a valid border: `t[n] ~= nil`, `t[n+1] == nil`).
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.arr.len()
    }

    /// `true` when the table has no live entry at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.arr.is_empty() && self.entries.len() == self.tombstones
    }

    /// Sets the array part from `SETLIST`-style stores (1-based start).
    pub fn set_array_from(&mut self, start: usize, vals: &[Value]) {
        for (i, v) in vals.iter().enumerate() {
            let idx = start + i;
            if idx == self.arr.len() + 1 && self.index_is_clear_of_ints(idx) {
                // Holes created by constructors such as `{nil, 2}` stay in the array.
                self.arr.push(*v);
            } else {
                self.set(Value::num(idx as f64), *v);
            }
        }
        self.trim();
        self.migrate();
    }

    /// Iteration step for `next`: `None` when `key` is not in the table, `Some(None)` at the end.
    #[must_use]
    pub fn next(&self, key: Value) -> Option<Option<(Value, Value)>> {
        let mut pos = if key.is_nil() {
            0
        } else if let Some(i) = array_index(key).filter(|i| *i <= self.arr.len()) {
            i
        } else if let Some(&slot) = self.index.get(&hkey(key)) {
            self.arr.len() + slot as usize + 1
        } else if array_index(key).is_some() {
            // An array element removed during the traversal shrank the array part: resume at
            // the hash part.
            self.arr.len()
        } else {
            return None;
        };
        while pos < self.arr.len() {
            let v = self.arr[pos];
            if !v.is_nil() {
                return Some(Some((Value::num((pos + 1) as f64), v)));
            }
            pos += 1;
        }
        let mut slot = pos - self.arr.len();
        while slot < self.entries.len() {
            let e = self.entries[slot];
            if !e.1.is_nil() {
                return Some(Some(e));
            }
            slot += 1;
        }
        Some(None)
    }
}
