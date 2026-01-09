pub mod ast;
mod codegen;
mod parser;
mod types;
mod typing;
mod visitor;

pub type Anyhow<T> = anyhow::Result<T>;
pub use anyhow::anyhow;

pub use crate::{parser::parse, types::Type};
