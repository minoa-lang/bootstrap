#![allow(unused)]

use core::num;
use std::{
    collections::HashMap,
    fmt,
    fs::File,
    io::{self, BufRead, BufReader},
    mem,
    num::ParseIntError,
    ops::RangeBounds
};

use bootstrap_macros::enum_utils;

use crate::tokens::{get_delim_len, is_whitespace_trivia, CharLiteral, Delimiter, EscapeSequence, Exponent, HexExponent, Literal, LiteralError, LiteralSegment, PatternKeyword, PatternKeywordEndianness, Punctuation, ReservedKeyword, Span, StrongKeyword, Token, TokenMeta, TokenStream, TokenTreeBuildError, TokenTreeBuilder, Trivia, TriviaElem, WeakKeyword};

#[enum_utils(display)]
pub enum LexError {
    #[fmt("Lexer file io: {_0}")]
    IO(io::Error),
    #[fmt("Lexer error while lexing literal: {_0}")]
    Literal(LiteralError),
    #[fmt("Lex error when parsing int: {_0}")]
    IntParse(ParseIntError),
    #[fmt("Unexpected end-of-line during lexing")]
    UnexpectedEOL,
    #[fmt("Missing space after comment start")]
    MissingSpaceInComment,
    #[fmt("Suffix doc comment may not appear before any token has been parsed")]
    SuffixDocCommentBeforeToken,
    #[fmt("String literal was not closed")]
    StringNoEnd,
    #[fmt("A raw string can be prefixed with at most 255 #'s")]
    RawStringTooDeep,
    #[fmt("Raw string literal was not closed")]
    RawStringNoEnd,
    #[fmt("Trying to close a sub-token tree when none is available")]
    EndOfRootTree,
    #[fmt("Missing matching closing delimiters: {_0}")]
    MissingDelimiter(String),
    #[fmt("No matching open delimiter for '{_0}'")]
    NoOpenDelimiter(&'static str),
    #[fmt("Closing unmatched delimiter, expected '{expected}', found {found}")]
    UnmatchedDelimiter{
        expected: &'static str,
        found: &'static str,
    },
    #[fmt("Pattern keyword has too many bits: {_0}, max: 65535")]
    ToManyBitsForPattern(u64),
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
    tok_tree:       TokenTreeBuilder,

    line_buf:       String,
    line_offset:    usize,
    line_has_token: bool,

    in_multi_str:   bool,
    interp_stack:   Vec<u32>,
    raw_str_end:    String,

    errors:         Vec<(Span, LexError)>,
}

impl Lexer {
    const LINE_BUF_MIN_SIZE: usize = 4096;

    pub fn new(source_file: File, path: &str) -> Self {
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
            toks: TokenStream::new(format!("file:/{path}")),
            tok_tree: TokenTreeBuilder::new(),
            line_buf: String::with_capacity(Self::LINE_BUF_MIN_SIZE),
            line_offset: 0,
            line_has_token: false,
            in_multi_str: false,
            interp_stack: Vec::new(),
            raw_str_end: String::with_capacity(256),
            errors: Vec::new(),
        }
    }

    pub fn lex(&mut self) -> Result<TokenStream, Vec<(Span, LexError)>> {
        while self.read_line()? {
            self.string_ended_check();
            while let Some(ch) = self.peek_char() {
                match ch {
                    _ if is_whitespace_trivia(ch) => self.lex_whitespace(),
                    _ if ch.is_alphabetic() => self.lex_kw_or_name(),
                    _ if ch.is_ascii_digit() => self.lex_numeric_literal(),
                    '\'' => self.lex_char(),
                    '/'  => self.lex_comment_or_punctuation(),
                    '_'  => self.let_name_or_underscore(),
                    '"'  => self.lex_string(),
                    '`'  => self.lex_raw_string(),
                    '#'  => self.lex_raw_string_or_punct(),
                    '.'  => self.lex_dot(),
                    '}'  => if self.interp_indent_depth() == Some(0) {
                        self.lex_string();
                    } else {
                        self.lex_punctuation();
                    }
                    _    => self.lex_punctuation(),
                }
            }
        }
        let end_idx = if self.toks.is_empty() { 0 } else { (self.toks.len() - 1) as u32 };
        self.string_ended_check();

        if !self.errors.is_empty() {
            Err(mem::take(&mut self.errors))
        } else {
            let tree_builder = mem::replace(&mut self.tok_tree, TokenTreeBuilder::new());
            let tree = match tree_builder.finalize(&self.toks) {
                Ok(tree) => tree,
                Err(err) => {
                    self.add_tree_build_errors(err);
                    return Err(mem::take(&mut self.errors))
                },
            };

            let mut stream = mem::replace(&mut self.toks, TokenStream::new(String::new()));
            stream.set_tree_data(tree);
            Ok(stream)
        }
    }

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
                // Now check if this might be a pattern keyword
                let name = name.to_string();

