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


    // TODO: once we have more than just lex error, group per file and from top to bottom of file
    pub fn log(&self) -> io::Result<()> {
        Ok(())
    }
}