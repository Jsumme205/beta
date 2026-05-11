use core::mem;

use crate::map::Encoded;

mod __sealed {
    pub trait Sealed {}
}

pub trait Array: __sealed::Sealed {
    type Element;

    fn broadcast(e: Self::Element) -> Self;

    #[doc(hidden)]
    unsafe fn get_unchecked(&self, idx: usize) -> *const Self::Element;
}

impl<T: Clone, const N: usize> __sealed::Sealed for [T; N] {}
impl<T: Clone, const N: usize> Array for [T; N] {
    type Element = T;

    fn broadcast(e: Self::Element) -> Self {
        core::array::repeat(e)
    }

    unsafe fn get_unchecked(&self, idx: usize) -> *const Self::Element {
        // We could do `get_unchecked` here but that would cause more codegen
        unsafe { self.as_ptr().add(idx) }
    }
}

fn get(src: &[u8], lens: [u8; 2]) -> [Option<Encoded>; 2] {
    lens.map(|len| {
        src.get(..(len as usize))
            .and_then(|v| unsafe { crate::map::Encoded::new(mem::transmute(v)) })
    })
}

pub unsafe trait Backend {
    type Register;
    type EltArray: Copy;
    type Mask;
    type Lens: Array<Element = u8>;

    #[doc(hidden)]
    type SrcElt;

    const N_ELEMENTS: usize;

    #[doc(hidden)]
    unsafe fn load(element_ptr: *const Self::EltArray) -> Self::Register;

    #[doc(hidden)]
    fn cmp(lhs: Self::Register, rhs: Self::Register) -> Self::Mask;

    #[doc(hidden)]
    fn index_from_mask(chunk_index: usize, mask: Self::Mask) -> Option<usize>;

    #[doc(hidden)]
    fn src_idx(source: &[u8], lens: &Self::Lens) -> Self::SrcElt {
        // father forgive me, for I have sinned

        let e_size = mem::size_of::<Self::SrcElt>() / Self::N_ELEMENTS;

        let mut dst = mem::MaybeUninit::<Self::SrcElt>::uninit();

        unsafe {
            for i in 0..Self::N_ELEMENTS {
                let slot = dst.as_mut_ptr().byte_add(i * e_size) as *mut Option<Encoded>;
                let len = lens.get_unchecked(i).read() as usize;
                let source = source.get(..len).and_then(Encoded::new);
                slot.write(source);
            }

            dst.assume_init()
        }
    }
}

#[macro_export]
macro_rules! define_backend {
    (
        unsafe;
        name = $BackendName:ident;
        array = [$ety:ty; $en:expr];
        register = $register:ty;
        mask = $maskty:ty;

        load = unsafe fn($eptr:ident) unsafe $lblock:block;
        cmp = fn($lhs:ident, $rhs:ident) unsafe $cblock:block;
        index_from_mask = fn($idx:ident, $mask:ident) $iblock:block;
    ) => {
        const _: () = {
            const _ASSERT_EN_IS_USIZE: usize = $en;
        };

        pub enum $BackendName {}

        unsafe impl $crate::backend::Backend for $BackendName {
            type Register = $register;
            type EltArray = [$ety; $en];
            type Lens = [u8; $en];
            type Mask = $maskty;
            type SrcElt = [Option<$crate::map::Encoded>; $en];

            const N_ELEMENTS: usize = $en;

            unsafe fn load(element_ptr: *const Self::EltArray) -> Self::Register {
                #[inline(always)]
                unsafe fn __do_load($eptr: *const [$ety; $en]) -> $register {
                    unsafe { $lblock }
                }

                // TODO: switch to let $eptr = element_ptr?

                unsafe { __do_load(element_ptr) }
            }

            fn cmp(lhs: Self::Register, rhs: Self::Register) -> Self::Mask {
                #[inline(always)]
                fn __do_cmp($lhs: $register, $rhs: $register) -> $maskty {
                    unsafe { $cblock }
                }

                __do_cmp(lhs, rhs)
            }

            fn index_from_mask(chunk_index: usize, mask: Self::Mask) -> Option<usize> {
                #[inline(always)]
                fn __do_idx($idx: usize, $mask: $maskty) -> Option<usize> {
                    $iblock
                }

                __do_idx(chunk_index, mask)
            }

            fn src_idx(source: &[u8], lens: &Self::Lens) -> Self::SrcElt {
                lens.map(|len| {
                    source.get(..(len as usize)).and_then(|v| unsafe {
                        $crate::map::Encoded::new(::core::mem::transmute(v))
                    })
                })
            }
        }
    };
}

pub mod avx256 {
    use core::arch::x86_64::__m256i;

    crate::define_backend! {
        unsafe;
        name = AVX256Backend;
        array = [u128; 2];
        register = __m256i;
        mask = i32;

        load = unsafe fn(array) unsafe {
            core::arch::x86_64::_mm256_loadu_si256(array.cast())
        };
        cmp = fn(lhs, rhs) unsafe {
            todo!()
        };
        index_from_mask = fn(idx, mask) {
            todo!()
        };
    }
}
