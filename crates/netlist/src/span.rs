use std::ops::Range;

/// a range of UTF-8 bytes in the original netlist, independent of its input
/// name for rich diagnostics
///
/// a zero-length span at the input length represents the end of input
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ByteSpan {
    pub offset: usize,
    pub length: usize,
}

impl ByteSpan {
    pub const fn new(offset: usize, length: usize) -> Self {
        Self { offset, length }
    }

    pub fn range(self) -> Range<usize> {
        self.offset..self.offset + self.length
    }
}
