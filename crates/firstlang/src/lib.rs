mod ast;
mod interpreter;
mod parser;

pub type Anyhow<T> = anyhow::Result<T>;
pub use anyhow::anyhow;

pub use crate::{
    interpreter::{Interpreter, Value},
    parser::parse,
};
