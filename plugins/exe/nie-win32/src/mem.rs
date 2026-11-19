//! Fixed-address guest memory and guest stacks (Linux).

use std::io;

/// The guest image: anonymous memory mapped at the PE's preferred base, so
/// every guest address is a host address.
pub struct Guest {
    base: u64,
    len: usize,
}

impl Guest {
    /// Map `len` bytes at `base`, failing if the range is taken.
    pub fn map(base: u64, len: usize) -> io::Result<Self> {
        // SAFETY: anonymous private mapping; MAP_FIXED_NOREPLACE never clobbers an existing mapping.
        let p = unsafe {
            libc::mmap(
                base as *mut libc::c_void,
                len,
                libc::PROT_READ | libc::PROT_WRITE | libc::PROT_EXEC,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | libc::MAP_FIXED_NOREPLACE,
                -1,
                0,
            )
        };
        if p == libc::MAP_FAILED {
            return Err(io::Error::last_os_error());
        }
        if p as u64 != base {
            // Kernels older than 4.17 ignore the flag and treat base as a hint.
            // SAFETY: unmapping the mapping created above.
            unsafe { libc::munmap(p, len) };
            return Err(io::Error::new(io::ErrorKind::AddrInUse, "image base is taken"));
        }
        Ok(Self { base, len })
    }

    pub fn base(&self) -> u64 {
        self.base
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Copy `data` to guest offset `off`.
    pub fn write(&mut self, off: usize, data: &[u8]) {
        assert!(off.checked_add(data.len()).is_some_and(|e| e <= self.len));
        // SAFETY: the range was bounds-checked against the mapping owned by self.
        unsafe {
            core::ptr::copy_nonoverlapping(data.as_ptr(), (self.base as *mut u8).add(off), data.len());
        }
    }

    /// Whole mapping as a slice.
    pub fn bytes(&self) -> &[u8] {
        // SAFETY: the mapping is live, readable and `len` bytes long.
        unsafe { core::slice::from_raw_parts(self.base as *const u8, self.len) }
    }

    /// Write a little-endian u64 at guest address `va`.
    pub fn write_u64(&mut self, va: u64, v: u64) {
        let off = (va - self.base) as usize;
        self.write(off, &v.to_le_bytes());
    }
}

impl Drop for Guest {
    fn drop(&mut self) {
        // SAFETY: unmapping exactly the range mapped in `map`.
        unsafe { libc::munmap(self.base as *mut libc::c_void, self.len) };
    }
}

/// A guest stack, 16-byte aligned top.
pub struct Stack {
    ptr: *mut libc::c_void,
    len: usize,
}

impl Stack {
    pub fn new(len: usize) -> io::Result<Self> {
        // SAFETY: anonymous private mapping with kernel-chosen address.
        let ptr = unsafe {
            libc::mmap(
                core::ptr::null_mut(),
                len,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                -1,
                0,
            )
        };
        if ptr == libc::MAP_FAILED {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { ptr, len })
    }

    pub fn top(&self) -> u64 {
        self.ptr as u64 + self.len as u64
    }
}

impl Drop for Stack {
    fn drop(&mut self) {
        // SAFETY: unmapping the mapping created in `new`.
        unsafe { libc::munmap(self.ptr, self.len) };
    }
}
