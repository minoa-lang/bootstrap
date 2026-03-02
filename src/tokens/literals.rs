use std::fmt;

use bootstrap_macros::enum_utils;


#[derive(Clone, PartialEq, Eq, Debug, Hash)]
#[enum_utils(display)]
pub enum Exponent {
    #[fmt("")]
    None,
    #[fmt("e{_0}")]
    Some(String),
    #[fmt("e+{_0}")]
    Pos(String),
    #[fmt("e-{_0}")]
    Neg(String),
}

#[derive(Clone, PartialEq, Eq, Debug, Hash)]
#[enum_utils(display)]
pub enum HexExponent {
    #[fmt("")]
    None,
    #[fmt("p{_0}")]
    Dec(String),
    #[fmt("p+{_0}")]
    DecPos(String),
    #[fmt("p-{_0}")]
    DecNeg(String),
    #[fmt("px{_0}")]
    Hex(String),
    #[fmt("px+{_0}")]
    HexPos(String),
    #[fmt("px-{_0}")]
    HexNeg(String),
}

#[derive(Clone, PartialEq, Eq, Debug, Hash)]
#[enum_utils(display)]
pub enum EscapeSequence {
    #[fmt("\\0")]
    Null,
    #[fmt("\\t")]
    Tab,
    #[fmt("\\n")]
    Newline,
    #[fmt("\\r")]
    CariageReturn,
    #[fmt("\\\"")]
    DblQuote,
    #[fmt("\\'")]
    Quote,
    #[fmt("\\\\")]
    Backslash,
    #[fmt("\\p")]
    SystemNewline,
    #[fmt("\\}}")]
    ClosingBrace,
    #[fmt("\\,")]
    Comma,
    #[fmt("\\x{_0}")]
    Hex(String),
    #[fmt("\\u{{{_0}}}")]
    Unicode(String),
    #[fmt("{_0}")]
    Unsupported(String),
}

impl EscapeSequence {
    pub fn len(&self) -> usize {
        match self {
            Self::Hex(_) => 4, // \xHH
            Self::Unicode(code) => code.len() + 4, // '\u{' + '}'
            Self::Unsupported(s) => s.len(),
            _ => 2,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug, Hash)]
#[enum_utils(display)]
pub enum CharLiteral {
    #[fmt("{_0}")]
    Char(char),
    #[fmt("{_0}")]
    Escape(EscapeSequence),
}

#[derive(Clone, PartialEq, Eq, Debug, Hash)]
#[enum_utils(display)]
pub enum StringLiteral {
    #[fmt("\"{_0}\"")]
    String(String),
    #[fmt("\"{_0}")]
    MultiStringSegment(String),
    // TODO: is {:#_0$} correct?
    #[fmt("{0}`{_1}`{0:#_0$}", "#".repeat(*_0))]
    Raw(usize, String),
    #[fmt("{}`{_1}", "#".repeat(*_0))]
    MultiRawSegment(usize, String),
}
 
#[derive(Clone, PartialEq, Eq, Debug, Hash)]
#[enum_utils(as_str(snake_case))]
pub enum Literal {
    Decimal{
        integral: String,
        exponent: Exponent,
    },
    DecimalFloat {
        integral: String,
        fraction: String,
        exponent: Exponent,
    },
    Binary(String),
    Octal(String),
    Hexadecimal(String),
    HexadecimalFloat {
        integral: String,
        fraction: String,
        exponent: HexExponent
    },
    Char(CharLiteral),
    String(String),
    MultiStringSegment{
        content: String,
        newline: bool,
    },
    RawString{
        // Only should hold up to 255, but we want to lex invalid strings to print them correctly
        depth:   usize,
        content: String
    },
    MultiRawStringSegment{
        // Only should hold up to 255, but we want to lex invalid strings to print them correctly
        depth:   usize,
        content: String,
        newline: bool,
    },

    InterpString {
        includes_end: bool,
        includes_start: bool,
        content: String,
    },
    MultiInterpString{
        includes_end: bool,
        includes_start: bool,
        content: String,
        newline: bool,
    },
}

impl fmt::Display for Literal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Literal::Decimal { integral, exponent } => write!(f, "{integral}{exponent}"),
            Literal::DecimalFloat { integral, fraction, exponent } => {
                write!(f, "{integral}")?;
                if !fraction.is_empty() {
                    write!(f, ".{fraction}")?;
                }
                write!(f, "{exponent}")?;
                Ok(())
            },
            Literal::Binary(bin) => write!(f, "0b{bin}"),
            Literal::Octal(oct) => write!(f, "0o{oct}"),
            Literal::Hexadecimal(hex) => write!(f, "0x{hex}"),
            Literal::HexadecimalFloat { integral, fraction, exponent } => {
                write!(f, "0x{integral}")?;
                if !fraction.is_empty() {
                    write!(f, ".{fraction}")?;
                }
                write!(f, "{exponent}")?;
                Ok(())
            },
            Literal::Char(ch) => write!(f, "{ch}"),
            Literal::String(s) => write!(f, "\"{s}\""),
            Literal::MultiStringSegment{ content, .. } => write!(f, "\"{content}"),
            Literal::RawString{ depth, content, .. } => write!(f, "{0}`{content}`{0}", "#".repeat(*depth)),
            Literal::MultiRawStringSegment{ content, .. } => write!(f, "`{content}"),
            Literal::InterpString{ includes_end, includes_start, content: s } => {
                write!(f, "{}{s}{}",
                    if *includes_end { "}" } else { "\"" },
                    if *includes_start { "\\{" } else { "\"" },
                )
            },
            Literal::MultiInterpString{ includes_end, includes_start, content, .. } => {
                write!(f, "\"{}{content}{}",
                    if *includes_end { "}" } else { "" },
                    if *includes_start { "\\{" } else { "" },
                )
            },
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[enum_utils(as_str, display)]
pub enum LiteralSegment {
    #[string("decimal integral")]
    DecIntegral,
    #[string("decimal fraction")]
    DecFraction,
    #[string("decimal exponent")]
    DecExponent,
    #[string("binary")]
    Binary,
    #[string("octal")]
    Octal,
    #[string("hexadecimal integral")]
    HexIntegral,
    #[string("hexadecimal fraction")]
    HexFraction,
    #[string("hexadecimal exponent")]
    HexExponent
}

#[derive(Clone, PartialEq, Eq, Debug)]
#[enum_utils(display)]
pub enum LiteralError {
    #[fmt("The {_0} literal does not contain a value")]
    EmptyLiteral(LiteralSegment),
    #[fmt("The {_0} literal may not have a leading underscore (`_`)")]
    LeadingUnderscore(LiteralSegment),
    #[fmt("The {_0} literal may not have a trailing underscore (`_`)")]
    TrailingUnderscore(LiteralSegment),
    #[fmt("Unexpected digit '{_1}' found in the {_0} literal")]
    UnsupportedDigit(LiteralSegment, char),
    #[fmt("Invalid character literal: {_0}")]
    InvalidCharacterLiteral(String),
    #[fmt("Unexpected escape sequence '{_0}' found")]
    UnexpectEscape(char),
    #[fmt("Invalid unicode escape: {_0}")]
    InvalidUnicodeEscape(String)
}