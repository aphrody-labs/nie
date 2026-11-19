//! Tagged Lua values: 16 bytes, `repr(C)`, laid out so generated code can read tag and payload
//! with two plain loads.

/// Type tags. `NIL` is zero so that zeroed memory is a stack of nils.
pub mod tag {
    /// `nil`.
    pub const NIL: u64 = 0;
    /// Boolean; payload 0 or 1.
    pub const BOOL: u64 = 1;
    /// Number; payload is the `f64` bit pattern.
    pub const NUM: u64 = 2;
    /// Interned string; payload is a `*const LStr`.
    pub const STR: u64 = 3;
    /// Table; payload is a `*mut Table`.
    pub const TABLE: u64 = 4;
    /// Function; payload is a `*mut Closure`.
    pub const FUNC: u64 = 5;
    /// Coroutine; payload is a `*mut Thread`.
    pub const THREAD: u64 = 6;
}

/// A Lua value.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Value {
    /// One of [`tag`].
    pub tag: u64,
    /// Payload, interpreted per tag.
    pub bits: u64,
}

impl Value {
    /// `nil`.
    pub const NIL: Self = Self {
        tag: tag::NIL,
        bits: 0,
    };

    /// Boolean value.
    #[inline]
    #[must_use]
    pub const fn bool(b: bool) -> Self {
        Self {
            tag: tag::BOOL,
            bits: b as u64,
        }
    }

    /// Number value.
    #[inline]
    #[must_use]
    pub const fn num(n: f64) -> Self {
        Self {
            tag: tag::NUM,
            bits: n.to_bits(),
        }
    }

    /// Number payload; only meaningful when `tag == NUM`.
    #[inline]
    #[must_use]
    pub const fn as_num(self) -> f64 {
        f64::from_bits(self.bits)
    }

    /// `false` for `nil` and `false`, `true` otherwise.
    #[inline]
    #[must_use]
    pub const fn truthy(self) -> bool {
        !(self.tag == tag::NIL || (self.tag == tag::BOOL && self.bits == 0))
    }

    /// `true` for `nil`.
    #[inline]
    #[must_use]
    pub const fn is_nil(self) -> bool {
        self.tag == tag::NIL
    }

    /// Lua type name as returned by `type()`.
    #[must_use]
    pub const fn type_name(self) -> &'static str {
        match self.tag {
            tag::NIL => "nil",
            tag::BOOL => "boolean",
            tag::NUM => "number",
            tag::STR => "string",
            tag::TABLE => "table",
            tag::FUNC => "function",
            tag::THREAD => "thread",
            _ => "userdata",
        }
    }

    /// Raw equality (`rawequal`): numbers by value, everything else by identity of payload.
    #[inline]
    #[must_use]
    pub fn raw_eq(self, other: Self) -> bool {
        if self.tag != other.tag {
            return false;
        }
        if self.tag == tag::NUM {
            self.as_num() == other.as_num()
        } else {
            self.bits == other.bits
        }
    }
}

/// An interned string: a length header followed by the bytes and a NUL.
#[repr(C)]
pub struct LStr {
    /// Byte length, NUL excluded.
    pub len: usize,
    /// Precomputed hash of the bytes.
    pub hash: u64,
}

impl LStr {
    /// The bytes of the string.
    ///
    /// # Safety
    /// `this` must point at a live string allocated by the heap.
    #[inline]
    #[must_use]
    pub unsafe fn bytes<'a>(this: *const Self) -> &'a [u8] {
        // SAFETY: strings are allocated as `header + len + 1` bytes in one block.
        unsafe { std::slice::from_raw_parts(this.add(1).cast::<u8>(), (*this).len) }
    }
}

impl Value {
    /// The string bytes of a `STR` value.
    ///
    /// # Safety
    /// `self.tag` must be [`tag::STR`] and the heap that owns it must be alive.
    #[inline]
    #[must_use]
    pub unsafe fn str_bytes<'a>(self) -> &'a [u8] {
        // SAFETY: caller contract.
        unsafe { LStr::bytes(self.bits as *const LStr) }
    }
}
