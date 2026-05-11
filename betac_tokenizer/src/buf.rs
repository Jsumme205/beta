use core::mem;
use core::mem::MaybeUninit;

const MAX_BUF_LEN: usize = u64::BITS as usize + "_uint64".len();

pub struct IntegerBuffer {
    buf: [MaybeUninit<u8>; MAX_BUF_LEN],
    initialised_len: u8,
}

impl IntegerBuffer {
    pub const fn new() -> Self {
        Self {
            buf: [MaybeUninit::uninit(); MAX_BUF_LEN],
            initialised_len: 0,
        }
    }

    pub fn push(&mut self, c: char) {
        let mut buf = [0u8; 4];
        let s = c.encode_utf8(&mut buf);
        let range = core::ops::Range {
            start: self.initialised_len as usize,
            end: self.initialised_len as usize + s.len(),
        };

        self.buf[range].copy_from_slice(unsafe { mem::transmute(s.as_bytes()) });
        self.initialised_len += s.len() as u8;
    }

    /// panics if `s` would exceed capacity
    pub fn push_str(&mut self, s: &str) {
        let range = core::ops::Range {
            start: self.initialised_len as usize,
            end: self.initialised_len as usize + s.len(),
        };

        self.buf[range].copy_from_slice(unsafe { mem::transmute(s.as_bytes()) });
        self.initialised_len += s.len() as u8;
    }

    pub fn as_initialized_str(&self) -> &str {
        unsafe {
            str::from_utf8_unchecked(&self.buf[..self.initialised_len as usize].assume_init_ref())
        }
    }

    pub const fn len(&self) -> usize {
        self.initialised_len as _
    }

    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
