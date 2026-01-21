use winnow::{
    Result,
    ascii::{digit1, multispace0, till_line_ending},
    combinator::{alt, not, preceded, repeat, terminated},
    error::{ContextError, ParserError},
    prelude::*,
    stream::ContainsToken,
    token::{any, one_of, rest, take_while},
};

use crate::{Anyhow, anyhow};

use super::token::{Keyword, LexedToken, LiteralType, Operator, Punctuation, Token, globs};

#[allow(clippy::wildcard_imports)]
use globs::*;

pub fn lex(source: &str) -> Anyhow<Vec<LexedToken<'_>>> {
    (preceded(trivia, repeat(.., terminated(token, trivia))), eoi)
        .map(|(mut tokens, eoi): (Vec<_>, _)| {
            tokens.push(eoi);
            tokens
        })
        .parse(source)
        .map_err(|e| anyhow!("{e}"))
}

fn eoi<'i>(i: &mut &str) -> Result<LexedToken<'i>> {
    rest.verify(str::is_empty)
        .value(LexedToken {
            token: Token::EndOfInput,
            lexeme: "",
        })
        .parse_next(i)
}

fn token<'i>(i: &mut &'i str) -> Result<LexedToken<'i>> {
    alt((valid_token, invalid_token))
        .with_taken()
        .map(|(token, lexeme)| LexedToken { token, lexeme })
        .parse_next(i)
}

fn valid_id_part(c: char) -> bool {
    c.is_alphanum() || c == '_'
}

fn skip_non_whitespace(i: &mut &str) -> Result<()> {
    any::<_, ContextError>
        .verify(|c: &char| !c.is_whitespace())
        .void()
        .parse_next(i)
}

fn comment(i: &mut &str) -> Result<Token> {
    ('#', till_line_ending).value(Token::Comment).parse_next(i)
}

fn trivia(i: &mut &str) -> Result<()> {
    preceded(multispace0, repeat(.., terminated(comment, multispace0)))
        .map(|()| ())
        .parse_next(i)
}

fn valid_token(i: &mut &str) -> Result<Token> {
    alt((
        int_literal,
        keyword,
        literal_type,
        identifier,
        operators_and_punctuation,
        comment,
    ))
    .parse_next(i)
}

fn invalid_token(i: &mut &str) -> Result<Token> {
    skip_non_whitespace.parse_next(i)?;
    while valid_token.parse_peek(*i).is_err() && skip_non_whitespace.parse_next(i).is_ok() {}
    Ok(Token::Invalid)
}

fn int_literal(i: &mut &str) -> Result<Token> {
    digit1.value(Token::IntLiteral).parse_next(i)
}

fn literal_type(i: &mut &str) -> Result<Token> {
    alt((Int, Bool)).map(Token::LiteralType).parse_next(i)
}

fn identifier(i: &mut &str) -> Result<Token> {
    (
        one_of(|c: char| c.is_alpha() || c == '_'),
        take_while(0.., valid_id_part),
    )
        .value(Token::Identifier)
        .parse_next(i)
}

fn keyword(i: &mut &str) -> Result<Token> {
    alt((
        Def, If, Else, While, Return, Class, SelfRef, New, Delete, True, False,
    ))
    .map(Token::Keyword)
    .parse_next(i)
}

fn operators_and_punctuation(i: &mut &str) -> Result<Token> {
    alt((
        Arrow.map(Token::Punctuation), // Needs to be above `Minus`
        LessEq.map(Token::Operator),
        GreatEq.map(Token::Operator),
        Less.map(Token::Operator),
        Great.map(Token::Operator),
        Equality.map(Token::Operator),
        NotEqual.map(Token::Operator),
        Plus.map(Token::Operator),
        Minus.map(Token::Operator),
        Multiply.map(Token::Operator),
        Divide.map(Token::Operator),
        Modulo.map(Token::Operator),
        Not.map(Token::Operator),
        OpenParen.map(Token::Punctuation),
        CloseParen.map(Token::Punctuation),
        OpenBrace.map(Token::Punctuation),
        CloseBrace.map(Token::Punctuation),
        Comma.map(Token::Punctuation),
        Colon.map(Token::Punctuation),
        Equals.map(Token::Punctuation), // Needs to be below equality operators
        Dot.map(Token::Punctuation),
    ))
    .parse_next(i)
}

