#![no_std]

use core::{
    fmt::{self},
    hash::Hash,
    mem,
};

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub enum TokenKind {
    Newline = b'\n',
    Space = 32,
    Exclaim = 33,
    DQuote = 34,
    Pound = 35,
    Dollar = 36,
    Percent = 37,
    Ampersand = 38,
    SQuote = 39,
    LParen = 40,
    RParen = 41,
    Star = 42,
    Plus = 43,
    Comma = 44,
    Dash = 45,
    Period = 46,
    Foreslash = 47,
    Colon = 58,
    Semicolon = 59,
    Lt = 60,
    Assign = 61,
    Gt = 62,
    Question = 63,
    AtSign = 64,
    LBracket = 91,
    Backslash = 92,
    RBracket = 93,
    Carrot = 94,
    Underscore = 95,
    Tick = 96,
    LBrace = 123,
    Pipe = 124,
    RBrace = 125,
    Tilde = 126,
    Null = b'\0',

    BindOp,
    EqEq,
    Arrow,
    LtEq,
    GtEq,
    PipePipe,
    AmpAmp,
    NegEq,
    MulEq,
    DivEq = 11,
    AndEq,
    RemEq,
    AddEq,
    OrEq,
    XorEq,
    Path,

    ImportKw,
    DefunKw,
    ComponentKw,
    ObjKw,
    ExtendsKw,
    Extend,
    ThisVal,
    ThisTy,
    PubKw,
    LetKw,
    ForKw,
    IfKw,
    WhileKw,
    LoopKw,
    SwitchKw = 48,
    CaseKw,
    ExternKw,
    MoveKw,
    MutKw,
    PackKw,
    UnionKw,
    EnumKw = 65,
    DefaultKw,
    FalseKw,
    TrueKw,
    AsyncKw,
    AwaitKw,
    TraitKw,
    ConstexprKw,
    UnsafeKw,

    MainMacro,
    DocMacro,
    IntrinsicMacro,
    InlineMacro,
    UseStdMacro,
    ExtendMacro,
    ErrorMacro,
    HiddenMacro,
    LangMacro,
    ExtenderMacro,
    OtherMacro,

    Int8,
    Int16,
    Int32,
    Int64,
    Uint8,
    Uint16,
    Uint32 = 97,
    Uint64,
    Usize,
    Isize,
    Float32,
    Float64,
    Any,
    Str,
    Bool,
    Char,

    ILit,
    SLit,
    CLit,
    Ident,

    ILitI8,
    ILitI16,
    ILitI32,
    ILitI64,
    ILitIsize,

    ILitU8,
    ILitU16,
    ILitU32,
    ILitU64,
    ILitUsize,

    Unknown,
    EOF,
}

#[macro_export]
macro_rules! matches_char {
    ($v:expr) => {
        ::core::matches!(
            $v,
            32..=47 | 58..=64 | 91..=96 | 123..=126
        )
    };
}

impl TokenKind {
    /// # Safety
    ///
    /// musut be in a valid ASCII single-char range
    pub const unsafe fn from_byte_unchecked(b: u8) -> Self {
        debug_assert!(matches_char!(b));

        unsafe { mem::transmute(b) }
    }

    pub const fn from_byte(b: u8) -> Option<Self> {
        if !matches_char!(b) {
            None
        } else {
            Some(unsafe { Self::from_byte_unchecked(b) })
        }
    }

    pub const fn is_literal(self) -> bool {
        use TokenKind::*;
        matches!(
            self,
            SLit | ILit
                | ILitI16
                | ILitI32
                | ILitI64
                | ILitIsize
                | ILitU8
                | ILitU16
                | ILitU32
                | ILitU64
                | ILitUsize
                | CLit
                | TrueKw
                | FalseKw
        )
    }

    pub const fn is_builtin_ty(self) -> bool {
        todo!()
    }

