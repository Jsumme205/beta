use crate::node::NodeId;

pub enum StringKind {
    Byte,
    UTF8,
}

pub enum NKind {
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

#[derive(Clone, Copy)]
pub enum RefKind {
    ImmRef = 0,
    MutRef = 1,
    MoveRef = 2,
    UnsafePtr = 3,
    MutUnsafePtr = 4,
    Ptr = 5,
    MutPtr = 6,
}

pub enum Type {
    Numerical(NKind),
    StrLit(StringKind),
    Char,
    Array {
        ty: NodeId,
        #[allow(non_snake_case)]
        N: NodeId,
    },
    Slice {
        ty: NodeId,
    },
    Ref {
        ty: NodeId,
        kind: RefKind,
    },
    // => obj Iterator<Item => int8>
    Obj {
        start: u32,
        end: u32,
        sym: NodeId,
    },
    Ptr {
        ty: NodeId,
        kind: RefKind,
    },
    Symbol {
        id: NodeId,
    },
    Any,
    Bool,
}
