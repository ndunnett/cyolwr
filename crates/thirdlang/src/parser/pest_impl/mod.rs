use pest::{
    self, Parser,
    iterators::{Pair, Pairs},
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
    AstBuilder::from_source(source)
}

#[derive(pest_derive::Parser)]
#[grammar = "parser/pest_impl/grammar.pest"]
struct ThirdlangParser;

struct AstBuilder;

impl AstBuilder {
    pub fn from_source(source: &str) -> Anyhow<Program> {
        let mut ast = Vec::new();

        for pair in ThirdlangParser::parse(Rule::Program, source)? {
            if pair.as_rule() == Rule::TopLevel {
                ast.push(Self::top_level(pair)?);
            }
        }

        Ok(ast)
    }

    fn ty(pair: Pair<Rule>) -> Anyhow<Type> {
        match pair.as_rule() {
            Rule::Type => Self::ty(Self::inner_pair(pair)?),
            Rule::IntType => Ok(Type::Int),
            Rule::BoolType => Ok(Type::Bool),
            Rule::ClassType => {
                let name = Self::inner_pair(pair)?.as_str().to_string();
                Ok(Type::Class(name))
            }
            Rule::Identifier => Ok(Type::Class(pair.as_str().to_string())),
            r => Err(anyhow!("expected type: {r:?}")),
        }
    }

    fn top_level(pair: Pair<Rule>) -> Anyhow<TopLevel> {
        let inner = Self::inner_pair(pair)?;

        match inner.as_rule() {
            Rule::ClassDef => Ok(TopLevel::Class(Self::class_def(inner)?)),
            Rule::Stmt => Ok(TopLevel::Stmt(Self::statement(inner)?)),
            r => Err(anyhow!("expected top-level rule: {r:?}")),
        }
    }

    fn class_def(pair: Pair<Rule>) -> Anyhow<ClassDef> {
        let mut pairs = pair.into_inner();
        let name = Self::next_pair(&mut pairs)?.as_str().to_string();
        let body = Self::next_pair(&mut pairs)?;

        let mut fields = Vec::new();
        let mut methods = Vec::new();

        for pair in body.into_inner() {
            match pair.as_rule() {
                Rule::FieldDef => fields.push(Self::field_def(pair)?),
                Rule::MethodDef => methods.push(Self::method_def(pair)?),
                _ => {}
            }
        }

        Ok(ClassDef {
            name,
            fields,
            methods,
        })
    }

    fn field_def(pair: Pair<Rule>) -> Anyhow<FieldDef> {
        let mut pairs = pair.into_inner();
        let name = Self::next_pair(&mut pairs)?.as_str().to_string();
        let ty = Self::ty(Self::next_pair(&mut pairs)?)?;
        Ok(FieldDef { name, ty })
    }

    fn method_def(pair: Pair<Rule>) -> Anyhow<MethodDef> {
        let mut pairs = pair.into_inner();
        let name = Self::next_pair(&mut pairs)?.as_str().to_string();
        Self::next_pair(&mut pairs)?; // skip self parameter

        let mut params = Vec::new();
        let mut return_type = Type::Unit;
        let mut body = Vec::new();

        for pair in pairs {
            match pair.as_rule() {
                Rule::TypedParam => {
                    let mut pairs = pair.into_inner();
                    let name = Self::next_pair(&mut pairs)?.as_str().to_string();
                    let ty = Self::ty(Self::next_pair(&mut pairs)?)?;
                    params.push((name, ty));
                }
                Rule::ReturnType => return_type = Self::ty(Self::inner_pair(pair)?)?,
                Rule::Block => body = Self::block(pair)?,
                _ => {}
            }
        }

        Ok(MethodDef {
            name,
            params,
            return_type,
            body,
        })
    }

    fn statement(pair: Pair<Rule>) -> Anyhow<Statement> {
        let inner = Self::inner_pair(pair)?;

        match inner.as_rule() {
            Rule::Function => Self::function(inner),
            Rule::Delete => Self::delete(inner),
            Rule::Return => Self::return_(inner),
            Rule::Assignment => Self::assignment(inner),
            Rule::Expr | Rule::Conditional | Rule::WhileLoop | Rule::Comparison => {
                Ok(Statement::Expression(Self::expression(inner)?))
            }
            _ => Err(anyhow!("expected statement: {inner:?}")),
        }
    }

    fn function(pair: Pair<Rule>) -> Anyhow<Statement> {
        let mut pairs = pair.into_inner();
        let name = Self::next_pair(&mut pairs)?.as_str().to_string();
        let mut params = Vec::new();
        let mut return_type = Type::Unknown;
        let mut body = Vec::new();

        for pair in pairs {
            match pair.as_rule() {
                Rule::TypedParam => {
                    let mut pairs = pair.into_inner();
                    let name = Self::next_pair(&mut pairs)?.as_str().to_string();
                    let ty = Self::ty(Self::next_pair(&mut pairs)?)?;
                    params.push((name, ty));
                }
                Rule::ReturnType => return_type = Self::ty(Self::inner_pair(pair)?)?,
                Rule::Block => body = Self::block(pair)?,
                _ => {}
            }
        }

        Ok(Statement::Function {
            name,
            params,
            return_type,
            body,
        })
    }

