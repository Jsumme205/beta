pub(crate) const MAX_PREFIX_LEN: usize = "_uint64".len();

pub struct InvalidPrefix {
    prefix: [u8; MAX_PREFIX_LEN],
    prefix_len: u8,
}

impl core::fmt::Debug for InvalidPrefix {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug)]
pub enum Error {
    UnknownToken,
    InvalidPrefix(InvalidPrefix),
    InvalidMultiChar,
    InvalidLiteral,
}

impl InvalidPrefix {
    pub fn new(s: &str) -> Self {
        let len = s.len();

        let mut prefix = [0u8; MAX_PREFIX_LEN];

        prefix[..len].copy_from_slice(s.as_bytes());
        Self {
            prefix,
            prefix_len: len as u8,
        }
    }

    pub fn as_str(&self) -> &str {
        unsafe { str::from_utf8_unchecked(&self.prefix[..self.prefix_len as usize]) }
    }
}
