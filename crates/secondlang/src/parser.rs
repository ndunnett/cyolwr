use pest::{
    self, Parser,
    iterators::{Pair, Pairs},
};

use crate::{
    Anyhow, Type, anyhow,
    ast::{BinaryOp, Expression, Program, Statement, TypedExpr, UnaryOp},
};

pub fn parse(source: &str) -> Anyhow<Program> {
    AstBuilder::from_source(source)
}

#[derive(pest_derive::Parser)]
#[grammar = "grammar.pest"]
struct SecondlangParser;

struct AstBuilder;

impl AstBuilder {
    pub fn from_source(source: &str) -> Anyhow<Program> {
        let mut ast = Vec::new();

        for pair in SecondlangParser::parse(Rule::Program, source)? {
            if pair.as_rule() == Rule::Stmt {
                ast.push(Self::statement(pair)?);
            }
        }

        Ok(ast)
    }

    fn ty(pair: Pair<Rule>) -> Anyhow<Type> {
        match pair.as_rule() {
            Rule::Type => Self::ty(pair.into_inner().next().unwrap()),
            Rule::IntType => Ok(Type::Int),
            Rule::BoolType => Ok(Type::Bool),
            r => Err(anyhow!("expected type: {r:?}")),
        }
    }

    fn statement(pair: Pair<Rule>) -> Anyhow<Statement> {
        let mut pairs = pair.into_inner();
        let inner = Self::next_pair(&mut pairs)?;

        match inner.as_rule() {
            Rule::Function => Self::function(inner),
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
                Rule::ReturnType => {
                    return_type = Self::ty(Self::next_pair(&mut pair.into_inner())?)?;
                }
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

    fn return_(pair: Pair<Rule>) -> Anyhow<Statement> {
        let mut pairs = pair.into_inner();
        let inner = Self::next_pair(&mut pairs)?;
        Ok(Statement::Return(Self::expression(inner)?))
    }

    fn assignment(pair: Pair<Rule>) -> Anyhow<Statement> {
        let mut pairs = pair.into_inner();
        let name = Self::next_pair(&mut pairs)?.as_str().to_string();
        let mut type_ann = None;
        let mut value = None;

        for pair in pairs {
            match pair.as_rule() {
                Rule::Type => type_ann = Some(Self::ty(pair)?),
                _ => value = Some(Self::expression(pair)?),
            }
        }

        let value = value.ok_or_else(|| anyhow!("failed to parse assignment value"))?;

        Ok(Statement::Assignment {
            name,
            type_ann,
            value,
        })
    }

    fn expression(pair: Pair<Rule>) -> Anyhow<TypedExpr> {
        let expr = match pair.as_rule() {
            Rule::Expr => return Self::expression(Self::next_pair(&mut pair.into_inner())?),
            Rule::Conditional => Self::conditional(pair),
            Rule::WhileLoop => Self::while_(pair),
            Rule::Comparison | Rule::Additive | Rule::Multiplicative => return Self::binary(pair),
            Rule::Unary => return Self::unary(pair),
            Rule::Call => return Self::call(pair),
            Rule::Literal => return Self::literal(&Self::next_pair(&mut pair.into_inner())?),
            Rule::Int | Rule::Bool => return Self::literal(&pair),
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

    fn call(pair: Pair<Rule>) -> Anyhow<TypedExpr> {
        let mut pairs = pair.into_inner();
        let mut expr = Self::expression(Self::next_pair(&mut pairs)?)?;

        for pair in pairs {
            if pair.as_rule() == Rule::CallArgs {
                let args = pair
                    .into_inner()
                    .map(Self::expression)
                    .collect::<Anyhow<Vec<_>>>()?;

                if let Expression::Var(name) = expr.expr {
                    expr = Expression::Call { name, args }.untyped();
                } else {
                    return Err(anyhow!("can only call named functions"));
                }
            }
        }

        Ok(expr)
    }

    fn literal(pair: &Pair<Rule>) -> Anyhow<TypedExpr> {
        match pair.as_rule() {
            Rule::Int => Ok(Expression::Int(pair.as_str().parse()?).typed(Type::Int)),
            Rule::Bool => Ok(Expression::Bool(pair.as_str() == "true").typed(Type::Bool)),
            _ => Err(anyhow!("expected literal: {pair}")),
        }
    }

    fn next_pair<'a>(pairs: &'a mut Pairs<Rule>) -> Anyhow<Pair<'a, Rule>> {
        pairs
            .next()
            .ok_or_else(|| anyhow!("failed to unwrap inner pair"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_typed_function() {
        let source = "def add(a: int, b: int) -> int { return a + b }";
        let program = parse(source).unwrap();

        if let Statement::Function {
            name,
            params,
            return_type,
            ..
        } = &program[0]
        {
            assert_eq!(name, "add");
            assert_eq!(params.len(), 2);
            assert_eq!(params[0], ("a".to_string(), Type::Int));
            assert_eq!(params[1], ("b".to_string(), Type::Int));
            assert_eq!(*return_type, Type::Int);
        } else {
            panic!("Expected Function");
        }
    }

    #[test]
    fn test_parse_typed_assignment() {
        let source = "x: int = 42";
        let program = parse(source).unwrap();

        if let Statement::Assignment {
            name,
            type_ann,
            value: _,
        } = &program[0]
        {
            assert_eq!(name, "x");
            assert_eq!(*type_ann, Some(Type::Int));
        } else {
            panic!("Expected Assignment");
        }
    }

    #[test]
    fn test_parse_fibonacci() {
        let source = r"
            def fib(n: int) -> int {
                if (n < 2) {
                    return n
                } else {
                    return fib(n - 1) + fib(n - 2)
                }
            }
            fib(10)
        ";
        let program = parse(source).unwrap();
        assert_eq!(program.len(), 2);
    }
}
