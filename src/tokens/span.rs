use std::fmt;


// Todo: Multi-line span support
#[derive(Clone, Copy, Debug)]
pub struct Span {
    pub line:        u32,
    pub column:      u32,
    pub byte_offset: usize,
    pub byte_len:    u32,
    pub char_offset: usize,
    pub char_len:    u32,
}

impl Span {
    pub fn new_location(line: u32, column: u32) -> Self {
        Self {
            line,
            column,
            ..Span::default()
        }
    }
}

impl Default for Span {
    fn default() -> Self {
        Self {
            line: 0,
            column: 0,
            byte_offset: 0,
            byte_len: 0,
            char_offset: 0,
            char_len: 0,
        }
    }
}

impl fmt::Display for Span {
    // TODO: Include end of span somehow
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.char_len == 0 {
            write!(f, "{}:{}", self.line, self.column)
        } else {
            let end_column = self.column + self.char_len;
            write!(f, "{}:{} - {}:{}", self.line, self.column, self.line, end_column)
        }

    }
}