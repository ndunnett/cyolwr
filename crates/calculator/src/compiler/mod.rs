#![cfg_attr(debug_assertions, allow(dead_code, unused_imports))]

use crate::{Anyhow, ast::Node, parse};

pub trait Compile {
    type Output;

    fn from_ast(ast: Vec<Node>) -> Anyhow<Self::Output>;

    fn from_source(source: &str) -> Anyhow<Self::Output> {
        let ast = parse(source)?;
        println!("\nSyntax Tree:\n{ast:#?}");
        Self::from_ast(ast)
    }
}

#[cfg(feature = "tree_walk")]
mod tree_walk;

#[cfg(feature = "jit_llvm")]
mod jit_llvm;

#[cfg(feature = "jit_cranelift")]
mod jit_cranelift;

#[cfg(feature = "bytecode")]
mod bytecode;
#[cfg(feature = "bytecode")]
mod opcode;
#[cfg(feature = "bytecode")]
mod vm;

cfg_if::cfg_if! {
    if #[cfg(feature = "jit_cranelift")] {
        pub use jit_cranelift::Jit as Engine;
    }
    else if #[cfg(feature = "jit_llvm")] {
        pub use jit_llvm::Jit as Engine;
    }
    else if #[cfg(feature = "bytecode")] {
        pub use vm::VirtualMachine as Engine;
    }
    else if #[cfg(feature = "tree_walk")]{
        pub use tree_walk::Interpreter as Engine;
    }
}
