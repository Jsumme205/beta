use core::{convert::Infallible, marker::PhantomData};

use allocator_api::Allocator;
use betac_yarn::Yarn;

use crate::{
    context::{AST, Interner},
    error::Error,
    node::NodeId,
    span::{Span, Spanned},
    ty::Type,
};

pub trait Place<'src> {
    type Output;

    fn from_symbol<A: Allocator>(
        sym: Yarn<'src>,
        span: impl Spanned,
        interner: &mut Interner<'src, A>,
        ast: &mut AST<'src, A>,
        alloc: &A,
    ) -> Result<Self::Output, Error>;
}

pub enum TyPlace<'src> {
    #[doc(hidden)]
    __Capture {
        __marker: PhantomData<&'src ()>,
        __never: Infallible,
    },
}

pub enum LetPlace<'src> {
    #[doc(hidden)]
    __Capture {
        __marker: PhantomData<&'src ()>,
        __never: Infallible,
    },
}

impl<'src> Place<'src> for TyPlace<'src> {
    type Output = (Type, u32);

    fn from_symbol<A: Allocator>(
        sym: Yarn<'src>,
        span: impl Spanned,
        interner: &mut Interner<'src, A>,
        ast: &mut AST<'src, A>,
        alloc: &A,
    ) -> Result<Self::Output, Error> {
        todo!()
    }
}

impl<'src> Place<'src> for LetPlace<'src> {
    type Output = NodeId;

    fn from_symbol<A: Allocator>(
        sym: Yarn<'src>,
        span: impl Spanned,
        _interner: &mut Interner<'src, A>,
        ast: &mut AST<'src, A>,
        alloc: &A,
    ) -> Result<Self::Output, Error> {
        let (start, end) = span.to_start_end_bounds();
        Ok(ast.node((sym, Span { start, end }), alloc)?)
    }
}
