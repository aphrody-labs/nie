//! A PE64 image laid out flat at its virtual addresses.

use anyhow::{Result, bail, ensure};

/// `IMAGE_SCN_MEM_EXECUTE`.
pub const SCN_EXEC: u32 = 0x2000_0000;
/// `IMAGE_SCN_MEM_WRITE`.
pub const SCN_WRITE: u32 = 0x8000_0000;

#[derive(Clone, Debug)]
pub struct Section {
    pub name: String,
    pub rva: u32,
    pub vsize: u32,
    pub raw_off: u32,
    pub raw_size: u32,
    pub chars: u32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DataDir {
    pub rva: u32,
    pub size: u32,
}

pub const DIR_EXPORT: usize = 0;
pub const DIR_IMPORT: usize = 1;
pub const DIR_EXCEPTION: usize = 3;
pub const DIR_DEBUG: usize = 6;
pub const DIR_TLS: usize = 9;
pub const DIR_LOAD_CONFIG: usize = 10;
pub const DIR_IAT: usize = 12;
pub const DIR_DELAY_IMPORT: usize = 13;

#[derive(Clone, Debug)]
pub struct Image {
    pub base: u64,
    pub size_of_image: u32,
    pub entry_rva: u32,
    pub linker: (u8, u8),
    pub timestamp: u32,
    pub dll_chars: u16,
    pub file_len: usize,
    pub sections: Vec<Section>,
    pub dirs: [DataDir; 16],
    /// Image bytes at virtual layout: `flat[rva]` is the byte at `base + rva`.
    pub flat: Vec<u8>,
}

fn u16_at(b: &[u8], o: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(b.get(o..o + 2).ok_or_else(|| anyhow::anyhow!("short read at {o:#x}"))?.try_into()?))
}

fn u32_at(b: &[u8], o: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(b.get(o..o + 4).ok_or_else(|| anyhow::anyhow!("short read at {o:#x}"))?.try_into()?))
}

fn u64_at(b: &[u8], o: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(b.get(o..o + 8).ok_or_else(|| anyhow::anyhow!("short read at {o:#x}"))?.try_into()?))
}

impl Image {
    pub fn parse(file: &[u8]) -> Result<Self> {
        ensure!(file.get(..2) == Some(b"MZ"), "no MZ header");
        let e = u32_at(file, 0x3c)? as usize;
        ensure!(file.get(e..e + 4) == Some(b"PE\0\0"), "no PE signature");
        ensure!(u16_at(file, e + 4)? == 0x8664, "not x86-64");
        let nsec = usize::from(u16_at(file, e + 6)?);
        let timestamp = u32_at(file, e + 8)?;
        let opt_size = usize::from(u16_at(file, e + 20)?);
        let o = e + 24;
        if u16_at(file, o)? != 0x20b {
            bail!("not PE32+");
        }
        let linker = (file[o + 2], file[o + 3]);
        let entry_rva = u32_at(file, o + 16)?;
        let base = u64_at(file, o + 24)?;
        let size_of_image = u32_at(file, o + 56)?;
        let size_of_headers = u32_at(file, o + 60)? as usize;
        let dll_chars = u16_at(file, o + 70)?;
        let ndirs = (u32_at(file, o + 108)? as usize).min(16);
        let mut dirs = [DataDir::default(); 16];
        for (i, d) in dirs.iter_mut().enumerate().take(ndirs) {
            *d = DataDir { rva: u32_at(file, o + 112 + 8 * i)?, size: u32_at(file, o + 116 + 8 * i)? };
        }
        let mut flat = vec![0u8; size_of_image as usize];
        let head = size_of_headers.min(file.len()).min(flat.len());
        flat[..head].copy_from_slice(&file[..head]);
        let mut sections = Vec::with_capacity(nsec);
        let so = o + opt_size;
        for i in 0..nsec {
            let s = so + 40 * i;
            let raw = file.get(s..s + 8).ok_or_else(|| anyhow::anyhow!("short section table"))?;
            let name = String::from_utf8_lossy(raw).trim_end_matches('\0').to_owned();
            let sec = Section {
                name,
                vsize: u32_at(file, s + 8)?,
                rva: u32_at(file, s + 12)?,
                raw_size: u32_at(file, s + 16)?,
                raw_off: u32_at(file, s + 20)?,
                chars: u32_at(file, s + 36)?,
            };
            let n = sec.raw_size.min(sec.vsize) as usize;
            let src = file.get(sec.raw_off as usize..sec.raw_off as usize + n)
                .ok_or_else(|| anyhow::anyhow!("section {} outside file", sec.name))?;
            let dst = flat.get_mut(sec.rva as usize..sec.rva as usize + n)
                .ok_or_else(|| anyhow::anyhow!("section {} outside image", sec.name))?;
            dst.copy_from_slice(src);
            sections.push(sec);
        }
        Ok(Self { base, size_of_image, entry_rva, linker, timestamp, dll_chars, file_len: file.len(), sections, dirs, flat })
    }

    pub fn slice(&self, rva: u32, len: usize) -> Option<&[u8]> {
        let s = rva as usize;
        self.flat.get(s..s.checked_add(len)?)
    }

    pub fn u32(&self, rva: u32) -> Option<u32> {
        Some(u32::from_le_bytes(self.slice(rva, 4)?.try_into().ok()?))
    }

    pub fn u64(&self, rva: u32) -> Option<u64> {
        Some(u64::from_le_bytes(self.slice(rva, 8)?.try_into().ok()?))
    }

    pub fn u16(&self, rva: u32) -> Option<u16> {
        Some(u16::from_le_bytes(self.slice(rva, 2)?.try_into().ok()?))
    }

    /// NUL-terminated ASCII string at `rva`, at most 512 bytes.
    pub fn cstr(&self, rva: u32) -> Option<String> {
        let s = self.flat.get(rva as usize..)?;
        let n = s.iter().take(512).position(|&b| b == 0)?;
        Some(String::from_utf8_lossy(&s[..n]).into_owned())
    }

    pub fn va_to_rva(&self, va: u64) -> Option<u32> {
        let r = va.checked_sub(self.base)?;
        (r < u64::from(self.size_of_image)).then_some(r as u32)
    }

    pub fn section_of(&self, rva: u32) -> Option<&Section> {
        self.sections.iter().find(|s| rva >= s.rva && rva < s.rva + s.vsize.max(s.raw_size))
    }

    pub fn is_exec(&self, rva: u32) -> bool {
        self.section_of(rva).is_some_and(|s| s.chars & SCN_EXEC != 0)
    }

    pub fn section(&self, name: &str) -> Option<&Section> {
        self.sections.iter().find(|s| s.name == name)
    }

    pub fn dir(&self, i: usize) -> DataDir {
        self.dirs[i]
    }
}
