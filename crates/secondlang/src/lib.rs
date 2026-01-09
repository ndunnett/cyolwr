pub mod ast;
mod codegen;
mod optimizer;
mod parser;
mod types;
mod typing;
mod visitor;

pub type Anyhow<T> = anyhow::Result<T>;
pub use anyhow::anyhow;

pub use crate::{
    codegen::{compile, create_context},
    optimizer::optimize,
    parser::parse,
    types::Type,
    typing::typecheck,
    visitor::Visitor,
};