impl ContainsToken<&'_ LexedToken<'_>> for Token {
    #[inline(always)]
    fn contains_token(&self, token: &'_ LexedToken<'_>) -> bool {
        *self == token.token
    }
}

impl ContainsToken<&'_ LexedToken<'_>> for LiteralType {
    #[inline(always)]
    fn contains_token(&self, token: &'_ LexedToken<'_>) -> bool {
        if let Token::LiteralType(ty) = token.token {
            *self == ty
        } else {
            false
        }
    }
}

impl ContainsToken<&'_ LexedToken<'_>> for Operator {
    #[inline(always)]
    fn contains_token(&self, token: &'_ LexedToken<'_>) -> bool {
        if let Token::Operator(op) = token.token {
            *self == op
        } else {
            false
        }
    }
}

impl ContainsToken<&'_ LexedToken<'_>> for Punctuation {
    #[inline(always)]
    fn contains_token(&self, token: &'_ LexedToken<'_>) -> bool {
        if let Token::Punctuation(p) = token.token {
            *self == p
        } else {
            false
        }
    }
}

impl ContainsToken<&'_ LexedToken<'_>> for Keyword {
    #[inline(always)]
    fn contains_token(&self, token: &'_ LexedToken<'_>) -> bool {
        if let Token::Keyword(kw) = token.token {
            *self == kw
        } else {
            false
        }
    }
}

impl ContainsToken<&'_ LexedToken<'_>> for &'_ [Token] {
    #[inline(always)]
    fn contains_token(&self, token: &'_ LexedToken<'_>) -> bool {
        self.contains(&token.token)
    }
}

impl<const LEN: usize> ContainsToken<&'_ LexedToken<'_>> for &'_ [Token; LEN] {
    #[inline(always)]
    fn contains_token(&self, token: &'_ LexedToken<'_>) -> bool {
        self.contains(&token.token)
    }
}

impl<const LEN: usize> ContainsToken<&'_ LexedToken<'_>> for [Token; LEN] {
    #[inline(always)]
    fn contains_token(&self, token: &'_ LexedToken<'_>) -> bool {
        self.contains(&token.token)
    }
}

impl<'i, E: ParserError<&'i str>> Parser<&'i str, Self, E> for LiteralType {
    fn parse_next(&mut self, input: &mut &'i str) -> Result<Self, E> {
        terminated(self.lexeme(), not(one_of(valid_id_part)))
            .value(*self)
            .parse_next(input)
    }
}

impl<'i, E: ParserError<&'i str>> Parser<&'i str, Self, E> for Operator {
    fn parse_next(&mut self, input: &mut &'i str) -> Result<Self, E> {
        self.lexeme().value(*self).parse_next(input)
    }
}

impl<'i, E: ParserError<&'i str>> Parser<&'i str, Self, E> for Punctuation {
    fn parse_next(&mut self, input: &mut &'i str) -> Result<Self, E> {
        self.lexeme().value(*self).parse_next(input)
    }
}

