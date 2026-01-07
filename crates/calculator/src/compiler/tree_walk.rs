use crate::{
    Anyhow, Compile,
    ast::{Node, Operator},
};

fn eval(node: Node) -> i32 {
    match node {
        Node::Int(n) => n,
        Node::UnaryExpr { op, child } => {
            let child = eval(*child);

            match op {
                Operator::Plus => child,
                Operator::Minus => -child,
            }
        }
        Node::BinaryExpr { op, left, right } => {
            let left_eval = eval(*left);
            let right_eval = eval(*right);

            match op {
                Operator::Plus => left_eval + right_eval,
                Operator::Minus => left_eval - right_eval,
            }
        }
    }
}

pub struct Interpreter;

impl Compile for Interpreter {
    type Output = i32;

    fn from_ast(ast: Vec<Node>) -> Anyhow<Self::Output> {
        Ok(ast.into_iter().map(eval).sum::<Self::Output>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Case {
        input: &'static str,
        expected: i32,
    }

    fn run_test_cases<const N: usize>(tests: [Case; N]) {
        for case in tests {
            match Interpreter::from_source(case.input) {
                Ok(result) => assert_eq!(result, case.expected),
                Err(e) => panic!("{e}"),
            }
        }
    }

    #[test]
    fn basics() {
        run_test_cases([
            Case {
                input: "1",
                expected: 1,
            },
            Case {
                input: "1 + 2",
                expected: 3,
            },
            Case {
                input: "2 + (2 - 1)",
                expected: 3,
            },
            Case {
                input: "(2 + 3) - 1",
                expected: 4,
            },
            Case {
                input: "1 + ((2 + 3) - (2 + 3))",
                expected: 1,
            },
        ]);
    }
}
