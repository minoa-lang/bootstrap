
use bootstrap_macros::enum_utils;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
//#[enum_utils(from_idx, as_str(snake_case), display)]
#[enum_utils(from_idx, as_str(snake_case), display)]
// #[enum_from_idx]
pub enum StrongKeyword {
    As,
    #[string("as?")]
    AsQuestion,
    #[string("as!")]
    AsExclaim,
    Assert,
    Async,
    Await,
    Bool,
    Bitfield,
    Break,
    Catch,
    Char,
    Char7,
    Char8,
    Char16,
    Char32,
    Const,
    Constraint,
    Continue,
    Cstr,
    Defer,
    Do,
    Dyn,
    Else,
    Enum,
    #[string("errdefer")]
    ErrDefer,
    False,
    Fallthrough,
    Fn,
    For,
    If,
    In,
    #[string("!in")]
    NotIn,
    Impl,
    Iptr,
    Is,
    #[string("!is")]
    NotIs,
    Isize,
    Let,
    #[string("let?")]
    LetQuestion,
    #[string("let!")]
    LetExclaim,
    Loop,
    Null,
    Match,
    Mod,
    Move,
    Mut,
    Pub,
    Ref,
    Return,
    Safe,
    #[string("Self")]
    SelfKw,
    Static,
    Str,
    Str7,
    Str8,
    Str16,
    Str32,
    Struct,
    Throw,
    Trait,
    True,
    Try,
    #[string("try?")]
    TryQuestion,
    #[string("try!")]
    TryExclaim,
    Type,
    Unsafe,
    Uptr,
    Use,
    Usize,
    With,
    While,
    When,
    Where,
    Yield,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
#[enum_utils(from_idx, as_str(snake_case), display)]
pub enum ReservedKeyword {
    Overide,
    Priv,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
#[enum_utils(from_idx, as_str(snake_case), display)]
pub enum WeakKeyword {
    Accessor,
    Adapt,
    Alias,
    Align,
    #[string("allowzero")]
    AllowZero,
    Assign,
    Associativity,
    Attr,
    Bench,
    Block,
    Chain,
    Consume,
    Derive,
    DidSet,
    Distinct,
    Expr,
    Extend,
    Flag,
    Field,
    FieldConvert,
    Full,
    Get,
    HigherThan,
    Iden,
    Infix,
    Init,
    Invar,
    Item,
    Lazy,
    Lib,
    Lit,
    Literal,
    LowerThan,
    Lsb,
    Member,
    MemberAttr,
    Meta,
    MetaPat,
    Msb,
    Names,
    Opaque,
    Overloaded,
    Package,
    Pat,
    Path,
    Peer,
    Post,
    Postfix,
    Pre,
    Precedence,
    Prefix,
    Prop,
    Raw,
    Record,
    Sealed,
    Set,
    Sparse,
    Stmt,
    Suffix,
    Super,
    Template,
    Test,
    Tls,
    Toks,
    Tt,
    Ty,
    Union,
    Unique,
    Vis,
    Volatile,
    WillSet,
    With,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
#[enum_utils(as_str(snake_case), display)]
pub enum PatternKeywordEndianness {
    #[string("")]
    None,
    Le,
    Be,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
#[enum_utils(display)]
pub enum PatternKeyword {
    #[fmt("b{bits}{endianness}")]
    B{
        bits: u16,
        endianness: PatternKeywordEndianness
    },
    #[fmt("u{bits}{endianness}")]
    U{
        bits: u16,
        endianness: PatternKeywordEndianness
    },
    #[fmt("i{bits}{endianness}")]
    I{
        bits: u16,
        endianness: PatternKeywordEndianness
    },
    #[fmt("f{bits}{endianness}")]
    F{
        bits: u16,
        endianness: PatternKeywordEndianness
    },
}
