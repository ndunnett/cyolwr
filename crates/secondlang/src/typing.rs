use std::collections::HashMap;

use crate::{
    Anyhow, Type, anyhow,
    ast::{BinaryOp, Expression, Program, Statement, TypedExpr, UnaryOp},
};

pub fn typecheck(program: &mut Program) -> Anyhow<()> {
    TypingEngine::check(program)
}

struct TypingEngine {
    env: Vec<HashMap<String, Type>>,
}

impl TypingEngine {
    pub fn check(program: &mut Program) -> Anyhow<()> {
        let mut checker = Self {
            env: vec![HashMap::new()],
        };

        checker.function_signatures(program);
        checker.program_body(program)
    }

    fn insert(&mut self, name: &str, ty: Type) {
        let ctx = self.env.last_mut().unwrap();
        ctx.insert(name.to_string(), ty);
    }

    fn lookup(&self, name: &str) -> Option<&Type> {
        self.env.iter().rev().find_map(|ctx| ctx.get(name))
    }

    fn scoped<T, F: FnOnce(&mut Self) -> Anyhow<T>>(&mut self, func: F) -> Anyhow<T> {
        self.env.push(HashMap::new());
        let result = func(self);
        self.env.pop();
        result
    }

    fn function_signatures(&mut self, program: &Program) {
        for stmt in program {
            if let Statement::Function {
                name,
                params,
                return_type,
                ..
            } = stmt
            {
                let params = params.iter().map(|(_, t)| t.clone()).collect();
                let ret = Box::new(return_type.clone());
                self.insert(name, Type::Function { params, ret });
            }
        }
    }

    fn program_body(&mut self, program: &mut Program) -> Anyhow<()> {
        for stmt in program {
            self.statement(stmt)?;
        }

        Ok(())
    }

    fn statement(&mut self, stmt: &mut Statement) -> Anyhow<Type> {
        match stmt {
            Statement::Function {
                name: _,
                params,
                return_type,
                body,
            } => self.scoped(|self_| {
                for (param_name, param_type) in params.iter() {
                    self_.insert(param_name, param_type.clone());
                }

                let mut body_type = Type::Unit;

                for body_stmt in body.iter_mut() {
                    body_type = self_.statement(body_stmt)?;
                }

                if *return_type != Type::Unknown && body_type != Type::Unit {
                    _ = return_type.unify(&body_type)?;
                }

                Ok(Type::Unit)
            }),

            Statement::Return(expr) | Statement::Expression(expr) => {
                self.expression(expr)?;
                Ok(expr.ty.clone())
            }

            Statement::Assignment {
                name,
                type_ann,
                value,
            } => {
                self.expression(value)?;

                let var_type = if let Some(ann) = type_ann {
                    _ = ann.unify(&value.ty)?;
                    ann.clone()
                } else {
                    value.ty.clone()
                };

                self.insert(name, var_type.clone());
                Ok(var_type)
            }
        }
    }

    fn expression(&mut self, expr: &mut TypedExpr) -> Anyhow<()> {
        expr.ty = match &mut expr.expr {
            Expression::Int(_) | Expression::Bool(_) => return Ok(()),
            Expression::Var(name) => self.var(name)?,
            Expression::Unary { op, expr: inner } => self.unary(*op, inner)?,
            Expression::Binary { op, left, right } => self.binary(*op, left, right)?,
            Expression::Call { name, args } => self.call(name, args)?,
            Expression::If {
                cond,
                then_branch,
                else_branch,
            } => self.if_expr(cond, then_branch, else_branch)?,
            Expression::While { cond, body } => self.while_expr(cond, body)?,
            Expression::Block(stmts) => self.block(stmts)?,
        };

        Ok(())
    }

    fn var(&self, name: &str) -> Anyhow<Type> {
        self.lookup(name)
            .cloned()
            .ok_or_else(|| anyhow!("undefined variable: {name}"))
    }

    fn unary(&mut self, op: UnaryOp, expr: &mut TypedExpr) -> Anyhow<Type> {
        self.expression(expr)?;

        match op {
            UnaryOp::Negative => {
                if expr.ty != Type::Int {
                    return Err(anyhow!("cannot negate non-integer type: {}", expr.ty));
                }

                Ok(Type::Int)
            }
            UnaryOp::Not => {
                if expr.ty != Type::Bool {
                    return Err(anyhow!("cannot negate non-boolean type: {}", expr.ty));
                }

                Ok(Type::Bool)
            }
        }
    }

