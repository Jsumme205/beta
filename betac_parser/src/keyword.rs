use crate::{
    Parser, TokenKind,
    lookup_table::{self, Table, encode},
};

static KEYWORD_LUT: Table<16> = lookup_table::table![
    [
        "mport"    pf | ' '                                => Import,
        "f"        pf | ' '                                => IfKw,
        "nt8"      pf | ')' | '{' | ',' | '>'  | ' ' | '.' => Int8,
        "nt16"     pf | ')' | '{' | ',' | '>'  | ' ' | '.' => Int16,
        "nt32"     pf | ')' | '{' | ',' | '>'  | ' ' | '.' => Int32,
        "nt64"     pf | ')' | '{' | ',' | '>'  | ' ' | '.' => Int64,
        "size"     pf | ')' | '{' | ',' | '>'  | ' ' | '.' => Isize,
    ],
    [
        "efun"     pf | ' '                                => Defun,
    ],
    [
        "omponent" pf | ' ' | '{'                          => Component,
        "ase"      pf | ' ' | ':'                          => CaseKw,
        "har"      pf | ' ' | '{' | ',' | '>' | ')' | '.'  => Char,
    ],
    [
        "bj"       pf | ' ' | '{'                          => Obj,
    ],
    [
        "xtends"   pf | ' '                                => Extends,
        "xtend"    pf | ' '                                => Extend,
        "xtern"    pf | ' ' | '"'                          => ExternKw,
        "num"      pf | ' ' | '{'                          => Enum,
    ],
    [
        "his"      pf | ' '                                => ThisVar,
    ],
    [
        "ub"       pf | ' ' | '('                          => Pub,
        "riv"      pf | ' ' | '('                          => Priv,
        "ack"      pf | ' ' | ')'                          => PackKw,
    ],
    [
        "et"       pf | ' '                                => LetKw,
        "oop"      pf | ' ' | '{'                          => LoopKw,
    ],
    [
        "or"       pf | ' ' | '('                          => ForKw,
        "loat32"   pf | ' ' | ')' | '{' | ',' | '>' | '.'  => Float32,
        "loat64"   pf | ' ' | ')' | '{' | ',' | '>' | '.'  => Float64,
    ],
    [
        "hile"     pf | ' ' | '('                          => WhileKw,
    ],
    [
        "witch"    pf | ' '                                => SwitchKw,
        "tr"       pf | ' ' | ')' | '{' | ',' | '>' | '.'  => Str,
    ],
    [
        "ove"      pf | ' '                                => MoveKw,
        "ut"       pf | ' '                                => MutKw,
    ],
    [
        "ny"       pf | ' ' | ')' | '{' | ',' | '>' | '.'  => Any,
    ],
    [
        "ool"      pf | ' ' | ')' | '{' | ',' | '>' | '.'  => Bool,
    ],
    [
        "nion"     pf | ' ' | '{'                          => Union,
        "int8"     pf | ')' | '{' | ',' | '>'  | ' ' | '.' => Uint8,
        "int16"    pf | ')' | '{' | ',' | '>'  | ' ' | '.' => Uint16,
        "int32"    pf | ')' | '{' | ',' | '>'  | ' ' | '.' => Uint32,
        "int64"    pf | ')' | '{' | ',' | '>'  | ' ' | '.' => Uint64,
        "size"     pf | ')' | '{' | ',' | '>'  | ' ' | '.' => Usize,
    ],
    [
        "his"      pf | ')' | '{' | ',' | '>'  | ' ' | '.' => ThisTy,
    ]
];

macro_rules! idx {
    ($b:ident[$v:expr]) => {
        $b[$v as usize]
    };
}

fn pool_index(b: u8) -> impl Fn() -> Option<usize> {
    static LUT: [usize; 256] = {
        let mut buf = [usize::MAX; 256];
        idx!(buf['i']) = 0;
        idx!(buf['d']) = 1;
        idx!(buf['c']) = 2;
        idx!(buf['o']) = 3;
        idx!(buf['e']) = 4;
        idx!(buf['t']) = 5;
        idx!(buf['p']) = 6;
        idx!(buf['l']) = 7;
        idx!(buf['f']) = 8;
        idx!(buf['w']) = 9;
        idx!(buf['s']) = 10;
        idx!(buf['m']) = 11;
        idx!(buf['a']) = 12;
        idx!(buf['b']) = 13;
        idx!(buf['u']) = 14;
        idx!(buf['T']) = 15;
        buf
    };

    move || LUT.get(b as usize).copied().filter(|v| *v != usize::MAX)
}

pub fn matches(b: u8, tk: &Parser<'_>) -> Option<TokenKind> {
    KEYWORD_LUT.find(
        &pool_index(b),
        &|parser, entry| &parser[entry],
        tk,
        &|v, e| encode(v) == e,
    )
}
