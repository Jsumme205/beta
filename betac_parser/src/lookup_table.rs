use std::mem;

use crate::{Parser, TokenKind};

#[derive(Clone, Copy)]
pub struct Entry {
    pub value: u128,
    pub len: u8,
    kind: TokenKind,
}

impl Entry {
    pub const fn reconstruct(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(&raw const self.value as *const u8, self.len as _) }
    }

    pub(crate) const fn __new(key: &[u8], value: TokenKind) -> Self {
        Entry {
            value: encode(key),
            len: key.len() as _,
            kind: value,
        }
    }
}

pub struct Table<const N: usize> {
    groups: [&'static [Entry]; N],
}

impl<const N: usize> Table<N> {
    pub(crate) const fn __new(groups: [&'static [Entry]; N]) -> Self {
        Self { groups }
    }

    pub fn find(
        &'static self,
        pool_index: &(dyn Fn() -> Option<usize> + '_),
        value: &dyn for<'a> Fn(&'a Parser<'_>, Entry) -> &'a [u8],
        parser: &Parser<'_>,
        compare: &(dyn Fn(&[u8], u128) -> bool + '_),
    ) -> Option<TokenKind> {
        let groups = self.groups.get(pool_index()?).copied()?;

        groups
            .iter()
            .find_map(|entry| compare(value(parser, *entry), entry.value).then(|| entry.kind))
    }
}

macro_rules! __table_impl {
    (
        $([
            $($base:literal $(pf $(| $postfix:literal)*)? => $kind:ident),* $(,)?
        ]),* $(,)?
    ) => {{
        use $crate::TokenKind::*;

        $crate::lookup_table::Table::__new([
            $(&[
                $($($(
                    $crate::lookup_table::Entry::__new(
                        ::core::concat!($base, $postfix).as_bytes(),
                        $kind
                    )
                ),*)?),*
            ]),*
        ])
    }};
}

pub(crate) use __table_impl as table;

pub(crate) const fn encode(v: &[u8]) -> u128 {
    if mem::size_of::<usize>() == 8 {
        unsafe { __encode_warm(v.as_ptr(), v.len()) }
    } else {
        unsafe { __encode_cold(v.as_ptr(), v.len()) }
    }
}

const unsafe fn __encode_warm(ptr: *const u8, len: usize) -> u128 {
    unsafe {
        if len > 8 {
            let x0 = ptr.cast::<u64>().read_unaligned() as u128;
            let x1 = ptr.add(len - 8).cast::<u64>().read_unaligned() as u128;
            x0 | (x1 << ((len - 8) * 8))
        } else if len > 3 {
            let x0 = ptr.cast::<u32>().read_unaligned() as u128;
            let x1 = ptr.add(len - 4).cast::<u32>().read_unaligned() as u128;
            x0 | (x1 << ((len - 4) * 8))
        } else if len > 0 {
            let x0 = ptr.read() as u128;
            let x1 = ptr.add(len / 2).read() as u128;
            let x2 = ptr.add(len - 1).read() as u128;
            x0 | x1 << (len / 2 * 8) | x2 << ((len - 1) * 8)
        } else {
            0u128
        }
    }
}

#[cold]
const unsafe fn __encode_cold(ptr: *const u8, len: usize) -> u128 {
    let mut buf = [0u8; mem::size_of::<u128>()];

    let mut i = 0;
    while i < len {
        buf[i] = unsafe { ptr.add(i).read() };
        i += 1;
    }

    u128::from_ne_bytes(buf)
}

trait Transform {
    fn transform<R>(self, f: impl FnOnce(Self) -> R) -> R
    where
        Self: Sized,
    {
        f(self)
    }
}

impl<T> Transform for T {}
