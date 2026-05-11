use core::{num::NonZero, ops::Range};

use allocator_api::{
    Allocator,
    collections::{HashMap, Vec},
};

use crate::{Error, span::Span};
use betac_token::Token;
use betac_tokenizer::{LitInterner, Spanner, buf::IntegerBuffer};
use betac_yarn::Yarn;

use crate::{
    expr::Expr,
    node::{NodeId, NodeKind},
    ty::Type,
};

pub struct ParseSpanner {
    line: u32,
    column: u32,
}

impl ParseSpanner {
    pub fn new() -> Self {
        Self { line: 1, column: 1 }
    }
}

impl Spanner for ParseSpanner {
    fn column(&self) -> u32 {
        self.column
    }

    fn line(&self) -> u32 {
        self.line
    }

    fn emit_new_column(&mut self) {
        self.column += 1;
    }

    fn emit_new_line(&mut self) {
        self.column = 1;
        self.line += 1;
    }
}

pub struct LiteralInterner<A> {
    pub(crate) interned: HashMap<Token, IntegerBuffer, wyhash::WyHasherBuilder, A>,
}

impl<A: Allocator> LitInterner<A> for LiteralInterner<A> {
    fn get_buf(&self, t: Token) -> Option<&betac_tokenizer::buf::IntegerBuffer> {
        self.interned.get(&t)
    }

    fn push_buf(&mut self, b: betac_tokenizer::buf::IntegerBuffer, token: Token, alloc: &A) {
        self.interned.insert(token, b, alloc);
    }
}

pub struct ParseContext<'src, 'sess, A> {
    pub(crate) spanner: &'sess mut ParseSpanner,
    pub(crate) interner: &'sess mut Interner<'src, A>,
    pub(crate) ast: &'sess mut AST<'src, A>,
}

pub struct SymbolInfo {
    pub kind: SymbolKind,

    // index of AST.(expressions | items | packages | unresolved_symbols)
    pub id: NodeId,

    // index of AST.symbols
    pub sym_id: NodeId,
}

#[derive(Clone, Copy)]
pub enum SymbolKind {
    VarName,
    Function,
    Object,
    //etc..
}

impl SymbolKind {
    pub(crate) const fn is_namable_ty(self) -> bool {
        todo!()
    }
}

pub struct Interner<'src, A> {
    resolved_interned_symbols: HashMap<Yarn<'src>, SymbolInfo, wyhash::WyHasherBuilder, A>,
    literal_interner: LiteralInterner<A>,
}

pub enum MaybeResolvedSymbol<'a> {
    Info(&'a SymbolInfo),
    Unresolved,
}

impl<'src, A> Interner<'src, A> {
    pub const fn lit(&mut self) -> &mut LiteralInterner<A> {
        &mut self.literal_interner
    }

    pub fn find(&self, yarn: &betac_yarn::Yarn<'src>) -> MaybeResolvedSymbol<'_> {
        match self.resolved_interned_symbols.get(yarn) {
            Some(info) => MaybeResolvedSymbol::Info(info),
            None => MaybeResolvedSymbol::Unresolved,
        }
    }
}

#[derive(Default)]
pub struct Ranges {
    expr_range: Option<Range<u32>>,
    symbol_range: Option<Range<u32>>,
    unresolved_symbol_range: Option<Range<u32>>,
    array_len_range: Option<Range<u32>>,
    types_range: Option<Range<u32>>,
}

pub struct AST<'src, A> {
    expressions: Vec<Expr<'src>, A>,
    symbols: Vec<SpannedSymbol<'src>, A>,
    unresolved_symbols: Vec<SpannedSymbol<'src>, A>,
    array_lens: Vec<usize, A>,
    types: Vec<Type, A>,
    ranges: Vec<Ranges, A>,
}

pub struct SpannedSymbol<'src> {
    pub sym: Yarn<'src>,
    pub span: Span,
}

fn push_create_id<T, A: Allocator>(
    vec: &mut Vec<T, A>,
    value: T,
    kind: NodeKind,
    alloc: &A,
) -> Result<NodeId, Exhausted> {
    let old_len = vec.len();
    vec.push(value, alloc);
    match NodeId::new(old_len as _, kind) {
        Some(val) => Ok(val),
        None => {
            core::hint::cold_path();
            Err(Exhausted(()))
        }
    }
}

fn push_create_insert_range<T, A>(
    vec: &mut Vec<T, A>,
    value: T,
    range: &mut Option<Range<u32>>,
    kind: NodeKind,
    alloc: &A,
) -> Result<(), Exhausted>
where
    A: Allocator,
{
    let nid = push_create_id(vec, value, kind, alloc)?;
    range
        .get_or_insert(Range {
            start: nid.index(),
            end: nid.index(),
        })
        .end += 1;
    Ok(())
}