    fn block(pair: Pair<Rule>) -> Anyhow<Vec<Statement>> {
        let mut block = Vec::new();

        for pair in pair.into_inner() {
            if pair.as_rule() == Rule::Stmt {
                block.push(Self::statement(pair)?);
            }
        }

        Ok(block)
    }

    fn delete(pair: Pair<Rule>) -> Anyhow<Statement> {
        let inner = Self::inner_pair(pair)?;
        Ok(Statement::Delete(Self::expression(inner)?))
    }

    fn return_(pair: Pair<Rule>) -> Anyhow<Statement> {
        let inner = Self::inner_pair(pair)?;
        Ok(Statement::Return(Self::expression(inner)?))
    }

    fn assignment(pair: Pair<Rule>) -> Anyhow<Statement> {
        let mut pairs = pair.into_inner();
        let target_pair = Self::next_pair(&mut pairs)?;

        let target = match target_pair.as_rule() {
            Rule::AssignTarget => {
                let mut pairs = target_pair.into_inner();
                let inner = Self::next_pair(&mut pairs)?;

                match inner.as_rule() {
                    Rule::FieldAccess => Self::assign_field_access(inner)?,
                    Rule::Identifier => AssignTarget::Var(inner.as_str().to_string()),
                    r => return Err(anyhow!("expected assignment target: {r:?}")),
                }
            }
            Rule::FieldAccess => Self::assign_field_access(target_pair)?,
            Rule::Identifier => AssignTarget::Var(target_pair.as_str().to_string()),
            r => return Err(anyhow!("expected assignment target: {r:?}")),
        };

        let mut type_ann = None;
        let mut value = None;

        for item in pairs {
            match item.as_rule() {
                Rule::Type => type_ann = Some(Self::ty(item)?),
                _ => value = Some(Self::expression(item)?),
            }
        }

        let value = value.ok_or_else(|| anyhow!("failed to parse assignment value"))?;

        Ok(Statement::Assignment {
            target,
            type_ann,
            value,
        })
    }

    fn assign_field_access(pair: Pair<Rule>) -> Anyhow<AssignTarget> {
        let mut pairs = pair.into_inner();
        let first = Self::next_pair(&mut pairs)?;

        let mut object = match first.as_rule() {
            Rule::SelfKeyword => Expression::SelfRef.untyped(),
            Rule::Identifier => Expression::Var(first.as_str().to_string()).untyped(),
            r => return Err(anyhow!("expected field access base: {r:?}")),
        };

        let mut field = String::new();

        for field_pair in pairs {
            if !field.is_empty() {
                object = Expression::FieldAccess {
                    object: Box::new(object),
                    field,
                }
                .untyped();
            }

            field = field_pair.as_str().to_string();
        }

        Ok(AssignTarget::Field {
            object: Box::new(object),
            field,
        })
    }

    fn expression(pair: Pair<Rule>) -> Anyhow<TypedExpr> {
        let expr = match pair.as_rule() {
            Rule::Expr => return Self::expression(Self::inner_pair(pair)?),
            Rule::Conditional => Self::conditional(pair),
            Rule::WhileLoop => Self::while_(pair),
            Rule::Comparison | Rule::Additive | Rule::Multiplicative => return Self::binary(pair),
            Rule::Unary => return Self::unary(pair),
            Rule::Postfix => return Self::postfix(pair),
            Rule::NewExpr => Self::new_expr(pair),
            Rule::FunctionCall => return Self::function_call(pair),
            Rule::Literal => return Self::literal(&Self::inner_pair(pair)?),
            Rule::Int | Rule::Bool => return Self::literal(&pair),
            Rule::SelfKeyword => Ok(Expression::SelfRef),
            Rule::Identifier => Ok(Expression::Var(pair.as_str().to_string())),
            Rule::Block => Ok(Expression::Block(Self::block(pair)?)),
            _ => Err(anyhow!("expected expression: {pair:?}")),
        }?;

        Ok(expr.untyped())
    }

    fn conditional(pair: Pair<Rule>) -> Anyhow<Expression> {
        let mut pairs = pair.into_inner();
        let cond = Box::new(Self::expression(Self::next_pair(&mut pairs)?)?);
        let then_branch = Self::block(Self::next_pair(&mut pairs)?)?;
        let else_branch = Self::block(Self::next_pair(&mut pairs)?)?;

        Ok(Expression::If {
            cond,
            then_branch,
            else_branch,
        })
    }

    fn while_(pair: Pair<Rule>) -> Anyhow<Expression> {
        let mut pairs = pair.into_inner();
        let cond = Box::new(Self::expression(Self::next_pair(&mut pairs)?)?);
        let body = Self::block(Self::next_pair(&mut pairs)?)?;
        Ok(Expression::While { cond, body })
    }

