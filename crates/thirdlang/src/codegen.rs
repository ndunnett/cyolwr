//! LLVM Code Generation for Thirdlang
//!
//! Extends Secondlang's codegen with:
//! - Class memory layout (fields as struct)
//! - Object allocation (malloc)
//! - Object deallocation (free + destructor)
//! - Method compilation (first param is self pointer)
//! - Field access/assignment via GEP
//! - LLVM New Pass Manager (NPM) for optimization

use std::collections::HashMap;

use inkwell::{
    AddressSpace, IntPredicate, OptimizationLevel,
    builder::Builder,
    context::Context,
    module::Module,
    passes::PassBuilderOptions,
    targets::{CodeModel, InitializationConfig, RelocMode, Target, TargetMachine},
    types::{BasicMetadataTypeEnum, BasicType, BasicTypeEnum, IntType, StructType},
    values::{BasicMetadataValueEnum, BasicValueEnum, FunctionValue, IntValue, PointerValue},
};

use crate::{
    Anyhow, anyhow,
    ast::{
        AssignTarget, BinaryOp, ClassDef, Expression, MethodDef, Program, Statement, TopLevel,
        TypedExpr, UnaryOp,
    },
    types::{ClassRegistry, Type},
};

pub fn create_context() -> Context {
    Context::create()
}

pub fn compile<'ctx>(
    ctx: &'ctx Context,
    program: &Program,
    classes: ClassRegistry,
    module_name: &str,
    passes: Option<&str>,
) -> Anyhow<IrModule<'ctx>> {
    CodeGen::new(ctx, module_name, classes).compile(program, passes)
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
    class_types: HashMap<String, StructType<'ctx>>,
    classes: ClassRegistry,
    current_fn: Option<FunctionValue<'ctx>>,
    current_class: Option<String>,
}

impl<'ctx> CodeGen<'ctx> {
    pub fn new(ctx: &'ctx Context, module_name: &str, classes: ClassRegistry) -> Self {
        let module = ctx.create_module(module_name);
        let builder = ctx.create_builder();

        Self {
            ctx,
            module,
            builder,
            variables: HashMap::new(),
            functions: HashMap::new(),
            class_types: HashMap::new(),
            classes,
            current_fn: None,
            current_class: None,
        }
    }

    pub fn compile(mut self, program: &Program, passes: Option<&str>) -> Anyhow<IrModule<'ctx>> {
        // Declare libc functions
        let i64_type = self.ctx.i64_type();
        let ptr_type = self.ctx.ptr_type(AddressSpace::default());
        let malloc_type = ptr_type.fn_type(&[i64_type.into()], false);
        self.module.add_function("malloc", malloc_type, None);
        let free_type = self.ctx.void_type().fn_type(&[ptr_type.into()], false);
        self.module.add_function("free", free_type, None);

        // First pass: create LLVM struct types for classes
        for item in program {
            if let TopLevel::Class(class) = item {
                self.create_class_type(class)?;
            }
        }

        // Second pass: declare all functions and methods
        for item in program {
            match item {
                TopLevel::Class(class) => {
                    self.declare_class_methods(class)?;
                }
                TopLevel::Stmt(Statement::Function {
                    name,
                    params,
                    return_type,
                    ..
                }) => {
                    self.declare_function(name, params, return_type)?;
                }
                TopLevel::Stmt(_) => {}
            }
        }

        // Third pass: compile function and method bodies
        for item in program {
            match item {
                TopLevel::Class(class) => {
                    self.compile_class(class)?;
                }
                TopLevel::Stmt(stmt @ Statement::Function { .. }) => {
                    self.compile_stmt(stmt)?;
                }
                TopLevel::Stmt(_) => {}
            }
        }

        // Fourth pass: create __main wrapper for all top-level non-function statements
        self.compile_main_wrapper_all(program)?;

        // Verify module
        self.module.verify().map_err(|e| anyhow!("{e}"))?;

