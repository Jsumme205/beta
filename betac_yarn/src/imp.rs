use core::{alloc::Layout, mem, num::NonZero, ptr, slice};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RawYarn {
    pub ptr: *mut u8,
    pub len: NonZero<usize>,
}

#[repr(u8)]
#[derive(Clone, Copy, PartialEq)]
pub enum Kind {
    Small = 0b01,
    Aliased = 0b00,
    Heap = 0b11,
    Static = 0b10,
}

impl RawYarn {
    pub const unsafe fn assume_small(self) -> Small {
        unsafe { mem::transmute::<Self, Small>(self) }
    }

    pub const unsafe fn assume_large(self) -> Large {
        unsafe { mem::transmute::<Self, Large>(self) }
    }

    pub const fn kind(self) -> Kind {
        unsafe { mem::transmute(self.assume_small().len_and_flags & MASK) }
    }

    pub const fn as_slice(&self) -> &[u8] {
        match self.kind() {
            Kind::Aliased | Kind::Heap | Kind::Static => unsafe {
                slice::from_raw_parts(self.assume_large().ptr, self.assume_large().len & LEN_MASK)
            },
            Kind::Small => unsafe {
                slice::from_raw_parts(
                    self.assume_small().data.as_ptr(),
                    (self.assume_small().len_and_flags & LEN8_MASK) as usize,
                )
            },
        }
    }

    pub const fn len(self) -> usize {
        match self.kind() {
            Kind::Aliased | Kind::Heap | Kind::Static => unsafe {
                self.assume_large().len & LEN_MASK
            },
            Kind::Small => unsafe { (self.assume_small().len_and_flags & LEN8_MASK) as usize },
        }
    }

    pub const unsafe fn from_raw_parts(ptr: *const u8, len: usize, kind: Kind) -> Self {
        if len <= MAX_SSO_LEN {
            unsafe { Self::small_unchecked(ptr, len) }
        } else {
            let large = Large {
                ptr,
                len: len << 2 | (kind as usize),
            };
            unsafe { mem::transmute(large) }
        }
    }

    pub const unsafe fn small_unchecked(ptr: *const u8, len: usize) -> Self {
        let mut buf = Small {
            len_and_flags: (len as u8) << 2 | Kind::Small as u8,
            data: [0u8; _],
        };

        unsafe {
            ptr::copy_nonoverlapping(ptr, buf.data.as_mut_ptr(), len);

            mem::transmute(buf)
        }
    }

    pub unsafe fn destroy(self) {
        if self.kind() != Kind::Heap {
            return;
        }

        unsafe {
            alloc::alloc::dealloc(
                self.assume_large().ptr as *mut u8,
                Layout::array::<u8>(self.len()).unwrap(),
            );
        }
    }
}

pub const MAX_SSO_LEN: usize = mem::size_of::<usize>() * 2 - 1;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Small {
    pub data: [u8; MAX_SSO_LEN],
    //   b0 b1  b2  b3  b4  b5   b6  b7
    // | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0
    pub len_and_flags: u8,
}

pub const MASK: u8 = 0b00000011;
pub const LEN8_MASK: u8 = !MASK;
pub const LEN_MASK: usize = !(MASK as usize);

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Large {
    pub ptr: *const u8,
    pub len: usize,
}
