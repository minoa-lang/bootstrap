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
    }
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
            }
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
    #[fmt("The {_0} literal may not have a leading underscore (`_`)")]
    LeadingUnderscore(LiteralSegment),
    #[fmt("The {_0} literal may not have a trailing underscore (`_`)")]
    TrailingUnderscore(LiteralSegment),
    #[fmt("Unexpected digit '{_1}' found in the {_0} literal")]
    UnsupportedDigit(LiteralSegment, char),
    #[fmt("A hexadecimal floats must contain an exponent")]
    HexFloatNoExp,
}