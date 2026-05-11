#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), allow(unused_variables))]

#[cfg(test)]
extern crate std;

use core::marker::PhantomData;
use core::slice;

use betac_token::{Token, TokenKind};

use crate::{buf::IntegerBuffer, map::Encoded};

pub use crate::error::Error;

pub mod backend;
pub mod error;
pub mod map;

pub mod buf;

#[macro_export]
macro_rules! test_println {
    ($($arg:tt)*) => {{
        #[cfg(test)]
        {
            ::std::println!($($arg)*)
        }
    }};
}

macro_rules! e {
    ($v:literal, $k:ident) => {
        (
            Encoded::from_str($v).unwrap(),
            TokenKind::$k,
            ($v).len() as u8,
        )
    };
}

macro_rules! smt {
    ($v:literal => $k:ident) => {
        (
            Encoded::from_str($v).unwrap(),
            TokenKind::$k,
            ($v).len() as u8,
        )
    };
}

static MACRO_MAP: [(Encoded, TokenKind, u8); 15] = [
    smt!("Main " => MainMacro),
    smt!("Doc " => DocMacro),
    smt!("Doc[" => DocMacro),
    e!("Intrinsic ", IntrinsicMacro),
    e!("Intrinsic[", IntrinsicMacro),
    e!("Inline ", InlineMacro),
    e!("Inline[", InlineMacro),
    e!("!UseStd;", UseStdMacro),
    e!("Extend[", ExtendMacro),
    e!("Extend ", ExtendMacro),
    e!("Error ", ErrorMacro),
    e!("Hidden", HiddenMacro),
    e!("Lang[", LangMacro),
    e!("Lang ", LangMacro),
    e!("defun ", OtherMacro),
];

static MULTI_CHAR_MAP: &[(&str, TokenKind)] = {
    use TokenKind::*;

    &[
        ("=>", BindOp),
        ("==", EqEq),
        ("->", Arrow),
        ("<=", LtEq),
        (">=", GtEq),
        ("||", PipePipe),
        ("&&", AmpAmp),
        ("-=", NegEq),
        ("*=", MulEq),
        ("/=", DivEq),
        ("&=", AndEq),
        ("%=", RemEq),
        ("+=", AddEq),
        ("|=", OrEq),
        ("^=", XorEq),
        ("::", Path),
    ]
};

pub struct TokenStream<'src, B> {
    src_ptr: *const u8,
    src_len: u32,
    cursor: u32,
    _marker1: PhantomData<&'src [u8]>,
    _marker2: PhantomData<B>,
}

unsafe impl<B> Send for TokenStream<'_, B> {}
unsafe impl<B> Sync for TokenStream<'_, B> {}

pub struct SourceTooLong(());

impl<'src, B> TokenStream<'src, B> {
    pub const fn new(source: &'src [u8]) -> Result<Self, SourceTooLong> {
        if source.len() > Token::MAX_OFFSET as usize {
            Err(SourceTooLong(()))
        } else {
            Ok(Self {
                src_ptr: source.as_ptr(),
                src_len: source.len() as u32,
                cursor: 0,
                _marker1: PhantomData,
                _marker2: PhantomData,
            })
        }
    }

