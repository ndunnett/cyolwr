use winnow::{
    combinator::{
        alt, delimited, opt, preceded, repeat, separated, separated_pair, seq, terminated,
    },
    error::{ContextError, ErrMode, ParserError},
    prelude::*,
    stream::TokenSlice,
    token::literal,
};

use crate::{
    Anyhow, anyhow,
    ast::{
        AssignTarget, BinaryOp, ClassDef, Expression, FieldDef, MethodDef, Program, Statement,
        TopLevel, TypedExpr, UnaryOp,
    },
    types::Type,
};

use super::{
    lex::lex,
    token::{Keyword, LexedToken, LiteralType, Operator, Punctuation, Token, globs},
};

#[allow(clippy::wildcard_imports)]
use globs::*;

pub type Tokens<'i> = TokenSlice<'i, LexedToken<'i>>;

pub fn parse(source: &str) -> Anyhow<Program> {
    let tokens = lex(source)?;

    program
        .parse(Tokens::new(&tokens))
        .map_err(|e| anyhow!("{}", e.inner()))
}

fn identifier<'i>(i: &mut Tokens<'i>) -> ModalResult<&'i str> {
    Token::Identifier.map(|t| t.lexeme).parse_next(i)
}

fn parse_type(i: &mut Tokens<'_>) -> ModalResult<Type> {
    alt((
        Int.value(Type::Int),
        Bool.value(Type::Bool),
        identifier.map(String::from).map(Type::Class),
    ))
    .parse_next(i)
}

fn program(i: &mut Tokens<'_>) -> ModalResult<Program> {
    terminated(repeat(.., top_level), Token::EndOfInput).parse_next(i)
}

fn top_level(i: &mut Tokens<'_>) -> ModalResult<TopLevel> {
    alt((class_def.map(TopLevel::Class), stmt.map(TopLevel::Stmt))).parse_next(i)
}

#[derive(Debug, Clone)]
enum ClassBodyItem {
    Field(FieldDef),
    Method(MethodDef),
}

fn class_def(i: &mut Tokens<'_>) -> ModalResult<ClassDef> {
    (
        preceded(Class, identifier),
        delimited(OpenBrace, class_body, CloseBrace),
    )
        .map(|(id, body)| {
            let (fields, methods): (Vec<_>, Vec<_>) = body
                .into_iter()
                .partition(|item| matches!(item, ClassBodyItem::Field(_)));

            let fields = fields
                .into_iter()
                .filter_map(|item| {
                    if let ClassBodyItem::Field(field) = item {
                        Some(field)
                    } else {
                        None
                    }
                })
                .collect();

            let methods = methods
                .into_iter()
                .filter_map(|item| {
                    if let ClassBodyItem::Method(method) = item {
                        Some(method)
                    } else {
                        None
                    }
                })
                .collect();

            ClassDef {
                name: String::from(id),
                fields,
                methods,
            }
        })
        .parse_next(i)
}

fn class_body(i: &mut Tokens<'_>) -> ModalResult<Vec<ClassBodyItem>> {
    repeat(
        ..,
        alt((
            field_def.map(ClassBodyItem::Field),
            method_def.map(ClassBodyItem::Method),
        )),
    )
    .parse_next(i)
}

fn field_def(i: &mut Tokens<'_>) -> ModalResult<FieldDef> {
    typed_param
        .map(|(name, ty)| FieldDef { name, ty })
        .parse_next(i)
}

fn method_def(i: &mut Tokens<'_>) -> ModalResult<MethodDef> {
    seq! {
        MethodDef {
            _: Def,
            name: identifier.map(String::from),
            _: OpenParen,
            params: preceded(SelfRef, opt(method_params)).map(Option::unwrap_or_default),
            _: CloseParen,
            return_type: opt(return_type).map(|ty| ty.unwrap_or(Type::Unit)),
            body: block,
        }
    }
    .parse_next(i)
}

fn method_params(i: &mut Tokens<'_>) -> ModalResult<Vec<(String, Type)>> {
    preceded(Comma, typed_params).parse_next(i)
}