                // We should never get here is name is empty
                let ch = name.chars().next().unwrap();
                match ch {
                    'i' | 'u' | 'f' | 'b' => 'block: {
                        let mut numeric_part = &name[1..];
                        let endianness = if numeric_part.ends_with("le") {
                            numeric_part = &numeric_part[..numeric_part.len() - 2];
                            PatternKeywordEndianness::Le
                        } else if numeric_part.ends_with("be") {
                            numeric_part = &numeric_part[..numeric_part.len() - 2];
                            PatternKeywordEndianness::Be
                        } else if numeric_part.ends_with(|ch: char| ch.is_numeric()) {
                            PatternKeywordEndianness::None
                        } else {
                            break 'block;
                        };

                        let bits = match numeric_part.parse::<u64>() {
                            Ok(bits) => bits,
                            Err(_) => break 'block,
                        };

                        if bits > u16::MAX as u64 {
                            self.add_error(Some(name_len), LexError::ToManyBitsForPattern(bits));
                            break 'block;
                        }
                        let bits = bits as u16;

                        let pat = match ch {
                            'i' => PatternKeyword::I { bits, endianness },
                            'u' => PatternKeyword::U { bits, endianness },
                            'f' => PatternKeyword::F { bits, endianness },
                            'b' => PatternKeyword::B { bits, endianness },
                            _ => unreachable!(),
                        };

                        let meta = self.consume_and_get_meta(name_len);
                        self.add_token(Token::PatternKw(pat), meta);
                        return;
                    }
                    _ => (),
                }

                Token::Name(name)
            },
        };

        let meta = self.consume_and_get_meta(name_len);
        self.add_token(token, meta);
    }

    fn let_name_or_underscore(&mut self) {
        let mut name = self.read(|ch| ch.is_alphanumeric() || ch == '_').to_string();
        let meta = self.consume_and_get_meta(name.len());
        let token = if name == "_" {
            Token::Punct(Punctuation::Underscore)
        } else {
            Token::Name(name.to_string())
        };
        self.add_token(token, meta);
    }

    fn lex_punctuation(&mut self) {
        let mut punct = self.read_find_from_offset(1, |ch| !(is_whitespace_trivia(ch) || ch.is_alphanumeric()));

        // Find the next delimiter, as stop after it, or if we are at the start, get it's lenght

        let single_puncts = &[
            '(',
            ')',
            '{',
            '}',
            '[',
            ']',
        ];

        let end_offset = match punct.find("[<") {
            Some(0) => {
                let meta = self.consume_and_get_meta(2);
                self.add_token(Token::OpenDelim(Delimiter::Vector), meta);
                return;
            },
            Some(offset) => Some(offset),
            None => None,
        };
        let end_offset = match punct.find(">]") {
            Some(0) => {
                let meta = self.consume_and_get_meta(2);
                self.add_token(Token::CloseDelim(Delimiter::Vector), meta);
                return;
            },
            Some(offset) => Some(offset),
            None => end_offset,
        };
        let end_offset = match punct.find(single_puncts) {
            Some(0) => {
                let token = match punct.chars().next().unwrap() {
                    '(' => Token::OpenDelim(Delimiter::Parenthesis),
                    ')' => Token::CloseDelim(Delimiter::Parenthesis),
                    '[' => Token::OpenDelim(Delimiter::Bracket),
                    ']' => Token::CloseDelim(Delimiter::Bracket),
                    '{' => {
                        self.increment_interp_indent();
                        Token::OpenDelim(Delimiter::Brace)
                    },
                    '}' => {
                        self.decrement_interp_indent();
                        Token::CloseDelim(Delimiter::Brace)
                    },
                    _ => unreachable!()
                };
                let meta = self.consume_and_get_meta(1);
                self.add_token(token, meta);
                return;
            }
            Some(offset) => Some(offset),
            None => end_offset,
        };

        let end_offset = end_offset.unwrap_or(punct.len());
        let punct = &punct[..end_offset];

        let token = match self.punct_map.get(punct) {
            Some(tok) => tok.clone(),
            None => Token::Punct(Punctuation::Other(punct.to_string())),
        };
        
        let meta = self.consume_and_get_meta(end_offset);
        self.add_token(token, meta);
    }

    fn lex_comment_or_punctuation(&mut self) {
        let line = self.cur_line();
        if line.starts_with("//") {
            self.lex_comment();
        } else {
            self.lex_punctuation();
        }
    }

    fn lex_dot(&mut self) {
        let mut chars = self.offset_from_cur_line(1).chars();
        if let Some(ch) = chars.next() && ch.is_ascii_digit() {
            let dot_token = Token::Punct(Punctuation::Dot);
            let dot_meta = self.consume_and_get_meta(1);
            self.add_token(dot_token, dot_meta);

            self.lex_decimal_literal(true);
        } else {
            self.lex_punctuation();
        }
    }

    fn lex_whitespace(&mut self) {
        let line = &self.line_buf[self.line_offset..];
        let whitespace = self.read(is_whitespace_trivia).to_string();
        let whitespace_len = whitespace.len();

        self.add_trivia(TriviaElem::Whitespace(whitespace));
        self.consume(whitespace_len);
    }

    fn lex_comment(&mut self) {
        let line = self.offset_from_cur_line(1);
        let mut chars = line.chars();
        let Some(comment_kind) = chars.next() else {
            self.add_error(Some(2), LexError::UnexpectedEOL);
            self.consume_line();
            return;
        };

        let line = match comment_kind {
            '/' => &line[2..],
            '!' => &line[2..],
            '<' => &line[2..],
            ' ' => &line[1..],
            _   => &line,
        };

        // common check for space after comment introducer
        let (line, is_missing_space) = match chars.next() {
            None      => ("".to_string(), false),
            Some(' ') => (line.to_string(), false),
            _         => (line.to_string(), true)
        };

        let trivia = match comment_kind {
            '/' => TriviaElem::DocComment(line.to_string()),
            '!' => TriviaElem::TopLevelDocComment(line.to_string()),
            '<' => TriviaElem::SuffixDocComment(line.to_string()),
            ' ' => TriviaElem::Comment(line.to_string()),
            _   => TriviaElem::Comment(line.to_string()),
        };

        if is_missing_space {
            self.add_error(Some(line.len()), LexError::MissingSpaceInComment);
        }
        self.add_trivia(trivia);
        self.consume_line();
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
            self.lex_decimal_literal(false)
        }
    }

    fn lex_decimal_literal(&mut self, int_only: bool) {
        let integral = self.read_numeric_offset(0).to_string();
        self.check_for_literal_error(0, &integral, LiteralSegment::DecIntegral);

        if int_only {
            let meta = self.consume_and_get_meta(integral.len());
            let token = Token::Literal(Literal::Decimal { integral: integral, exponent: Exponent::None });
            self.add_token(token, meta);
            return;
        }

        let mut cur_offset = integral.len();
        let fraction_str = self.offset_from_cur_line(cur_offset);
        let fraction = if fraction_str.starts_with('.') && fraction_str[1..].starts_with(|ch: char| ch.is_ascii_digit()) {
            cur_offset += 1;
            let frac = self.read_numeric_offset(cur_offset).to_string();
            self.check_for_literal_error(cur_offset, &frac, LiteralSegment::DecFraction);

            frac
        } else {
            String::new()
        };
        cur_offset += fraction.len();

        let exponent_str = self.offset_from_cur_line(cur_offset);
        let (exponent, exp_len) = 'exp_lex: { if exponent_str.starts_with('e') && exponent_str[1..].starts_with(|ch: char| ch == '+' || ch == '-' || ch.is_ascii_digit()) {
            cur_offset += 1;
            let tmp = self.offset_from_cur_line(cur_offset);

            // guarded by second condition
            let exp_kind = tmp.chars().next().unwrap();
            if exp_kind == '+' || exp_kind == '-' {
                cur_offset += 1;
            }

            let exp = self.read_numeric_offset(cur_offset).to_string();
            self.check_for_literal_error(cur_offset, &exp, LiteralSegment::DecExponent);

            let exp_len = exp.len();
            let exp = match exp_kind {
                '+' => Exponent::Pos(exp),
                '-' => Exponent::Neg(exp),
                _   => Exponent::Some(exp),
            };
            (exp, exp_len)
        } else {
            (Exponent::None, 0)
        }};

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
    fn check_for_literal_error(&mut self, offset: usize, lit_str: &str, segment: LiteralSegment) {
        let err_len = Some(lit_str.len() + offset);

        if lit_str.starts_with('_') {
            self.add_error(err_len, LexError::Literal(LiteralError::LeadingUnderscore(segment)));
        }
        if lit_str.ends_with('_') {
            self.add_error(err_len, LexError::Literal(LiteralError::TrailingUnderscore(segment)));
        }

        if lit_str.len() == 0 {
            self.add_error(err_len, LexError::Literal(LiteralError::EmptyLiteral(segment)));
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
            self.add_error(err_len, LexError::Literal(LiteralError::UnsupportedDigit(segment, ch)));
        }
    }

    fn lex_prefixed_literal<F>(&mut self, segment: LiteralSegment, gen_lit: F) where 
        F: Fn(String) -> Literal,
    {
        let lit_str = self.read_numeric_offset(2).to_string();
        self.check_for_literal_error(2, &lit_str, segment);

        let lit_len = lit_str.len();

        let token = Token::Literal(gen_lit(lit_str));
        let meta = self.consume_and_get_meta(lit_len + 2);
        self.add_token(token, meta);
    }

    fn lex_hexadecimal_literal(&mut self) {
        let mut cur_offset = 2; // '0x' prefix
        let integral = self.read_numeric_hex_offset(cur_offset).to_string();
        self.check_for_literal_error(2, &integral, LiteralSegment::HexIntegral);

        cur_offset += integral.len();
        let fraction = if self.offset_from_cur_line(cur_offset).starts_with('.') {
            cur_offset += 1;
            let frac = self.read_numeric_hex_offset(cur_offset).to_string();
            self.check_for_literal_error(cur_offset, &frac, LiteralSegment::HexFraction);

            frac
        } else {
            String::new()
        };
        cur_offset += fraction.len();

        let (exponent, exp_len) = if self.offset_from_cur_line(cur_offset).starts_with('p') {
            cur_offset += 1;
            let exp_indicator = self.offset_from_cur_line(cur_offset);

            let (exp_indicator, is_hex_exp) = if exp_indicator.starts_with('x') {
                cur_offset += 1;
                (&exp_indicator[1..], true)
            } else {
                (exp_indicator, false)
            };

            if exp_indicator.starts_with(|ch: char| ch == '+' || ch == '-' || ch.is_ascii_hexdigit()) {
                // guarded by condition
                let exp_kind = exp_indicator.chars().next().unwrap();
                if exp_kind == '+' || exp_kind == '-' {
                    cur_offset += 1;
                }

                let exp = if is_hex_exp {
                    self.read_numeric_hex_offset(cur_offset)
                } else {
                    self.read_numeric_offset(cur_offset)
                }.to_string();

                self.check_for_literal_error(cur_offset, &exp, LiteralSegment::HexExponent);

                let exp_len = exp.len();
                let exp = match exp_kind {
                    '+' => HexExponent::HexPos(exp),
                    '-' => HexExponent::HexNeg(exp),
                    _   => HexExponent::Hex(exp),
                };
                (Some(exp), exp_len)
            } else {
                (None, 0)
            }
        } else {
            (None, 0)
        };

        let lit = if let Some(exponent) = exponent {
            cur_offset += exp_len;
            Literal::HexadecimalFloat {
                integral,
                fraction,
                exponent,
            }
        } else {
            cur_offset = 2 + integral.len();
            Literal::Hexadecimal(integral)
        };

        let meta = self.consume_and_get_meta(cur_offset);
        self.add_token(Token::Literal(lit), meta);
    }

    fn lex_char(&mut self) {
        let (tok, len) = if self.offset_from_cur_line(1).starts_with("\\") {
            let sequence = self.lex_escape_sequence(1);
            let len = sequence.len();

            let token = Token::Literal(Literal::Char(CharLiteral::Escape(sequence)));
            (token, len)
        } else {
            let mut chars = self.offset_from_cur_line(1).chars();
            let Some(ch) = chars.next() else {
                self.add_char_lit_error(LexError::UnexpectedEOL);
                self.consume_line();
                return;
            };

            let len = ch.len_utf8();
            let token = Token::Literal(Literal::Char(CharLiteral::Char(ch)));
            (token, len)
        };

        // check index of ' , as we need to handle it closing after multiple values
        match self.offset_from_cur_line(len + 1).find('\'') {
            Some(offset) => if offset != 0 {
                self.add_char_lit_error(LexError::Literal(LiteralError::InvalidCharacterLiteral("Literal was not closed with a `'`".to_string())));
                self.consume_line();
            },
            None => {
                self.add_char_lit_error(LexError::UnexpectedEOL);
                self.consume_line();
            },
        }
        
        let meta = self.consume_and_get_meta(len + 2);  // + 2 for outer `'`s
        self.add_token(tok, meta);
    }

    // TODO: String escape sequences validation

    // Does not handle interpolated strings yet
    fn lex_string(&mut self) {
        let inner = &self.cur_line();
        let is_continuation = inner.starts_with("}");
        let is_start = inner.starts_with("\"");

        if let Some(depth) = self.interp_indent_depth() {
            if depth > 0 && is_continuation {
                self.lex_punctuation();
                return;
            }

            if is_start && !self.line_has_token {
                let token = Token::Literal(Literal::InterpIndicator);
                let meta = self.consume_and_get_meta(1);
                self.add_token(token, meta);
                return;
            }
        }

        #[derive(Clone, Copy, PartialEq, Eq)]
        enum EndKind {
            None,
            End,
            Interp,
        }
        
        let inner = self.offset_from_cur_line(1);
        let mut end_kind = EndKind::None;
        let mut prev_char_is_escape = false;
        let end = inner.find(|ch: char|
            match ch {
            '\\' => {
                prev_char_is_escape = true;
                false
            },
            '"' => if prev_char_is_escape {
                prev_char_is_escape = false;
                false
            } else {
                end_kind = EndKind::End;
                true
            }
            '{' if prev_char_is_escape => {
                end_kind = EndKind::Interp;
                true
            },
            _ => {
                prev_char_is_escape = false;
                false
            },
        }).unwrap_or(inner.len() - self.eol_size());

        let inner = &inner[..end - ((end_kind == EndKind::Interp) as usize)];

        let token = match end_kind {
            EndKind::End => if is_continuation {
                Token::Literal(Literal::InterpString {
                    includes_end: true,
                    includes_start: false,
                    content: inner.to_string(),
                })
            } else {
                Token::Literal(Literal::String(inner.to_string()))
            },
            EndKind::Interp => {
                let newline = !inner.ends_with('\\');
                let inner = &inner[..inner.len() - (!newline as usize)];
                Token::Literal(Literal::MultiInterpString { 
                    includes_start: true,
                    includes_end: is_continuation,
                    content: inner.to_string(),
                    newline
                })
            },
            EndKind::None => if is_continuation {
                Token::Literal(Literal::InterpString{
                    includes_end: is_continuation,
                    includes_start: false,
                    content: inner.to_string()
                })
            } else {
                let newline = !inner.ends_with('\\');
                let inner = &inner[..inner.len() - (!newline as usize)];
                Token::Literal(Literal::MultiStringSegment{ content: inner.to_string(), newline })
            },
        };

        let meta = self.consume_and_get_meta(inner.len() + (end_kind as usize) + 1);
        self.add_token(token, meta);

        if end_kind == EndKind::Interp {
            self.push_interp_indent();
        } else if is_continuation {
            self.pop_interp_indent();
        }
        self.in_multi_str = end_kind != EndKind::End;
    }

    /// Lexes raw strings starting with '`'
    fn lex_raw_string(&mut self) {
        let inner = &self.offset_from_cur_line(1);

        let end = if !self.raw_str_end.is_empty() {
            &self.raw_str_end
        } else {
            "`"
        };

        let (inner, token, is_end) = match self.offset_from_cur_line(1).find(end) {
            Some(end) => {
                let inner = &inner[..end];
                let token = Token::Literal(Literal::RawString{ depth: 0, content: inner.to_string() });
                (inner, token, true)
            }
            None => {
                let end = inner.len() - self.eol_size();
                let newline = !inner[..end].ends_with('\\');
                let inner = &inner[..end - (!newline as usize)];
                let token = Token::Literal(Literal::MultiRawStringSegment{ depth: 0, content: inner.to_string(), newline });
                (inner, token, false)
            },
        };

        let token_len = inner.len() + (is_end as usize) + 1;
        if is_end {
            self.raw_str_end.clear();
        } else if self.raw_str_end.is_empty() {
            self.raw_str_end.push('`');
        }
        let meta = self.consume_and_get_meta(token_len);
        self.add_token(token, meta);
    }

    /// Lexer raw string starting with '#'
    fn lex_raw_string_or_punct(&mut self) {
        let inner = self.cur_line();
        let depth = inner.find(|ch: char| ch != '#').unwrap_or(inner.len());

        let invalid_depth = depth > 255;
        let is_raw_str = inner[depth..].starts_with('`');
        
        if !is_raw_str {
            self.lex_punctuation();
            return;
        }
        
        self.raw_str_end.push('`');
        for _ in 0..depth {
            self.raw_str_end.push('#');
        }
        let inner = self.offset_from_cur_line(depth + 1);
        
        let (inner, token, is_end) = match inner.find(&self.raw_str_end) {
            Some(end) => {
                let inner = &inner[..end];
                let token = Token::Literal(Literal::RawString{ depth, content: inner.to_string() });
                (inner, token, true)
            },
            None => {
                let end = inner.len() - self.eol_size();
                let newline = !inner[..end].ends_with('\\');
                let inner = &inner[..end - (!newline as usize)];
                let token = Token::Literal(Literal::MultiRawStringSegment{ depth, content: inner.to_string(), newline });
                (inner, token, false)
            },
        };

        let token_len = inner.len() + ((is_end as usize) + 1) * self.raw_str_end.len();
        if depth > 255 {
            self.add_error(Some(token_len), LexError::RawStringTooDeep);
        }
        if is_end {
            self.raw_str_end.clear();
        }
        
        let meta = self.consume_and_get_meta(token_len);
        self.add_token(token, meta);
    }

    
    /// Lexes an escape sequence at a given offset, the offset is the location of the `\``
    /// 
    /// Returns the escape sequence and length in bytes
    //
    // Within string literals, this is used only to validate the actual sequence
    //
    // If the sequence is not supported, EscapeSequence::Unsupported(...) is returned, this could be used within meta-functions
    fn lex_escape_sequence(&mut self, offset: usize) -> EscapeSequence {

        // Need to get it as a string, as `add_char_lit_error` wants to mutably borrow the entire self, although it does not actually need it
        let sequence = self.offset_from_cur_line(offset + 1);
        let Some(ch) = sequence.chars().next() else {
            self.add_char_lit_error(LexError::UnexpectedEOL);
            return EscapeSequence::Unsupported("\\".to_string());
        };

        match ch {
            '0'  => EscapeSequence::Null,
            't'  => EscapeSequence::Tab,
            'n'  => EscapeSequence::Newline,
            'r'  => EscapeSequence::CariageReturn,
            '"'  => EscapeSequence::DblQuote,
            '\'' => EscapeSequence::Quote,
            '\\' => EscapeSequence::Backslash,
            'p'  => EscapeSequence::SystemNewline,
            '}'  => EscapeSequence::ClosingBrace,
            ','  => EscapeSequence::Comma,
            'x'  => {
                    let sequence = &sequence[1..];
                    let end = sequence.find(|ch: char| !ch.is_ascii_hexdigit()).unwrap_or(sequence.len());
                    
                    if end != 2 {
                        // + 2 for leading '\x'
                        let sequence = self.offset_from_cur_line(offset)[..end + 2].to_string();
                        self.add_char_lit_error(LexError::Literal(LiteralError::InvalidUnicodeEscape(format!("Expected exactly 2 hex characters after 'x', found {end}"))));
                        EscapeSequence::Unsupported(sequence)
                    } else {
                        let code = &sequence[..end];
                        EscapeSequence::Hex(code.to_string())
                    }
                },
            'u' => {
                let has_valid_start = sequence.starts_with("u{");
                let search_sequence = &sequence[1 + (has_valid_start as usize)..];

                let mut has_valid_end = false;
                let (code_len, hit_eol) = match search_sequence.find(|ch: char| {
                    has_valid_end = ch == '}';
                    has_valid_end || ch == '\''
                }) {
                    Some(len) => (len, false),
                    None => (search_sequence.len(), true),
                };
                let code = search_sequence[..code_len].to_string();

                // +2 for leading '\u'
                let seq_end = code.len() + 2 + (has_valid_start as usize) + (has_valid_end as usize);

                let mut is_valid = true;
                if !has_valid_start {
                    is_valid = false;
                    self.add_char_lit_error(LexError::Literal(LiteralError::InvalidUnicodeEscape("No leading '{' found after 'u'".to_string())));
                }
                if !has_valid_end {
                    is_valid = false;
                    self.add_char_lit_error(LexError::Literal(LiteralError::InvalidUnicodeEscape("No trailing '}' found".to_string())));
                }

                if code.find(|ch: char| !ch.is_ascii_hexdigit()).is_some() {
                    is_valid = false;
                    self.add_char_lit_error(LexError::Literal(LiteralError::InvalidUnicodeEscape(format!("Only hex digits are allowed, found {}", code))));
                } else if code.len() > 8 {
                    is_valid = false;
                    self.add_char_lit_error(LexError::Literal(LiteralError::InvalidUnicodeEscape(format!("Only up to 8 hex digits are allowed, found {}", code.len()))));
                }
                if hit_eol {
                    self.add_char_lit_error(LexError::UnexpectedEOL);
                }

                if is_valid {
                    EscapeSequence::Unicode(code)
                } else {
                    EscapeSequence::Unsupported(self.offset_from_cur_line(offset)[..seq_end].to_string())
                }
            }
            _ => {
                let sequence = self.offset_from_cur_line(offset)[..2].to_string();
                self.add_char_lit_error(LexError::Literal(LiteralError::UnexpectEscape(ch)));
                EscapeSequence::Unsupported(sequence)
            }
        }
    }

    fn string_ended_check(&mut self) {
        let (string_start, raw_string_start) = match self.cur_line().find(|ch: char| !is_whitespace_trivia(ch)) {
            Some(offset) => {
                let post_whitespace = &self.offset_from_cur_line(offset);
                let string_start = post_whitespace.starts_with('"');
                let raw_string_start = post_whitespace.starts_with('`');
                (string_start, raw_string_start)
            }
            None => (false, false),
        };
        
        if !string_start && self.in_multi_str {
            self.add_error(None, LexError::StringNoEnd);
            self.in_multi_str = false;
        }
        if !raw_string_start && !self.raw_str_end.is_empty() {
            self.add_error(None, LexError::RawStringNoEnd);
            self.raw_str_end.clear();
        }
    }
}

