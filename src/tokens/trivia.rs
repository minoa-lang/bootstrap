use std::fmt;


pub fn is_whitespace_trivia(ch: char) -> bool {
    matches!(ch,
        // Horizontal
        '\u{0009}' |
        '\u{0020}' |
        '\u{200E}' |
        '\u{200F}' |
        // Vertical
        '\u{000A}' |
        '\u{000B}' |
        '\u{000C}' |
        '\u{000D}' |
        '\u{0085}' |
        '\u{2028}' |
        '\u{2029}'
    )
}

#[derive(Clone, Debug)]
pub enum TriviaElem {
    Whitespace(String),
    Comment(String),
    DocComment(String),
}

impl fmt::Display for TriviaElem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TriviaElem::Whitespace(s) => write!(f, "{s}"),
            TriviaElem::Comment(s)    => write!(f, "{s}"),
            TriviaElem::DocComment(s) => write!(f, "{s}"),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Trivia {
    leading:  Vec<TriviaElem>,
    trailing: Vec<TriviaElem>,
}

impl Trivia {
    pub fn new() -> Self {
        Trivia {
            leading: Vec::new(),
            trailing: Vec::new(),
        }
    }

    pub fn add_leading(&mut self, trivia: TriviaElem) {
        self.leading.push(trivia);
    }

    pub fn add_trailing(&mut self, trivia: TriviaElem) {
        self.trailing.push(trivia);
    }

    pub fn add_leading_whitespace(&mut self, whitespace: String) {
        self.leading.push(TriviaElem::Whitespace(whitespace));
    }

    pub fn add_trailing_whitespace(&mut self, whitespace: String) {
        self.trailing.push(TriviaElem::Whitespace(whitespace));
    }

    pub fn add_leading_comment(&mut self, comment: String) {
        self.leading.push(TriviaElem::Comment(comment));
    }

    pub fn add_trailing_comment(&mut self, comment: String) {
        self.trailing.push(TriviaElem::Comment(comment));
    }


    pub fn leading(&self) -> &[TriviaElem] {
        &self.leading
    }

    pub fn trailing(&self) -> &[TriviaElem] {
        &self.trailing
    }
}