fn stmt(i: &mut Tokens<'_>) -> ModalResult<Statement> {
    alt((function, simple_stmt)).parse_next(i)
}

fn simple_stmt(i: &mut Tokens<'_>) -> ModalResult<Statement> {
    alt((
        delete,
        return_stmt,
        assignment,
        expr.map(Statement::Expression),
    ))
    .parse_next(i)
}

fn function(i: &mut Tokens<'_>) -> ModalResult<Statement> {
    seq! {
        Statement::Function {
            _: Def,
            name: identifier.map(String::from),
            _: OpenParen,
            params: opt(typed_params).map(Option::unwrap_or_default),
            _: CloseParen,
            return_type: opt(return_type).map(|ty| ty.unwrap_or(Type::Unit)),
            body: block,
        }
    }
    .parse_next(i)
}

fn typed_params(i: &mut Tokens<'_>) -> ModalResult<Vec<(String, Type)>> {
    separated(1.., typed_param, Comma).parse_next(i)
}

fn typed_param(i: &mut Tokens<'_>) -> ModalResult<(String, Type)> {
    separated_pair(identifier, Colon, parse_type)
        .map(|(id, ty)| (String::from(id), ty))
        .parse_next(i)
}

fn return_type(i: &mut Tokens<'_>) -> ModalResult<Type> {
    preceded(Arrow, parse_type).parse_next(i)
}

fn block(i: &mut Tokens<'_>) -> ModalResult<Vec<Statement>> {
    delimited(OpenBrace, repeat(.., stmt), CloseBrace).parse_next(i)
}

fn return_stmt(i: &mut Tokens<'_>) -> ModalResult<Statement> {
    preceded(Return, expr).map(Statement::Return).parse_next(i)
}

fn delete(i: &mut Tokens<'_>) -> ModalResult<Statement> {
    preceded(Delete, expr).map(Statement::Delete).parse_next(i)
}

fn assignment(i: &mut Tokens<'_>) -> ModalResult<Statement> {
    seq! {
        Statement::Assignment {
            target: assign_target,
            type_ann: opt(preceded(Colon, parse_type)),
            _: Equals,
            value: expr,
        }
    }
    .parse_next(i)
}

fn assign_target(i: &mut Tokens<'_>) -> ModalResult<AssignTarget> {
    alt((
        field_access,
        identifier.map(String::from).map(AssignTarget::Var),
    ))
    .parse_next(i)
}

fn expr(i: &mut Tokens<'_>) -> ModalResult<TypedExpr> {
    alt((conditional, while_loop, comparison)).parse_next(i)
}

fn conditional(i: &mut Tokens<'_>) -> ModalResult<TypedExpr> {
    seq! {
        Expression::If {
           _: If,
           _: OpenParen,
           cond: expr.map(Box::new),
           _: CloseParen,
           then_branch: block,
           _: Else,
           else_branch: block,
        }
    }
    .map(Expression::untyped)
    .parse_next(i)
}

fn while_loop(i: &mut Tokens<'_>) -> ModalResult<TypedExpr> {
    seq! {
        Expression::While {
            _: While,
            _: OpenParen,
            cond: expr.map(Box::new),
            _: CloseParen,
            body: block,
        }
    }
    .map(Expression::untyped)
    .parse_next(i)
}

fn binary<'i, F1, F2, E>(operator: F1, operand: F2) -> impl Parser<Tokens<'i>, TypedExpr, E>
where
    F1: Parser<Tokens<'i>, BinaryOp, E> + Copy,
    F2: Parser<Tokens<'i>, TypedExpr, E> + Copy,
    E: ParserError<Tokens<'i>>,
{
    (operand, repeat(.., (operator, operand))).map(|(mut expr, ops): (_, Vec<_>)| {
        for (op, operand) in ops {
            expr = Expression::Binary {
                op,
                left: Box::new(expr),
                right: Box::new(operand),
            }
            .untyped();
        }

        expr
    })
}

