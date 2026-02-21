use std::fmt;

mod keywords;
use bootstrap_macros::enum_utils;
pub use keywords::*;

mod punct;
pub use punct::*;

mod trivia;
pub use trivia::*;

mod span;
pub use span::*;


#[derive(Clone, PartialEq, Eq, Debug, Hash)]
#[enum_utils(as_str(snake_case), display)]
pub enum Token {
    #[fmt("{_0}")]
    StrongKw(StrongKeyword),
    #[fmt("{_0}")]
    ReservedKw(ReservedKeyword),
    #[fmt("{_0}")]
    WeakKw(WeakKeyword),
    #[fmt("{_0}")]
    PatternKw(PatternKeyword),
    #[fmt("{}", _0.as_open_str())]
    OpenDelim(Delimiter),
    #[fmt("{}", _0.as_close_str())]
    CloseDelim(Delimiter),
    #[fmt("{_0}")]
    Punct(Punctuation),
    #[fmt("{_0}")]
    Name(String),
}

pub struct TokenMeta {
    pub span:   Span,
    pub trivia: Trivia,
}


pub struct TokenStream {
    tokens: Vec<Token>,
    metadata: Vec<TokenMeta>
}

impl TokenStream {
    pub fn new() -> Self {
        TokenStream {
            tokens: Vec::new(),
            metadata: Vec::new(),
        }
    }

    pub fn push(&mut self, token: Token, meta: TokenMeta) {
        self.tokens.push(token);
        self.metadata.push(meta);
    }

    pub fn tokens(&self) -> &[Token] {
        &self.tokens
    }

    pub fn metadata(&self) -> &[TokenMeta] {
        &self.metadata
    }

    pub fn tokens_with_meta(&self) -> impl Iterator<Item = (&Token, &TokenMeta)> {
        self.tokens.iter().zip(self.metadata.iter())
    }

    pub fn get_token_meta(&self, idx: usize) -> &TokenMeta {
        assert!(idx < self.metadata.len());
        &self.metadata[idx]
    }

    pub fn last_meta_mut(&mut self) -> Option<&mut TokenMeta> {
        self.metadata.last_mut()
    }

    pub fn last_mut(&mut self) -> Option<&mut Token> {
        self.tokens.last_mut()
    }

    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }

    pub fn len(&self) -> usize {
        self.tokens.len()
    }


    pub fn no_trivia_formatter<'a>(&'a self) -> NoTriviaTokenStreamFormatter<'a> {
        NoTriviaTokenStreamFormatter { tokens: self }
    }

    pub fn csv_formatter<'a>(&'a self) -> CsvTokenStreamFormatter<'a> {
        CsvTokenStreamFormatter { tokens: self }
    }
}

impl fmt::Display for TokenStream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (tok, meta) in self.tokens.iter().zip(self.metadata.iter()) {
            for trivia in meta.trivia.leading() {
                write!(f, "{trivia}")?;
            }
            
            write!(f, "{tok}")?;
            
            for trivia in meta.trivia.trailing() {
                write!(f, "{trivia}")?;
            }
        }

        Ok(())
    }
}

pub struct NoTriviaTokenStreamFormatter<'a> {
    tokens: &'a TokenStream
}

impl fmt::Display for NoTriviaTokenStreamFormatter<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for tok in self.tokens.tokens() {
            write!(f, "{tok} ")?;
        }

        Ok(())
    }
}

pub struct CsvTokenStreamFormatter<'a> {
    tokens: &'a TokenStream
}

impl fmt::Display for CsvTokenStreamFormatter<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("token,kind,line,column,byte_offset,byte_len,char_offset,char_len\n")?;

        for (tok, meta) in self.tokens.tokens_with_meta() {
            write!(f, "{tok},{},{},{},{},{},{},{}\n",
                tok.as_str(),
                meta.span.line,
                meta.span.column,
                meta.span.byte_offset,
                meta.span.byte_len,
                meta.span.char_offset,
                meta.span.char_len,
            )?;
        }

        Ok(())
    }
}
