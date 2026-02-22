#![allow(unused)]

use std::{
    collections::HashMap,
    fmt,
    fs::File,
    io::{self, BufRead, BufReader},
    mem,
    num::ParseIntError,
    ops::RangeBounds
};

use crate::tokens::{is_whitespace_trivia, CharLiteral, EscapeSequence, Exponent, HexExponent, Literal, LiteralError, LiteralSegment, Punctuation, ReservedKeyword, Span, StrongKeyword, Token, TokenMeta, TokenStream, Trivia, TriviaElem, WeakKeyword};

pub enum LexError {
    IO(io::Error),
    Literal(LiteralError),
    IntParse(ParseIntError),
    UnexpectedEOL,
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LexError::IO(err) => write!(f, "Lexer file io: {err}"),
            LexError::Literal(err) => write!(f, "Lex error while lexing literal: {err}"),
            LexError::IntParse(err) => write!(f, "Lex error when parsing int: {err}"),
            LexError::UnexpectedEOL => write!(f, "Unexpected end-of-line during lexing"),
        }
    }
}

pub struct Lexer {
    reader:         BufReader<File>,

    line:           u32,
    column:         u32,
    byte_idx:       usize,
    char_idx:       usize,

    kw_map:         HashMap<&'static str, Token>,
    punct_map:      HashMap<&'static str, Token>,

    trivia:         Trivia,
    toks:           TokenStream,

    line_buf:       String,
    line_offset:    usize,
    line_has_token: bool,

    errors:         Vec<LexError>,
}

impl Lexer {
    const LINE_BUF_MIN_SIZE: usize = 4096;

    pub fn new(source_file: File) -> Self {
        let reader = BufReader::new(source_file);

        // Inefficient, but easy
        let mut kw_map = HashMap::new();
        for kw in StrongKeyword::for_all() {
            kw_map.insert(kw.as_str(), Token::StrongKw(kw));
        }
        for kw in ReservedKeyword::for_all() {
            kw_map.insert(kw.as_str(), Token::ReservedKw(kw));
        }
        for kw in WeakKeyword::for_all() {
            kw_map.insert(kw.as_str(), Token::WeakKw(kw));
        }

        let mut punct_map = HashMap::new();
        for punct in Punctuation::for_all() {
            if let Punctuation::Other(_) = punct { continue; }
            punct_map.insert(punct.as_str(), Token::Punct(punct));
        }

        Self {
            reader,
            line: 0,
            column: 0,
            byte_idx: 0,
            char_idx: 0,
            kw_map,
            punct_map,
            trivia: Trivia::new(),
            toks: TokenStream::new(),
            line_buf: String::with_capacity(Self::LINE_BUF_MIN_SIZE),
            line_offset: 0,
            line_has_token: false,
            errors: Vec::new(),
        }
    }

    pub fn lex(&mut self) -> Result<TokenStream, Vec<LexError>> {
        let mut line = String::new();

        while self.read_line()? {
            let mut idx = 0;
            while let Some(ch) = self.peek_char() {

                match ch {
                    _ if is_whitespace_trivia(ch) => self.lex_whitespace(&line, idx),
                    _ if ch.is_alphabetic() => self.lex_kw_or_name(),
                    _ if ch.is_numeric() => self.lex_numeric_literal(),
                    '\'' => self.lex_char(),
                    _ => self.lex_punctuation(),
                }
            }
        }

        if !self.errors.is_empty() {
            Err(mem::take(&mut self.errors))
        } else {
            Ok(mem::replace(&mut self.toks, TokenStream::new()))
        }
    }

    // UTILITIES

    fn cur_line(&self) -> &str {
        &self.line_buf[self.line_offset..]
    }

    fn offset_from_cur_line(&self, offset: usize) -> &str {
        if self.line_offset + offset >= self.line_buf.len() {
            return "";
        }
        &self.line_buf[self.line_offset + offset..]
    }