    fn binary(pair: Pair<Rule>) -> Anyhow<TypedExpr> {
        let mut pairs = pair.into_inner();
        let mut expr = Self::expression(Self::next_pair(&mut pairs)?)?;

        while let Ok(op) = Self::binary_op(&mut pairs) {
            let next_pair = Self::next_pair(&mut pairs)?;
            let right = Box::new(Self::expression(next_pair)?);
            let left = Box::new(expr);
            expr = Expression::Binary { op, left, right }.untyped();
        }

        Ok(expr)
    }

    fn binary_op(pairs: &mut Pairs<Rule>) -> Anyhow<BinaryOp> {
        let Some(pair) = pairs.next() else {
            return Err(anyhow!("failed to unwrap operator"));
        };

        match pair.as_str() {
            "+" => Ok(BinaryOp::Add),
            "-" => Ok(BinaryOp::Subtract),
            "*" => Ok(BinaryOp::Multiply),
            "/" => Ok(BinaryOp::Divide),
            "%" => Ok(BinaryOp::Modulo),
            "<" => Ok(BinaryOp::LessThan),
            ">" => Ok(BinaryOp::GreaterThan),
            "<=" => Ok(BinaryOp::LessEqual),
            ">=" => Ok(BinaryOp::GreaterEqual),
            "==" => Ok(BinaryOp::Equal),
            "!=" => Ok(BinaryOp::NotEqual),
            unknown => Err(anyhow!("expected binary operator: {unknown}")),
        }
    }

    fn unary(pair: Pair<Rule>) -> Anyhow<TypedExpr> {
        let mut pairs = pair.into_inner();
        let pair = Self::next_pair(&mut pairs)?;

        if pair.as_rule() == Rule::UnaryOp {
            let op = Self::unary_op(&pair)?;
            let expr = Box::new(Self::expression(Self::next_pair(&mut pairs)?)?);
            Ok(Expression::Unary { op, expr }.untyped())
        } else {
            Self::expression(pair)
        }
    }

    fn unary_op(pair: &Pair<Rule>) -> Anyhow<UnaryOp> {
        match pair.as_str() {
            "-" => Ok(UnaryOp::Negative),
            "!" => Ok(UnaryOp::Not),
            unknown => Err(anyhow!("expected unary operator: {unknown}")),
        }
    }

    fn postfix(pair: Pair<Rule>) -> Anyhow<TypedExpr> {
        let mut pairs = pair.into_inner();
        let mut expr = Self::expression(Self::next_pair(&mut pairs)?)?;

        for pair in pairs {
            let pair = if pair.as_rule() == Rule::PostfixOp {
                Self::inner_pair(pair)?
            } else {
                pair
            };

            match pair.as_rule() {
                Rule::MethodCall => {
                    let mut pairs = pair.into_inner();
                    let method = Self::next_pair(&mut pairs)?.as_str().to_string();
                    let args = pairs.map(Self::expression).collect::<Anyhow<Vec<_>>>()?;

                    expr = Expression::MethodCall {
                        object: Box::new(expr),
                        method,
                        args,
                    }
                    .untyped();
                }
                Rule::FieldAccessOp => {
                    let field = Self::inner_pair(pair)?.as_str().to_string();

                    expr = Expression::FieldAccess {
                        object: Box::new(expr),
                        field,
                    }
                    .untyped();
                }
                _ => {}
            }
        }

        Ok(expr)
    }

    fn new_expr(pair: Pair<Rule>) -> Anyhow<Expression> {
        let mut pairs = pair.into_inner();
        let class = Self::next_pair(&mut pairs)?.as_str().to_string();
        let args = pairs.map(Self::expression).collect::<Anyhow<Vec<_>>>()?;
        Ok(Expression::New { class, args })
    }

    fn function_call(pair: Pair<Rule>) -> Anyhow<TypedExpr> {
        let mut pairs = pair.into_inner();
        let name = Self::next_pair(&mut pairs)?.as_str().to_string();
        let args = pairs.map(Self::expression).collect::<Anyhow<Vec<_>>>()?;
        Ok(Expression::Call { name, args }.untyped())
    }

    fn literal(pair: &Pair<Rule>) -> Anyhow<TypedExpr> {
        match pair.as_rule() {
            Rule::Int => Ok(Expression::Int(pair.as_str().parse()?).typed(Type::Int)),
            Rule::Bool => Ok(Expression::Bool(pair.as_str() == "true").typed(Type::Bool)),
            _ => Err(anyhow!("expected literal: {pair}")),
        }
    }

    fn next_pair<'a>(pairs: &mut Pairs<'a, Rule>) -> Anyhow<Pair<'a, Rule>> {
        pairs
            .next()
            .ok_or_else(|| anyhow!("failed to unwrap inner pair"))
    }

    fn inner_pair(pair: Pair<Rule>) -> Anyhow<Pair<Rule>> {
        pair.into_inner()
            .next()
            .ok_or_else(|| anyhow!("failed to unwrap inner pair"))
    }
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
