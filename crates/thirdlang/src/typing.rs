use std::collections::HashMap;

use crate::{
    Anyhow, anyhow,
    ast::{
        AssignTarget, BinaryOp, ClassDef, Expression, FieldDef, MethodDef, Program, Statement,
        TopLevel, TypedExpr, UnaryOp,
    },
    types::{ClassInfo, ClassRegistry, MethodInfo, Type},
};

pub fn typecheck(program: &mut Program) -> Anyhow<ClassRegistry> {
    TypingEngine::check(program)
}

struct TypingEngine {
    env: Vec<HashMap<String, Type>>,
    classes: ClassRegistry,
    current_class: Option<String>,
    current_return_type: Option<Type>,
}

impl TypingEngine {
    pub fn check(program: &mut Program) -> Anyhow<ClassRegistry> {
        let mut checker = Self {
            env: vec![HashMap::new()],
            classes: ClassRegistry::new(),
            current_class: None,
            current_return_type: None,
        };

        checker.register_classes(program)?;
        checker.register_functions(program);
        checker.check_classes(program)?;
        checker.check_program(program)?;
        Ok(checker.classes)
    }

    fn register_type(&mut self, name: &str, ty: Type) {
        let ctx = self.env.last_mut().unwrap();
        ctx.insert(name.to_string(), ty);
    }

    fn lookup_type(&self, name: &str) -> Option<&Type> {
        self.env.iter().rev().find_map(|ctx| ctx.get(name))
    }

    fn scoped<T, F: FnOnce(&mut Self) -> Anyhow<T>>(&mut self, func: F) -> Anyhow<T> {
        self.env.push(HashMap::new());
        let result = func(self);
        self.env.pop();
        result
    }

    fn validate_type(&self, ty: &Type) -> Anyhow<()> {
        if let Type::Class(name) = ty
            && self.lookup_type(name).is_none()
        {
            Err(anyhow!("unknown class: {name}"))
        } else {
            Ok(())
        }
    }

    fn register_classes(&mut self, program: &Program) -> Anyhow<()> {
        for item in program {
            if let TopLevel::Class(class) = item {
                let mut class_info = ClassInfo::new(class.name.clone());

                for field in &class.fields {
                    self.validate_type(&field.ty)?;
                    class_info.add_field(field.name.clone(), field.ty.clone());
                }

                for method in &class.methods {
                    class_info.add_method(MethodInfo {
                        name: method.name.clone(),
                        params: method.params.clone(),
                        return_type: method.return_type.clone(),
                        is_constructor: method.is_constructor(),
                        is_destructor: method.is_destructor(),
                    });
                }

                self.classes.insert(class.name.clone(), class_info);
            }
        }

        Ok(())
    }

    fn register_functions(&mut self, program: &Program) {
        for stmt in program {
            if let TopLevel::Stmt(Statement::Function {
                name,
                params,
                return_type,
                ..
            }) = stmt
            {
                let params = params.iter().map(|(_, t)| t.clone()).collect();
                let ret = Box::new(return_type.clone());
                self.register_type(name, Type::Function { params, ret });
            }
        }
    }

    fn check_classes(&mut self, program: &mut Program) -> Anyhow<()> {
        for item in program {
            if let TopLevel::Class(class) = item {
                self.class(class)?;
            }
        }

        Ok(())
    }

    fn check_program(&mut self, program: &mut Program) -> Anyhow<()> {
        for item in program {
            if let TopLevel::Stmt(stmt) = item {
                self.statement(stmt)?;
            }
        }

        Ok(())
    }

    fn class(&mut self, class: &mut ClassDef) -> Anyhow<()> {
        self.current_class = Some(class.name.clone());

        for method in &mut class.methods {
            self.method(&class.name, method)?;
        }

        self.current_class = None;
        Ok(())
    }

    fn method(&mut self, class_name: &str, method: &mut MethodDef) -> Anyhow<()> {
        self.scoped(|ctx| {
            ctx.register_type("self", Type::Class(class_name.to_string()));

            for (param_name, param_type) in &method.params {
                ctx.validate_type(param_type)?;
                ctx.register_type(param_name, param_type.clone());
            }

            ctx.current_return_type = Some(method.return_type.clone());

            for stmt in &mut method.body {
                ctx.statement(stmt)?;
            }

            ctx.current_return_type = None;
            Ok(())
        })
    }

