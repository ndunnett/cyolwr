use std::num::ParseIntError;

use winnow::{
    ascii::{dec_int, multispace0, till_line_ending},
    combinator::{
        alt, delimited, not, opt, preceded, repeat, separated, separated_pair, seq, terminated,
    },
    error::{FromExternalError, InputError, ParserError},
    prelude::*,
    token::{one_of, take_while},
};

use crate::{
    Anyhow, anyhow,
    ast::{
        AssignTarget, BinaryOp, ClassDef, Expression, FieldDef, MethodDef, Program, Statement,
        TopLevel, TypedExpr, UnaryOp,
    },
    types::Type,
};

pub fn parse(source: &str) -> Anyhow<Program> {
    program::<InputError<_>>
        .parse(source)
        .map_err(|e| anyhow!("{e}"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Keyword {
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
    const fn lexeme(self) -> &'static str {
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

    fn any<'i, E: ParserError<&'i str>>(i: &mut &'i str) -> ModalResult<&'i str, E> {
        alt((
            Self::Def,
            Self::If,
            Self::Else,
            Self::While,
            Self::Return,
            Self::Class,
            Self::SelfRef,
            Self::New,
            Self::Delete,
            Self::True,
            Self::False,
        ))
        .parse_next(i)
    }
}

impl<'i, E: ParserError<&'i str>> Parser<&'i str, &'i str, E> for Keyword {
    fn parse_next(&mut self, input: &mut &'i str) -> Result<&'i str, E> {
        ws(terminated(
            self.lexeme(),
            not(one_of(|c: char| c.is_alphanum() || c == '_')),
        ))
        .parse_next(input)
    }
}

fn comments<'i, E: ParserError<&'i str>>(i: &mut &'i str) -> Result<(), E> {
    (
        multispace0,
        repeat(.., ('#', till_line_ending, multispace0)).map(|_: Vec<_>| ()),
    )
        .void()
        .parse_next(i)
}

fn ws<'i, F, O, E: ParserError<&'i str>>(inner: F) -> impl Parser<&'i str, O, E>
where
    F: Parser<&'i str, O, E>,
{
    delimited(comments, inner, comments)
}

fn identifier<'i, E: ParserError<&'i str>>(i: &mut &'i str) -> ModalResult<&'i str, E> {
    ws((
        not(Keyword::any),
        (
            one_of(|c: char| c.is_alpha() || c == '_'),
            take_while(0.., |c: char| c.is_alphanum() || c == '_'),
        ),
    )
        .take())
    .parse_next(i)
}

fn parse_type<'i, E: ParserError<&'i str>>(i: &mut &'i str) -> ModalResult<Type, E> {
    alt((
        ws("int").value(Type::Int),
        ws("bool").value(Type::Bool),
        identifier.map(String::from).map(Type::Class),
    ))
    .parse_next(i)
}

fn program<'i, E>(i: &mut &'i str) -> ModalResult<Program, E>
where
    E: ParserError<&'i str> + FromExternalError<&'i str, ParseIntError>,
{
    repeat(.., top_level).parse_next(i)
}

fn top_level<'i, E>(i: &mut &'i str) -> ModalResult<TopLevel, E>
where
    E: ParserError<&'i str> + FromExternalError<&'i str, ParseIntError>,
{
    alt((class_def.map(TopLevel::Class), stmt.map(TopLevel::Stmt))).parse_next(i)
}

#[derive(Debug, Clone)]
enum ClassBodyItem {
    Field(FieldDef),
    Method(MethodDef),
}

fn class_def<'i, E>(i: &mut &'i str) -> ModalResult<ClassDef, E>
where
    E: ParserError<&'i str> + FromExternalError<&'i str, ParseIntError>,
{
    (
        preceded(Keyword::Class, identifier),
        delimited(ws('{'), class_body, ws('}')),
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

fn class_body<'i, E>(i: &mut &'i str) -> ModalResult<Vec<ClassBodyItem>, E>
where
    E: ParserError<&'i str> + FromExternalError<&'i str, ParseIntError>,
{
    repeat(
        ..,
        alt((
            field_def.map(ClassBodyItem::Field),
            method_def.map(ClassBodyItem::Method),
        )),
    )
    .parse_next(i)
}

fn field_def<'i, E>(i: &mut &'i str) -> ModalResult<FieldDef, E>
where
    E: ParserError<&'i str> + FromExternalError<&'i str, ParseIntError>,
{
    typed_param
        .map(|(name, ty)| FieldDef { name, ty })
        .parse_next(i)
}

fn method_def<'i, E>(i: &mut &'i str) -> ModalResult<MethodDef, E>
where
    E: ParserError<&'i str> + FromExternalError<&'i str, ParseIntError>,
{
    (
        preceded(Keyword::Def, identifier),
        delimited(
            ws('('),
            preceded(Keyword::SelfRef, opt(method_params)),
            ws(')'),
        ),
        opt(return_type),
        block,
    )
        .map(|(id, params, return_type, body)| MethodDef {
            name: String::from(id),
            params: params.unwrap_or_default(),
            return_type: return_type.unwrap_or(Type::Unit),
            body,
        })
        .parse_next(i)
}

fn method_params<'i, E>(i: &mut &'i str) -> ModalResult<Vec<(String, Type)>, E>
where
    E: ParserError<&'i str> + FromExternalError<&'i str, ParseIntError>,
{
    preceded(ws(','), typed_params).parse_next(i)
}

fn stmt<'i, E>(i: &mut &'i str) -> ModalResult<Statement, E>
where
    E: ParserError<&'i str> + FromExternalError<&'i str, ParseIntError>,
{
    alt((function, simple_stmt)).parse_next(i)
}

