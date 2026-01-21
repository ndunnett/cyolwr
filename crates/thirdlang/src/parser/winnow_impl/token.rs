#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LexedToken<'i> {
    pub token: Token,
    pub lexeme: &'i str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Token {
    IntLiteral,
    LiteralType(LiteralType),
    Identifier,
    Operator(Operator),
    Punctuation(Punctuation),
    Keyword(Keyword),
    Comment,
    Invalid,
    EndOfInput,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiteralType {
    Int,
    Bool,
}

impl LiteralType {
    pub const fn lexeme(self) -> &'static str {
        match self {
            Self::Int => "int",
            Self::Bool => "bool",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operator {
    LessEq,
    GreatEq,
    Less,
    Great,
    Equality,
    NotEqual,
    Plus,
    Minus,
    Multiply,
    Divide,
    Modulo,
    Not,
}

impl Operator {
    pub const fn lexeme(self) -> &'static str {
        match self {
            Self::LessEq => "<=",
            Self::GreatEq => ">=",
            Self::Less => "<",
            Self::Great => ">",
            Self::Equality => "==",
            Self::NotEqual => "!=",
            Self::Plus => "+",
            Self::Minus => "-",
            Self::Multiply => "*",
            Self::Divide => "/",
            Self::Modulo => "%",
            Self::Not => "!",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Punctuation {
    OpenParen,
    CloseParen,
    OpenBrace,
    CloseBrace,
    Comma,
    Colon,
    Arrow,
    Equals,
    Dot,
}

impl Punctuation {
    pub const fn lexeme(self) -> &'static str {
        match self {
            Self::OpenParen => "(",
            Self::CloseParen => ")",
            Self::OpenBrace => "{",
            Self::CloseBrace => "}",
            Self::Comma => ",",
            Self::Colon => ":",
            Self::Arrow => "->",
            Self::Equals => "=",
            Self::Dot => ".",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keyword {
    Def,
    If,
    Else,
    While,
    Return,
    Class,
    SelfRef,
    New,
    Delete,
    True,
    False,
}

impl Keyword {
    pub const fn lexeme(self) -> &'static str {
        match self {
            Self::Def => "def",
            Self::If => "if",
            Self::Else => "else",
            Self::While => "while",
            Self::Return => "return",
            Self::Class => "class",
            Self::SelfRef => "self",
            Self::New => "new",
            Self::Delete => "delete",
            Self::True => "true",
            Self::False => "false",
        }
    }
}

#[allow(clippy::enum_glob_use, unused)]
pub mod globs {
    pub use super::Keyword::*;
    pub use super::LiteralType::*;
    pub use super::Operator::*;
    pub use super::Punctuation::*;
}

impl PartialEq<Token> for LexedToken<'_> {
    fn eq(&self, other: &Token) -> bool {
        self.token == *other
    }
}

impl PartialEq<Keyword> for LexedToken<'_> {
    fn eq(&self, other: &Keyword) -> bool {
        if let Token::Keyword(kw) = self.token {
            *other == kw
        } else {
            false
        }
    }
}

impl PartialEq<LiteralType> for LexedToken<'_> {
    fn eq(&self, other: &LiteralType) -> bool {
        if let Token::LiteralType(ty) = self.token {
            *other == ty
        } else {
            false
        }
    }
}

impl PartialEq<Operator> for LexedToken<'_> {
    fn eq(&self, other: &Operator) -> bool {
        if let Token::Operator(op) = self.token {
            *other == op
        } else {
            false
        }
    }
}

impl PartialEq<Punctuation> for LexedToken<'_> {
    fn eq(&self, other: &Punctuation) -> bool {
        if let Token::Punctuation(p) = self.token {
            *other == p
        } else {
            false
        }
    }
}

impl std::fmt::Display for LexedToken<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.token {
            Token::IntLiteral => write!(f, "int({})", self.lexeme),
            Token::LiteralType(ty) => ty.fmt(f),
            Token::Identifier => write!(f, "identifier(\"{}\")", self.lexeme),
            Token::Operator(op) => op.fmt(f),
            Token::Punctuation(p) => p.fmt(f),
            Token::Keyword(kw) => kw.fmt(f),
            Token::Comment => write!(f, "comment(\"{}\")", self.lexeme),
            Token::Invalid => write!(f, "invalid(\"{}\")", self.lexeme),
            Token::EndOfInput => f.write_str("<EOI>"),
        }
    }
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IntLiteral => f.write_str("<int>"),
            Self::LiteralType(ty) => ty.fmt(f),
            Self::Identifier => f.write_str("<identifier>"),
            Self::Operator(op) => op.fmt(f),
            Self::Punctuation(p) => p.fmt(f),
            Self::Keyword(kw) => kw.fmt(f),
            Self::Comment => f.write_str("<comment>"),
            Self::Invalid => f.write_str("<invalid>"),
            Self::EndOfInput => f.write_str("<EOI>"),
        }
    }
}

impl std::fmt::Display for LiteralType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.lexeme())
    }
}

impl std::fmt::Display for Operator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.lexeme())
    }
}

impl std::fmt::Display for Punctuation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.lexeme())
    }
}

impl std::fmt::Display for Keyword {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.lexeme())
    }
}
