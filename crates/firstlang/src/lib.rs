mod ast;
mod interpreter;
mod parser;

pub type Anyhow<T> = anyhow::Result<T>;
pub use anyhow::anyhow;

pub use interpreter::Value;

pub fn run(source: &str) -> Anyhow<Value> {
    println!("{source}");
    Ok(Value::Unit)
}
