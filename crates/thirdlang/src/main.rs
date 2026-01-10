use std::path::PathBuf;

use clap::Parser;

use thirdlang::{Anyhow, compile, create_context, optimize, parse, typecheck};

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
    #[arg(long)]
    passes: Option<String>,
}

fn main() -> Anyhow<()> {
    let args = Args::parse();
    let source = std::fs::read_to_string(&args.path)?;
    let mut program = parse(&source)?;
    let classes = typecheck(&mut program)?;

    if args.optimize {
        program = optimize(program);
    }

    if args.check {
        println!("Type check passed!");
    } else if args.ast {
        println!("{program:#?}");
    } else {
        let name = args
            .path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");

        let passes = if args.optimize {
            args.passes
                .as_deref()
                .or(Some("default<O2>"))
        } else {
            None
        };

        let ctx = create_context();
        let module = compile(&ctx, &program, classes, name, passes)?;

        if args.ir {
            println!("{module}");
        } else {
            println!("{}", module.execute()?);
        }
    }

    Ok(())
}
