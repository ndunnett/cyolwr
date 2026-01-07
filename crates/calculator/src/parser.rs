use pest::{
    self, Parser,
    iterators::{Pair, Pairs},
};

use crate::{
    Anyhow, anyhow,
    ast::{Node, Operator},
};

pub fn parse(source: &str) -> Anyhow<Vec<Node>> {
    AstBuilder::from_source(source)
}

#[derive(pest_derive::Parser)]
#[grammar = "grammar.pest"]
struct CalcParser;

struct AstBuilder;

impl AstBuilder {
    pub fn from_source(source: &str) -> Anyhow<Vec<Node>> {
        let mut ast = Vec::new();

        for pair in CalcParser::parse(Rule::Program, source)? {
            if pair.as_rule() == Rule::Expr {
                ast.push(Self::expr(pair)?);
            }
        }

        Ok(ast)
    }

    fn expr(pair: Pair<Rule>) -> Anyhow<Node> {
        match pair.as_rule() {
            Rule::Expr => Self::expr(Self::next_pair(&mut pair.into_inner())?),
            Rule::Term => Self::term(pair),
            Rule::UnaryExpr => Self::unary(pair),
            Rule::BinaryExpr => Self::binary(pair),
            _ => Err(anyhow!(
                "expected Expr, Term, UnaryExpr, or BinaryExpr: {pair:?}"
            )),
        }
    }

    fn term(pair: Pair<Rule>) -> Anyhow<Node> {
        if pair.as_rule() != Rule::Term {
            return Err(anyhow!("expected Term: {pair:?}"));
        }

        let mut pairs = pair.into_inner();
        let inner = Self::next_pair(&mut pairs)?;

        match inner.as_rule() {
            Rule::Int => Ok(Node::Int(inner.as_str().parse()?)),
            Rule::Expr => Self::expr(inner),
            _ => Err(anyhow!("expected Int or Expr: {inner:?}")),
        }
    }

    fn unary(pair: Pair<Rule>) -> Anyhow<Node> {
        let mut pairs = pair.into_inner();
        let op = Self::operator(&mut pairs)?;
        let child_pair = Self::next_pair(&mut pairs)?;
        let child = Box::new(Self::term(child_pair)?);
        Ok(Node::UnaryExpr { op, child })
    }

    fn binary(pair: Pair<Rule>) -> Anyhow<Node> {
        let mut pairs = pair.into_inner();
        let first_pair = Self::next_pair(&mut pairs)?;

        let mut expr = match first_pair.as_rule() {
            Rule::UnaryExpr => Self::unary(first_pair)?,
            _ => Self::term(first_pair)?,
        };

        while let Ok(op) = Self::operator(&mut pairs) {
            let next_pair = Self::next_pair(&mut pairs)?;
            let right = Box::new(Self::term(next_pair)?);
            let left = Box::new(expr);
            expr = Node::BinaryExpr { op, left, right };
        }

        Ok(expr)
    }

