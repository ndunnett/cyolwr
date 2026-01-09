use std::path::PathBuf;

use clap::Parser;

use thirdlang::Anyhow;

const ABOUT: &str = "Thirdlang Compiler";
const LONG_ABOUT: &str = r"Thirdlang Compiler

An object-oriented language with explicit memory management.

Optimization Passes (for --passes):
  - dce           Dead Code Elimination
  - mem2reg       Promote allocas to SSA registers
  - instcombine   Combine redundant instructions
  - simplifycfg   Simplify control flow graph
  - gvn           Global Value Numbering
  - default<O0>   No optimization (verify only)
  - default<O1>   Light optimization
  - default<O2>   Standard optimization (recommended)
  - default<O3>   Aggressive optimization

Features:
  - Classes with fields and methods
  - Constructors (__init__) and destructors (__del__)
  - Object creation (new) and deletion (delete)
  - Static type checking with inference
  - JIT compilation via LLVM
  - LLVM New Pass Manager for optimization";

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Parser)]
#[command(version, about = ABOUT, long_about = LONG_ABOUT)]
pub struct Args {
    /// Path to source file
    path: PathBuf,
    /// Type check only
    #[arg(long, group = "action")]
    check: bool,
    /// Print AST
    #[arg(long, group = "action")]
    ast: bool,
    /// Print LLVM IR
    #[arg(long, group = "action")]
    ir: bool,
    /// Perform optimizations
    #[arg(short = 'O', long)]
    optimize: bool,
    /// Custom passes
    #[arg(long, default_value = "dce,mem2reg,instcombine,simplifycfg")]
    passes: String,
}

fn main() -> Anyhow<()> {
    let args = Args::parse();
    let source = std::fs::read_to_string(&args.path)?;
    println!("{source}");
    Ok(())
}
