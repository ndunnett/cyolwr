use crate::{Anyhow, ast::Program, types::ClassRegistry};

#[cfg(feature = "llvm_backend")]
mod llvm_impl;

#[cfg(feature = "llvm_backend")]
pub use llvm_impl::Context;

pub trait ModuleContext<'src> {
    type CodegenContext;
    type CodegenModule: CodeGenerator;

    fn new(name: &str, classes: ClassRegistry, program: &'src Program) -> Self;
    fn ctx(&self) -> &Self::CodegenContext;
    fn codegen(&'src self) -> Self::CodegenModule;
}

pub trait CodeGenerator {
    type IrModule: IntermediateRepresentation;

    fn compile(self) -> Anyhow<Self::IrModule>;
}

pub trait IntermediateRepresentation: std::fmt::Display {
    fn run_passes(&self, passes: Option<&str>) -> Anyhow<()>;
    fn execute(&self) -> Anyhow<i64>;
}
