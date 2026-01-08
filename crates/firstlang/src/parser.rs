use pest::{
    self, Parser,
    iterators::{Pair, Pairs},
};

use crate::{
    Anyhow, anyhow,
    ast::{BinaryOp, Expression, Program, Statement, UnaryOp},
};

pub fn parse(source: &str) -> Anyhow<Program> {
    AstBuilder::from_source(source)
}

#[derive(pest_derive::Parser)]
#[grammar = "grammar.pest"]
struct FirstlangParser;

struct AstBuilder;

impl AstBuilder {
    pub fn from_source(source: &str) -> Anyhow<Program> {
        let mut ast = Vec::new();

        for pair in FirstlangParser::parse(Rule::Program, source)? {
            if pair.as_rule() == Rule::Stmt {
                ast.push(Self::statement(pair)?);
            }
        }

        Ok(ast)
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
        let mut body = Vec::new();

        for pair in pairs {
            match pair.as_rule() {
                Rule::Identifier => params.push(pair.as_str().to_string()),
                Rule::Block => body = Self::block(pair)?,
                _ => {}
            }
        }

        Ok(Statement::Function { name, params, body })
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
        let value = Self::expression(Self::next_pair(&mut pairs)?)?;
        Ok(Statement::Assignment { name, value })
    }

    fn expression(pair: Pair<Rule>) -> Anyhow<Expression> {
        match pair.as_rule() {
            Rule::Expr => Self::expression(Self::next_pair(&mut pair.into_inner())?),
            Rule::Conditional => Self::conditional(pair),
            Rule::WhileLoop => Self::while_(pair),
            Rule::Comparison | Rule::Additive | Rule::Multiplicative => Self::binary(pair),
            Rule::Unary => Self::unary(pair),
            Rule::Call => Self::call(pair),
            Rule::Literal => Self::literal(&Self::next_pair(&mut pair.into_inner())?),
            Rule::Int | Rule::Bool => Self::literal(&pair),
            Rule::Identifier => Ok(Expression::Var(pair.as_str().to_string())),
            Rule::Block => Ok(Expression::Block(Self::block(pair)?)),
            _ => Err(anyhow!("expected expression: {pair:?}")),
        }
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

    fn binary(pair: Pair<Rule>) -> Anyhow<Expression> {
        let mut pairs = pair.into_inner();
        let mut expr = Self::expression(Self::next_pair(&mut pairs)?)?;

        while let Ok(op) = Self::binary_op(&mut pairs) {
            let next_pair = Self::next_pair(&mut pairs)?;
            let right = Box::new(Self::expression(next_pair)?);
            let left = Box::new(expr);
            expr = Expression::Binary { op, left, right };
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

    fn unary(pair: Pair<Rule>) -> Anyhow<Expression> {
        let mut pairs = pair.into_inner();
        let pair = Self::next_pair(&mut pairs)?;

        if pair.as_rule() == Rule::UnaryOp {
            let op = Self::unary_op(&pair)?;
            let expr = Box::new(Self::expression(Self::next_pair(&mut pairs)?)?);
            Ok(Expression::Unary { op, expr })
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

    fn call(pair: Pair<Rule>) -> Anyhow<Expression> {
        let mut pairs = pair.into_inner();
        let mut expr = Self::expression(Self::next_pair(&mut pairs)?)?;

        for pair in pairs {
            if pair.as_rule() == Rule::CallArgs {
                let args = pair
                    .into_inner()
                    .map(Self::expression)
                    .collect::<Anyhow<Vec<_>>>()?;

                if let Expression::Var(name) = expr {
                    expr = Expression::Call { name, args };
                } else {
                    return Err(anyhow!("can only call named functions"));
                }
            }
        }

        Ok(expr)
    }

    fn literal(pair: &Pair<Rule>) -> Anyhow<Expression> {
        match pair.as_rule() {
            Rule::Int => Ok(Expression::Int(pair.as_str().parse()?)),
            Rule::Bool => Ok(Expression::Bool(pair.as_str() == "true")),
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

    struct Case {
        input: &'static str,
        ast: Statement,
    }

    fn run_test_cases<const N: usize>(tests: [Case; N]) {
        for case in tests {
            match parse(case.input) {
                Ok(ast) => assert_eq!(case.ast, ast[0], "expected left, actual right"),
                Err(e) => panic!("Unexpected error:\n{e}"),
            }
        }
    }

    #[test]
    fn test_parse_literal() {
        run_test_cases([Case {
            input: "42",
            ast: Statement::Expression(Expression::Int(42)),
        }]);
    }

    #[test]
    fn test_parse_bool() {
        run_test_cases([Case {
            input: "true",
            ast: Statement::Expression(Expression::Bool(true)),
        }]);
    }

    #[test]
    fn test_parse_binary() {
        run_test_cases([Case {
            input: "1 + 2",
            ast: Statement::Expression(Expression::Binary {
                op: BinaryOp::Add,
                left: Box::new(Expression::Int(1)),
                right: Box::new(Expression::Int(2)),
            }),
        }]);
    }

    #[test]
    fn test_parse_assignment() {
        run_test_cases([Case {
            input: "x = 42",
            ast: Statement::Assignment {
                name: "x".to_string(),
                value: Expression::Int(42),
            },
        }]);
    }

    #[test]
    fn test_parse_function() {
        run_test_cases([Case {
            input: "def add(a, b) { return a + b }",
            ast: Statement::Function {
                name: "add".to_string(),
                params: vec!["a".to_string(), "b".to_string()],
                body: vec![Statement::Return(Expression::Binary {
                    op: BinaryOp::Add,
                    left: Box::new(Expression::Var("a".to_string())),
                    right: Box::new(Expression::Var("b".to_string())),
                })],
            },
        }]);
    }

    #[test]
    fn test_parse_call() {
        run_test_cases([Case {
            input: "add(1, 2)",
            ast: Statement::Expression(Expression::Call {
                name: "add".to_string(),
                args: vec![Expression::Int(1), Expression::Int(2)],
            }),
        }]);
    }

    #[test]
    fn test_parse_conditional() {
        run_test_cases([Case {
            input: "if (x < 10) { 1 } else { 2 }",
            ast: Statement::Expression(Expression::If {
                cond: Box::new(Expression::Binary {
                    op: BinaryOp::LessThan,
                    left: Box::new(Expression::Var("x".to_string())),
                    right: Box::new(Expression::Int(10)),
                }),
                then_branch: vec![Statement::Expression(Expression::Int(1))],
                else_branch: vec![Statement::Expression(Expression::Int(2))],
            }),
        }]);
    }

    #[test]
    fn test_parse_while() {
        run_test_cases([Case {
            input: "while (x < 10) { x = x + 1 }",
            ast: Statement::Expression(Expression::While {
                cond: Box::new(Expression::Binary {
                    op: BinaryOp::LessThan,
                    left: Box::new(Expression::Var("x".to_string())),
                    right: Box::new(Expression::Int(10)),
                }),
                body: vec![Statement::Assignment {
                    name: "x".to_string(),
                    value: Expression::Binary {
                        op: BinaryOp::Add,
                        left: Box::new(Expression::Var("x".to_string())),
                        right: Box::new(Expression::Int(1)),
                    },
                }],
            }),
        }]);
    }

    #[test]
    fn test_parse_fibonacci() {
        let source = r"
            def fib(n) {
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
