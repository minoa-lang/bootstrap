use std::io;

use crate::{lex::LexError, log};

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
        for err in &self.lex_errors {
            log!(Error, "{err}")?;
        }

        Ok(())
    }
}