fn comparison(i: &mut Tokens<'_>) -> ModalResult<TypedExpr> {
    binary(comp_op, additive).parse_next(i)
}

fn comp_op(i: &mut Tokens<'_>) -> ModalResult<BinaryOp> {
    alt((
        LessEq.value(BinaryOp::LessEqual),
        GreatEq.value(BinaryOp::GreaterEqual),
        Less.value(BinaryOp::LessThan),
        Great.value(BinaryOp::GreaterThan),
        Equality.value(BinaryOp::Equal),
        NotEqual.value(BinaryOp::NotEqual),
    ))
    .parse_next(i)
}

fn additive(i: &mut Tokens<'_>) -> ModalResult<TypedExpr> {
    binary(add_op, multiplicative).parse_next(i)
}

fn add_op(i: &mut Tokens<'_>) -> ModalResult<BinaryOp> {
    alt((Plus.value(BinaryOp::Add), Minus.value(BinaryOp::Subtract))).parse_next(i)
}

fn multiplicative(i: &mut Tokens<'_>) -> ModalResult<TypedExpr> {
    binary(mul_op, unary).parse_next(i)
}

fn mul_op(i: &mut Tokens<'_>) -> ModalResult<BinaryOp> {
    alt((
        Multiply.value(BinaryOp::Multiply),
        Divide.value(BinaryOp::Divide),
        Modulo.value(BinaryOp::Modulo),
    ))
    .parse_next(i)
}

fn unary(i: &mut Tokens<'_>) -> ModalResult<TypedExpr> {
    (repeat(.., unary_op), postfix)
        .map(|(ops, mut expr): (Vec<_>, _)| {
            for op in ops {
                expr = Expression::Unary {
                    op,
                    expr: Box::new(expr),
                }
                .untyped();
            }

            expr
        })
        .parse_next(i)
}

fn unary_op(i: &mut Tokens<'_>) -> ModalResult<UnaryOp> {
    alt((Minus.value(UnaryOp::Negative), Not.value(UnaryOp::Not))).parse_next(i)
}

fn postfix(i: &mut Tokens<'_>) -> ModalResult<TypedExpr> {
    (primary, repeat(.., postfix_op))
        .map(|(mut expr, ops): (_, Vec<_>)| {
            for op in ops {
                expr = match op {
                    (field, None) => Expression::FieldAccess {
                        object: Box::new(expr),
                        field,
                    },
                    (method, Some(args)) => Expression::MethodCall {
                        object: Box::new(expr),
                        method,
                        args,
                    },
                }
                .untyped();
            }

            expr
        })
        .parse_next(i)
}

fn postfix_op(i: &mut Tokens<'_>) -> ModalResult<(String, Option<Vec<TypedExpr>>)> {
    alt((method_call, field_access_op)).parse_next(i)
}

fn method_call(i: &mut Tokens<'_>) -> ModalResult<(String, Option<Vec<TypedExpr>>)> {
    preceded(Dot, (identifier, delimited(OpenParen, args, CloseParen)))
        .map(|(id, args)| (String::from(id), Some(args)))
        .parse_next(i)
}

fn field_access_op(i: &mut Tokens<'_>) -> ModalResult<(String, Option<Vec<TypedExpr>>)> {
    preceded(Dot, identifier)
        .map(|id| (String::from(id), None))
        .parse_next(i)
}

fn field_access(i: &mut Tokens<'_>) -> ModalResult<AssignTarget> {
    (
        alt((self_keyword, var_expr)),
        preceded(Dot, identifier).map(String::from),
        repeat(.., preceded(Dot, identifier)),
    )
        .map(|(mut object, mut field, chain): (_, _, Vec<_>)| {
            for next in chain {
                object = Expression::FieldAccess {
                    object: Box::new(object),
                    field,
                }
                .untyped();

                field = String::from(next);
            }

            AssignTarget::Field {
                object: Box::new(object),
                field,
            }
        })
        .parse_next(i)
}

fn self_keyword(i: &mut Tokens<'_>) -> ModalResult<TypedExpr> {
    SelfRef.value(Expression::SelfRef.untyped()).parse_next(i)
}

