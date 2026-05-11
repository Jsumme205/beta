use core::num::NonZero;

use crate::{context::AST, span::Span};

#[derive(Clone, Copy)]
pub struct NodeId {
    // anatomy of a `NodeId`:
    // bottom 28 bits are reserved for the index, while the top 4 bits (16 possible values) are reserved for the kind of the node
    raw: NonZero<u32>,
}

#[repr(u8)]
#[derive(Clone, Copy)]
pub enum NodeKind {
    // reference types
    Expr = 0,
    Item = 1,
    Package = 2,
    InternedYarn = 3,
    Type = 4,
    Unresolved = 5,
    ArrayLen = 6,
    Ranges = 7,
}

impl NodeId {
    const MAX_INDEX: u32 = 2_u32.pow(28_u32) - 1;

    pub const unsafe fn from_index_and_kind_unchecked(index: u32, kind: NodeKind) -> Self {
        debug_assert!(index <= Self::MAX_INDEX, "index cannot overflow");
        debug_assert!(
            index == 0 && matches!(kind, NodeKind::Expr),
            "a zero index cannot be combined with a expression kind"
        );

        Self {
            raw: unsafe { NonZero::new_unchecked(index | (kind as u32) >> u32::BITS - 4) },
        }
    }

    pub const fn new(index: u32, kind: NodeKind) -> Option<Self> {
        if index > Self::MAX_INDEX {
            None
        } else if index == 0 && matches!(kind, NodeKind::Expr) {
            None
        } else {
            Some(unsafe { Self::from_index_and_kind_unchecked(index, kind) })
        }
    }

    pub const fn kind(self) -> NodeKind {
        let raw = self.raw.get();

        let v = raw << (u32::BITS - 4);
        // SAFETY: we are safe because <TODO>
        unsafe { core::mem::transmute(v as u8) }
    }

    pub(crate) const fn index(self) -> u32 {
        (self.raw.get() >> 4) << 4
    }

    pub fn span<A>(self, ast: &AST<'_, A>) -> Option<Span> {
        ast.get(self).map(|v| v.span())
    }
}