    pub const fn is_binary_op(self) -> bool {
        matches!(
            self,
            TokenKind::AddEq
                | Self::GtEq
                | Self::Lt
                | Self::LtEq
                | Self::EqEq
                | Self::Pipe
                | Self::Ampersand
                | Self::Carrot
                | Self::AmpAmp
                | Self::PipePipe
                | Self::Gt
        )
    }
}

/// a single token within the parser
///
///
/// it is just a wrapper around a `u64`, but logically, the struct looks something like this:
///
/// ```rust
/// struct Token {
///     pub kind: TokenKind,
///     pub start: u28,
///     pub end: u28
/// }
///
/// ```
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct Token(u64);

impl Token {
    pub const MAX_OFFSET: u32 = 2u32.pow(28) - 1;
    const START_SHIFT: u32 = u64::BITS - 7 - 27;

    pub const unsafe fn from_raw_unchecked(raw: u64) -> Self {
        unsafe { mem::transmute(raw) }
    }

    pub const unsafe fn from_raw_parts_unchecked(kind: TokenKind, start: u32, end: u32) -> Self {
        debug_assert!(start <= Self::MAX_OFFSET && end <= Self::MAX_OFFSET);

        unsafe {
            Self::from_raw_unchecked(
                (kind as u8 as u64) << (u64::BITS - 7)
                    | ((start as u64) << Self::START_SHIFT)
                    | end as u64,
            )
        }
    }

    pub const fn try_from_parts(kind: TokenKind, start: u32, end: u32) -> Option<Self> {
        if start > Self::MAX_OFFSET || end > Self::MAX_OFFSET {
            None
        } else {
            Some(unsafe { Self::from_raw_parts_unchecked(kind, start, end) })
        }
    }

    pub const fn eof(start: u32) -> Option<Self> {
        Self::try_from_parts(TokenKind::EOF, start, start + 1)
    }

    pub const fn kind(self) -> TokenKind {
        let raw = (self.0 >> u64::BITS - 7) as u8;
        // SAFETY: due to the
        unsafe { core::mem::transmute(raw) }
    }

    pub const fn start(self) -> u32 {
        ((self.0 >> Self::START_SHIFT) as u32) & 0x1FFF_FFFF
    }

    pub const fn end(self) -> u32 {
        (self.0 as u32) & Self::MAX_OFFSET
    }
}

impl core::fmt::Debug for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let range = core::ops::Range {
            start: self.start(),
            end: self.end(),
        };

        f.debug_struct("Token")
            .field("kind", &self.kind())
            .field("span", &range)
            .finish_non_exhaustive()
    }
}

impl Hash for Token {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.start().hash(state);
        self.end().hash(state);
        self.kind().hash(state);
    }
}

impl PartialEq for Token {
    fn eq(&self, other: &Self) -> bool {
        self.start() == other.start() && self.end() == other.end() && self.kind() == other.kind()
    }
}

impl Eq for Token {}

#[macro_export]
macro_rules! Kind {
    (unsafe) => {
        $crate::TokenKind::UnsafeKw
    };
    (mut) => {
        $crate::TokenKind::MutKw
    };
    (move) => {
        $crate::TokenKind::MoveKw
    };
    (<ident>) => {
        $crate::TokenKind::Ident
    };
    (int8) => {
        $crate::TokenKind::Int8
    };
    (int16) => {
        $crate::TokenKind::Int16
    };
    (int32) => {
        $crate::TokenKind::Int32
    };
    (int64) => {$crate::TokenKind::Int64};
    (isize) => {$crate::TokenKind::Isize};
    (uint8) => {$crate::TokenKind::Uint8};
    (uint16) => {$crate::TokenKind::Uint16};
    (uint32) => {$crate::TokenKind::Uint32};
    (uint64) => {$crate::TokenKind::Uint64};
    (usize) => {$crate::TokenKind::Usize};
    (any) => {$crate::TokenKind::Any};
    (bool) => {$crate::TokenKind::Bool};
    (char) => {$crate::TokenKind::Char};
    (&) => {$crate::TokenKind::Ampersand};
    (.) => {$crate::TokenKind::Period};


    ($($t:tt),*) => {
        &[$($crate::Kind![$t]),*]
    }
}