    fn statement(&mut self, stmt: &mut Statement) -> Anyhow<Type> {
        match stmt {
            Statement::Function {
                params,
                return_type,
                body,
                ..
            } => self.function(params, return_type, body),
            Statement::Return(expr) | Statement::Expression(expr) => {
                self.expression(expr)?;
                Ok(expr.ty.clone())
            }
            Statement::Assignment {
                target,
                type_ann,
                value,
            } => self.assignment(target, type_ann.as_ref(), value),
            Statement::Delete(expr) => {
                self.expression(expr)?;

                if !expr.ty.is_class() {
                    return Err(anyhow!("can't delete non-class type: {}", expr.ty));
                }

                Ok(Type::Unit)
            }
        }
    }

    fn function(
        &mut self,
        params: &[(String, Type)],
        return_type: &Type,
        body: &mut [Statement],
    ) -> Anyhow<Type> {
        self.scoped(|ctx| {
            for (name, ty) in params {
                ctx.register_type(name, ty.clone());
            }

            let mut ty = Type::Unit;

            for stmt in body {
                ty = ctx.statement(stmt)?;
            }

            if *return_type != Type::Unknown && ty != Type::Unit {
                _ = return_type.unify(&ty)?;
            }

            Ok(Type::Unit)
        })
    }

    fn assignment(
        &mut self,
        target: &mut AssignTarget,
        type_ann: Option<&Type>,
        value: &mut TypedExpr,
    ) -> Anyhow<Type> {
        self.expression(value)?;

        match target {
            AssignTarget::Var(name) => {
                if let Some(annotation) = type_ann {
                    self.validate_type(annotation)?;
                    _ = annotation.unify(&value.ty)?;
                    self.register_type(name, annotation.clone());
                } else if let Some(existing) = self.lookup_type(name) {
                    _ = existing.unify(&value.ty)?;
                } else {
                    self.register_type(name, value.ty.clone());
                }
            }
            AssignTarget::Field { object, field } => {
                self.expression(object)?;

                let Some(name) = object.ty.class_name() else {
                    return Err(anyhow!(
                        "can't access field on non-class type: {}",
                        object.ty
                    ));
                };

                let Some(info) = self.classes.get(name) else {
                    return Err(anyhow!("unknown class: {name}"));
                };

                let Some(ty) = info.get_field(field) else {
                    return Err(anyhow!("unknown field {field} on class {name}"));
                };

                let _ = ty.unify(&value.ty)?;
            }
        }

        Ok(value.ty.clone())
    }

