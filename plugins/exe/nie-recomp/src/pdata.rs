//! `.pdata` (`RUNTIME_FUNCTION`) and `UNWIND_INFO` decoding.

use crate::image::{DIR_EXCEPTION, Image};

pub const UNW_FLAG_EHANDLER: u8 = 1;
pub const UNW_FLAG_UHANDLER: u8 = 2;
pub const UNW_FLAG_CHAININFO: u8 = 4;

#[derive(Clone, Copy, Debug)]
pub struct Entry {
    pub begin: u32,
    pub end: u32,
    pub unwind: u32,
    pub flags: u8,
    pub prolog: u8,
    /// Language handler RVA when the unwind info has one.
    pub handler: Option<u32>,
    /// Parent function start (RVA) when this entry is a chained fragment.
    pub chain: Option<u32>,
}

impl Entry {
    /// A real function start: not a chained fragment of another entry.
    pub fn is_root(&self) -> bool {
        self.flags & UNW_FLAG_CHAININFO == 0
    }
}

/// Every non-empty `RUNTIME_FUNCTION`, in table order.
pub fn entries(img: &Image) -> Vec<Entry> {
    let d = img.dir(DIR_EXCEPTION);
    let Some(table) = img.slice(d.rva, d.size as usize) else {
        return Vec::new();
    };
    let mut out = Vec::with_capacity(table.len() / 12);
    for c in table.as_chunks::<12>().0 {
        let begin = u32::from_le_bytes(c[0..4].try_into().unwrap());
        let end = u32::from_le_bytes(c[4..8].try_into().unwrap());
        let unwind = u32::from_le_bytes(c[8..12].try_into().unwrap());
        if begin == 0 && end == 0 {
            continue;
        }
        let (flags, prolog, handler, chain) = unwind_info(img, unwind);
        out.push(Entry { begin, end, unwind, flags, prolog, handler, chain });
    }
    out
}

fn unwind_info(img: &Image, unwind: u32) -> (u8, u8, Option<u32>, Option<u32>) {
    let Some(h) = img.slice(unwind & !1, 4) else {
        return (0, 0, None, None);
    };
    let flags = h[0] >> 3;
    let codes = usize::from(h[2]);
    let after = (unwind & !1) + 4 + (((codes + 1) & !1) * 2) as u32;
    let handler = (flags & (UNW_FLAG_EHANDLER | UNW_FLAG_UHANDLER) != 0
        && flags & UNW_FLAG_CHAININFO == 0)
        .then(|| img.u32(after))
        .flatten();
    let chain = (flags & UNW_FLAG_CHAININFO != 0).then(|| img.u32(after)).flatten();
    (flags, h[1], handler, chain)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chained_entries_are_not_roots() {
        let e = Entry { begin: 0, end: 1, unwind: 0, flags: UNW_FLAG_CHAININFO, prolog: 0, handler: None, chain: Some(0) };
        assert!(!e.is_root());
        let r = Entry { flags: UNW_FLAG_EHANDLER, ..e };
        assert!(r.is_root());
    }
}
