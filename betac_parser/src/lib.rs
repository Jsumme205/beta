#![cfg_attr(not(any(test, debug_assertions)), no_std)]

#[cfg(any(test, debug_assertions))]
extern crate std;

pub(crate) mod bindings {
    cfg_select! {
        any(test, debug_assertions) => {
            pub(crate) type Backtrace = std::backtrace::Backtrace;
        },
        _ => {
            pub(crate) struct DummyBacktrace {
                __priv: ()
            }

            impl DummyBacktrace {

                pub fn force_capture() -> Self {
                    Self { __priv: () }
                }
            }

            impl core::fmt::Debug for DummyBacktrace {
                fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                    write!(f, "DummyBacktrace")
                }
            }

            impl core::fmt::Display for DummyBacktrace {
                fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                    Ok(())
                }
            }

            pub(crate) type Backtrace = DummyBacktrace;
        }

    }

    pub fn capture_backtrace() -> self::Backtrace {
        self::Backtrace::force_capture()
    }
}

use core::marker::PhantomData;
use std::ptr;

use allocator_api::Allocator;
use betac_token::{Token, TokenKind};
use betac_tokenizer::TokenStream;
use betac_yarn::Yarn;

use crate::{
    context::{AST, Interner, ParseContext, ParseSpanner, Unresolved},
    error::Error,
    expr::{BinOpKind, Expr, ExprKind, UnaryOpKind},
    node::NodeId,
    place::{LetPlace, Place, TyPlace},
    qual::QualifierBitset,
    span::Span,
    ty::{RefKind, Type},
};

pub mod context;
pub mod error;
pub mod expr;
pub mod item;
pub mod node;
pub mod qual;
pub mod span;
pub mod ty;

pub(crate) mod place;

pub(crate) mod postfix;

macro_rules! ast_fn {
    () => {};
    ($fn_name:ident($self_:ident, $ast_name:ident, $interner_name:ident, $spanner_name:ident, $($alloc:ident)?) -> Result<$T:ty, Error>
        $block:block
    ) => {
        #[allow(unused)]
        fn $fn_name<A>($self_: &mut Self, $ast_name: &mut AST<'src, A>, $interner_name: &mut Interner<'src, A>, $spanner_name: &mut ParseSpanner, $($alloc: &A)?) -> Result<$T, Error>
        where
            A: Allocator
        {
            $block
        }
    };
}

pub struct Parser<'src, B> {
    tk: TokenStream<'src, B>,
}

pub trait Scope<'src> {
    fn properties(immediate_scope: TokenKind) -> ScopeProperties;

    fn add_expr<A: Allocator>(
        &mut self,
        expr: Expr<'src>,
        ast: &mut AST<'src, A>,
        alloc: &A,
    ) -> Result<(), Error>;
}

impl<'src> Scope<'src> for Expr<'src> {
    fn properties(_immediate_scope: TokenKind) -> ScopeProperties {
        ScopeProperties {
            is_let_expr: true,
            requires_explicit_type: false,
            allows_binding_in_expr: true,
        }
    }

    fn add_expr<A: Allocator>(
        &mut self,
        mut expr: Expr<'src>,
        ast: &mut AST<'src, A>,
        alloc: &A,
    ) -> Result<(), Error> {
        match (&mut self.kind, &mut expr.kind) {
            (ExprKind::LetExpr { rhs: rh, .. }, ExprKind::Ref { .. }) => {
                *rh = ast.node(expr, alloc)?;
            }
            _ => todo!(),
        }

        Ok(())
    }
}

pub enum Position {
    TypePosition,
    ExprPosition,
}

#[derive(Clone, Copy)]
pub struct ScopeProperties {
    pub allows_binding_in_expr: bool,
    pub requires_explicit_type: bool,
    pub is_let_expr: bool,
}