        // Run optimization passes if specified
        if let Some(pass_pipeline) = passes {
            self.run_passes(pass_pipeline)?;
        }

        Ok(IrModule {
            module: self.module,
        })
    }

    pub fn run_passes(&self, passes: &str) -> Anyhow<()> {
        // Initialize native target for the current machine
        Target::initialize_native(&InitializationConfig::default()).map_err(|e| anyhow!("{e}"))?;

        // Get the default target triple for this machine
        let triple = TargetMachine::get_default_triple();

        // Get the target from the triple
        let target = Target::from_triple(&triple).map_err(|e| anyhow!("{e}"))?;

        // Create target machine with default settings
        let Some(target_machine) = target.create_target_machine(
            &triple,
            "generic", // CPU
            "",        // Features
            OptimizationLevel::Default,
            RelocMode::Default,
            CodeModel::Default,
        ) else {
            return Err(anyhow!("Failed to create target machine"));
        };

        // Create pass builder options
        let pass_options = PassBuilderOptions::create();
        pass_options.set_verify_each(true); // Verify IR after each pass

        // Run the passes
        self.module
            .run_passes(passes, &target_machine, pass_options)
            .map_err(|e| anyhow!("{e}"))
    }

    fn llvm_type(&self, ty: &Type) -> Anyhow<IntType<'ctx>> {
        match ty {
            Type::Bool => Ok(self.ctx.bool_type()),
            Type::Function { .. } | Type::Method { .. } => {
                Err(anyhow!("cannot get LLVM type for function/method type"))
            }
            _ => Ok(self.ctx.i64_type()),
        }
    }

    fn llvm_basic_type(&self, ty: &Type) -> Anyhow<BasicTypeEnum<'ctx>> {
        match ty {
            Type::Bool => Ok(self.ctx.bool_type().into()),
            Type::Class(name) => {
                let Some(_) = self.class_types.get(name) else {
                    return Err(anyhow!("unknown class type: {name}"));
                };

                Ok(self.ctx.ptr_type(AddressSpace::default()).into())
            }
            _ => Ok(self.ctx.i64_type().into()),
        }
    }

    fn value_to_int(&self, val: BasicValueEnum<'ctx>) -> Anyhow<IntValue<'ctx>> {
        match val {
            BasicValueEnum::IntValue(i) => Ok(i),
            BasicValueEnum::PointerValue(p) => {
                Ok(self
                    .builder
                    .build_ptr_to_int(p, self.ctx.i64_type(), "ptoi")?)
            }
            _ => Err(anyhow!("cannot convert value to int")),
        }
    }

    fn build_entry_alloca(
        &self,
        function: FunctionValue<'ctx>,
        name: &str,
        ty: &Type,
    ) -> Anyhow<PointerValue<'ctx>> {
        let builder = self.ctx.create_builder();

        let Some(entry) = function.get_first_basic_block() else {
            return Err(anyhow!("failed to get basic block"));
        };

        match entry.get_first_instruction() {
            Some(inst) => builder.position_before(&inst),
            None => builder.position_at_end(entry),
        }

        let alloca_type: BasicTypeEnum = if ty.is_class() {
            self.ctx.ptr_type(AddressSpace::default()).into()
        } else {
            self.ctx.i64_type().into()
        };

        Ok(builder.build_alloca(alloca_type, name)?)
    }

    fn create_class_type(&mut self, class: &ClassDef) -> Anyhow<StructType<'ctx>> {
        let Some(class_info) = self.classes.get(&class.name) else {
            return Err(anyhow!("class {} not found in registry", class.name));
        };

        let mut field_types: Vec<BasicTypeEnum> = Vec::new();

        for field_name in &class_info.field_order {
            let Some(field_type) = class_info.get_field(field_name) else {
                return Err(anyhow!("field {field_name} not found in {}", class.name));
            };

            field_types.push(self.llvm_basic_type(field_type)?);
        }

        let struct_type = self.ctx.opaque_struct_type(&class.name);
        struct_type.set_body(&field_types, false);

        self.class_types.insert(class.name.clone(), struct_type);
        Ok(struct_type)
    }

    fn declare_class_methods(&mut self, class: &ClassDef) -> Anyhow<()> {
        for method in &class.methods {
            self.declare_method(&class.name, method)?;
        }

        Ok(())
    }

    fn declare_method(
        &mut self,
        class_name: &str,
        method: &MethodDef,
    ) -> Anyhow<FunctionValue<'ctx>> {
        let ptr_type = self.ctx.ptr_type(AddressSpace::default());

        // Self pointer is first parameter
        let mut param_types = vec![BasicMetadataTypeEnum::from(ptr_type)];

        // Class-typed parameters are passed as pointers
        for (_, param_type) in &method.params {
            let llvm_type: BasicMetadataTypeEnum = if param_type.is_class() {
                ptr_type.into()
            } else {
                self.llvm_type(param_type)?.into()
            };

            param_types.push(llvm_type);
        }

        // Return type - always use i64 for simplicity (bools are extended to i64)
        let ret_type = self.ctx.i64_type();

        let fn_type = ret_type.fn_type(&param_types, false);
        let fn_name = format!("{}__{}", class_name, method.name);
        let function = self.module.add_function(&fn_name, fn_type, None);

        // Set parameter names
        function.get_nth_param(0).unwrap().set_name("self");
        for (i, (param_name, _)) in method.params.iter().enumerate() {
            function
                .get_nth_param((i + 1) as u32)
                .unwrap()
                .set_name(param_name);
        }

        self.functions.insert(fn_name, function);
        Ok(function)
    }

    fn compile_class(&mut self, class: &ClassDef) -> Anyhow<()> {
        self.current_class = Some(class.name.clone());

        for method in &class.methods {
            self.compile_method(&class.name, method)?;
        }

        self.current_class = None;
        Ok(())
    }

    fn compile_method(&mut self, class_name: &str, method: &MethodDef) -> Anyhow<()> {
        let fn_name = format!("{}__{}", class_name, method.name);

        let Some(function) = self.functions.get(&fn_name).copied() else {
            return Err(anyhow!("method {fn_name} not declared"));
        };

        let entry = self.ctx.append_basic_block(function, "entry");
        self.builder.position_at_end(entry);
        self.current_fn = Some(function);
        self.variables.clear();

        // Allocate 'self' parameter (pointer to object)
        let self_ptr = function.get_nth_param(0).unwrap().into_pointer_value();
        let self_ty = Type::Class(class_name.to_string());
        let self_alloca = self.build_entry_alloca(function, "self", &self_ty)?;
        self.builder.build_store(self_alloca, self_ptr)?;
        self.variables.insert("self".to_string(), self_alloca);

        // Allocate other parameters
        for (i, (param_name, param_type)) in method.params.iter().enumerate() {
            let param_value = function.get_nth_param((i + 1) as u32).unwrap();
            let alloca = self.build_entry_alloca(function, param_name, param_type)?;
            self.builder.build_store(alloca, param_value)?;
            self.variables.insert(param_name.clone(), alloca);
        }

        // Compile body
        let mut last_value = None;
        for body_stmt in &method.body {
            last_value = self.compile_stmt(body_stmt)?;
        }

        // Add return if needed
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

        Ok(())
    }

    fn compile_main_wrapper_all(&mut self, program: &Program) -> Anyhow<()> {
        let stmts = program
            .iter()
            .filter_map(|item| match item {
                TopLevel::Stmt(stmt) if !matches!(stmt, Statement::Function { .. }) => Some(stmt),
                _ => None,
            })
            .collect::<Vec<_>>();

        if stmts.is_empty() {
            return Ok(());
        }

        let ret_type = self.ctx.i64_type();
        let fn_type = ret_type.fn_type(&[], false);
        let function = self.module.add_function("__main", fn_type, None);
        let entry = self.ctx.append_basic_block(function, "entry");
        self.builder.position_at_end(entry);
        self.current_fn = Some(function);
        self.variables.clear();
        let mut last_value: Option<IntValue> = None;

        for stmt in stmts {
            last_value = self.compile_stmt(stmt)?;
        }

        let ret_val = last_value.unwrap_or_else(|| self.ctx.i64_type().const_int(0, false));
        self.builder.build_return(Some(&ret_val))?;
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
            .map(|(_, t)| self.llvm_type(t).map(Into::into))
            .collect::<Anyhow<Vec<BasicMetadataTypeEnum>>>()?;

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
                let int_val = self.value_to_int(value)?;
                self.builder.build_return(Some(&int_val))?;
                Ok(Some(int_val))
            }
            Statement::Assignment { target, value, .. } => self.compile_assignment(target, value),
            Statement::Delete(expr) => self.compile_delete(expr),
            Statement::Expression(expr) => {
                let val = self.compile_expr(expr)?;
                Ok(Some(self.value_to_int(val)?))
            }
        }
    }

    fn compile_function(
        &mut self,
        name: &str,
        params: &[(String, Type)],
        body: &[Statement],
    ) -> Anyhow<Option<IntValue<'ctx>>> {
        let Some(function) = self.functions.get(name).copied() else {
            return Err(anyhow!("function {name} not declared"));
        };

        let entry = self.ctx.append_basic_block(function, "entry");
        self.builder.position_at_end(entry);
        self.current_fn = Some(function);
        self.variables.clear();

        for (i, (param_name, param_ty)) in params.iter().enumerate() {
            let param_value = function.get_nth_param(i as u32).unwrap().into_int_value();
            let alloca = self.build_entry_alloca(function, param_name, param_ty)?;
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
        target: &AssignTarget,
        value: &TypedExpr,
    ) -> Anyhow<Option<IntValue<'ctx>>> {
        let val = self.compile_expr(value)?;

        match target {
            AssignTarget::Var(name) => {
                if let Some(ptr) = self.variables.get(name) {
                    self.builder.build_store(*ptr, val)?;
                } else {
                    let function = self.current_fn.unwrap();
                    let alloca = self.build_entry_alloca(function, name, &value.ty)?;
                    self.builder.build_store(alloca, val)?;
                    self.variables.insert(name.clone(), alloca);
                }
            }
            AssignTarget::Field { object, field } => {
                let obj_val = self.compile_expr(object)?;
                let obj_ptr = obj_val.into_pointer_value();

                let Some(class_name) = object.ty.class_name() else {
                    return Err(anyhow!("expected class type"));
                };

                let Some(class_info) = self.classes.get(class_name) else {
                    return Err(anyhow!("class not found"));
                };

                let Some(field_idx) = class_info.field_index(field) else {
                    return Err(anyhow!("field not found"));
                };

                let Some(struct_type) = self.class_types.get(class_name) else {
                    return Err(anyhow!("class type not found"));
                };

                let field_ptr = self.builder.build_struct_gep(
                    *struct_type,
                    obj_ptr,
                    field_idx as u32,
                    "field_ptr",
                )?;

                self.builder.build_store(field_ptr, val)?;
            }
        }

        Ok(Some(self.value_to_int(val)?))
    }

    fn compile_delete(&mut self, expr: &TypedExpr) -> Anyhow<Option<IntValue<'ctx>>> {
        let obj_val = self.compile_expr(expr)?;
        let obj_ptr = obj_val.into_pointer_value();

        if let Type::Class(class_name) = &expr.ty {
            let class_info = self.classes.get(class_name);

            if let Some(info) = class_info
                && info.has_destructor
            {
                let dtor_name = format!("{class_name}____del__");

                if let Some(dtor) = self.functions.get(&dtor_name) {
                    self.builder.build_call(*dtor, &[obj_ptr.into()], "dtor")?;
                }
            }
        }

        let free_fn = self.module.get_function("free").unwrap();
        self.builder.build_call(free_fn, &[obj_ptr.into()], "")?;
        Ok(None)
    }

    fn compile_expr(&mut self, expr: &TypedExpr) -> Anyhow<BasicValueEnum<'ctx>> {
        match &expr.expr {
            Expression::Int(n) => Ok(self.ctx.i64_type().const_int(*n as u64, false).into()),
            Expression::Bool(b) => Ok(self.ctx.bool_type().const_int(*b as u64, false).into()),
            Expression::Var(name) => self.compile_var(name, &expr.ty),
            Expression::SelfRef => self.compile_self_ref(),
            Expression::Unary { op, expr: inner } => self.compile_unary_expr(*op, inner),
            Expression::Binary { op, left, right } => self.compile_binary_expr(*op, left, right),
            Expression::Call { name, args } => self.compile_call(name, args),
            Expression::MethodCall {
                object,
                method,
                args,
            } => self.compile_method_call(object, method, args),
            Expression::FieldAccess { object, field } => self.compile_field_access(object, field),
            Expression::New { class, args } => self.compile_new(class, args),
            Expression::If {
                cond,
                then_branch,
                else_branch,
            } => self.compile_if(cond, then_branch, else_branch),
            Expression::While { cond, body } => self.compile_while(cond, body),
            Expression::Block(stmts) => self.compile_block(stmts),
        }
    }

    fn compile_var(&self, name: &str, ty: &Type) -> Anyhow<BasicValueEnum<'ctx>> {
        let Some(ptr) = self.variables.get(name) else {
            return Err(anyhow!("undefined variable: {name}"));
        };

        let load_type = if ty.is_class() {
            self.ctx
                .ptr_type(AddressSpace::default())
                .as_basic_type_enum()
        } else {
            self.ctx.i64_type().as_basic_type_enum()
        };

        let val = self.builder.build_load(load_type, *ptr, name)?;
        Ok(val)
    }

    fn compile_self_ref(&self) -> Anyhow<BasicValueEnum<'ctx>> {
        let Some(ptr) = self.variables.get("self") else {
            return Err(anyhow!("'self' not in scope"));
        };

        let val =
            self.builder
                .build_load(self.ctx.ptr_type(AddressSpace::default()), *ptr, "self")?;

        Ok(val)
    }

    fn compile_unary_expr(
        &mut self,
        op: UnaryOp,
        expr: &TypedExpr,
    ) -> Anyhow<BasicValueEnum<'ctx>> {
        let val = self.compile_expr(expr)?.into_int_value();

        let result = match op {
            UnaryOp::Negative => self.builder.build_int_neg(val, "neg")?,
            UnaryOp::Not => self.builder.build_not(val, "not")?,
        };

        Ok(result.into())
    }

    fn compile_binary_expr(
        &mut self,
        op: BinaryOp,
        left: &TypedExpr,
        right: &TypedExpr,
    ) -> Anyhow<BasicValueEnum<'ctx>> {
        let l = self.compile_expr(left)?.into_int_value();
        let r = self.compile_expr(right)?.into_int_value();

        let result = match op {
            BinaryOp::Add => self.builder.build_int_add(l, r, "add")?,
            BinaryOp::Subtract => self.builder.build_int_sub(l, r, "sub")?,
            BinaryOp::Multiply => self.builder.build_int_mul(l, r, "mul")?,
            BinaryOp::Divide => self.builder.build_int_signed_div(l, r, "div")?,
            BinaryOp::Modulo => self.builder.build_int_signed_rem(l, r, "mod")?,
            BinaryOp::LessThan => self.build_comparison("lt", IntPredicate::SLT, l, r)?,
            BinaryOp::GreaterThan => self.build_comparison("gt", IntPredicate::SGT, l, r)?,
            BinaryOp::LessEqual => self.build_comparison("le", IntPredicate::SLE, l, r)?,
            BinaryOp::GreaterEqual => self.build_comparison("ge", IntPredicate::SGE, l, r)?,
            BinaryOp::Equal => self.build_comparison("eq", IntPredicate::EQ, l, r)?,
            BinaryOp::NotEqual => self.build_comparison("ne", IntPredicate::NE, l, r)?,
        };

        Ok(result.into())
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

    fn compile_call(&mut self, name: &str, args: &[TypedExpr]) -> Anyhow<BasicValueEnum<'ctx>> {
        let Some(function) = self.functions.get(name).copied() else {
            return Err(anyhow!("undefined function: {name}"));
        };

        let arg_values = args
            .iter()
            .map(|a| self.compile_expr(a).map(Into::into))
            .collect::<Anyhow<Vec<BasicMetadataValueEnum>>>()?;

        let call = self.builder.build_call(function, &arg_values, "call")?;
        Ok(call.try_as_basic_value().unwrap_basic())
    }

    fn compile_method_call(
        &mut self,
        object: &TypedExpr,
        method: &str,
        args: &[TypedExpr],
    ) -> Anyhow<BasicValueEnum<'ctx>> {
        let obj_val = self.compile_expr(object)?;
        let obj_ptr = obj_val.into_pointer_value();

        let Some(class_name) = object.ty.class_name() else {
            return Err(anyhow!("expected class type"));
        };

        let fn_name = format!("{class_name}__{method}");

        let Some(function) = self.functions.get(&fn_name).copied() else {
            return Err(anyhow!("undefined method: {fn_name}"));
        };

        let mut arg_values = vec![BasicMetadataValueEnum::from(obj_ptr)];

        for arg in args {
            arg_values.push(self.compile_expr(arg)?.into());
        }

        let call = self.builder.build_call(function, &arg_values, "call")?;
        Ok(call.try_as_basic_value().unwrap_basic())
    }

    fn compile_field_access(
        &mut self,
        object: &TypedExpr,
        field: &str,
    ) -> Anyhow<BasicValueEnum<'ctx>> {
        let obj_val = self.compile_expr(object)?;
        let obj_ptr = obj_val.into_pointer_value();

        let Some(class_name) = object.ty.class_name() else {
            return Err(anyhow!("expected class type"));
        };

        let Some(class_info) = self.classes.get(class_name) else {
            return Err(anyhow!("class not found"));
        };

        let Some(field_idx) = class_info.field_index(field) else {
            return Err(anyhow!("field not found"));
        };

        let Some(field_type) = class_info.get_field(field) else {
            return Err(anyhow!("field not found"));
        };

        let Some(struct_type) = self.class_types.get(class_name) else {
            return Err(anyhow!("class type not found"));
        };

        let field_ptr =
            self.builder
                .build_struct_gep(*struct_type, obj_ptr, field_idx as u32, "field_ptr")?;

        let load_type = self.llvm_basic_type(field_type)?;
        let val = self.builder.build_load(load_type, field_ptr, "field")?;
        Ok(val)
    }

    fn compile_new(&mut self, class: &str, args: &[TypedExpr]) -> Anyhow<BasicValueEnum<'ctx>> {
        // Get struct type and size
        let Some(struct_type) = self.class_types.get(class) else {
            return Err(anyhow!("class type not found"));
        };

        // Calculate size (number of fields * 8 bytes)
        let Some(class_info) = self.classes.get(class) else {
            return Err(anyhow!("class not found"));
        };

        let size = (class_info.size() * 8).max(8) as u64; // At least 8 bytes
        let size_val = self.ctx.i64_type().const_int(size, false);

        // Call malloc
        let malloc_fn = self.module.get_function("malloc").unwrap();
        let ptr = self
            .builder
            .build_call(malloc_fn, &[size_val.into()], "obj")?
            .try_as_basic_value()
            .unwrap_basic()
            .into_pointer_value();

        // Initialize fields to zero
        for (i, _) in class_info.field_order.iter().enumerate() {
            let field_ptr =
                self.builder
                    .build_struct_gep(*struct_type, ptr, i as u32, "init_field")?;
            let zero = self.ctx.i64_type().const_int(0, false);
            self.builder.build_store(field_ptr, zero)?;
        }

        // Call constructor if exists
        let ctor_name = format!("{class}____init__");

        if let Some(ctor) = self.functions.get(&ctor_name).copied() {
            let mut ctor_args = vec![BasicMetadataValueEnum::from(ptr)];

            for arg in args {
                ctor_args.push(self.compile_expr(arg)?.into());
            }

            self.builder.build_call(ctor, &ctor_args, "")?;
        }

        Ok(ptr.into())
    }

    fn compile_if(
        &mut self,
        cond: &TypedExpr,
        then_branch: &[Statement],
        else_branch: &[Statement],
    ) -> Anyhow<BasicValueEnum<'ctx>> {
        let cond_val = self.compile_expr(cond)?.into_int_value();
        let cond_bool = self
            .builder
            .build_int_truncate(cond_val, self.ctx.bool_type(), "cond")?;

        let function = self.current_fn.unwrap();
        let then_bb = self.ctx.append_basic_block(function, "then");
        let else_bb = self.ctx.append_basic_block(function, "else");
        let merge_bb = self.ctx.append_basic_block(function, "merge");

        self.builder
            .build_conditional_branch(cond_bool, then_bb, else_bb)?;

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

        if then_has_terminator && else_has_terminator {
            unsafe { merge_bb.delete() }.map_err(|()| anyhow!("failed to delete merge block"))?;
            Ok(self.ctx.i64_type().const_int(0, false).into())
        } else {
            self.builder.position_at_end(merge_bb);
            let phi = self.builder.build_phi(self.ctx.i64_type(), "phi")?;

            if !then_has_terminator {
                phi.add_incoming(&[(&then_val, then_end)]);
            }

            if !else_has_terminator {
                phi.add_incoming(&[(&else_val, else_end)]);
            }

            Ok(phi.as_basic_value())
        }
    }

    fn compile_while(
        &mut self,
        cond: &TypedExpr,
        body: &[Statement],
    ) -> Anyhow<BasicValueEnum<'ctx>> {
        let function = self.current_fn.unwrap();
        let cond_bb = self.ctx.append_basic_block(function, "while_cond");
        let body_bb = self.ctx.append_basic_block(function, "while_body");
        let end_bb = self.ctx.append_basic_block(function, "while_end");
        self.builder.build_unconditional_branch(cond_bb)?;

        self.builder.position_at_end(cond_bb);
        let cond_val = self.compile_expr(cond)?.into_int_value();

        let cond_bool = self
            .builder
            .build_int_truncate(cond_val, self.ctx.bool_type(), "cond")?;

        self.builder
            .build_conditional_branch(cond_bool, body_bb, end_bb)?;

        self.builder.position_at_end(body_bb);

        for stmt in body {
            self.compile_stmt(stmt)?;
        }

        if self
            .builder
            .get_insert_block()
            .unwrap()
            .get_terminator()
            .is_none()
        {
            self.builder.build_unconditional_branch(cond_bb)?;
        }

        self.builder.position_at_end(end_bb);
        Ok(self.ctx.i64_type().const_int(0, false).into())
    }

    fn compile_block(&mut self, stmts: &[Statement]) -> Anyhow<BasicValueEnum<'ctx>> {
        let mut last_val: BasicValueEnum = self.ctx.i64_type().const_int(0, false).into();

        for stmt in stmts {
            if let Some(v) = self.compile_stmt(stmt)? {
                last_val = v.into();
            }
        }

        Ok(last_val)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{parse, typecheck};

    #[test]
    fn test_compile_class() {
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
            p = new Point(10, 20)
            p.get_x()
        ";

        let mut program = parse(source).unwrap();
        let classes = typecheck(&mut program).unwrap();

        let context = Context::create();
        let codegen = CodeGen::new(&context, "test", classes);
        codegen.compile(&program, None).unwrap();
    }
}