    fn binary(
        &mut self,
        op: BinaryOp,
        left: &mut TypedExpr,
        right: &mut TypedExpr,
    ) -> Anyhow<Type> {
        self.expression(left)?;
        self.expression(right)?;

        match op {
            BinaryOp::Add
            | BinaryOp::Subtract
            | BinaryOp::Multiply
            | BinaryOp::Divide
            | BinaryOp::Modulo => {
                if left.ty != Type::Int || right.ty != Type::Int {
                    return Err(anyhow!(
                        "arithmetic operation requires int operands, got {} and {}",
                        left.ty,
                        right.ty
                    ));
                }

                Ok(Type::Int)
            }
            BinaryOp::LessThan
            | BinaryOp::GreaterThan
            | BinaryOp::LessEqual
            | BinaryOp::GreaterEqual => {
                if left.ty != Type::Int || right.ty != Type::Int {
                    return Err(anyhow!(
                        "comparison requires int operands, got {} and {}",
                        left.ty,
                        right.ty
                    ));
                }

                Ok(Type::Bool)
            }
            BinaryOp::Equal | BinaryOp::NotEqual => {
                _ = left.ty.unify(&right.ty)?;
                Ok(Type::Bool)
            }
        }
    }

    fn call(&mut self, name: &str, args: &mut [TypedExpr]) -> Anyhow<Type> {
        let func_type = self
            .lookup(name)
            .ok_or_else(|| anyhow!("undefined function: {name}"))?
            .clone();

        if let Type::Function { params, ret } = func_type {
            if args.len() != params.len() {
                return Err(anyhow!(
                    "function {} expects {} arguments, got {}",
                    name,
                    params.len(),
                    args.len()
                ));
            }

            for (arg, param_type) in args.iter_mut().zip(params.iter()) {
                self.expression(arg)?;
                _ = arg.ty.unify(param_type)?;
            }

            Ok(*ret)
        } else {
            Err(anyhow!("{name} is not a function"))
        }
    }

    fn if_expr(
        &mut self,
        cond: &mut TypedExpr,
        then_branch: &mut [Statement],
        else_branch: &mut [Statement],
    ) -> Anyhow<Type> {
        self.expression(cond)?;

        if cond.ty != Type::Bool {
            return Err(anyhow!("if condition must be bool, got {}", cond.ty));
        }

        let then_type = self.scoped(|ctx| {
            let mut ty = Type::Unit;

            for stmt in then_branch.iter_mut() {
                ty = ctx.statement(stmt)?;
            }

            Ok(ty)
        })?;

        let else_type = self.scoped(|ctx| {
            let mut ty = Type::Unit;

            for stmt in else_branch.iter_mut() {
                ty = ctx.statement(stmt)?;
            }

            Ok(ty)
        })?;

        then_type.unify(&else_type)
    }

    fn while_expr(&mut self, cond: &mut TypedExpr, body: &mut [Statement]) -> Anyhow<Type> {
        self.expression(cond)?;

        if cond.ty != Type::Bool {
            return Err(anyhow!("while condition must be bool, got {}", cond.ty));
        }

        self.scoped(|ctx| {
            for stmt in body.iter_mut() {
                ctx.statement(stmt)?;
            }

            Ok(Type::Unit)
        })
    }

    fn block(&mut self, stmts: &mut [Statement]) -> Anyhow<Type> {
        self.scoped(|ctx| {
            let mut ty = Type::Unit;

            for stmt in stmts.iter_mut() {
                ty = ctx.statement(stmt)?;
            }

            Ok(ty)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;

    fn typecheck_source(source: &str) -> Anyhow<Program> {
        let mut program = parse(source)?;
        typecheck(&mut program)?;
        Ok(program)
    }

    #[test]
    fn test_typecheck_literals() {
        let program = typecheck_source("42").unwrap();
        if let Statement::Expression(expr) = &program[0] {
            assert_eq!(expr.ty, Type::Int);
        }
    }

    #[test]
    fn test_typecheck_arithmetic() {
        let program = typecheck_source("1 + 2 * 3").unwrap();
        if let Statement::Expression(expr) = &program[0] {
            assert_eq!(expr.ty, Type::Int);
        }
    }

    #[test]
    fn test_typecheck_comparison() {
        let program = typecheck_source("1 < 2").unwrap();
        if let Statement::Expression(expr) = &program[0] {
            assert_eq!(expr.ty, Type::Bool);
        }
    }

    #[test]
    fn test_typecheck_function() {
        let source = r"
            def add(a: int, b: int) -> int {
                return a + b
            }
            add(1, 2)
        ";
        let program = typecheck_source(source).unwrap();
        // Function call should have int type
        if let Statement::Expression(expr) = &program[1] {
            assert_eq!(expr.ty, Type::Int);
        }
    }

    #[test]
    fn test_typecheck_type_error() {
        let source = "1 + true";
        let result = typecheck_source(source);
        assert!(result.is_err());
    }

    #[test]
    fn test_typecheck_fibonacci() {
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
        let program = typecheck_source(source).unwrap();
        if let Statement::Expression(expr) = &program[1] {
            assert_eq!(expr.ty, Type::Int);
        }
    }
}