fn simple_stmt<'i, E>(i: &mut &'i str) -> ModalResult<Statement, E>
where
    E: ParserError<&'i str> + FromExternalError<&'i str, ParseIntError>,
{
    alt((
        delete,
        return_stmt,
        assignment,
        expr.map(Statement::Expression),
    ))
    .parse_next(i)
}

fn function<'i, E>(i: &mut &'i str) -> ModalResult<Statement, E>
where
    E: ParserError<&'i str> + FromExternalError<&'i str, ParseIntError>,
{
    (
        preceded(Keyword::Def, identifier),
        delimited(ws('('), typed_params, ws(')')),
        opt(return_type),
        block,
    )
        .map(|(id, params, return_type, body)| Statement::Function {
            name: String::from(id),
            params,
            return_type: return_type.unwrap_or(Type::Unit),
            body,
        })
        .parse_next(i)
}

fn typed_params<'i, E>(i: &mut &'i str) -> ModalResult<Vec<(String, Type)>, E>
where
    E: ParserError<&'i str> + FromExternalError<&'i str, ParseIntError>,
{
    separated(.., typed_param, ws(',')).parse_next(i)
}

fn typed_param<'i, E>(i: &mut &'i str) -> ModalResult<(String, Type), E>
where
    E: ParserError<&'i str> + FromExternalError<&'i str, ParseIntError>,
{
    separated_pair(identifier, ws(':'), parse_type)
        .map(|(id, ty)| (String::from(id), ty))
        .parse_next(i)
}

fn return_type<'i, E>(i: &mut &'i str) -> ModalResult<Type, E>
where
    E: ParserError<&'i str> + FromExternalError<&'i str, ParseIntError>,
{
    preceded(ws("->"), parse_type).parse_next(i)
}

fn block<'i, E>(i: &mut &'i str) -> ModalResult<Vec<Statement>, E>
where
    E: ParserError<&'i str> + FromExternalError<&'i str, ParseIntError>,
{
    delimited(ws('{'), repeat(.., stmt), ws('}')).parse_next(i)
}

fn return_stmt<'i, E>(i: &mut &'i str) -> ModalResult<Statement, E>
where
    E: ParserError<&'i str> + FromExternalError<&'i str, ParseIntError>,
{
    preceded(Keyword::Return, expr)
        .map(Statement::Return)
        .parse_next(i)
}

fn delete<'i, E>(i: &mut &'i str) -> ModalResult<Statement, E>
where
    E: ParserError<&'i str> + FromExternalError<&'i str, ParseIntError>,
{
    preceded(Keyword::Delete, expr)
        .map(Statement::Delete)
        .parse_next(i)
}

