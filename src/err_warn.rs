use std::io;

pub enum LexError {
    FileIO(io::Error),
}


pub struct ErrorWarnState {
    pub lex_errors: Vec<LexError>,
}

impl ErrorWarnState {
    pub fn new() -> Self {
        Self {
            lex_errors: Vec::new(),
        }
    }

    pub fn add_lexer_error(&mut self, err: LexError) {
        self.lex_errors.push(err);
    }
}