pub struct QualifierBitset(u8);

macro_rules! define_quals {
    ($($VALUE:ident = $val:expr),* $(,)?) => {
        $(
            pub const $VALUE: Self = Self($val);
        )*
    };
}

impl QualifierBitset {
    define_quals! {
        PUBLIC = 1 << 0,
        CONSTEXPR = 1 << 1,
        EMPTY = 0,
        MUTABLE = 1 << 2,
    }

    pub const fn and(self, val: Self) -> Self {
        Self(self.0 | val.0)
    }
}
