use rustyline::{DefaultEditor, Result, error::ReadlineError};

use calculator::{Compile, Engine};

fn main() -> Result<()> {
    let mut rl = DefaultEditor::new()?;
    println!("Calculator prompt. Expressions are line evaluated.");

    loop {
        match rl.readline(">> ") {
            Ok(line) => {
                let line = line.trim();

                if line.is_empty() {
                    continue;
                }

                match Engine::from_source(line) {
                    Ok(result) => println!("\nResult:\n{result}"),
                    Err(e) => eprintln!("{e}"),
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("CTRL-C");
                break;
            }
            Err(ReadlineError::Eof) => {
                println!("CTRL-D");
                break;
            }
            Err(e) => {
                eprintln!("Error: {e:?}");
                break;
            }
        }
    }

    Ok(())
}
