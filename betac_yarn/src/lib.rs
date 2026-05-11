#![no_std]

use core::{fmt::Debug, hash::Hash, marker::PhantomData, mem, ops::Deref};

use alloc::boxed::Box;
extern crate alloc;

mod imp;

#[repr(transparent)]
pub struct Yarn<'a> {
    imp: imp::RawYarn,
    _marker: PhantomData<&'a str>,
}

impl Debug for Yarn<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self)
    }
}

impl<'a> Yarn<'a> {
    pub const fn from_ref(s: &'a str) -> Self {
        Self {
            imp: unsafe { imp::RawYarn::from_raw_parts(s.as_ptr(), s.len(), imp::Kind::Aliased) },
            _marker: PhantomData,
        }
    }

    pub const unsafe fn from_utf8_unchecked(s: &'a [u8]) -> Self {
        unsafe { Self::from_ref(str::from_utf8_unchecked(s)) }
    }

    pub const fn len(&self) -> usize {
        self.imp.len()
    }

    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub const fn alias<'b>(&'b self) -> Yarn<'b> {
        Self {
            imp: unsafe {
                imp::RawYarn::from_raw_parts(
                    self.imp.as_slice().as_ptr(),
                    self.len(),
                    imp::Kind::Aliased,
                )
            },
            _marker: PhantomData,
        }
    }

    pub const fn as_str(&self) -> &str {
        unsafe { str::from_utf8_unchecked(self.imp.as_slice()) }
    }

    pub fn immortalize(self) -> Yarn<'static> {
        match self.imp.kind() {
            imp::Kind::Heap | imp::Kind::Small | imp::Kind::Static => unsafe {
                mem::transmute(self)
            },
            imp::Kind::Aliased => {
                let v = alloc::boxed::Box::from(self.as_str());
                Yarn::from_boxed_str(v)
            }
        }
    }
}

impl Yarn<'_> {
    pub const fn from_boxed_str(s: alloc::boxed::Box<str>) -> Self {
        let ptr = s.as_ptr();
        let len = s.len();
        mem::forget(s);
        Self {
            imp: unsafe { imp::RawYarn::from_raw_parts(ptr, len, imp::Kind::Heap) },
            _marker: PhantomData,
        }
    }
}

impl Deref for Yarn<'_> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl Yarn<'static> {
    pub const fn from_static_str(s: &'static str) -> Yarn<'static> {
        Self {
            imp: unsafe { imp::RawYarn::from_raw_parts(s.as_ptr(), s.len(), imp::Kind::Static) },
            _marker: PhantomData,
        }
    }
}

impl<'a> Drop for Yarn<'a> {
    fn drop(&mut self) {
        unsafe {
            self.imp.destroy();
        }
    }
}

impl Hash for Yarn<'_> {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        state.write(self.as_bytes());
    }
}

impl PartialEq for Yarn<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}

impl Eq for Yarn<'_> {}
