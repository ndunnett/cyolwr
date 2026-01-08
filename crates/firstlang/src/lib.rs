mod ast;
mod interpreter;
mod parser;

pub type Anyhow<T> = anyhow::Result<T>;
pub use anyhow::anyhow;

pub use crate::{interpreter::Value, parser::parse};

pub fn run(source: &str) -> Anyhow<Value> {
    let ast = parse(source)?;
    println!("{ast:#?}");
    Ok(Value::Unit)
}