    /// Reads a line up to (and including) the next '\n'
    fn read_line(&mut self) -> Result<bool, Vec<LexError>> {
        // bookkeeping
        self.line_offset = 0;
        self.line += 1;
        self.column = 1;
        self.line_has_token = false;

        // actual read
        self.line_buf.clear();
        match self.reader.read_line(&mut self.line_buf) {
            Ok(num_bytes) => Ok(num_bytes != 0),
            Err(err) => Err(vec![LexError::IO(err)]),
        }
    }

    fn peek_char(&self) -> Option<char> {
        if self.line_offset >= self.line_buf.len() {
            return None;
        }
        let line = &self.line_buf[self.line_offset..];
        line.chars().next()
    }
 
    fn read<'a, P: FnMut(char) -> bool>(&'a self, mut pat: P) -> &'a str {
        let line = &self.line_buf[self.line_offset..];
        let len = line.find(|ch| !pat(ch)).unwrap_or(line.len());
        &line[..len]
    }

    fn read_offset<'a, P: FnMut(char) -> bool>(&'a self, offset: usize, mut pat: P) -> &'a str {
        let offset = offset.min(self.line_buf.len() - self.line_offset);
        let offset_line = &self.line_buf[self.line_offset + offset..];
        let len = offset_line.find(|ch| !pat(ch)).unwrap_or(offset_line.len());
        &self.line_buf[self.line_offset + offset..][..len]
    }

    fn read_numeric_offset<'a>(&'a self, offset: usize) -> &'a str {
        let offset = offset.min(self.line_buf.len() - self.line_offset);
        let offset_line = &self.line_buf[self.line_offset + offset..];
        let len = offset_line.find(|ch: char| !(ch.is_numeric() || ch == '_')).unwrap_or(offset_line.len());
        &self.line_buf[self.line_offset + offset..][..len]
    }

    fn read_numeric_hex_offset<'a>(&'a self, offset: usize) -> &'a str {
        let offset = offset.min(self.line_buf.len() - self.line_offset);
        let offset_line = &self.line_buf[self.line_offset + offset..];
        let len = offset_line.find(|ch: char| !(ch.is_numeric() || ('a'..='f').contains(&ch) || ('A'..='F').contains(&ch) || ch == '_')).unwrap_or(offset_line.len());
        &self.line_buf[self.line_offset + offset..][..len]
    }

    fn consume(&mut self, num_bytes: usize) -> usize {
        let num_bytes = num_bytes.min(self.line_buf.len() - self.line_offset);
        let line = &self.line_buf[self.line_offset..][..num_bytes];
        let num_chars = line.chars().count();

        self.consume_cnt(num_bytes, num_chars);

        num_chars
    }

    fn consume_line(&mut self) -> usize {
        let line = self.cur_line();
        let num_chars = line.chars().count();
        self.consume_cnt(line.len(), num_chars);
        num_chars
    }

    fn consume_and_get_meta(&mut self, num_bytes: usize) -> TokenMeta {
        let num_bytes = num_bytes.min(self.line_buf.len() - self.line_offset);
        let line = &self.line_buf[self.line_offset..][..num_bytes];
        let num_chars = line.chars().count();

        let meta = TokenMeta {
            span: Span {
                line: self.line,
                column: self.column,
                byte_offset: self.byte_idx,
                byte_len: num_bytes as u32,
                char_offset: self.char_idx,
                char_len: num_chars as u32,
            },
            trivia: mem::take(&mut self.trivia),
        };

        self.consume_cnt(num_bytes, num_chars);

        meta
    }

    fn consume_invalid_char(&mut self) {
        let line = self.offset_from_cur_line(1);
        let num_bytes = line.find('\'').map_or(line.len(), |len| len + 1);
        self.consume(num_bytes);
    }

    fn consume_cnt(&mut self, num_bytes: usize, num_chars: usize) {
        self.line_offset += num_bytes;
        self.byte_idx += num_bytes;
        self.column += num_chars as u32;
        self.char_idx += num_chars;
    }

    fn add_whitespace_trivia(&mut self, trivia: TriviaElem) {
        if self.line_offset > 0 && self.line_has_token {
            let Some(meta) = self.toks.last_meta_mut() else { unreachable!() };
            meta.trivia.add_trailing(trivia);
        } else {
            self.trivia.add_leading(trivia);
        }
    }

    fn add_token(&mut self, token: Token, meta: TokenMeta) {
        self.line_has_token = true;
        self.toks.push(token, meta);
    }

    // LEXING

    // NOTE: while this does not fully follow the reference, most commonly used characters should be handled correctly
    fn lex_kw_or_name(&mut self) {
        struct PatchMapEntry {
            orig: Token,
            punct: Punctuation,
            trailing_punct: bool,
            token: Token,
        }

        const PATCH_MAP: [PatchMapEntry; 8] = [
            PatchMapEntry { orig: Token::StrongKw(StrongKeyword::As) , punct: Punctuation::Question, trailing_punct: true , token: Token::StrongKw(StrongKeyword::AsQuestion) },
            PatchMapEntry { orig: Token::StrongKw(StrongKeyword::As) , punct: Punctuation::Exclaim , trailing_punct: true , token: Token::StrongKw(StrongKeyword::AsExclaim) },
            PatchMapEntry { orig: Token::StrongKw(StrongKeyword::Let), punct: Punctuation::Question, trailing_punct: true , token: Token::StrongKw(StrongKeyword::LetQuestion) },
            PatchMapEntry { orig: Token::StrongKw(StrongKeyword::Let), punct: Punctuation::Exclaim , trailing_punct: true , token: Token::StrongKw(StrongKeyword::LetExclaim) },
            PatchMapEntry { orig: Token::StrongKw(StrongKeyword::Try), punct: Punctuation::Question, trailing_punct: true , token: Token::StrongKw(StrongKeyword::TryQuestion) },
            PatchMapEntry { orig: Token::StrongKw(StrongKeyword::Try), punct: Punctuation::Exclaim , trailing_punct: true , token: Token::StrongKw(StrongKeyword::TryExclaim) },
            PatchMapEntry { orig: Token::StrongKw(StrongKeyword::Is) , punct: Punctuation::Exclaim , trailing_punct: false, token: Token::StrongKw(StrongKeyword::NotIs) },
            PatchMapEntry { orig: Token::StrongKw(StrongKeyword::In) , punct: Punctuation::Exclaim , trailing_punct: false, token: Token::StrongKw(StrongKeyword::NotIn) },
        ];

        let mut name = self.read(|ch| ch.is_alphanumeric() || ch == '_');
        let mut name_len = name.len();
        let token = match self.kw_map.get(name).cloned() {
            Some(mut tok) => {
                // patching special tokens
                for entry in PATCH_MAP.iter().filter(|entry| entry.orig == tok) {
                    if entry.trailing_punct {
                        if self.offset_from_cur_line(name_len).starts_with(entry.punct.as_str()) {
                            continue;
                        }
                        tok = entry.token.clone();
                        name_len += 1;
                        break;
                    } else {
                        let Some(prev_tok) = self.toks.last_mut() else { continue; };
                        let Token::Punct(punct) = prev_tok else { continue; };
                        if *punct != entry.punct { continue; }
                        *prev_tok = entry.token.clone();
                        
                        let char_len = self.consume(name_len);
                        let Some(last_meta) = self.toks.last_meta_mut() else { unreachable!() };
                        last_meta.span.byte_len += name_len as u32;
                        last_meta.span.char_len += char_len as u32;
                        return;
                    }
                }

                tok
            },
            None => {
                Token::Name(name.to_string())
            },
        };

        let meta = self.consume_and_get_meta(name_len);
        self.add_token(token, meta);
    }

    fn lex_punctuation(&mut self) {
        let punct = self.read(|ch| !(is_whitespace_trivia(ch) || ch.is_alphanumeric()));

        let token = match self.punct_map.get(punct) {
            Some(tok) => tok.clone(),
            None => Token::Punct(Punctuation::Other(punct.to_string())),
        };

        let meta = self.consume_and_get_meta(punct.len());
        self.add_token(token, meta);
    }

    fn lex_whitespace(&mut self, line: &str, idx: usize) {
        let line = &self.line_buf[self.line_offset..];
        let whitespace = self.read(is_whitespace_trivia).to_string();
        let whitespace_len = whitespace.len();

        self.add_whitespace_trivia(TriviaElem::Whitespace(whitespace));
        self.consume(whitespace_len);
    }

    fn lex_numeric_literal(&mut self) {
        let line = self.cur_line();
        if line.starts_with("0b") {
            self.lex_prefixed_literal(LiteralSegment::Binary, |s| Literal::Binary(s))
        } else if line.starts_with("0o") {
            self.lex_prefixed_literal(LiteralSegment::Octal, |s| Literal::Octal(s))
        } else if line.starts_with("0x") {
            self.lex_hexadecimal_literal()
        } else {
            self.lex_decimal_literal()
        }
    }

    fn lex_decimal_literal(&mut self) {
        let integral = self.read_numeric_offset(0).to_string();
        self.check_for_literal_error(&integral, LiteralSegment::DecIntegral);

        let mut cur_offset = integral.len();
        let fraction = if self.offset_from_cur_line(cur_offset).starts_with('.') {
            cur_offset += 1;
            let frac = self.read_numeric_offset(cur_offset).to_string();
            self.check_for_literal_error(&frac, LiteralSegment::DecFraction);

            frac
        } else {
            String::new()
        };
        cur_offset += fraction.len();

        let (exponent, exp_len) = if self.offset_from_cur_line(cur_offset).starts_with('e') {
            cur_offset += 1;
            let tmp = self.offset_from_cur_line(cur_offset);
            let exp_kind = match tmp.as_bytes()[0] {
                b'+' => {
                    cur_offset += 1;
                    '+'
                },
                b'-' => {
                    cur_offset += 1;
                    '-'
                },
                _ => '\0',
            };

            let exp = self.read_numeric_offset(cur_offset).to_string();
            self.check_for_literal_error(&exp, LiteralSegment::DecExponent);

            let exp_len = exp.len();
            let exp = match exp_kind {
                '+' => Exponent::Pos(exp),
                '-' => Exponent::Neg(exp),
                _   => Exponent::Some(exp),
            };
            (exp, exp_len)
        } else {
            (Exponent::None, 0)
        };

        let meta = self.consume_and_get_meta(cur_offset + exp_len);

        let lit = if !fraction.is_empty() || matches!(exponent, Exponent::Neg(_)) {
            Literal::DecimalFloat {
                integral,
                fraction,
                exponent,
            }
        } else {
            Literal::Decimal {
                integral,
                exponent,
            }
        };

        self.add_token(Token::Literal(lit), meta);
    }

    // Even when we error, just return the token, as we could try to compile further to collect other errors
    fn check_for_literal_error(&mut self, lit_str: &str, segment: LiteralSegment) {
        if lit_str.starts_with('_') {
            self.errors.push(LexError::Literal(LiteralError::LeadingUnderscore(segment)));
        }
        if lit_str.ends_with('_') {
            self.errors.push(LexError::Literal(LiteralError::TrailingUnderscore(segment)));
        }

        // Handles numeric characters not in the allowed pattern
        let char_validate = match segment {
            LiteralSegment::DecIntegral |
            LiteralSegment::DecFraction |
            LiteralSegment::DecExponent => |ch| ('0'..='9').contains(&ch),
            LiteralSegment::Binary      => |ch| ch == '0' || ch =='1',
            LiteralSegment::Octal       => |ch| ('0'..='7').contains(&ch),
            LiteralSegment::HexIntegral |
            LiteralSegment::HexFraction |
            LiteralSegment::HexExponent => |ch| ('0'..='9').contains(&ch) || ('a'..='f').contains(&ch) || ('A'..='F').contains(&ch),
        };

        for ch in lit_str.chars().filter(|ch| *ch != '_' && !char_validate(*ch)) {
            self.errors.push(LexError::Literal(LiteralError::UnsupportedDigit(segment, ch)));
        }
    }

    fn lex_prefixed_literal<F>(&mut self, segment: LiteralSegment, gen_lit: F) where 
        F: Fn(String) -> Literal,
    {
        let lit_str = self.read_numeric_offset(2).to_string();
        self.check_for_literal_error(&lit_str, segment);

        let lit_len = lit_str.len();

        let token = Token::Literal(gen_lit(lit_str));
        let meta = self.consume_and_get_meta(lit_len + 2);
        self.add_token(token, meta);
    }

    fn lex_hexadecimal_literal(&mut self) {
        let mut cur_offset = 2; // '0x' prefix
        let integral = self.read_numeric_hex_offset(cur_offset).to_string();
        self.check_for_literal_error(&integral, LiteralSegment::HexIntegral);

        cur_offset += integral.len();
        let fraction = if self.offset_from_cur_line(cur_offset).starts_with('.') {
            cur_offset += 1;
            let frac = self.read_numeric_hex_offset(cur_offset).to_string();
            self.check_for_literal_error(&frac, LiteralSegment::HexFraction);

            frac
        } else {
            String::new()
        };
        cur_offset += fraction.len();

        let (exponent, exp_len) = if self.offset_from_cur_line(cur_offset).starts_with('p') {
            cur_offset += 1;
            if self.offset_from_cur_line(cur_offset).starts_with('x') {
                cur_offset += 1;
                let tmp = self.offset_from_cur_line(cur_offset);
                let exp_kind = match tmp.as_bytes()[0] {
                    b'+' => {
                        cur_offset += 1;
                        '+'
                    },
                    b'-' => {
                        cur_offset += 1;
                        '-'
                    },
                    _ => '\0',
                };

                let exp = self.read_numeric_hex_offset(cur_offset).to_string();
                self.check_for_literal_error(&exp, LiteralSegment::HexExponent);

                let exp_len = exp.len();
                let exp = match exp_kind {
                    '+' => HexExponent::HexPos(exp),
                    '-' => HexExponent::HexNeg(exp),
                    _   => HexExponent::Hex(exp),
                };
                (exp, exp_len)
            } else { 
                let tmp = self.offset_from_cur_line(cur_offset);
                let exp_kind = match tmp.as_bytes()[0] {
                    b'+' => {
                        cur_offset += 1;
                        '+'
                    },
                    b'-' => {
                        cur_offset += 1;
                        '-'
                    },
                    _ => '\0',
                };
                
                let exp = self.read_numeric_hex_offset(cur_offset).to_string();
                self.check_for_literal_error(&exp, LiteralSegment::DecExponent);
                
                let exp_len = exp.len();
                let exp = match exp_kind {
                    '+' => HexExponent::DecPos(exp),
                    '-' => HexExponent::DecNeg(exp),
                    _   => HexExponent::Dec(exp),
                };
                (exp, exp_len)
            }
        } else {
            (HexExponent::None, 0)
        };

        let meta = self.consume_and_get_meta(cur_offset + exp_len);

        let lit = if !fraction.is_empty() || exponent != HexExponent::None {
            if exponent == HexExponent::None {
                self.errors.push(LexError::Literal(LiteralError::HexFloatNoExp));
            }

            Literal::HexadecimalFloat {
                integral,
                fraction,
                exponent,
            }
        } else {
            Literal::Hexadecimal(integral)
        };

        self.add_token(Token::Literal(lit), meta);
    }

    fn lex_char(&mut self) {
        if self.offset_from_cur_line(1).starts_with('\\') {
            if let Some((sequence, len)) = self.lex_escape_sequence(1) {
                let meta = self.consume_and_get_meta(len);
                let token = Token::Literal(Literal::Char(CharLiteral::Escape(sequence)));
                self.add_token(token, meta);
            }
        } else {
            let mut chars = self.offset_from_cur_line(1).chars();
            let Some(ch) = chars.next() else {
                self.errors.push(LexError::UnexpectedEOL);
                self.consume_line();
                return;
            };

            match chars.next() {
                Some(ch) => if ch != '\'' {
                    self.errors.push(LexError::Literal(LiteralError::InvalidCharacterLiteral("Literal was not closed with a `'`".to_string())));
                    self.consume_line();
                    return;
                },
                None => {
                    self.errors.push(LexError::UnexpectedEOL);
                    self.consume_line();
                    return;
                },
            }

            let meta = self.consume_and_get_meta(ch.len_utf8() + 2);
            let token = Token::Literal(Literal::Char(CharLiteral::Char(ch)));
            self.add_token(token, meta);
        }
    }

    // Within string literals, this is used only to validate the actual sequence

    /// Lexes an escape sequence at a given offset, the offset is the location of the `\``
    /// 
    ///  Returns the escape sequence and length in bytes
    fn lex_escape_sequence(&mut self, offset: usize) -> Option<(EscapeSequence, usize)> {
        let sequence = self.offset_from_cur_line(offset + 1);
        let mut chars = sequence.chars();
        let Some(ch) = chars.next() else {
            self.errors.push(LexError::UnexpectedEOL);
            self.consume_line();
            return None;
        };

        match ch {
            '0'  => Some((EscapeSequence::Null, 4)),
            't'  => Some((EscapeSequence::Tab, 4)),
            'n'  => Some((EscapeSequence::Newline, 4)),
            'r'  => Some((EscapeSequence::CariageReturn, 4)),
            '"'  => Some((EscapeSequence::DblQuote, 4)),
            '\'' => Some((EscapeSequence::Quote, 4)),
            '\\' => Some((EscapeSequence::Backslash, 4)),
            'p'  => Some((EscapeSequence::SystemNewline, 4)),
            'x'  => {
                
                let hex_val = match u8::from_str_radix(&sequence[1..3], 16) {
                    Ok(val) => val,
                    Err(err) => {
                        self.errors.push(LexError::IntParse(err));
                        self.consume_invalid_char();
                        return None;
                    },
                };
                Some((EscapeSequence::Hex(hex_val), 6))
            },
            'u' => {
                if chars.next() != Some('{') {
                    self.errors.push(LexError::Literal(LiteralError::InvalidUnicodeEscape("No leading '{' found after 'u'".to_string())));
                    self.consume_invalid_char();
                    return None;
                }

                let inner_sequence = &sequence[2..];
                let code_end = inner_sequence.find('}').unwrap_or(inner_sequence.len());
                let code = &inner_sequence[..code_end];

                if code_end > 8 {
                    self.errors.push(LexError::Literal(LiteralError::InvalidUnicodeEscape("Only 8 hex digits are allowed".to_string())));
                    self.consume_invalid_char();
                    return None;
                }

                let hex_val = match u32::from_str_radix(code, 16) {
                    Ok(val) => val,
                    Err(err) => {
                        self.errors.push(LexError::IntParse(err));
                        self.consume_invalid_char();
                        return None;
                    },
                };

                let mut chars = inner_sequence[code_end..].chars();
                if chars.next() != Some('}') {
                    self.errors.push(LexError::Literal(LiteralError::InvalidUnicodeEscape("No trailing '}' found".to_string())));
                    self.consume_invalid_char();
                    return None;
                }
                if chars.next() != Some('\'') {
                    self.errors.push(LexError::Literal(LiteralError::InvalidUnicodeEscape("Character literal was not ended on a `'`".to_string())));
                    self.consume_invalid_char();
                    return None;
                }

                // "'\u{" + code + "}'"
                let len = 4 + code.len() + 2;

                Some((EscapeSequence::Unicode(hex_val), len))
            }
            _ => {
                self.errors.push(LexError::Literal(LiteralError::UnexpectEscape(ch)));
                None
            }
        }
    }
}