use std::collections::HashMap;

use inkwell::{
    IntPredicate, OptimizationLevel,
    basic_block::BasicBlock,
    builder::Builder,
    context::Context,
    module::Module,
    types::BasicMetadataTypeEnum,
    values::{BasicMetadataValueEnum, FunctionValue, IntValue, PointerValue},
};

use crate::{
    Anyhow, Type, anyhow,
    ast::{BinaryOp, Expression, Program, Statement, TypedExpr, UnaryOp},
};

pub fn create_context() -> Context {
    Context::create()
}

pub fn compile<'ctx>(
    ctx: &'ctx Context,
    program: &Program,
    module_name: &str,
) -> Anyhow<IrModule<'ctx>> {
    CodeGen::new(ctx, module_name).compile(program)
}

pub struct IrModule<'ctx> {
    module: Module<'ctx>,
}

impl IrModule<'_> {
    pub fn execute(&self) -> Anyhow<i64> {
        let engine = self
            .module
            .create_jit_execution_engine(OptimizationLevel::Default)
            .map_err(|e| anyhow!("{e}"))?;

        unsafe {
            let func = engine.get_function::<unsafe extern "C" fn() -> i64>("__main")?;
            Ok(func.call())
        }
    }
}

impl std::fmt::Display for IrModule<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.module.to_string())
    }
}

pub struct CodeGen<'ctx> {
    ctx: &'ctx Context,
    module: Module<'ctx>,
    builder: Builder<'ctx>,
    variables: HashMap<String, PointerValue<'ctx>>,
    functions: HashMap<String, FunctionValue<'ctx>>,
    current_fn: Option<FunctionValue<'ctx>>,
}

impl<'ctx> CodeGen<'ctx> {
    pub fn new(ctx: &'ctx Context, module_name: &str) -> Self {
        let module = ctx.create_module(module_name);
        let builder = ctx.create_builder();

        Self {
            ctx,
            module,
            builder,
            variables: HashMap::new(),
            functions: HashMap::new(),
            current_fn: None,
        }
    }

    pub fn compile(mut self, program: &Program) -> Anyhow<IrModule<'ctx>> {
        // First pass: declare all functions
        for stmt in program {
            if let Statement::Function {
                name,
                params,
                return_type,
                ..
            } = stmt
            {
                self.declare_function(name, params, return_type)?;
            }
        }

        // Second pass: compile function bodies only (not top-level expressions)
        for stmt in program {
            if let Statement::Function { .. } = stmt {
                self.compile_stmt(stmt)?;
            }
        }

        // Third pass: create __main wrapper for top-level expression
        if let Some(Statement::Expression(expr)) = program.last() {
            self.compile_main_wrapper(expr)?;
        }

        self.module.verify().map_err(|e| anyhow!("{e}"))?;

