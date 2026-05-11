use core::fmt;

use allocator_api::{Allocator, Deinit, collections::Vec};
use betac_yarn::Yarn;

#[derive(Debug)]
pub enum Error {
    Other,
    Unexpected {
        found: Option<betac_token::TokenKind>,
        expected: betac_token::TokenKind,
    },
    Tokenizer(betac_tokenizer::Error),
    UnresolvedSymbol {
        sym: Yarn<'static>,
    },
    InvalidId,
    Exausted,
}

impl From<crate::context::Exhausted> for Error {
    fn from(_: crate::context::Exhausted) -> Self {
        Self::Exausted
    }
}

impl From<betac_tokenizer::Error> for Error {
    fn from(value: betac_tokenizer::Error) -> Self {
        Self::Tokenizer(value)
    }
}

pub struct ReportedError {
    _err: Error,
    __backtrace: crate::bindings::Backtrace,
}

pub struct Reporter<A> {
    errors: Vec<ReportedError, A>,
}

impl<A> Reporter<A> {
    pub const fn new() -> Self {
        Self { errors: Vec::new() }
    }
}

impl<A: Allocator> Reporter<A> {
    pub fn emit(&mut self, error: impl Into<Error>, alloc: &A) {
        self.errors.push(
            ReportedError {
                _err: error.into(),
                __backtrace: crate::bindings::capture_backtrace(),
            },
            alloc,
        );
    }

    pub fn report(&mut self, _writer: &mut dyn fmt::Write) -> fmt::Result {
        todo!()
    }
}

unsafe impl<A: Allocator> Deinit<A> for Reporter<A> {
    unsafe fn deinit(&mut self, allocator: &A) {
        unsafe {
            self.errors.deinit(allocator);
        }
    }
}
