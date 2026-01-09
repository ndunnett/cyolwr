use std::path::PathBuf;

use clap::Parser;

use secondlang::{Anyhow, compile, create_context, optimize, parse, typecheck};

const ABOUT: &str = "Secondlang Compiler";
const LONG_ABOUT: &str = r"Secondlang Compiler

Optimization passes (with -O):
  - Constant folding:          1 + 2 * 3 -> 7
  - Algebraic simplification:  x + 0 -> x, x * 1 -> x";

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
}

fn main() -> Anyhow<()> {
    let args = Args::parse();
    let source = std::fs::read_to_string(&args.path)?;
    let mut program = parse(&source)?;
    typecheck(&mut program)?;

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

        let ctx = create_context();
        let module = compile(&ctx, &program, name)?;

        if args.ir {
            println!("{module}");
        } else {
            println!("{}", module.execute()?);
        }
    }

    Ok(())
}