        Ok(IrModule {
            module: self.module,
        })
    }

    fn llvm_type(&self, ty: &Type) -> Anyhow<inkwell::types::IntType<'ctx>> {
        match ty {
            Type::Bool => Ok(self.ctx.bool_type()),
            Type::Int | Type::Unit | Type::Unknown => Ok(self.ctx.i64_type()),
            Type::Function { .. } => Err(anyhow!("cannot get LLVM type for function type")),
        }
    }

    fn build_comparison(
        &self,
        name: &str,
        predicate: IntPredicate,
        left: IntValue<'ctx>,
        right: IntValue<'ctx>,
    ) -> Anyhow<IntValue<'ctx>> {
        let cmp = self
            .builder
            .build_int_compare(predicate, left, right, name)?;
        Ok(self
            .builder
            .build_int_z_extend(cmp, self.ctx.i64_type(), "ext")?)
    }

    fn build_entry_alloca(
        &self,
        function: FunctionValue<'ctx>,
        name: &str,
        ty: &Type,
    ) -> Anyhow<PointerValue<'ctx>> {
        let builder = self.ctx.create_builder();
        let entry = function.get_first_basic_block().unwrap();

        match entry.get_first_instruction() {
            Some(inst) => builder.position_before(&inst),
            None => builder.position_at_end(entry),
        }

        let llvm_type = self.llvm_type(ty)?;
        Ok(builder.build_alloca(llvm_type, name)?)
    }

    fn compile_main_wrapper(&mut self, expr: &TypedExpr) -> Anyhow<()> {
        // Create __main function: fn() -> i64
        let ret_type = self.ctx.i64_type();
        let fn_type = ret_type.fn_type(&[], false);
        let function = self.module.add_function("__main", fn_type, None);

        // Create entry block
        let entry = self.ctx.append_basic_block(function, "entry");
        self.builder.position_at_end(entry);
        self.current_fn = Some(function);

        // Compile the expression and return its value
        let value = self.compile_expr(expr)?;
        self.builder.build_return(Some(&value))?;

        Ok(())
    }

    fn declare_function(
        &mut self,
        name: &str,
        params: &[(String, Type)],
        return_type: &Type,
    ) -> Anyhow<FunctionValue<'ctx>> {
        let ret_type = self.llvm_type(return_type)?;
        let param_types = params
            .iter()
            .map(|(_, t)| self.llvm_type(t).map(BasicMetadataTypeEnum::from))
            .collect::<Anyhow<Vec<_>>>()?;

        let fn_type = ret_type.fn_type(&param_types, false);
        let function = self.module.add_function(name, fn_type, None);

        for (i, (param_name, _)) in params.iter().enumerate() {
            function
                .get_nth_param(i as u32)
                .unwrap()
                .set_name(param_name);
        }

        self.functions.insert(name.to_string(), function);
        Ok(function)
    }

    fn compile_stmt(&mut self, stmt: &Statement) -> Anyhow<Option<IntValue<'ctx>>> {
        match stmt {
            Statement::Function {
                name, params, body, ..
            } => self.compile_function(name, params, body),
            Statement::Return(expr) => {
                let value = self.compile_expr(expr)?;
                self.builder.build_return(Some(&value))?;
                Ok(Some(value))
            }
            Statement::Assignment { name, value, .. } => self.compile_assignment(name, value),
            Statement::Expression(expr) => {
                let val = self.compile_expr(expr)?;
                Ok(Some(val))
            }
        }
    }

    fn compile_function(
        &mut self,
        name: &str,
        params: &[(String, Type)],
        body: &[Statement],
    ) -> Anyhow<Option<IntValue<'ctx>>> {
        let function = self
            .functions
            .get(name)
            .copied()
            .ok_or_else(|| anyhow!("function {name} not declared"))?;

        let entry = self.ctx.append_basic_block(function, "entry");
        self.builder.position_at_end(entry);
        self.current_fn = Some(function);
        self.variables.clear();

        for (i, (param_name, param_type)) in params.iter().enumerate() {
            let param_value = function.get_nth_param(i as u32).unwrap().into_int_value();
            let alloca = self.build_entry_alloca(function, param_name, param_type)?;
            self.builder.build_store(alloca, param_value)?;
            self.variables.insert(param_name.clone(), alloca);
        }

        let mut last_value = None;

        for body_stmt in body {
            last_value = self.compile_stmt(body_stmt)?;
        }

        if self
            .builder
            .get_insert_block()
            .unwrap()
            .get_terminator()
            .is_none()
        {
            if let Some(val) = last_value {
                self.builder.build_return(Some(&val))?;
            } else {
                let zero = self.ctx.i64_type().const_int(0, false);
                self.builder.build_return(Some(&zero))?;
            }
        }

        Ok(None)
    }

    fn compile_assignment(
        &mut self,
        name: &str,
        value: &TypedExpr,
    ) -> Anyhow<Option<IntValue<'ctx>>> {
        let val = self.compile_expr(value)?;

        if let Some(ptr) = self.variables.get(name) {
            self.builder.build_store(*ptr, val)?;
        } else {
            let function = self.current_fn.unwrap();
            let alloca = self.build_entry_alloca(function, name, &value.ty)?;
            self.builder.build_store(alloca, val)?;
            self.variables.insert(name.to_string(), alloca);
        }

        Ok(Some(val))
    }

    fn compile_expr(&mut self, expr: &TypedExpr) -> Anyhow<IntValue<'ctx>> {
        match &expr.expr {
            Expression::Int(n) => Ok(self.ctx.i64_type().const_int(*n as u64, false)),
            Expression::Bool(b) => Ok(self.ctx.bool_type().const_int(*b as u64, false)),
            Expression::Var(name) => self.compile_var_expr(name),
            Expression::Unary { op, expr: inner } => self.compile_unary_expr(*op, inner),
            Expression::Binary { op, left, right } => self.compile_binary_expr(*op, left, right),
            Expression::Call { name, args } => self.compile_call_expr(name, args),
            Expression::If {
                cond,
                then_branch,
                else_branch,
            } => self.compile_if_expr(cond, then_branch, else_branch),
            Expression::While { cond, body } => self.compile_while_expr(cond, body),
            Expression::Block(stmts) => self.compile_block_expr(stmts),
        }
    }

    fn compile_var_expr(&self, name: &str) -> Anyhow<IntValue<'ctx>> {
        let ptr = self
            .variables
            .get(name)
            .ok_or_else(|| anyhow!("undefined variable: {name}"))?;

        let val = self.builder.build_load(self.ctx.i64_type(), *ptr, name)?;

        Ok(val.into_int_value())
    }

    fn compile_unary_expr(&mut self, op: UnaryOp, inner: &TypedExpr) -> Anyhow<IntValue<'ctx>> {
        let val = self.compile_expr(inner)?;

        match op {
            UnaryOp::Negative => Ok(self.builder.build_int_neg(val, "neg")?),
            UnaryOp::Not => Ok(self.builder.build_not(val, "not")?),
        }
    }

    fn compile_binary_expr(
        &mut self,
        op: BinaryOp,
        left: &TypedExpr,
        right: &TypedExpr,
    ) -> Anyhow<IntValue<'ctx>> {
        let l = self.compile_expr(left)?;
        let r = self.compile_expr(right)?;

        match op {
            BinaryOp::Add => Ok(self.builder.build_int_add(l, r, "add")?),
            BinaryOp::Subtract => Ok(self.builder.build_int_sub(l, r, "sub")?),
            BinaryOp::Multiply => Ok(self.builder.build_int_mul(l, r, "mul")?),
            BinaryOp::Divide => Ok(self.builder.build_int_signed_div(l, r, "div")?),
            BinaryOp::Modulo => Ok(self.builder.build_int_signed_rem(l, r, "mod")?),
            BinaryOp::LessThan => self.build_comparison("lt", IntPredicate::SLT, l, r),
            BinaryOp::GreaterThan => self.build_comparison("gt", IntPredicate::SGT, l, r),
            BinaryOp::LessEqual => self.build_comparison("le", IntPredicate::SLE, l, r),
            BinaryOp::GreaterEqual => self.build_comparison("ge", IntPredicate::SGE, l, r),
            BinaryOp::Equal => self.build_comparison("eq", IntPredicate::EQ, l, r),
            BinaryOp::NotEqual => self.build_comparison("ne", IntPredicate::NE, l, r),
        }
    }

    fn compile_call_expr(&mut self, name: &str, args: &[TypedExpr]) -> Anyhow<IntValue<'ctx>> {
        let function = self
            .functions
            .get(name)
            .copied()
            .ok_or_else(|| anyhow!("undefined function: {name}"))?;

        let arg_values = args
            .iter()
            .map(|a| self.compile_expr(a).map(BasicMetadataValueEnum::from))
            .collect::<Anyhow<Vec<_>>>()?;

        let call = self.builder.build_call(function, &arg_values, "call")?;
        Ok(call.try_as_basic_value().unwrap_basic().into_int_value())
    }

    fn compile_if_expr(
        &mut self,
        cond: &TypedExpr,
        then_branch: &[Statement],
        else_branch: &[Statement],
    ) -> Anyhow<IntValue<'ctx>> {
        let cond_val = self.compile_expr(cond)?;

        // Convert to i1 for branch
        let cond_bool = self
            .builder
            .build_int_truncate(cond_val, self.ctx.bool_type(), "cond")?;

        let function = self.current_fn.unwrap();
        let then_bb = self.ctx.append_basic_block(function, "then");
        let else_bb = self.ctx.append_basic_block(function, "else");
        let merge_bb = self.ctx.append_basic_block(function, "merge");

        self.builder
            .build_conditional_branch(cond_bool, then_bb, else_bb)?;

        // Then branch
        self.builder.position_at_end(then_bb);
        let mut then_val = self.ctx.i64_type().const_int(0, false);

        for stmt in then_branch {
            if let Some(v) = self.compile_stmt(stmt)? {
                then_val = v;
            }
        }

        let then_end = self.builder.get_insert_block().unwrap();
        let then_has_terminator = then_end.get_terminator().is_some();

        if !then_has_terminator {
            self.builder.build_unconditional_branch(merge_bb)?;
        }

        // Else branch
        self.builder.position_at_end(else_bb);
        let mut else_val = self.ctx.i64_type().const_int(0, false);

        for stmt in else_branch {
            if let Some(v) = self.compile_stmt(stmt)? {
                else_val = v;
            }
        }

        let else_end = self.builder.get_insert_block().unwrap();
        let else_has_terminator = else_end.get_terminator().is_some();

        if !else_has_terminator {
            self.builder.build_unconditional_branch(merge_bb)?;
        }

        // Merge - only if at least one branch reaches it
        if then_has_terminator && else_has_terminator {
            // Both branches return/terminate, merge block is unreachable
            // Remove it and return a dummy value
            unsafe { merge_bb.delete() }.map_err(|()| anyhow!("failed to delete merge block"))?;

            // Return a dummy value - the actual return happened in the branches
            Ok(self.ctx.i64_type().const_int(0, false))
        } else {
            self.builder.position_at_end(merge_bb);
            let phi = self.builder.build_phi(self.ctx.i64_type(), "phi")?;

            // Only add incoming from branches that don't have terminators
            if !then_has_terminator {
                phi.add_incoming(&[(&then_val, then_end)]);
            }
            if !else_has_terminator {
                phi.add_incoming(&[(&else_val, else_end)]);
            }

            Ok(phi.as_basic_value().into_int_value())
        }
    }

    fn compile_while_expr(
        &mut self,
        cond: &TypedExpr,
        body: &[Statement],
    ) -> Anyhow<IntValue<'ctx>> {
        let function = self.current_fn.unwrap();
        let cond_bb = self.ctx.append_basic_block(function, "while_cond");
        let body_bb = self.ctx.append_basic_block(function, "while_body");
        let end_bb = self.ctx.append_basic_block(function, "while_end");
        self.builder.build_unconditional_branch(cond_bb)?;

        // Condition
        self.builder.position_at_end(cond_bb);
        let cond_val = self.compile_expr(cond)?;
        let cond_bool = self
            .builder
            .build_int_truncate(cond_val, self.ctx.bool_type(), "cond")?;
        self.builder
            .build_conditional_branch(cond_bool, body_bb, end_bb)?;

        // Body
        self.builder.position_at_end(body_bb);

        for stmt in body {
            self.compile_stmt(stmt)?;
        }

        if self
            .builder
            .get_insert_block()
            .and_then(BasicBlock::get_terminator)
            .is_none()
        {
            self.builder.build_unconditional_branch(cond_bb)?;
        }

        // End
        self.builder.position_at_end(end_bb);
        Ok(self.ctx.i64_type().const_int(0, false))
    }

    fn compile_block_expr(&mut self, stmts: &[Statement]) -> Anyhow<IntValue<'ctx>> {
        let mut last_val = self.ctx.i64_type().const_int(0, false);

        for stmt in stmts {
            if let Some(v) = self.compile_stmt(stmt)? {
                last_val = v;
            }
        }

        Ok(last_val)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{parse, typecheck};

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

    fn compile(source: &str) {
        let program = parse_and_check(source);
        let context = Context::create();
        let codegen = CodeGen::new(&context, "test");
        assert!(codegen.compile(&program).is_ok());
    }

    #[test]
    fn test_compile_simple_function() {
        let source = r"
            def answer() -> int {
                return 42
            }
            answer()
        ";
        compile(source);
    }

    #[test]
    fn test_compile_add() {
        let source = r"
            def add(a: int, b: int) -> int {
                return a + b
            }
            add(3, 4)
        ";
        compile(source);
    }
}