    fn operator(pairs: &mut Pairs<Rule>) -> Anyhow<Operator> {
        let Some(pair) = pairs.next() else {
            return Err(anyhow!("failed to unwrap operator"));
        };

        if pair.as_rule() != Rule::Operator {
            return Err(anyhow!("expected Operator: {pair:?}"));
        }

        match pair.as_str() {
            "+" => Ok(Operator::Plus),
            "-" => Ok(Operator::Minus),
            _ => unreachable!(),
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
        fmt: &'static str,
        ast: Node,
    }

    fn run_test_cases<const N: usize>(tests: [Case; N]) {
        for case in tests {
            match parse(case.input) {
                Ok(ast) => {
                    assert_eq!(ast[0], case.ast);
                    assert_eq!(format!("{}", ast[0]), case.fmt);
                }
                Err(e) => panic!("{e}"),
            }
        }
    }

    #[test]
    fn basics() {
        assert!(parse("b").is_err());

        run_test_cases([Case {
            input: "1",
            fmt: "1",
            ast: Node::Int(1),
        }]);
    }

    #[test]
    fn unary_expr() {
        run_test_cases([
            Case {
                input: "+1",
                fmt: "+1",
                ast: Node::UnaryExpr {
                    op: Operator::Plus,
                    child: Box::new(Node::Int(1)),
                },
            },
            Case {
                input: "-2",
                fmt: "-2",
                ast: Node::UnaryExpr {
                    op: Operator::Minus,
                    child: Box::new(Node::Int(2)),
                },
            },
        ]);
    }

    #[test]
    fn binary_expr() {
        run_test_cases([
            Case {
                input: "1 + 2",
                fmt: "1 + 2",
                ast: Node::BinaryExpr {
                    op: Operator::Plus,
                    left: Box::new(Node::Int(1)),
                    right: Box::new(Node::Int(2)),
                },
            },
            Case {
                input: "1   -  \t  2",
                fmt: "1 - 2",
                ast: Node::BinaryExpr {
                    op: Operator::Minus,
                    left: Box::new(Node::Int(1)),
                    right: Box::new(Node::Int(2)),
                },
            },
        ]);
    }

    #[test]
    fn nested_expr() {
        run_test_cases([
            Case {
                input: "(1 + 2) + 3",
                fmt: "1 + 2 + 3",
                ast: Node::BinaryExpr {
                    op: Operator::Plus,
                    left: Box::new(Node::BinaryExpr {
                        op: Operator::Plus,
                        left: Box::new(Node::Int(1)),
                        right: Box::new(Node::Int(2)),
                    }),
                    right: Box::new(Node::Int(3)),
                },
            },
            Case {
                input: "1 + (2 + 3)",
                fmt: "1 + 2 + 3",
                ast: Node::BinaryExpr {
                    op: Operator::Plus,
                    left: Box::new(Node::Int(1)),
                    right: Box::new(Node::BinaryExpr {
                        op: Operator::Plus,
                        left: Box::new(Node::Int(2)),
                        right: Box::new(Node::Int(3)),
                    }),
                },
            },
            Case {
                input: "1 + (2 + (3 + 4))",
                fmt: "1 + 2 + 3 + 4",
                ast: Node::BinaryExpr {
                    op: Operator::Plus,
                    left: Box::new(Node::Int(1)),
                    right: Box::new(Node::BinaryExpr {
                        op: Operator::Plus,
                        left: Box::new(Node::Int(2)),
                        right: Box::new(Node::BinaryExpr {
                            op: Operator::Plus,
                            left: Box::new(Node::Int(3)),
                            right: Box::new(Node::Int(4)),
                        }),
                    }),
                },
            },
            Case {
                input: "(1 + 2) + (3 - 4)",
                fmt: "1 + 2 + 3 - 4",
                ast: Node::BinaryExpr {
                    op: Operator::Plus,
                    left: Box::new(Node::BinaryExpr {
                        op: Operator::Plus,
                        left: Box::new(Node::Int(1)),
                        right: Box::new(Node::Int(2)),
                    }),
                    right: Box::new(Node::BinaryExpr {
                        op: Operator::Minus,
                        left: Box::new(Node::Int(3)),
                        right: Box::new(Node::Int(4)),
                    }),
                },
            },
        ]);
    }

    #[test]
    fn multiple_operators() {
        run_test_cases([Case {
            input: "1+2+3",
            fmt: "1 + 2 + 3",
            ast: Node::BinaryExpr {
                op: Operator::Plus,
                left: Box::new(Node::BinaryExpr {
                    op: Operator::Plus,
                    left: Box::new(Node::Int(1)),
                    right: Box::new(Node::Int(2)),
                }),
                right: Box::new(Node::Int(3)),
            },
        }]);
    }

    #[test]
    fn negative_first_number() {
        run_test_cases([
            Case {
                input: "-1 + 2",
                fmt: "-1 + 2",
                ast: Node::BinaryExpr {
                    op: Operator::Plus,
                    left: Box::new(Node::UnaryExpr {
                        op: Operator::Minus,
                        child: Box::new(Node::Int(1)),
                    }),
                    right: Box::new(Node::Int(2)),
                },
            },
            Case {
                input: "-2 + 5",
                fmt: "-2 + 5",
                ast: Node::BinaryExpr {
                    op: Operator::Plus,
                    left: Box::new(Node::UnaryExpr {
                        op: Operator::Minus,
                        child: Box::new(Node::Int(2)),
                    }),
                    right: Box::new(Node::Int(5)),
                },
            },
        ]);
    }

    #[test]
    fn whitespace_handling() {
        run_test_cases([
            Case {
                input: "1+2\n",
                fmt: "1 + 2",
                ast: Node::BinaryExpr {
                    op: Operator::Plus,
                    left: Box::new(Node::Int(1)),
                    right: Box::new(Node::Int(2)),
                },
            },
            Case {
                input: "1 + 2\r\n",
                fmt: "1 + 2",
                ast: Node::BinaryExpr {
                    op: Operator::Plus,
                    left: Box::new(Node::Int(1)),
                    right: Box::new(Node::Int(2)),
                },
            },
        ]);
    }
}