impl<'src, B> Parser<'src, B> {
    fn expect<F, A: Allocator>(
        &mut self,
        interner: &mut Interner<'src, A>,
        spanner: &mut ParseSpanner,
        alloc: &A,
        mut pred: F,
    ) -> Result<Token, Error>
    where
        F: FnMut(TokenKind) -> bool,
    {
        let next = self.tk.next_token(spanner, interner.lit(), alloc)?;

        if pred(next.kind()) {
            Ok(next)
        } else {
            Err(Error::Unexpected {
                found: None,
                expected: next.kind(),
            })
        }
    }

    fn span(&self, span: impl span::Spanned) -> Option<Yarn<'src>> {
        let (start, end) = span.to_start_end_bounds();

        self.tk
            .as_src_slice()
            .get(start as usize..=end as usize)
            .map(|src| unsafe { Yarn::from_utf8_unchecked(src) })
    }

    fn pointer<A>(
        &mut self,
        ast: &mut AST<'src, A>,
        interner: &mut Interner<'src, A>,
        spanner: &mut ParseSpanner,
        alloc: &A,
    ) -> Result<(Type, u32), Error>
    where
        A: Allocator,
    {
        self.pointer_or_reference(
            Parser::<'src, B>::pointer_kind::<A>,
            ast,
            interner,
            spanner,
            Position::TypePosition,
            PKind::Pointer,
            alloc,
        )
    }

    fn pointer_kind<A>(
        &mut self,
        interner: &mut Interner<'src, A>,
        spanner: &mut ParseSpanner,
        alloc: &A,
    ) -> Result<(RefKind, u32), Error>
    where
        A: Allocator,
    {
        todo!()
    }

    fn reference_kind<A>(
        &mut self,
        interner: &mut Interner<'src, A>,
        spanner: &mut ParseSpanner,
        alloc: &A,
    ) -> Result<(RefKind, u32), Error>
    where
        A: Allocator,
    {
        todo!()
    }

    fn reference<A>(
        &mut self,
        ast: &mut AST<'src, A>,
        interner: &mut Interner<'src, A>,
        spanner: &mut ParseSpanner,
        alloc: &A,
        position: Position,
    ) -> Result<(Type, u32), Error>
    where
        A: Allocator,
    {
        self.pointer_or_reference(
            Parser::<'src, B>::reference_kind::<A>,
            ast,
            interner,
            spanner,
            position,
            PKind::Reference,
            alloc,
        )
    }

    fn pointer_or_reference<A>(
        &mut self,
        ref_kind: fn(
            &mut Parser<'src, B>,
            &mut Interner<'src, A>,
            &mut ParseSpanner,
            &A,
        ) -> Result<(RefKind, u32), Error>,
        ast: &mut AST<'src, A>,
        interner: &mut Interner<'src, A>,
        spanner: &mut ParseSpanner,
        position: Position,
        kind: PKind,
        alloc: &A,
    ) -> Result<(Type, u32), Error>
    where
        A: Allocator,
    {
        let (ref_kind, _) = ref_kind(self, interner, spanner, alloc)?;
        let next = self.tk.next_token(spanner, interner.lit(), alloc)?;
        let (rest, end) = match next.kind() {
            TokenKind::LBracket => self.slice(ast, interner, spanner, alloc)?,
            TokenKind::Ampersand => self.reference(ast, interner, spanner, alloc, position)?,
            TokenKind::Star => self.pointer(ast, interner, spanner, alloc)?,
            TokenKind::Ident => self.ident::<_, TyPlace<'src>>(next, ast, interner, alloc)?,
            _ => todo!("proper error handling"),
        };

        Ok((kind.resolve_ty(ast.node(rest, alloc)?, ref_kind), end))
    }

    fn builtin(kind: TokenKind) -> Type {
        use TokenKind::*;
        assert!(kind.is_builtin_ty());

        match kind {
            Int8 => Type::Numerical(ty::NKind::Int8),
            Int16 => Type::Numerical(ty::NKind::Int16),
            Int32 => Type::Numerical(ty::NKind::Int32),
            Int64 => Type::Numerical(ty::NKind::Int64),
            Uint8 => Type::Numerical(ty::NKind::Uint8),
            Uint16 => Type::Numerical(ty::NKind::Uint16),
            Uint32 => Type::Numerical(ty::NKind::Uint32),
            Uint64 => Type::Numerical(ty::NKind::Uint64),
            Float32 => Type::Numerical(ty::NKind::Float32),
            Float64 => Type::Numerical(ty::NKind::Float64),
            Any => Type::Any,
            Str => Type::StrLit(ty::StringKind::UTF8),
            Bool => Type::Bool,
            _ => unreachable!(),
        }
    }

    pub fn expr<A>(
        &mut self,
        ast: &mut AST<'src, A>,
        interner: &mut Interner<'src, A>,
        spanner: &mut ParseSpanner,
        alloc: &A,
    ) -> Result<NodeId, Error>
    where
        A: Allocator,
    {
        let next = self.tk.next_token(spanner, interner.lit(), alloc)?;

        let expr = match next.kind() {
            TokenKind::IfKw => {
                let IfResult {
                    cond_nid,
                    block_nid,
                    rest_nid,
                    end,
                } = self.r#if(ast, interner, spanner, alloc)?;
                Expr {
                    ty: None,
                    span: span::Span {
                        start: next.start(),
                        end,
                    },
                    kind: ExprKind::If {
                        cond: cond_nid,
                        block: block_nid,
                        rest: rest_nid,
                    },
                    qual: QualifierBitset::EMPTY,
                    _capture: PhantomData,
                }
            }
            TokenKind::LetKw => {
                let LetResult {
                    sym_nid,
                    ty,
                    rest,
                    qual,
                    end,
                } = self.r#let(ast, interner, spanner, alloc)?;

                Expr {
                    ty: ty.map(|(ty, _)| ast.node(ty, alloc)).transpose()?,
                    span: span::Span {
                        start: next.start(),
                        end,
                    },
                    kind: ExprKind::LetExpr {
                        rhs: rest,
                        sym: sym_nid,
                    },
                    qual,
                    _capture: PhantomData,
                }
            }

            _ => todo!(),
        };

        Ok(ast.node(expr, alloc)?)
    }

    fn r#type<A>(
        &mut self,
        ast: &mut AST<'src, A>,
        interner: &mut Interner<'src, A>,
        spanner: &mut ParseSpanner,
        predicate: Option<&dyn Fn(TokenKind) -> bool>,
        alloc: &A,
    ) -> Result<Option<(Type, Span)>, Error>
    where
        A: Allocator,
    {
        let next = if let Some(predicate) = predicate {
            self.expect(interner, spanner, alloc, |kind| predicate(kind))?
        } else {
            self.tk.next_token(spanner, interner.lit(), alloc)?
        };

        if let TokenKind::BindOp = next.kind() {
            return Ok(None);
        }

        let next = self.expect(interner, spanner, alloc, |kind| {
            matches!(
                kind,
                TokenKind::Ampersand | TokenKind::Star | TokenKind::Ident
            ) || kind.is_builtin_ty()
                || if let None = &predicate {
                    matches!(
                        kind,
                        TokenKind::MoveKw | TokenKind::MutKw | TokenKind::UnsafeKw
                    )
                } else {
                    true
                }
        })?;

        match next.kind() {
            TokenKind::Ampersand => {
                let (ty, end) =
                    self.reference(ast, interner, spanner, alloc, Position::TypePosition)?;
                Ok(Some((ty, span::Span { start: 0, end })))
            }
            _ => todo!(),
        }
    }

    ast_fn! {
        slice(self, ast, interner, spanner, alloc) -> Result<(Type, u32), Error> {
            todo!()
        }

    }

    ast_fn! {
        r#if(
            self,
            ast,
            interner,
            spanner,
            alloc
        ) -> Result<IfResult, Error>
        {

            let cond_nid = self.conditional(ast, interner, spanner, alloc)?;
            let body = self.expr(ast, interner, spanner, alloc)?;
            let rest = self.expr(ast, interner, spanner, alloc)?;
            let end = rest.span(ast).unwrap().end;
            Ok(IfResult {
                cond_nid,
                block_nid: body,
                rest_nid: rest,
                end: 0,
            })
        }
    }

    ast_fn! {
        r#let(self, ast, interner, spanner, alloc) -> Result<LetResult, Error> {
            let (qual, sym_nid) = self.mutablity(ast, interner, spanner, alloc)?;
            let ty = self.r#type(ast, interner, spanner, Some(&|kind| matches!(kind, TokenKind::Colon | TokenKind::BindOp)), alloc)?;
            let rest = self.expr(ast, interner, spanner, alloc)?;
            let end = rest.span(ast).unwrap().end;

            Ok(LetResult { sym_nid, ty, rest, qual, end })
        }
    }

    ast_fn! {
        mutablity(self, ast, interner, spanner, alloc) -> Result<(QualifierBitset, NodeId), Error> {


            let next = self.expect(interner, spanner, alloc, |kind| matches!(kind, TokenKind::Ident | TokenKind::MutKw))?;

            match next.kind() {
                TokenKind::Ident => {
                    let sym = self.ident::<A, LetPlace<'src>>(next, ast, interner, alloc)?;
                    Ok((QualifierBitset::EMPTY, sym))
                },
                TokenKind::MutKw => {
                    let (_, nid) = self.mutablity(ast, interner, spanner, alloc)?;
                    Ok((QualifierBitset::MUTABLE, nid))
                },
                _ => unreachable!()
            }
        }
    }

    ast_fn! {

        conditional(
            self,
            ast,
            interner,
            spanner,
            alloc
        ) -> Result<NodeId, Error>
        {
            let lhs = self.expr(ast, interner, spanner, alloc)?;
            let binop = self.expect(interner, spanner, alloc, |kind| kind.is_binary_op())?;
            let rhs = self.expr(ast, interner, spanner, alloc)?;

            let start = lhs.span(ast).expect("LMAO").start;
            let end = rhs.span(ast).expect("LMAO").end;

            let nid = ast.node(
                Expr {
                    span: span::Span { start, end },
                    kind: ExprKind::BinOp {
                        rhs,
                        lhs,
                        kind: Self::to_binop_kind(binop)?,
                    },
                    _capture: PhantomData,
                    ty: None,
                    qual: QualifierBitset::EMPTY,
                },
                alloc,
            )?;

            Ok(nid)
        }
    }

    fn ident<A, P>(
        &mut self,
        token: Token,
        ast: &mut AST<'src, A>,
        interner: &mut Interner<'src, A>,
        alloc: &A,
    ) -> Result<P::Output, Error>
    where
        A: Allocator,
        P: Place<'src>,
    {
        let ident = self.span(token).expect("this shouldn't be none");
        P::from_symbol(ident, token, interner, ast, alloc)
    }

    fn to_binop_kind(token: Token) -> Result<BinOpKind, Error> {
        todo!()
    }
}

struct IfResult {
    pub cond_nid: NodeId,
    pub block_nid: NodeId,
    pub rest_nid: NodeId,
    pub end: u32,
}

struct LetResult {
    pub sym_nid: NodeId,
    pub ty: Option<(Type, Span)>,
    pub rest: NodeId,
    pub qual: QualifierBitset,
    pub end: u32,
}

pub enum PKind {
    Pointer,
    Reference,
}

impl PKind {
    fn resolve_ty(self, nid: NodeId, kind: RefKind) -> Type {
        match self {
            Self::Pointer => Type::Ptr { ty: nid, kind },
            Self::Reference => Type::Ref { ty: nid, kind },
        }
    }
}