fn assignment<'i, E>(i: &mut &'i str) -> ModalResult<Statement, E>
where
    E: ParserError<&'i str> + FromExternalError<&'i str, ParseIntError>,
{
    seq! { Statement::Assignment {
        target: assign_target,
        type_ann: opt(preceded(ws(':'), parse_type)),
        _: ws('='),
        value: expr,
    } }
    .parse_next(i)
}

fn assign_target<'i, E>(i: &mut &'i str) -> ModalResult<AssignTarget, E>
where
    E: ParserError<&'i str> + FromExternalError<&'i str, ParseIntError>,
{
    alt((
        field_access,
        identifier.map(String::from).map(AssignTarget::Var),
    ))
    .parse_next(i)
}

fn expr<'i, E>(i: &mut &'i str) -> ModalResult<TypedExpr, E>
where
    E: ParserError<&'i str> + FromExternalError<&'i str, ParseIntError>,
{
    alt((conditional, while_loop, comparison)).parse_next(i)
}

fn conditional<'i, E>(i: &mut &'i str) -> ModalResult<TypedExpr, E>
where
    E: ParserError<&'i str> + FromExternalError<&'i str, ParseIntError>,
{
    seq! {Expression::If {
       _: Keyword::If,
       _: ws('('),
       cond: expr.map(Box::new),
       _: ws(')'),
       then_branch: block,
       _: Keyword::Else,
       else_branch: block,
    }}
    .map(Expression::untyped)
    .parse_next(i)
}

fn while_loop<'i, E>(i: &mut &'i str) -> ModalResult<TypedExpr, E>
where
    E: ParserError<&'i str> + FromExternalError<&'i str, ParseIntError>,
{
    seq! {Expression::While {
       _: Keyword::While,
       _: ws('('),
       cond: expr.map(Box::new),
       _: ws(')'),
       body: block,
    }}
    .map(Expression::untyped)
    .parse_next(i)
}

fn binary<'i, F1, F2, E>(operator: F1, operand: F2) -> impl Parser<&'i str, TypedExpr, E>
where
    E: ParserError<&'i str> + FromExternalError<&'i str, ParseIntError>,
    F1: Parser<&'i str, BinaryOp, E> + Copy,
    F2: Parser<&'i str, TypedExpr, E> + Copy,
{
    alt((
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
        }),
        operand,
    ))
}

fn comparison<'i, E>(i: &mut &'i str) -> ModalResult<TypedExpr, E>
where
    E: ParserError<&'i str> + FromExternalError<&'i str, ParseIntError>,
{
    binary(comp_op, additive).parse_next(i)
}

fn comp_op<'i, E>(i: &mut &'i str) -> ModalResult<BinaryOp, E>
where
    E: ParserError<&'i str> + FromExternalError<&'i str, ParseIntError>,
{
    alt((
        ws("<=").value(BinaryOp::LessEqual),
        ws(">=").value(BinaryOp::GreaterEqual),
        ws("<").value(BinaryOp::LessThan),
        ws(">").value(BinaryOp::GreaterThan),
        ws("==").value(BinaryOp::Equal),
        ws("!=").value(BinaryOp::NotEqual),
    ))
    .parse_next(i)
}

fn additive<'i, E>(i: &mut &'i str) -> ModalResult<TypedExpr, E>
where
    E: ParserError<&'i str> + FromExternalError<&'i str, ParseIntError>,
{
    binary(add_op, multiplicative).parse_next(i)
}

fn add_op<'i, E>(i: &mut &'i str) -> ModalResult<BinaryOp, E>
where
    E: ParserError<&'i str> + FromExternalError<&'i str, ParseIntError>,
{
    alt((
        ws("+").value(BinaryOp::Add),
        ws("-").value(BinaryOp::Subtract),
    ))
    .parse_next(i)
}

fn multiplicative<'i, E>(i: &mut &'i str) -> ModalResult<TypedExpr, E>
where
    E: ParserError<&'i str> + FromExternalError<&'i str, ParseIntError>,
{
    binary(mul_op, unary).parse_next(i)
}

