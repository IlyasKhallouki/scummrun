//! A file mapped read-only (libc mmap): a reading's files are mapped, never copied (plan.md, "Memory layout").

use std::fs::File;
use std::io;
use std::os::unix::io::AsRawFd;
use std::path::Path;

/// A read-only private map of a whole file; an empty file maps to an empty slice.
pub struct Map {
    ptr: *const u8,
    len: usize,
}

// SAFETY: the map is read-only and owned; its pages are never written through it.
unsafe impl Send for Map {}
unsafe impl Sync for Map {}

impl Map {
    pub fn open(path: &Path) -> io::Result<Map> {
        let f = File::open(path)?;
        let len = f.metadata()?.len() as usize;
        if len == 0 {
            return Ok(Map { ptr: std::ptr::NonNull::<u8>::dangling().as_ptr(), len: 0 });
        }
        // SAFETY: a fresh read-only private mapping of `len` bytes of an open file
        let p = unsafe { libc::mmap(std::ptr::null_mut(), len, libc::PROT_READ, libc::MAP_PRIVATE, f.as_raw_fd(), 0) };
        if p == libc::MAP_FAILED {
            return Err(io::Error::last_os_error());
        }
        Ok(Map { ptr: p as *const u8, len })
    }

    #[inline]
    pub fn bytes(&self) -> &[u8] {
        // SAFETY: ptr/len describe the live mapping (or an empty slice)
        unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl Drop for Map {
    fn drop(&mut self) {
        if self.len > 0 {
            // SAFETY: unmaps the mapping made in `open`
            unsafe {
                libc::munmap(self.ptr as *mut libc::c_void, self.len);
            }
        }
    }
}

#[inline]
pub fn u32_at(b: &[u8], pos: usize) -> u32 {
    u32::from_le_bytes(b[pos..pos + 4].try_into().unwrap())
}

#[inline]
pub fn u64_at(b: &[u8], pos: usize) -> u64 {
    u64::from_le_bytes(b[pos..pos + 8].try_into().unwrap())
}
