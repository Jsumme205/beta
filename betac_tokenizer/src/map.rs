use core::{mem, ptr, slice};

use betac_token::TokenKind;

use crate::test_println;

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Encoded(pub(crate) u128);

impl Encoded {
    pub const EMPTY: Self = unsafe { Self::new_unchecked(b"") };

    pub const fn new(value: &[u8]) -> Option<Self> {
        if value.len() > mem::size_of::<u128>() {
            None
        } else {
            Some(unsafe { Self::new_unchecked(value) })
        }
    }

    pub const fn from_str(s: &str) -> Option<Self> {
        Self::new(s.as_bytes())
    }

    pub const unsafe fn reconstruct(&'static self, len: u8) -> &'static str {
        unsafe {
            str::from_utf8_unchecked(slice::from_raw_parts(
                &raw const *self as *const u8,
                len as usize,
            ))
        }
    }

    /// # Safety
    /// `len` must be less than or equal to the size of a `u128`
    pub const unsafe fn new_unchecked(value: &[u8]) -> Self {
        unsafe { Self::from_raw_parts(value.as_ptr(), value.len()) }
    }

    /// # Safety
    /// `ptr` must point to a valid byte slice
    ///
    /// `len` must be the length of that string slice
    ///
    /// `len` must be less than or equal to the size of a `u128`
    pub const unsafe fn from_raw_parts(ptr: *const u8, len: usize) -> Self {
        unsafe {
            core::hint::assert_unchecked(len <= mem::size_of::<u128>());
        }

        #[cfg_attr(target_pointer_width = "64", inline)]
        const unsafe fn __encode64(ptr: *const u8, len: usize) -> u128 {
            unsafe {
                if len > 8 {
                    let x0 = ptr.cast::<u64>().read_unaligned() as u128;
                    let x1 = ptr.add(len.unchecked_sub(8)).cast::<u32>().read_unaligned() as u128;
                    x0 | x1 << len.unchecked_sub(8).unchecked_mul(8)
                } else if len > 3 {
                    let x0 = ptr.cast::<u32>().read_unaligned() as u128;
                    let x1 = ptr.add(len.unchecked_sub(4)).cast::<u32>().read_unaligned() as u128;
                    x0 | (x1 << len.unchecked_sub(4).unchecked_mul(8))
                } else if len > 0 {
                    let x0 = ptr.read() as u128;
                    let x1 = ptr.add(len.checked_div(2).unwrap_unchecked()).read() as u128;
                    let x2 = ptr.add(len.unchecked_sub(1)).read() as u128;

                    x0 | x1 << (len.checked_div(2).unwrap_unchecked().unchecked_mul(8))
                        | x2 << (len.unchecked_sub(1).unchecked_mul(8))
                } else {
                    0
                }
            }
        }

        #[cfg_attr(not(target_pointer_width = "64"), inline)]
        const unsafe fn __encode32(ptr: *const u8, len: usize) -> u128 {
            let mut buf: [u8; _] = [0u8; mem::size_of::<u128>()];

            unsafe {
                ptr::copy_nonoverlapping(ptr, buf.as_mut_ptr(), len);
            }
            u128::from_ne_bytes(buf)
        }

        let v = unsafe {
            if mem::size_of::<usize>() == 8 {
                __encode64(ptr, len)
            } else {
                __encode32(ptr, len)
            }
        };

        Self(v)
    }
}

pub struct Set<T: 'static, const N: usize> {
    pub(crate) groups: [&'static [T]; N],
}

impl<T: 'static, const N: usize> Set<T, N> {
    fn borrow(&self) -> BorrowedSet<'_, T> {
        BorrowedSet {
            groups: &self.groups,
        }
    }
}

pub struct BorrowedSet<'a, T: 'static> {
    pub(crate) groups: &'a [&'static [T]],
}

impl<T: 'static> Copy for BorrowedSet<'_, T> {}
impl<T: 'static> Clone for BorrowedSet<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

pub struct TokenSet<const N: usize> {
    pub(crate) encoded: Set<Encoded, N>,
    pub(crate) lengths: Set<u8, N>,
    pub(crate) kinds: Set<betac_token::TokenKind, N>,
}

impl<const N: usize> TokenSet<N> {
    pub fn borrow(&self) -> BorrowwedTokenSet<'_> {
        BorrowwedTokenSet {
            encoded: self.encoded.borrow(),
            lengths: self.lengths.borrow(),
            kinds: self.kinds.borrow(),
        }
    }
}

pub struct BorrowwedTokenSet<'a> {
    encoded: BorrowedSet<'a, Encoded>,
    lengths: BorrowedSet<'a, u8>,
    kinds: BorrowedSet<'a, betac_token::TokenKind>,
}

#[macro_export]
macro_rules! set {
    (
        // one set
        $([

            $($base:literal $(| $pf:literal)* => $Kind:ident),* $(,)?
        ]),* $(,)?
    ) => {
        $crate::map::TokenSet {
            encoded: $crate::map::Set {
                groups: [
                    $(&[
                        $($($crate::map::Encoded::new((::core::concat!($base, $pf)).as_bytes()).expect("lmao")),*),*
                    ]),*
                ]
            },
            lengths: $crate::map::Set {

                groups: [
                    $(&[
                        $($((::core::concat!($base, $pf)).len() as u8),*),*
                    ]),*
                ]
            },
            kinds: $crate::map::Set {
                groups: [
                    $(&[
                        $($({
                            let _ = $pf;
                            ::betac_token::TokenKind::$Kind
                        }),*),*
                    ]),*
                ]
            },
        }
    };
}