fn primary(i: &mut Tokens<'_>) -> ModalResult<TypedExpr> {
    alt((
        new_expr,
        function_call,
        literal_value,
        self_keyword,
        var_expr,
        delimited(OpenParen, expr, CloseParen),
    ))
    .parse_next(i)
}

fn var_expr(i: &mut Tokens<'_>) -> ModalResult<TypedExpr> {
    identifier
        .map(String::from)
        .map(Expression::Var)
        .map(Expression::untyped)
        .parse_next(i)
}

fn new_expr(i: &mut Tokens<'_>) -> ModalResult<TypedExpr> {
    seq! {
        Expression::New {
            _: New,
                class: identifier.map(String::from),
            _: OpenParen,
                args: args,
            _: CloseParen
        }
    }
    .map(Expression::untyped)
    .parse_next(i)
}

fn function_call(i: &mut Tokens<'_>) -> ModalResult<TypedExpr> {
    seq! {
        Expression::Call {
            name: identifier.map(String::from),
            _: OpenParen,
            args: args,
            _: CloseParen
        }
    }
    .map(Expression::untyped)
    .parse_next(i)
}

fn args(i: &mut Tokens<'_>) -> ModalResult<Vec<TypedExpr>> {
    separated(.., expr, Comma).parse_next(i)
}

fn literal_value(i: &mut Tokens<'_>) -> ModalResult<TypedExpr> {
    alt((
        int_value.map(|x| Expression::Int(x).typed(Type::Int)),
        bool_value.map(|x| Expression::Bool(x).typed(Type::Bool)),
    ))
    .parse_next(i)
}

fn int_value(i: &mut Tokens<'_>) -> ModalResult<i64> {
    Token::IntLiteral
        .try_map(|t| t.lexeme.parse::<i64>())
        .parse_next(i)
}

fn bool_value(i: &mut Tokens<'_>) -> ModalResult<bool> {
    alt((True.value(true), False.value(false))).parse_next(i)
}

impl<'i> Parser<Tokens<'i>, LexedToken<'i>, ErrMode<ContextError>> for Token {
    fn parse_next(&mut self, tokens: &mut Tokens<'i>) -> ModalResult<LexedToken<'i>> {
        literal(*self).parse_next(tokens).map(|a| a[0])
    }
}

impl<'i> Parser<Tokens<'i>, LexedToken<'i>, ErrMode<ContextError>> for Keyword {
    fn parse_next(&mut self, tokens: &mut Tokens<'i>) -> ModalResult<LexedToken<'i>> {
        literal(*self).parse_next(tokens).map(|a| a[0])
    }
}

impl<'i> Parser<Tokens<'i>, LexedToken<'i>, ErrMode<ContextError>> for LiteralType {
    fn parse_next(&mut self, tokens: &mut Tokens<'i>) -> ModalResult<LexedToken<'i>> {
        literal(*self).parse_next(tokens).map(|a| a[0])
    }
}

impl<'i> Parser<Tokens<'i>, LexedToken<'i>, ErrMode<ContextError>> for Operator {
    fn parse_next(&mut self, tokens: &mut Tokens<'i>) -> ModalResult<LexedToken<'i>> {
        literal(*self).parse_next(tokens).map(|a| a[0])
    }
}

