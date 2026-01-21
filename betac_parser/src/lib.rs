use core::slice;
use std::{
    mem::{self, MaybeUninit},
    ops::Index,
};

use crate::lookup_table::Entry;

pub(crate) mod keyword;
pub(crate) mod lookup_table;

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TokenKind {
    Null = b'\0',
    Esc = 0x1B,
    Space = b' ',
    Exclaim = b'!',
    Quotation = b'"',
    NumSign = b'#',
    Dollar = b'$',
    Rem = b'%',
    Amp = b'&',
    SingleQ = b'\'',
    LParen = b'(',
    RParen = b')',
    Star = b'*',
    Add = b'+',
    Comma = b',',
    Sub = b'-',
    Dot = b'.',
    Slash = b'/',
    Colon = b':',
    Semicolon = b';',
    Gt = b'>',
    Question = b'?',
    At = b'@',
    LBracket = b'[',
    Backslash = b'\\',
    RBracket = b']',
    Carrot = b'^',
    Underscore = b'_',
    Tick = b'`',
    LBrace = b'{',
    Pipe = b'|',
    RBrace = b'}',
    Tilde = b'~',

    Bind,
    EqEq,
    SkinnyArrow,
    LtEq,
    GtEq,
    PipePipe,
    AndAnd,
    SubEq,
    MulEq,
    DivEq,
    AndEq,
    RemEq,
    AddEq,
    OrEq,
    XorEq,
    Path,

    Import,
    Defun,
    Component,
    Obj,
    Union,
    Enum,
    Extends,
    Extend,
    ThisVar,
    ThisTy,
    Pub,
    Priv,
    LetKw,
    ForKw,
    IfKw,
    WhileKw,
    LoopKw,
    SwitchKw,
    CaseKw,
    ExternKw,
    MoveKw,
    MutKw,
    PackKw,

    MainMacro,
    DocMacro,
    IntrinsicMacro,
    InlineMacro,
    UseStdMacro,
    ExtendMacro,
    ErrorMacro,
    HiddenMacro,
    LangMacro,
    AtDefun,

    Int8,
    Int16,
    Int32,
    Int64,
    Uint8,
    Uint16,
    Uint32,
    Uint64,
    Usize,
    Isize,
    Float32,
    Float64,

    Any,
    Str,
    Bool,
    Char,

    LineComment,
    BlockComment,

    Eof,
    Unknown,
}

impl TokenKind {
    pub(crate) const unsafe fn from_byte_unchecked(b: u8) -> Self {
        unsafe { mem::transmute(b) }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Token {
    pub kind: TokenKind,
    hi: u16,
    lo: u16,
}

impl Token {
    pub const fn offset(self) -> u32 {
        unsafe {
            (&raw const self as *const u8)
                .add(1)
                .cast::<u32>()
                .read_unaligned()
        }
    }

    pub const fn new(kind: TokenKind, offset: u32) -> Self {
        let mut this = MaybeUninit::<Self>::uninit();
        unsafe {
            this.as_mut_ptr()
                .byte_add(1)
                .cast::<u32>()
                .write_unaligned(offset);
            this.as_mut_ptr().cast::<TokenKind>().write(kind);

            this.assume_init()
        }
    }

    pub const fn eof(pos: u32) -> Self {
        Self::new(TokenKind::Eof, pos)
    }
}

#[derive(Clone)]
struct Iter<'sess> {
    inner: slice::Iter<'sess, u8>,
}

impl<'sess> Iter<'sess> {
    fn as_slice(&self) -> &[u8] {
        self.inner.as_slice()
    }
}

impl Iterator for Iter<'_> {
    type Item = u8;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().copied()
    }
}

pub struct Parser<'sess> {
    inner: Iter<'sess>,
    pos: u32,
}

impl Index<Entry> for Parser<'_> {
    type Output = [u8];

    fn index(&self, index: Entry) -> &Self::Output {
        &self.as_slice()[..index.len as usize]
    }
}

const NULL: u8 = 0x00;

impl<'sess> Parser<'sess> {
    pub fn new(src: &'sess [u8]) -> Self {
        Self {
            inner: Iter { inner: src.iter() },
            pos: 0,
        }
    }

    pub fn bump(&mut self) -> Option<u8> {
        let v = self.inner.next()?;
        self.pos += 1;
        Some(v)
    }

    pub fn as_slice(&self) -> &[u8] {
        self.inner.as_slice()
    }

    pub fn slice(&self, to: usize) -> &[u8] {
        &self.as_slice()[..to]
    }

    pub fn next(&self) -> u8 {
        let mut iter = self.inner.clone();
        iter.next().unwrap_or(NULL)
    }
}

impl Parser<'_> {
    pub fn next_token(&mut self) -> Token {
        let Some(next) = self.bump() else {
            return Token::eof(self.pos);
        };

        let pos = self.pos;

        let kind = match next {
            byte if can_be_multi_char(byte) => match self.handle_maybe_multi_char() {
                Some(token) => token,
                None => unsafe { TokenKind::from_byte_unchecked(byte) },
            },
            byte if byte.is_ascii_alphabetic() || byte == b'_' => {
                match keyword::matches(byte, self) {
                    Some(token) => token,
                    None => self.handle_ident(),
                }
            }
            b'@' => {
                let next = self.next();

                match self.handle_macro() {
                    Some(kind) => kind,
                    None if next == b' ' => TokenKind::At,
                    None => TokenKind::Unknown,
                }
            }
            //byte
            b => {
                debug_byte(b);
                TokenKind::Unknown
            }
        };

        Token::new(kind, pos)
    }

    fn handle_maybe_multi_char(&mut self) -> Option<TokenKind> {
        None
    }

    fn handle_macro(&mut self) -> Option<TokenKind> {
        None
    }

    fn handle_ident(&mut self) -> TokenKind {
        todo!()
    }
}

const fn can_be_multi_char(b: u8) -> bool {
    matches!(
        b,
        b'=' | b'-' | b'<' | b'>' | b'|' | b'&' | b'*' | b'/' | b'%' | b':'
    )
}

fn debug_byte(b: u8) {
    if b == b' ' {
        println!("caught: <space>")
    } else if b == b'\0' {
        println!("caught: <null>")
    } else {
        println!("caught: {}", b as char)
    }
}