pub(crate) const KEYWORD_SET: TokenSet<16> = crate::set![
    [
        "ny" | ' ' | ')' | ',' | ']' | '>' | '{' | ';' => Any,
        "sync" | ' ' => AsyncKw,
        "wait" | ';' | ' ' | '\n' | ',' | '.' => AwaitKw,
    ],
    [
        "ool" | ' ' | ')' | ',' | ']' | '>' | '{' | ';' => Bool,
    ],
    [
        "ase" | ':' | ' ' => CaseKw,
        "har" | ' ' | ')' | ',' | ']' => Char,
        "omponent" | ' ' | '{' => ComponentKw,
        "onstexpr" | ' ' | '{' => ConstexprKw
    ],
    [
        "efun" | ' ' | '(' | '[' => DefunKw,
        "efault" | ' ' | ':' => DefaultKw,
    ],
    [
        "num" | ' ' | '{' => EnumKw,
        "xtend" | ' ' | ':' => Extend,
        "xtends" | ' ' => ExtendsKw,
        "xtern" | '"' | ' ' => ExternKw,
    ],
    [
        "loat32" | ' ' | ')' | ',' | ']' | '>' | ';' | '{' | '?' | '!' => Float32,
        "loat64" | ' ' | ')' | ',' | ']' | '>' | ';' | '{' | '?' | '!' => Float64,
        "or" | '(' | '{' | ' ' => ForKw,
        "alse" | ')' | ' ' | ';' => FalseKw
    ],
    [
        "f"     | '(' | ' ' => IfKw,
        "mport" | ' ' => ImportKw,
        "nt8"   | ' ' | ')' | ',' | ']' | '>' | ';' | '{' | '?' | '!' => Int8,
        "nt16"  | ' ' | ')' | ',' | ']' | '>' | ';' | '{' | '?' | '!' => Int16,
        "nt32"  | ' ' | ')' | ',' | ']' | '>' | ';' | '{' | '?' | '!' => Int32,
        "nt64"  | ' ' | ')' | ',' | ']' | '>' | ';' | '{' | '?' | '!' => Int64,
        "size"  | ' ' | ')' | ',' | ']' | '>' | ';' | '{' | '?' | '!' => Isize,
    ],
    [
        "et"    | ' ' => LetKw,
        "oop"   | ' ' | '{' => LoopKw,
    ],
    [
        "ove" | ' ' => MoveKw,
        "ut" | ' ' => MutKw,
    ],
    [
        "bj" | ' '  | '{' => ObjKw,
    ],
    [
        "ack" | ' ' | ')' => PackKw,
        "ub" | ' ' | '(' => PubKw,
    ],
    [
        "tr" | ' ' | ')' | ',' | ']' | '>' | ';' | '{' => Str,
        "witch" | ' ' | '(' => SwitchKw,
    ],
    [
        "his" | ' ' | ')' | ',' | ']' | '>' | ';' | '{' => ThisTy,
    ],
    [
        "his" | ' ' | ')' | '.' | ',' | ';' => ThisVal,
        "rait" | ' ' => TraitKw,
        "rue" | ' ' | ')' | '.' | ',' | ';' => TrueKw,
    ],
    [
        "int8"   | ' ' | ')' | ',' | ']' | '>' | ';' | '{' | '?' | '!' => Uint8,
        "int16"  | ' ' | ')' | ',' | ']' | '>' | ';' | '{' | '?' | '!' => Uint16,
        "int32"  | ' ' | ')' | ',' | ']' | '>' | ';' | '{' | '?' | '!' => Uint32,
        "int64"  | ' ' | ')' | ',' | ']' | '>' | ';' | '{' | '?' | '!' => Uint64,
        "size"   | ' ' | ')' | ',' | ']' | '>' | ';' | '{' | '?' | '!' => Usize,
        "nion" | ' ' | '{' => UnionKw,
        "nsafe" | ' ' | '{' => UnsafeKw,
    ],
    [
        "hile" | '{' | '(' | ' ' => WhileKw,
    ],
];

const fn index(c: char) -> Option<usize> {
    match c {
        'a' => Some(0),
        'b' => Some(1),
        'c' => Some(2),
        'd' => Some(3),
        'e' => Some(4),
        'f' => Some(5),
        'i' => Some(6),
        'l' => Some(7),
        'm' => Some(8),
        'o' => Some(9),
        'p' => Some(10),
        's' => Some(11),
        'T' => Some(12),
        't' => Some(13),
        'u' => Some(14),
        'w' => Some(15),
        _ => None,
    }
}

pub fn matches(c: char, source: &[u8]) -> Option<(TokenKind, &'static str)> {
    let TokenSet {
        encoded,
        lengths,
        kinds,
    } = &KEYWORD_SET;

    test_println!("FIRST: {c}");

    let index = index(c)?;

    let encoded = *encoded.groups.get(index)?;
    let lengths = *lengths.groups.get(index)?;
    let kinds = *kinds.groups.get(index)?;

    let ptr = source.as_ptr();
    let src_len = source.len();

    for ((encoded, &len), &kind) in encoded.iter().zip(lengths).zip(kinds) {
        if (len as usize) > src_len {
            continue;
        }

        let source_encoded = unsafe { Encoded::from_raw_parts(ptr, len as usize) };

        if unlikely(*encoded == source_encoded) {
            return Some((kind, unsafe { encoded.reconstruct(len) }));
        }
    }

    None
}

#[cold]
fn __cold() {}

fn unlikely(b: bool) -> bool {
    if b {
        __cold();
    }
    b
}