impl<'i> Parser<Tokens<'i>, LexedToken<'i>, ErrMode<ContextError>> for Punctuation {
    fn parse_next(&mut self, tokens: &mut Tokens<'i>) -> ModalResult<LexedToken<'i>> {
        literal(*self).parse_next(tokens).map(|a| a[0])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Case {
        input: &'static str,
        expected: Program,
    }

    fn run_tests<const N: usize>(cases: [Case; N]) {
        for case in cases {
            let ast = parse(case.input).unwrap();
            assert_eq!(ast, case.expected);
        }
    }

    fn int_lit(value: i64) -> TypedExpr {
        Expression::Int(value).typed(Type::Int)
    }

    fn bool_lit(value: bool) -> TypedExpr {
        Expression::Bool(value).typed(Type::Bool)
    }

    fn var(name: &str) -> TypedExpr {
        Expression::Var(String::from(name)).untyped()
    }

    fn self_ref() -> TypedExpr {
        Expression::SelfRef.untyped()
    }

    fn unary(op: UnaryOp, expr: TypedExpr) -> TypedExpr {
        Expression::Unary {
            op,
            expr: Box::new(expr),
        }
        .untyped()
    }

    fn bin(op: BinaryOp, left: TypedExpr, right: TypedExpr) -> TypedExpr {
        Expression::Binary {
            op,
            left: Box::new(left),
            right: Box::new(right),
        }
        .untyped()
    }

    fn call(name: &str, args: Vec<TypedExpr>) -> TypedExpr {
        Expression::Call {
            name: String::from(name),
            args,
        }
        .untyped()
    }

    fn method_call(object: TypedExpr, method: &str, args: Vec<TypedExpr>) -> TypedExpr {
        Expression::MethodCall {
            object: Box::new(object),
            method: String::from(method),
            args,
        }
        .untyped()
    }

    fn field(object: TypedExpr, field: &str) -> TypedExpr {
        Expression::FieldAccess {
            object: Box::new(object),
            field: String::from(field),
        }
        .untyped()
    }

    fn new_expr(class: &str, args: Vec<TypedExpr>) -> TypedExpr {
        Expression::New {
            class: String::from(class),
            args,
        }
        .untyped()
    }

    fn assign_var(name: &str, type_ann: Option<Type>, value: TypedExpr) -> Statement {
        Statement::Assignment {
            target: AssignTarget::Var(String::from(name)),
            type_ann,
            value,
        }
    }

    fn assign_field(object: TypedExpr, field: &str, value: TypedExpr) -> Statement {
        Statement::Assignment {
            target: AssignTarget::Field {
                object: Box::new(object),
                field: String::from(field),
            },
            type_ann: None,
            value,
        }
    }

    #[test]
    fn empty_program() {
        run_tests([Case {
            input: "",
            expected: vec![],
        }]);
    }

    #[test]
    fn class_definitions() {
        run_tests([Case {
            input: r"
                class Point {
                    x: int
                    y: bool

                    def __init__(self, x: int, y: bool) {
                        self.x = x
                        self.y = y
                    }

                    def get_x(self) -> int {
                        return self.x
                    }

                    def __del__(self) { }
                }
            ",
            expected: vec![TopLevel::Class(ClassDef {
                name: String::from("Point"),
                fields: vec![
                    FieldDef {
                        name: String::from("x"),
                        ty: Type::Int,
                    },
                    FieldDef {
                        name: String::from("y"),
                        ty: Type::Bool,
                    },
                ],
                methods: vec![
                    MethodDef {
                        name: String::from("__init__"),
                        params: vec![
                            (String::from("x"), Type::Int),
                            (String::from("y"), Type::Bool),
                        ],
                        return_type: Type::Unit,
                        body: vec![
                            assign_field(self_ref(), "x", var("x")),
                            assign_field(self_ref(), "y", var("y")),
                        ],
                    },
                    MethodDef {
                        name: String::from("get_x"),
                        params: vec![],
                        return_type: Type::Int,
                        body: vec![Statement::Return(field(self_ref(), "x"))],
                    },
                    MethodDef {
                        name: String::from("__del__"),
                        params: vec![],
                        return_type: Type::Unit,
                        body: vec![],
                    },
                ],
            })],
        }]);
    }

    #[test]
    fn function_definitions() {
        run_tests([Case {
            input: "def add(x: int, y: int) -> int { return x + y }",
            expected: vec![TopLevel::Stmt(Statement::Function {
                name: String::from("add"),
                params: vec![
                    (String::from("x"), Type::Int),
                    (String::from("y"), Type::Int),
                ],
                return_type: Type::Int,
                body: vec![Statement::Return(bin(BinaryOp::Add, var("x"), var("y")))],
            })],
        }]);
    }

    #[test]
    fn assignments_and_delete() {
        run_tests([Case {
            input: r"
                class Point { x: int }
                p: Point = new Point(1, 2)
                p.x = 3
                delete p
            ",
            expected: vec![
                TopLevel::Class(ClassDef {
                    name: String::from("Point"),
                    fields: vec![FieldDef {
                        name: String::from("x"),
                        ty: Type::Int,
                    }],
                    methods: vec![],
                }),
                TopLevel::Stmt(assign_var(
                    "p",
                    Some(Type::Class(String::from("Point"))),
                    new_expr("Point", vec![int_lit(1), int_lit(2)]),
                )),
                TopLevel::Stmt(assign_field(var("p"), "x", int_lit(3))),
                TopLevel::Stmt(Statement::Delete(var("p"))),
            ],
        }]);
    }

    #[test]
    fn control_flow_expressions() {
        run_tests([Case {
            input: r"
                if (x < 1) { y = 2 } else { y = 3 }
                while (y != 0) { y = y - 1 }
            ",
            expected: vec![
                TopLevel::Stmt(Statement::Expression(
                    Expression::If {
                        cond: Box::new(bin(BinaryOp::LessThan, var("x"), int_lit(1))),
                        then_branch: vec![assign_var("y", None, int_lit(2))],
                        else_branch: vec![assign_var("y", None, int_lit(3))],
                    }
                    .untyped(),
                )),
                TopLevel::Stmt(Statement::Expression(
                    Expression::While {
                        cond: Box::new(bin(BinaryOp::NotEqual, var("y"), int_lit(0))),
                        body: vec![assign_var(
                            "y",
                            None,
                            bin(BinaryOp::Subtract, var("y"), int_lit(1)),
                        )],
                    }
                    .untyped(),
                )),
            ],
        }]);
    }

    #[test]
    fn postfix_chains_and_args() {
        run_tests([Case {
            input: "result = p.get(1).field.other(2 + 3 * 4)",
            expected: vec![TopLevel::Stmt(assign_var(
                "result",
                None,
                method_call(
                    field(method_call(var("p"), "get", vec![int_lit(1)]), "field"),
                    "other",
                    vec![bin(
                        BinaryOp::Add,
                        int_lit(2),
                        bin(BinaryOp::Multiply, int_lit(3), int_lit(4)),
                    )],
                ),
            ))],
        }]);
    }

    #[test]
    fn unary_and_precedence() {
        run_tests([Case {
            input: "check = !(1 + 2 * 3 == 7)",
            expected: vec![TopLevel::Stmt(assign_var(
                "check",
                None,
                unary(
                    UnaryOp::Not,
                    bin(
                        BinaryOp::Equal,
                        bin(
                            BinaryOp::Add,
                            int_lit(1),
                            bin(BinaryOp::Multiply, int_lit(2), int_lit(3)),
                        ),
                        int_lit(7),
                    ),
                ),
            ))],
        }]);
    }

    #[test]
    fn function_calls_and_new() {
        run_tests([Case {
            input: r"
                class Point { }
                def tick() { return 1 }
                tick()
                new Point()
            ",
            expected: vec![
                TopLevel::Class(ClassDef {
                    name: String::from("Point"),
                    fields: vec![],
                    methods: vec![],
                }),
                TopLevel::Stmt(Statement::Function {
                    name: String::from("tick"),
                    params: vec![],
                    return_type: Type::Unit,
                    body: vec![Statement::Return(int_lit(1))],
                }),
                TopLevel::Stmt(Statement::Expression(call("tick", vec![]))),
                TopLevel::Stmt(Statement::Expression(new_expr("Point", vec![]))),
            ],
        }]);
    }

    #[test]
    fn bool_literals() {
        run_tests([Case {
            input: "true false",
            expected: vec![
                TopLevel::Stmt(Statement::Expression(bool_lit(true))),
                TopLevel::Stmt(Statement::Expression(bool_lit(false))),
            ],
        }]);
    }
}
