pub mod ast;
mod compiler;
mod parser;

pub type Anyhow<T> = anyhow::Result<T>;
pub use anyhow::anyhow;

pub use crate::{
    compiler::{Compile, Engine},
    parser::parse,
};