    pub const fn as_slice(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.src_ptr, self.src_len as usize) }
    }

    pub const fn as_src_slice(&self) -> &'src [u8] {
        // SAFETY: this is save because it came from a slice with the lifetime of `'src`
        unsafe { slice::from_raw_parts(self.src_ptr, self.src_len as usize) }
    }

    const fn advance(&mut self, n: u32) {
        self.cursor += n;
    }

    const fn bump(&mut self) -> Option<char> {
        if self.cursor <= self.src_len {
            Some(unsafe { self.bump_unchecked() })
        } else {
            None
        }
    }

    const unsafe fn peek_unchecked(&self) -> char {
        let next = unsafe { self.src_ptr.add((self.cursor + 1) as _).read() };
        next as char
    }

    #[allow(clippy::int_plus_one)]
    const fn peek(&self) -> Option<char> {
        if self.cursor + 1 <= self.src_len {
            Some(unsafe { self.peek_unchecked() })
        } else {
            None
        }
    }

    const unsafe fn bump_unchecked(&mut self) -> char {
        let next = unsafe { self.src_ptr.add(self.cursor as usize).read() };
        self.cursor += 1;
        next as char
    }

    pub fn next_token<A>(
        &mut self,
        spanner: &mut (dyn Spanner + '_),
        lit_interner: &mut (dyn LitInterner<A> + '_),
        alloc: &A,
    ) -> Result<Token, Error> {
        let beginning = self.cursor;
        let mut buf = None;

        let kind = loop {
            let Some(next) = self.bump() else {
                return Ok(Token::eof(self.cursor).expect("lmao"));
            };

            spanner.emit_new_column();

            let kind = match next {
                ' ' => continue,
                '\n' => {
                    spanner.emit_new_line();
                    continue;
                }
                c if can_be_multi_char(c) => unsafe { self.handle_maybe_multi_char(c)? },
                c if c.is_ascii_alphabetic() || c == '_' => self.handle_ident_or_keyword()?,
                c if c.is_numeric() => match self.handle_number_literal(c) {
                    Ok((v, b)) => {
                        buf = Some(b);
                        v
                    }
                    Err(e) => return Err(e),
                },
                _ => todo!(),
            };

            break kind;
        };

        let token = unsafe { Token::from_raw_parts_unchecked(kind, beginning, self.cursor) };

        if let Some(buf) = buf {
            lit_interner.push_buf(buf, token, alloc);
        }

        Ok(token)
    }

    unsafe fn handle_maybe_multi_char(&mut self, first: char) -> Result<TokenKind, Error> {
        let Some(next) = self.peek() else {
            return unsafe { Ok(TokenKind::from_byte_unchecked(first as u8)) };
        };

        let s = [first as u8, next as u8];
        let s = unsafe { str::from_utf8_unchecked(&s) };

        // SAFETY: this is safe because the `peek` call earlier
        unsafe { self.bump_unchecked() };

        MULTI_CHAR_MAP
            .iter()
            .copied()
            .find_map(|(sm, kind)| if sm == s { Some(kind) } else { None })
            .ok_or(crate::Error::InvalidMultiChar)
    }

    fn handle_ident_or_keyword(&mut self) -> Result<TokenKind, Error> {
        todo!()
    }

    fn handle_number_literal(&mut self, first: char) -> Result<(TokenKind, IntegerBuffer), Error> {
        let can_be_non_base_ten = first == '0';

        let Some(next) = self.peek() else {
            return Ok((TokenKind::ILit, IntegerBuffer::new()));
        };

        match next {
            'x' | 'X' if can_be_non_base_ten => self.handle_definite_hexadecimal(),
            'b' if can_be_non_base_ten => self.handle_definite_binary(),
            'o' if can_be_non_base_ten => self.handle_definite_octal(),
            c if c.is_ascii_digit() => self.handle_base_ten(),
            _ => return Err(Error::InvalidLiteral),
        }
    }

    fn handle_definite_hexadecimal(&mut self) -> Result<(TokenKind, IntegerBuffer), Error> {
        self.handle_numeric_inner(
            |c| c.is_ascii_hexdigit(),
            |buf, postfix| {
                if buf.is_empty() {
                    Err(Error::InvalidLiteral)
                } else {
                    Ok(postfix
                        .map(|p| p.to_token_kind())
                        .unwrap_or(TokenKind::ILit))
                }
            },
        )
    }

    fn handle_definite_octal(&mut self) -> Result<(TokenKind, IntegerBuffer), Error> {
        self.handle_numeric_inner(
            |c| matches!(c, '0'..='7'),
            |buf, postfix| {
                if buf.is_empty() {
                    Err(Error::InvalidLiteral)
                } else {
                    Ok(postfix
                        .map(|p| p.to_token_kind())
                        .unwrap_or(TokenKind::ILit))
                }
            },
        )
    }

    fn handle_definite_binary(&mut self) -> Result<(TokenKind, IntegerBuffer), Error> {
        todo!()
    }

    fn handle_base_ten(&mut self) -> Result<(TokenKind, IntegerBuffer), Error> {
        todo!()
    }

    // TODO: literals with a lot of underscores would cause a panic
    fn handle_numeric_inner(
        &mut self,
        mut should_continue: impl FnMut(char) -> bool,
        mut reached_end_of_lit: impl FnMut(
            &mut buf::IntegerBuffer,
            Option<Postfix>,
        ) -> Result<TokenKind, Error>,
    ) -> Result<(TokenKind, IntegerBuffer), Error> {
        self.bump();
        let mut buf = buf::IntegerBuffer::new();

        let mut postfix = None;

        loop {
            let Some(next) = self.bump() else {
                return reached_end_of_lit(&mut buf, postfix).map(|v| (v, buf));
            };

            let should_continue = next == '_'
                || should_continue(next)
                || !is_seperator(next)
                || next == 'u'
                || next == 'i'
                || next == 'f';

            if !should_continue {
                return reached_end_of_lit(&mut buf, postfix).map(|v| (v, buf));
            }

            buf.push(next);

            if next == 'u' || next == 'i' || next == 'f' {
                postfix = self.start_handle_integer_postfix()?;
            }
        }
    }

    fn start_handle_integer_postfix(&mut self) -> Result<Option<Postfix>, Error> {
        todo!()
    }
}

#[derive(Clone, Copy)]
pub enum Postfix {
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
}

impl Postfix {
    const fn to_token_kind(self) -> TokenKind {
        todo!()
    }
}

pub const fn is_seperator(c: char) -> bool {
    matches!(
        c,
        ' ' | ';' | '\n' | ')' | ']' | '+' | '-' | '/' | '*' | '&'
    )
}

pub trait Spanner {
    fn emit_new_column(&mut self);
    fn emit_new_line(&mut self);

    fn line(&self) -> u32;
    fn column(&self) -> u32;
}

pub trait LitInterner<A> {
    fn push_buf(&mut self, b: buf::IntegerBuffer, token: Token, alloc: &A);

    fn get_buf(&self, t: Token) -> Option<&buf::IntegerBuffer>;
}

pub struct NoopSpanner;

impl Spanner for NoopSpanner {
    fn emit_new_column(&mut self) {}
    fn emit_new_line(&mut self) {}
    fn column(&self) -> u32 {
        0
    }

    fn line(&self) -> u32 {
        0
    }
}

const fn can_be_multi_char(c: char) -> bool {
    matches!(
        c,
        '=' | '-' | '<' | '>' | '|' | '&' | '*' | '/' | '%' | '+' | '^' | ':'
    )
}
