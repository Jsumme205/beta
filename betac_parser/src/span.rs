#[derive(Clone, Copy)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

impl From<(u32, u32)> for Span {
    fn from((start, end): (u32, u32)) -> Self {
        Self { start, end }
    }
}

pub trait Spanned: Sized {
    fn to_start_end_bounds(self) -> (u32, u32);
}

impl Spanned for betac_token::Token {
    fn to_start_end_bounds(self) -> (u32, u32) {
        (self.start(), self.end())
    }
}

impl Spanned for Span {
    fn to_start_end_bounds(self) -> (u32, u32) {
        (self.start, self.end)
    }
}
