
#[derive(Clone, Copy, Debug)]
pub struct Span {
    pub line:        u32,
    pub column:      u32,
    pub byte_offset: usize,
    pub byte_len:    u32,
    pub char_offset: usize,
    pub char_len:    u32,
}