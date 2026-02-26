use bootstrap_macros::enum_utils;


#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Delimiter {
    Parenthesis,
    Brace,
    Bracket,
    Vector,
}

impl Delimiter {
    pub fn as_open_str(self) -> &'static str {
        match self {
            Self::Parenthesis => "(",
            Self::Brace       => "{",
            Self::Bracket     => "[",
            Self::Vector      => "[<",
        }
    }

    pub fn as_close_str(self) -> &'static str {
        match self {
            Self::Parenthesis => ")",
            Self::Brace       => "}",
            Self::Bracket     => "]",
            Self::Vector      => ">]",
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug, Hash)]
#[enum_utils(as_str, display, from_idx)]
pub enum Punctuation {
    #[string(".")]
    Dot,
    #[string("..")]
    DotDot,
    #[string("...")]
    DotDotDot,
    #[string(",")]
    Comma,
    #[string(":")]
    Colon,
    #[string(";")] 
    SemiColon,
    #[string("_")]
    Underscore,
    #[string("-")]
    Minus,
    #[string("=")]
    Equals,
    #[string(":=")]
    ColonEquals,
    #[string("^")]
    Caret,
    #[string("*")]
    Asterisk,
    #[string("+")]
    Plus,
    #[string("@")]
    At,
    #[string("#")]
    Hash,
    #[string("$")]
    Dollar,
    #[string("$$")]
    DollarDollar,
    #[string("!")]
    Exclaim,
    #[string("?")]
    Question,
    #[string("?.")]
    QuestionDot,
    #[string("\\")]
    Backslash,
    #[string("&")]
    And,
    #[string("&&")]
    AndAnd,
    #[string("|")]
    Or,
    #[string("||")]
    OrOr,
    #[string("->")]
    Arrow,
    #[string("=>")]
    DblArrow,
    #[string("")]
    #[fmt("{_0}")]
    Other(String),
}