fn mul_op<'i, E>(i: &mut &'i str) -> ModalResult<BinaryOp, E>
where
    E: ParserError<&'i str> + FromExternalError<&'i str, ParseIntError>,
{
    alt((
        ws("*").value(BinaryOp::Multiply),
        ws("/").value(BinaryOp::Divide),
        ws("%").value(BinaryOp::Modulo),
    ))
    .parse_next(i)
}

fn unary<'i, E>(i: &mut &'i str) -> ModalResult<TypedExpr, E>
where
    E: ParserError<&'i str> + FromExternalError<&'i str, ParseIntError>,
{
    alt((
        (unary_op, unary).map(|(op, expr)| {
            Expression::Unary {
                op,
                expr: Box::new(expr),
            }
            .untyped()
        }),
        postfix,
    ))
    .parse_next(i)
}

fn unary_op<'i, E>(i: &mut &'i str) -> ModalResult<UnaryOp, E>
where
    E: ParserError<&'i str> + FromExternalError<&'i str, ParseIntError>,
{
    alt((
        ws("-").value(UnaryOp::Negative),
        ws("!").value(UnaryOp::Not),
    ))
    .parse_next(i)
}

fn postfix<'i, E>(i: &mut &'i str) -> ModalResult<TypedExpr, E>
where
    E: ParserError<&'i str> + FromExternalError<&'i str, ParseIntError>,
{
    let expr = primary.parse_next(i)?;

    repeat(0.., postfix_op)
        .fold(
            // not sure about this
            move || expr.clone(),
            |expr, op| match op {
                (field, None) => Expression::FieldAccess {
                    object: Box::new(expr),
                    field,
                }
                .untyped(),
                (method, Some(args)) => Expression::MethodCall {
                    object: Box::new(expr),
                    method,
                    args,
                }
                .untyped(),
            },
        )
        .parse_next(i)
}

fn postfix_op<'i, E>(i: &mut &'i str) -> ModalResult<(String, Option<Vec<TypedExpr>>), E>
where
    E: ParserError<&'i str> + FromExternalError<&'i str, ParseIntError>,
{
    alt((method_call, field_access_op)).parse_next(i)
}

fn method_call<'i, E>(i: &mut &'i str) -> ModalResult<(String, Option<Vec<TypedExpr>>), E>
where
    E: ParserError<&'i str> + FromExternalError<&'i str, ParseIntError>,
{
    preceded(ws('.'), (identifier, delimited(ws('('), args, ws(')'))))
        .map(|(id, args)| (String::from(id), Some(args)))
        .parse_next(i)
}

fn field_access_op<'i, E>(i: &mut &'i str) -> ModalResult<(String, Option<Vec<TypedExpr>>), E>
where
    E: ParserError<&'i str> + FromExternalError<&'i str, ParseIntError>,
{
    preceded(ws('.'), identifier)
        .map(|id| (String::from(id), None))
        .parse_next(i)
}

fn field_access<'i, E>(i: &mut &'i str) -> ModalResult<AssignTarget, E>
where
    E: ParserError<&'i str> + FromExternalError<&'i str, ParseIntError>,
{
    (
        (
            alt((
                self_keyword,
                identifier
                    .map(String::from)
                    .map(|s| Expression::Var(s).untyped()),
            )),
            preceded(ws('.'), identifier),
        ),
        repeat(0.., preceded(ws('.'), identifier)),
    )
        .map(|((mut object, target), chain): (_, Vec<_>)| {
            let mut field = String::from(target);

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

fn self_keyword<'i, E>(i: &mut &'i str) -> ModalResult<TypedExpr, E>
where
    E: ParserError<&'i str> + FromExternalError<&'i str, ParseIntError>,
{
    Keyword::SelfRef
        .map(|_| Expression::SelfRef.untyped())
        .parse_next(i)
}

fn primary<'i, E>(i: &mut &'i str) -> ModalResult<TypedExpr, E>
where
    E: ParserError<&'i str> + FromExternalError<&'i str, ParseIntError>,
{
    alt((
        new_expr,
        function_call,
        literal_value,
        self_keyword,
        var_expr,
        delimited(ws('('), expr, ws(')')),
    ))
    .parse_next(i)
}

fn var_expr<'i, E>(i: &mut &'i str) -> ModalResult<TypedExpr, E>
where
    E: ParserError<&'i str> + FromExternalError<&'i str, ParseIntError>,
{
    identifier
        .map(|id| Expression::Var(String::from(id)).untyped())
        .parse_next(i)
}