// Utilities
impl Lexer {
    
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
    fn read_line(&mut self) -> Result<bool, Vec<(Span, LexError)>> {
        // bookkeeping
        self.line_offset = 0;
        self.line += 1;
        self.column = 1;
        self.line_has_token = false;

        // actual read
        self.line_buf.clear();
        match self.reader.read_line(&mut self.line_buf) {
            Ok(num_bytes) => Ok(num_bytes != 0),
            Err(err) => Err(vec![(Span::default(), LexError::IO(err))]),
        }
    }

    fn peek_char(&self) -> Option<char> {
        if self.line_offset >= self.line_buf.len() {
            return None;
        }
        let line = &self.line_buf[self.line_offset..];
        line.chars().next()
    }

    fn eol_size(&self) -> usize {
        let bytes = self.line_buf.as_bytes();
        let len = self.line_buf.len();
        match len {
            0 => 0,
            1 => (bytes[0] == b'\n') as usize,
            _ => if bytes[len - 1] == b'\n' {
                1 + ((bytes[len - 2] == b'\r') as usize)
            } else {
                0
            }
        }
    }
 
    fn read<'a, P: FnMut(char) -> bool>(&'a self, mut pat: P) -> &'a str {
        let line = &self.line_buf[self.line_offset..];
        let len = line.find(|ch| !pat(ch)).unwrap_or(line.len());
        &line[..len]
    }

    fn read_find_from_offset<'a, P: FnMut(char) -> bool>(&'a self, offset: usize, mut pat: P) -> &'a str {
        let line = &self.line_buf[self.line_offset + offset..];
        let len = line.find(|ch| !pat(ch)).map_or(line.len() + offset, |len| len + offset);
        &self.line_buf[self.line_offset..][..len]
    }

    fn read_at_offset<'a, P: FnMut(char) -> bool>(&'a self, offset: usize, mut pat: P) -> &'a str {
        let offset = offset.min(self.line_buf.len() - self.line_offset);
        let offset_line = &self.line_buf[self.line_offset + offset..];
        let len = offset_line.find(|ch| !pat(ch)).unwrap_or(offset_line.len());
        &self.line_buf[self.line_offset + offset..][..len]
    }

    fn read_numeric_offset<'a>(&'a self, offset: usize) -> &'a str {
        self.read_at_offset(offset, |ch: char| ch.is_ascii_digit() || ch == '_')
    }

    fn read_numeric_hex_offset<'a>(&'a self, offset: usize) -> &'a str {
        self.read_at_offset(offset, |ch: char| ch.is_ascii_hexdigit() || ch == '_')
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
        let num_bytes = line.find('\'').map_or(line.len(), |len| len + 2);
        self.consume(num_bytes);
    }

    fn consume_cnt(&mut self, num_bytes: usize, num_chars: usize) {
        self.line_offset += num_bytes;
        self.byte_idx += num_bytes;
        self.column += num_chars as u32;
        self.char_idx += num_chars;
    }

    fn add_trivia(&mut self, trivia: TriviaElem) {
        if matches!(trivia, TriviaElem::SuffixDocComment(_)) {
            match self.toks.last_meta_mut() {
                Some(meta) => meta.trivia.add_trailing(trivia),
                None => {
                    self.add_error(Some(self.cur_line().len()), LexError::SuffixDocCommentBeforeToken);
                    self.trivia.add_leading(trivia);
                },
            }
        } else if self.line_offset > 0 && self.line_has_token {
            let Some(meta) = self.toks.last_meta_mut() else { unreachable!() };
            meta.trivia.add_trailing(trivia);
        } else {
            self.trivia.add_leading(trivia);
        }
    }

    fn add_token(&mut self, token: Token, meta: TokenMeta) {
        let tok_idx = self.toks.len()as u32;

        let (should_begin, should_end) = match token {
            Token::OpenDelim(_) => (true, false),
            Token::CloseDelim(delim) => if tok_idx == 0 {
                self.add_error(Some(token.as_str().len()), LexError::NoOpenDelimiter(delim.as_close_str()));
                (false, false)
            } else {
                (false, true)
            },
            Token::Literal(Literal::InterpString { includes_end, includes_start, .. }) => (includes_start, includes_end),
            Token::Literal(Literal::MultiInterpString { includes_end, includes_start, .. }) => (includes_start, includes_end),
            _ => (false, false),
        };


        self.line_has_token = true;
        self.toks.push(token, meta);

        if should_end {
            if let Err(err) = self.tok_tree.end_subtree(tok_idx, &self.toks) {
                self.add_tree_build_errors(err);
            }
        }

        if should_begin {
            self.tok_tree.begin_subtree(tok_idx);
        }
    }

    fn add_error(&mut self, byte_len: Option<usize>, err: LexError) {
        let span = match byte_len {
            Some(len) if len != 0 => {
                let len = len - 1;
                let line = &self.cur_line()[..len];
                let char_len = line.chars().count() as u32;

                Span {
                    line: self.line,
                    column: self.column,
                    byte_offset: self.byte_idx,
                    byte_len: len as u32,
                    char_offset: self.char_idx,
                    char_len,
                }
            },
            _ => {
                Span::new_location(self.line, self.column)
            },
        };

        self.errors.push((span, err));
    }

    fn add_spanned_error(&mut self, span: Span, err: LexError) {
        self.errors.push((span, err));
    }

    fn add_char_lit_error(&mut self, err: LexError) {
        let line = self.cur_line();

        let mut escaped = false;
        let end = line[1..].find(|ch| {
            if ch == '\\' {
                escaped = true;
                false
            } else if ch == '\'' {
                let ret = escaped;
                escaped = false;
                ret
                
            } else {
                escaped = false;
                false
            }
        }).map_or(line.len(), |len| len - 1);
        
        self.add_error(Some(end), err);
    }

    fn increment_interp_indent(&mut self) {
        if let Some(top) = self.interp_stack.last_mut() {
            *top += 1;
        }
    }

    fn decrement_interp_indent(&mut self) {
        if let Some(top) = self.interp_stack.last_mut() {
            *top -= 1;
        }
    }

    fn push_interp_indent(&mut self) {
        self.interp_stack.push(0);
    }

    fn pop_interp_indent(&mut self) {
        if matches!(self.interp_stack.last(), Some(0)) {
            self.interp_stack.pop();
        }
    }

    fn interp_indent_depth(&self) -> Option<u32> {
        self.interp_stack.last().cloned()
    }

    fn add_tree_build_errors(&mut self, error: TokenTreeBuildError) {
        match error {
            TokenTreeBuildError::EndOfRootTree                  => self.add_error(None, LexError::EndOfRootTree),
            TokenTreeBuildError::Missing(errors)                 => for (span, item) in errors { //LexError::MissingDelimiter(items),
                self.add_spanned_error(span, LexError::MissingDelimiter(item));
            },
            TokenTreeBuildError::Unexpected { span, expected, found } => self.add_spanned_error(span, LexError::UnmatchedDelimiter { expected, found }),
        }
    }
}