    fn expression(&mut self, expr: &mut TypedExpr) -> Anyhow<()> {
        expr.ty = match &mut expr.expr {
            Expression::Int(_) | Expression::Bool(_) => return Ok(()),
            Expression::Var(name) => self.var(name)?,
            Expression::SelfRef => self.self_ref()?,
            Expression::Unary { op, expr: inner } => self.unary(*op, inner)?,
            Expression::Binary { op, left, right } => self.binary(*op, left, right)?,
            Expression::Call { name, args } => self.call(name, args)?,
            Expression::MethodCall {
                object,
                method,
                args,
            } => self.method_call(object, method, args)?,
            Expression::FieldAccess { object, field } => self.field_access(object, field)?,
            Expression::New { class, args } => self.new_expr(class, args)?,
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
        let Some(ty) = self.lookup_type(name).cloned() else {
            return Err(anyhow!("undefined variable: {name}"));
        };

        Ok(ty)
    }

    fn self_ref(&self) -> Anyhow<Type> {
        let Some(name) = &self.current_class else {
            return Err(anyhow!("'self' can only be used inside a method"));
        };

        Ok(Type::Class(name.clone()))
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
        let Some(func_type) = self.lookup_type(name).cloned() else {
            return Err(anyhow!("undefined function: {name}"));
        };

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

    fn method_call(
        &mut self,
        object: &mut TypedExpr,
        method: &str,
        args: &mut [TypedExpr],
    ) -> Anyhow<Type> {
        self.expression(object)?;

        let Some(class_name) = object.ty.class_name() else {
            return Err(anyhow!(
                "can't call method on non-class type: {}",
                object.ty
            ));
        };

        let Some(class_info) = self.classes.get(class_name) else {
            return Err(anyhow!("unknown class: {class_name}"));
        };

        let Some(info) = class_info.get_method(method) else {
            return Err(anyhow!("unknown method {method} on class {class_name}"));
        };

        if args.len() != info.params.len() {
            return Err(anyhow!(
                "Method {}.{} expects {} arguments, got {}",
                class_name,
                method,
                info.params.len(),
                args.len()
            ));
        }

        let return_type = info.return_type.clone();
        let mut params = info.params.clone().into_iter().map(|p| p.1);

        for (arg, ty) in args.iter_mut().zip(params) {
            self.expression(arg)?;
            _ = arg.ty.unify(&ty)?;
        }

        Ok(return_type)
    }

    fn field_access(&mut self, object: &mut TypedExpr, field: &str) -> Anyhow<Type> {
        self.expression(object)?;

        let Some(class_name) = object.ty.class_name() else {
            return Err(anyhow!(
                "can't access field on non-class type: {}",
                object.ty
            ));
        };

        let Some(class_info) = self.classes.get(class_name) else {
            return Err(anyhow!("unknown class: {class_name}"));
        };

        let Some(ty) = class_info.get_field(field) else {
            return Err(anyhow!("unknown field {field} on class {class_name}"));
        };

        Ok(ty.clone())
    }

    fn new_expr(&mut self, class: &str, args: &mut [TypedExpr]) -> Anyhow<Type> {
        let Some(class_info) = self.classes.get(class) else {
            return Err(anyhow!("unknown class: {class}"));
        };

        if let Some(ctor) = class_info.get_method("__init__") {
            if args.len() != ctor.params.len() {
                return Err(anyhow!(
                    "constructor for {} expects {} arguments, got {}",
                    class,
                    ctor.params.len(),
                    args.len()
                ));
            }

            let mut params = ctor.params.clone().into_iter().map(|p| p.1);

            for (arg, ty) in args.iter_mut().zip(params) {
                self.expression(arg)?;
                _ = arg.ty.unify(&ty)?;
            }
        } else if !args.is_empty() {
            return Err(anyhow!(
                "class {} has no constructor but {} arguments provided",
                class,
                args.len()
            ));
        }

        Ok(Type::Class(class.to_string()))
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

    fn typecheck_source(source: &str) -> Anyhow<ClassRegistry> {
        let mut program = parse(source)?;
        typecheck(&mut program)
    }

    #[test]
    fn test_typecheck_class() {
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

        let classes = typecheck_source(source).unwrap();
        assert!(classes.contains_key("Point"));
    }

    #[test]
    fn test_typecheck_new_expr() {
        let source = r"
            class Point {
                x: int
                def __init__(self, x: int) {
                    self.x = x
                }
            }
            p = new Point(42)
        ";

        typecheck_source(source).unwrap();
    }

    #[test]
    fn test_typecheck_method_call() {
        let source = r"
            class Counter {
                value: int
                def __init__(self, start: int) {
                    self.value = start
                }
                def get(self) -> int {
                    return self.value
                }
            }
            c = new Counter(10)
            c.get()
        ";

        typecheck_source(source).unwrap();
    }

    #[test]
    fn test_typecheck_wrong_field_type() {
        let source = r"
            class Point {
                x: int
                def __init__(self, x: int) {
                    self.x = true
                }
            }
        ";

        assert!(typecheck_source(source).is_err());
    }

    #[test]
    fn test_typecheck_delete() {
        let source = r"
            class Point { x: int }
            p = new Point()
            delete p
        ";

        typecheck_source(source).unwrap();
    }

    #[test]
    fn test_typecheck_delete_non_class() {
        let source = r"
            x: int = 42
            delete x
        ";

        assert!(typecheck_source(source).is_err());
    }
}