fn new_expr<'i, E>(i: &mut &'i str) -> ModalResult<TypedExpr, E>
where
    E: ParserError<&'i str> + FromExternalError<&'i str, ParseIntError>,
{
    seq!(
        _: Keyword::New,
        identifier,
        delimited(ws('('), args, ws(')')))
    .map(|(id, args)| {
        Expression::New {
            class: String::from(id),
            args,
        }
        .untyped()
    })
    .parse_next(i)
}

fn function_call<'i, E>(i: &mut &'i str) -> ModalResult<TypedExpr, E>
where
    E: ParserError<&'i str> + FromExternalError<&'i str, ParseIntError>,
{
    (identifier, delimited(ws('('), args, ws(')')))
        .map(|(id, args)| {
            Expression::Call {
                name: String::from(id),
                args,
            }
            .untyped()
        })
        .parse_next(i)
}

fn args<'i, E>(i: &mut &'i str) -> ModalResult<Vec<TypedExpr>, E>
where
    E: ParserError<&'i str> + FromExternalError<&'i str, ParseIntError>,
{
    separated(0.., expr, ws(',')).parse_next(i)
}

fn literal_value<'i, E>(i: &mut &'i str) -> ModalResult<TypedExpr, E>
where
    E: ParserError<&'i str> + FromExternalError<&'i str, ParseIntError>,
{
    alt((
        int_value.map(|x| Expression::Int(x).typed(Type::Int)),
        bool_value.map(|x| Expression::Bool(x).typed(Type::Bool)),
    ))
    .parse_next(i)
}

fn int_value<'i, E>(i: &mut &'i str) -> ModalResult<i64, E>
where
    E: ParserError<&'i str> + FromExternalError<&'i str, ParseIntError>,
{
    ws(dec_int).parse_next(i)
}

fn bool_value<'i, E: ParserError<&'i str>>(i: &mut &'i str) -> ModalResult<bool, E> {
    alt((Keyword::True.value(true), Keyword::False.value(false))).parse_next(i)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_class_def() {
        let source = r"
            class Point {
                x: int
                y: int

                def __init__(self, x: int, y: int) {
                    self.x = x
                    self.y = y
                }

                def get_x(self) -> int {
                    return self.x
                }
            }
        ";

        let program = parse(source).unwrap();
        assert_eq!(program.len(), 1);

        if let TopLevel::Class(class) = &program[0] {
            assert_eq!(class.name, "Point");
            assert_eq!(class.fields.len(), 2);
            assert_eq!(class.methods.len(), 2);
        } else {
            panic!("Expected class definition");
        }
    }

    #[test]
    fn test_parse_new_expr() {
        let source = r"
            class Point { x: int }
            new Point(42)
        ";

        let program = parse(source).unwrap();
        assert_eq!(program.len(), 2);

        if let TopLevel::Stmt(Statement::Expression(expr)) = &program[1] {
            if let Expression::New { class, args } = &expr.expr {
                assert_eq!(class, "Point");
                assert_eq!(args.len(), 1);
            } else {
                panic!("Expected new expression");
            }
        } else {
            panic!("Expected statement");
        }
    }

    #[test]
    fn test_parse_method_call() {
        let source = r"
            class Point { x: int def get_x(self) -> int { return self.x } }
            p = new Point(10)
            p.get_x()
        ";

        let program = parse(source).unwrap();
        assert_eq!(program.len(), 3);
    }

    #[test]
    fn test_parse_delete() {
        let source = r"
            class Point { x: int }
            p = new Point(42)
            delete p
        ";

        let program = parse(source).unwrap();
        assert_eq!(program.len(), 3);

        if let TopLevel::Stmt(Statement::Delete(_)) = &program[2] {
            // OK
        } else {
            panic!("Expected delete statement");
        }
    }

    #[test]
    fn test_parse_field_assignment() {
        let source = r"
            class Point { x: int def set_x(self, val: int) { self.x = val } }
        ";

        let program = parse(source).unwrap();
        assert_eq!(program.len(), 1);
    }
}