impl<'src, A> AST<'src, A>
where
    A: Allocator,
{
    pub fn node(&mut self, node: impl Into<ASTNode<'src>>, alloc: &A) -> Result<NodeId, Exhausted> {
        match node.into() {
            ASTNode::Expr(expr) => {
                push_create_id(&mut self.expressions, expr, NodeKind::Expr, alloc)
            }
            ASTNode::Symbol(sym, span) => push_create_id(
                &mut self.symbols,
                SpannedSymbol { sym, span },
                NodeKind::InternedYarn,
                alloc,
            ),
            ASTNode::Unresolved(sym, span) => push_create_id(
                &mut self.unresolved_symbols,
                SpannedSymbol { sym, span },
                NodeKind::Unresolved,
                alloc,
            ),
            ASTNode::ArrayLen(len) => {
                push_create_id(&mut self.array_lens, len, NodeKind::ArrayLen, alloc)
            }
            ASTNode::Type(ty) => push_create_id(&mut self.types, ty, NodeKind::Type, alloc),
        }
    }

    pub fn start_range<'a>(&'a mut self, alloc: &'a A) -> StartRange<'a, 'src, A> {
        StartRange {
            ast: self,
            alloc,
            ranges: Ranges {
                ..Default::default()
            },
        }
    }
}

impl<'src, A> AST<'src, A> {
    pub fn get_mut(&mut self, nid: NodeId) -> Option<NodeRefMut<'_, 'src>> {
        match nid.kind() {
            NodeKind::Expr => Some(NodeRefMut::Expr(
                self.expressions.get_mut(nid.index() as usize)?,
            )),
            _ => None,
        }
    }

    pub fn get(&self, nid: NodeId) -> Option<NodeRef<'_, 'src>> {
        todo!()
    }
}

pub enum NodeRefMut<'a, 'src> {
    Expr(&'a mut Expr<'src>),
}

pub enum NodeRef<'a, 'src> {
    Expr(&'a Expr<'src>),
    Symbol(&'a SpannedSymbol<'src>),
}

impl<'a, 'src> NodeRefMut<'a, 'src> {
    pub fn unwrap_expr(self) -> &'a mut Expr<'src> {
        match self {
            Self::Expr(e) => e,
        }
    }
}

impl<'a, 'src> NodeRef<'a, 'src> {
    pub fn unwrap_expr(self) -> &'a Expr<'src> {
        match self {
            Self::Expr(e) => e,
            Self::Symbol(_) => panic!(),
        }
    }

    pub fn span(&self) -> Span {
        match self {
            NodeRef::Expr(e) => e.span,
            NodeRef::Symbol(SpannedSymbol { span, .. }) => *span,
        }
    }
}

pub struct StartRange<'a, 'src, A> {
    pub(crate) ast: &'a mut AST<'src, A>,
    alloc: &'a A,
    ranges: Ranges,
}

impl<'src, A: Allocator> StartRange<'_, 'src, A> {
    pub fn node(&mut self, node: impl Into<ASTNode<'src>>) -> Result<(), Error> {
        match node.into() {
            ASTNode::Expr(expr) => push_create_insert_range(
                &mut self.ast.expressions,
                expr,
                &mut self.ranges.expr_range,
                NodeKind::Expr,
                self.alloc,
            )?,
            ASTNode::Symbol(sym, span) => push_create_insert_range(
                &mut self.ast.symbols,
                SpannedSymbol { span, sym },
                &mut self.ranges.symbol_range,
                NodeKind::InternedYarn,
                self.alloc,
            )?,
            ASTNode::Unresolved(sym, span) => push_create_insert_range(
                &mut self.ast.unresolved_symbols,
                SpannedSymbol { sym, span },
                &mut self.ranges.unresolved_symbol_range,
                NodeKind::Unresolved,
                self.alloc,
            )?,
            ASTNode::ArrayLen(len) => push_create_insert_range(
                &mut self.ast.array_lens,
                len,
                &mut self.ranges.array_len_range,
                NodeKind::ArrayLen,
                self.alloc,
            )?,
            ASTNode::Type(ty) => push_create_insert_range(
                &mut self.ast.types,
                ty,
                &mut self.ranges.types_range,
                NodeKind::Type,
                self.alloc,
            )?,
        };
        Ok(())
    }

    pub fn finish(self) -> Result<NodeId, Exhausted> {
        push_create_id(
            &mut self.ast.ranges,
            self.ranges,
            NodeKind::Ranges,
            self.alloc,
        )
    }
}

pub enum ASTNode<'src> {
    Expr(Expr<'src>),
    Symbol(Yarn<'src>, Span),
    Unresolved(Yarn<'src>, Span),
    ArrayLen(usize),
    Type(Type),
}

pub struct Unresolved<'src>(pub Yarn<'src>, pub Span);

impl From<Type> for ASTNode<'_> {
    fn from(value: Type) -> Self {
        Self::Type(value)
    }
}

impl<'src> From<Expr<'src>> for ASTNode<'src> {
    fn from(value: Expr<'src>) -> Self {
        Self::Expr(value)
    }
}

impl<'src> From<Unresolved<'src>> for ASTNode<'src> {
    fn from(value: Unresolved<'src>) -> Self {
        Self::Unresolved(value.0, value.1)
    }
}

impl<'src> From<(Yarn<'src>, Span)> for ASTNode<'src> {
    fn from(value: (Yarn<'src>, Span)) -> Self {
        Self::Symbol(value.0, value.1)
    }
}

impl From<usize> for ASTNode<'_> {
    fn from(value: usize) -> Self {
        Self::ArrayLen(value)
    }
}

pub struct Exhausted(());
