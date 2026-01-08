use std::io::{BufRead, Write};

use firstlang::{Anyhow, run};

struct Editor {
    buffer: String,
    stdin: std::io::Stdin,
    stdout: std::io::Stdout,
}

impl Editor {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            stdin: std::io::stdin(),
            stdout: std::io::stdout(),
        }
    }

    /// Prints the given prompt and reads `stdin` into `self.buffer`. Returns false when no bytes are read.
    fn read_line(&mut self, prompt: &'static str) -> Anyhow<bool> {
        write!(self.stdout, "{prompt} ")?;
        self.stdout.flush()?;
        let bytes_read = self.stdin.lock().read_line(&mut self.buffer)?;
        Ok(bytes_read != 0)
    }

    /// Returns the depth of open brackets in `self.buffer`, ignoring brackets within quotations.
    fn bracket_depth(&self) -> i32 {
        let mut depth = 0;
        let mut in_string = false;
        let mut escape = false;

        for byte in self.buffer.bytes() {
            match byte {
                _ if escape => escape = false,
                b'"' if !escape => in_string = !in_string,
                b'\\' if in_string => escape = true,
                _ if in_string => {}
                b'{' | b'(' | b'[' => depth += 1,
                b'}' | b')' | b']' => depth -= 1,
                _ => {}
            }
        }

        depth
    }

    /// Runs an interactive prompt, returning the input. Inputs are trimmed; an input of `\n` denotes EOF.
    pub fn prompt(&mut self) -> Anyhow<&str> {
        self.buffer.clear();

        if !self.read_line(">>>")? {
            return Ok("\n");
        }

        while self.bracket_depth() > 0 {
            if !self.read_line("...")? {
                break;
            }
        }

        Ok(self.buffer.trim())
    }
}

fn main() -> Anyhow<()> {
    if let Some(path) = std::env::args().nth(1) {
        // Run the file at the given path (first argument) as a script
        let source = std::fs::read_to_string(path)?;
        let value = run(&source)?;
        println!("{value}");
    } else {
        // Start the REPL
        println!("Firstlang REPL v0.1.0\nType expressions to evaluate, or 'quit' to exit.\n");
        let mut editor = Editor::new();

        loop {
            match editor.prompt()? {
                "" => {}
                "quit" | "exit" | "\n" => break,
                input => {
                    let value = run(input)?;
                    println!("{value}");
                }
            }
        }
    }

    Ok(())
}