impl<'i, E: ParserError<&'i str>> Parser<&'i str, Self, E> for Keyword {
    fn parse_next(&mut self, input: &mut &'i str) -> Result<Self, E> {
        terminated(self.lexeme(), not(one_of(valid_id_part)))
            .value(*self)
            .parse_next(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Case {
        input: &'static str,
        expected: Vec<Token>,
    }

    fn run_tests<const N: usize>(cases: [Case; N]) {
        for case in cases {
            let tokens = lex(case.input).unwrap();
            assert_eq!(tokens, case.expected);
        }
    }

    #[test]
    fn smoke_test() {
        run_tests([Case {
            input: "x = 1",
            expected: vec![
                Token::Identifier,
                Token::Punctuation(Equals),
                Token::IntLiteral,
                Token::EndOfInput,
            ],
        }]);
    }

    #[test]
    fn bool_literals_and_keywords() {
        run_tests([Case {
            input: "true false if else while",
            expected: vec![
                Token::Keyword(True),
                Token::Keyword(False),
                Token::Keyword(If),
                Token::Keyword(Else),
                Token::Keyword(While),
                Token::EndOfInput,
            ],
        }]);
    }

    #[test]
    fn identifiers_and_keywords() {
        run_tests([
            Case {
                input: "def if else while return class self new delete true false",
                expected: vec![
                    Token::Keyword(Def),
                    Token::Keyword(If),
                    Token::Keyword(Else),
                    Token::Keyword(While),
                    Token::Keyword(Return),
                    Token::Keyword(Class),
                    Token::Keyword(SelfRef),
                    Token::Keyword(New),
                    Token::Keyword(Delete),
                    Token::Keyword(True),
                    Token::Keyword(False),
                    Token::EndOfInput,
                ],
            },
            // Deviating from the spec here if I'm understanding it correctly:
            // `Identifier = @{ !KEYWORD ~ (ASCII_ALPHA | "_") ~ (ASCII_ALPHANUMERIC | "_")* }`
            // It seems odd to disallow identifiers prefixed by a keyword, so I'll allow it.
            Case {
                input: "classy true_value iffy selfless",
                expected: vec![
                    Token::Identifier,
                    Token::Identifier,
                    Token::Identifier,
                    Token::Identifier,
                    Token::EndOfInput,
                ],
            },
            Case {
                input: "class classA def def_1 _",
                expected: vec![
                    Token::Keyword(Class),
                    Token::Identifier,
                    Token::Keyword(Def),
                    Token::Identifier,
                    Token::Identifier,
                    Token::EndOfInput,
                ],
            },
        ]);
    }

    #[test]
    fn literals() {
        run_tests([Case {
            input: "int bool 0 42 true false",
            expected: vec![
                Token::LiteralType(Int),
                Token::LiteralType(Bool),
                Token::IntLiteral,
                Token::IntLiteral,
                Token::Keyword(True),
                Token::Keyword(False),
                Token::EndOfInput,
            ],
        }]);
    }

    #[test]
    fn operators() {
        run_tests([Case {
            input: "<= >= < > == != + - * / % !",
            expected: vec![
                Token::Operator(LessEq),
                Token::Operator(GreatEq),
                Token::Operator(Less),
                Token::Operator(Great),
                Token::Operator(Equality),
                Token::Operator(NotEqual),
                Token::Operator(Plus),
                Token::Operator(Minus),
                Token::Operator(Multiply),
                Token::Operator(Divide),
                Token::Operator(Modulo),
                Token::Operator(Not),
                Token::EndOfInput,
            ],
        }]);
    }

    #[test]
    fn punctuation() {
        run_tests([Case {
            input: "( ) { } , : -> = .",
            expected: vec![
                Token::Punctuation(OpenParen),
                Token::Punctuation(CloseParen),
                Token::Punctuation(OpenBrace),
                Token::Punctuation(CloseBrace),
                Token::Punctuation(Comma),
                Token::Punctuation(Colon),
                Token::Punctuation(Arrow),
                Token::Punctuation(Equals),
                Token::Punctuation(Dot),
                Token::EndOfInput,
            ],
        }]);
    }

    #[test]
    fn comments_and_whitespace() {
        run_tests([
            Case {
                input: r"
                    # comment
                    x	# trailing

                    # double
                    # comment

                    y
                ",
                expected: vec![Token::Identifier, Token::Identifier, Token::EndOfInput],
            },
            Case {
                input: "# nothing\n# but\n# comments",
                expected: vec![Token::EndOfInput],
            },
        ]);
    }

    #[test]
    fn invalid_tokens() {
        run_tests([
            Case {
                input: "@",
                expected: vec![Token::Invalid, Token::EndOfInput],
            },
            Case {
                input: "@|@|@|@|@",
                expected: vec![Token::Invalid, Token::EndOfInput],
            },
        ]);
    }
}
