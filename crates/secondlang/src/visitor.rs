use crate::ast::{BinaryOp, Expression, Program, Statement, TypedExpr, UnaryOp};

pub trait Visitor: Default {
    fn visit_program(program: Program) -> Program {
        let mut visitor = Self::default();
        visitor.visit_stmts(program)
    }

    fn visit_stmts(&mut self, stmts: Vec<Statement>) -> Vec<Statement> {
        let mut visited = Vec::with_capacity(stmts.len());

        for stmt in stmts {
            visited.push(self.visit_stmt(stmt));
        }

        visited
    }

    fn visit_exprs(&mut self, exprs: Vec<TypedExpr>) -> Vec<TypedExpr> {
        let mut visited = Vec::with_capacity(exprs.len());

        for expr in exprs {
            visited.push(self.visit_expr(expr));
        }

        visited
    }

    fn visit_stmt(&mut self, stmt: Statement) -> Statement {
        match stmt {
            Statement::Function {
                name,
                params,
                return_type,
                body,
            } => {
                let visited_body = self.visit_stmts(body);

                Statement::Function {
                    name,
                    params,
                    return_type,
                    body: visited_body,
                }
            }
            Statement::Return(expr) => Statement::Return(self.visit_expr(expr)),
            Statement::Assignment {
                name,
                type_ann,
                value,
            } => Statement::Assignment {
                name,
                type_ann,
                value: self.visit_expr(value),
            },
            Statement::Expression(expr) => Statement::Expression(self.visit_expr(expr)),
        }
    }

    fn visit_expr(&mut self, expr: TypedExpr) -> TypedExpr {
        match expr.expr {
            Expression::Int(n) => self.visit_int(n),
            Expression::Bool(b) => self.visit_bool(b),
            Expression::Var(name) => self.visit_var(name),
            Expression::Unary { op, expr: inner } => self.visit_unary(op, *inner),
            Expression::Binary { op, left, right } => self.visit_binary(op, *left, *right),
            Expression::Call { name, args } => self.visit_call(name, args),
            Expression::If {
                cond,
                then_branch,
                else_branch,
            } => self.visit_if(*cond, then_branch, else_branch),
            Expression::While { cond, body } => self.visit_while(*cond, body),
            Expression::Block(stmts) => self.visit_block(stmts),
        }
        .typed(expr.ty)
    }

    fn visit_int(&mut self, n: i64) -> Expression {
        Expression::Int(n)
    }

    fn visit_bool(&mut self, b: bool) -> Expression {
        Expression::Bool(b)
    }

    fn visit_var(&mut self, name: String) -> Expression {
        Expression::Var(name)
    }

    fn visit_unary(&mut self, op: UnaryOp, expr: TypedExpr) -> Expression {
        Expression::Unary {
            op,
            expr: Box::new(self.visit_expr(expr)),
        }
    }

    fn visit_binary(&mut self, op: BinaryOp, left: TypedExpr, right: TypedExpr) -> Expression {
        Expression::Binary {
            op,
            left: Box::new(self.visit_expr(left)),
            right: Box::new(self.visit_expr(right)),
        }
    }

    fn visit_call(&mut self, name: String, args: Vec<TypedExpr>) -> Expression {
        Expression::Call {
            name,
            args: self.visit_exprs(args),
        }
    }

    fn visit_if(
        &mut self,
        cond: TypedExpr,
        then_branch: Vec<Statement>,
        else_branch: Vec<Statement>,
    ) -> Expression {
        Expression::If {
            cond: Box::new(self.visit_expr(cond)),
            then_branch: self.visit_stmts(then_branch),
            else_branch: self.visit_stmts(else_branch),
        }
    }

    fn visit_while(&mut self, cond: TypedExpr, body: Vec<Statement>) -> Expression {
        Expression::While {
            cond: Box::new(self.visit_expr(cond)),
            body: self.visit_stmts(body),
        }
    }

    fn visit_block(&mut self, stmts: Vec<Statement>) -> Expression {
        Expression::Block(self.visit_stmts(stmts))
    }
}
