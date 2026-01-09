use crate::{
    Visitor,
    ast::{BinaryOp, Expression, Program, TypedExpr, UnaryOp},
};

pub fn optimize(program: Program) -> Program {
    let program = ConstantFolder::visit_program(program);
    AlgebraicSimplifier::visit_program(program)
}

pub struct ConstantFolder;

impl Default for ConstantFolder {
    fn default() -> Self {
        Self
    }
}

impl Visitor for ConstantFolder {
    fn visit_binary(&mut self, op: BinaryOp, left: TypedExpr, right: TypedExpr) -> Expression {
        let left = self.visit_expr(left);
        let right = self.visit_expr(right);

        if let (Expression::Int(left), Expression::Int(right)) = (&left.expr, &right.expr) {
            if let Some(val) = match op {
                BinaryOp::Add => Some(left + right),
                BinaryOp::Subtract => Some(left - right),
                BinaryOp::Multiply => Some(left * right),
                BinaryOp::Divide if *right != 0 => Some(left / right),
                BinaryOp::Modulo if *right != 0 => Some(left % right),
                _ => None,
            } {
                return Expression::Int(val);
            }

            if let Some(val) = match op {
                BinaryOp::LessThan => Some(*left < *right),
                BinaryOp::GreaterThan => Some(*left > *right),
                BinaryOp::LessEqual => Some(*left <= *right),
                BinaryOp::GreaterEqual => Some(*left >= *right),
                BinaryOp::Equal => Some(*left == *right),
                BinaryOp::NotEqual => Some(*left != *right),
                _ => None,
            } {
                return Expression::Bool(val);
            }
        }

        Expression::Binary {
            op,
            left: Box::new(left),
            right: Box::new(right),
        }
    }

    fn visit_unary(&mut self, op: UnaryOp, expr: TypedExpr) -> Expression {
        let e = self.visit_expr(expr);

        match (&op, &e.expr) {
            (UnaryOp::Negative, Expression::Int(n)) => Expression::Int(-n),
            (UnaryOp::Not, Expression::Bool(b)) => Expression::Bool(!b),
            _ => Expression::Unary {
                op,
                expr: Box::new(e),
            },
        }
    }
}

pub struct AlgebraicSimplifier;

impl Default for AlgebraicSimplifier {
    fn default() -> Self {
        Self
    }
}

impl Visitor for AlgebraicSimplifier {
    fn visit_binary(&mut self, op: BinaryOp, left: TypedExpr, right: TypedExpr) -> Expression {
        let left = self.visit_expr(left);
        let right = self.visit_expr(right);

        match (&op, &left.expr, &right.expr) {
            (BinaryOp::Multiply, _, Expression::Int(0))
            | (BinaryOp::Multiply, Expression::Int(0), _) => {
                return Expression::Int(0);
            }
            (BinaryOp::Add | BinaryOp::Subtract, _, Expression::Int(0))
            | (BinaryOp::Divide | BinaryOp::Multiply, _, Expression::Int(1)) => {
                return left.expr;
            }
            (BinaryOp::Add, Expression::Int(0), _)
            | (BinaryOp::Multiply, Expression::Int(1), _) => {
                return right.expr;
            }
            _ => {}
        }

        Expression::Binary {
            op,
            left: Box::new(left),
            right: Box::new(right),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Anyhow, ast::Statement, parse, typecheck};

    fn parse_and_check(source: &str) -> Program {
        let result = (|| -> Anyhow<_> {
            let mut program = parse(source)?;
            typecheck(&mut program)?;
            Ok(program)
        })();

        match result {
            Ok(program) => program,
            Err(e) => panic!("Unexpected Error:\n{e}"),
        }
    }

    fn fold(source: &str) -> Program {
        ConstantFolder::visit_program(parse_and_check(source))
    }

    fn simplify(source: &str) -> Program {
        AlgebraicSimplifier::visit_program(parse_and_check(source))
    }

    fn fold_and_simplify(source: &str) -> Program {
        AlgebraicSimplifier::visit_program(fold(source))
    }

    #[test]
    fn test_constant_folding_arithmetic() {
        let folded = fold("def test() -> int { return 1 + 2 * 3 }");

        // After folding: 1 + 2 * 3 should become 7
        if let Statement::Function { body, .. } = &folded[0]
            && let Statement::Return(expr) = &body[0]
        {
            assert_eq!(expr.expr, Expression::Int(7));
        }
    }

    #[test]
    fn test_constant_folding_comparison() {
        let folded = fold("def test() -> bool { return 5 < 10 }");

        if let Statement::Function { body, .. } = &folded[0]
            && let Statement::Return(expr) = &body[0]
        {
            assert_eq!(expr.expr, Expression::Bool(true));
        }
    }

    #[test]
    fn test_algebraic_simplification_add_zero() {
        let simplified = simplify("def test(x: int) -> int { return x + 0 }");

        // x + 0 should become x
        if let Statement::Function { body, .. } = &simplified[0]
            && let Statement::Return(expr) = &body[0]
        {
            assert_eq!(expr.expr, Expression::Var("x".to_string()));
        }
    }

    #[test]
    fn test_algebraic_simplification_mul_zero() {
        let simplified = simplify("def test(x: int) -> int { return x * 0 }");

        // x * 0 should become 0
        if let Statement::Function { body, .. } = &simplified[0]
            && let Statement::Return(expr) = &body[0]
        {
            assert_eq!(expr.expr, Expression::Int(0));
        }
    }

    #[test]
    fn test_algebraic_simplification_mul_one() {
        let simplified = simplify("def test(x: int) -> int { return x * 1 }");

        // x * 1 should become x
        if let Statement::Function { body, .. } = &simplified[0]
            && let Statement::Return(expr) = &body[0]
        {
            assert_eq!(expr.expr, Expression::Var("x".to_string()));
        }
    }

    #[test]
    fn test_combined_optimizations() {
        let opt = fold_and_simplify("def test(x: int) -> int { return x * (1 + 0) }");

        if let Statement::Function { body, .. } = &opt[0]
            && let Statement::Return(expr) = &body[0]
        {
            assert_eq!(expr.expr, Expression::Var("x".to_string()));
        }
    }
}
