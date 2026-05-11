use core::marker::PhantomData;

use betac_yarn::Yarn;

use crate::{node::NodeId, qual::QualifierBitset, span::Span, ty::RefKind};

pub struct Expr<'src> {
    pub ty: Option<NodeId>,
    pub kind: ExprKind,
    pub span: Span,
    pub qual: QualifierBitset,
    pub _capture: PhantomData<Yarn<'src>>,
}

#[repr(u8)]
#[derive(Clone, Copy)]
pub enum BinOpKind {
    Gt,
    Lt,
    GtEq,
    LtEq,
    Eq,
    BitOr,
    BitAnd,
    BitXor,
    LogOr,
    LogAnd,
}

#[repr(u8)]
#[derive(Clone, Copy)]
pub enum UnaryOpKind {
    Not,
}

pub enum ExprKind {
    LetExpr {
        rhs: NodeId,
        sym: NodeId,
    },
    Ref {
        rhs: NodeId,
        kind: RefKind,
    },
    Ident {
        sym: NodeId,
        last: bool,
    },
    If {
        cond: NodeId,
        block: NodeId,
        rest: NodeId,
    },
    Block {
        exprs: NodeId,
    },
    BinOp {
        rhs: NodeId,
        lhs: NodeId,
        kind: BinOpKind,
    },
    UnaryOp {
        kind: UnaryOpKind,
        rhs: NodeId,
    },
    End,
}

// expr: let <sym> = &<ident>;
//
// would turn into a (simplified, psudo-code):
//
// LetExpr {
//  sym: <sym>.node_id,
//  rhs: Ref {
//         rhs: <ident>.node_id
//    }
//    .node_id
// }
