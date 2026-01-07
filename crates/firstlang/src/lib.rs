mod ast;
mod interpreter;
mod parser;

pub use interpreter::Value;

pub fn run(source: &str) -> Result<Value, String> {
    println!("{source}");
    Ok(Value::Unit)
}
