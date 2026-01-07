use crate::{
    Anyhow, Compile, anyhow,
    ast::Node,
    compiler::{
        bytecode::{Bytecode, Interpreter},
        opcode::{OpCode, combine_u8s},
    },
};

const STACK_SIZE: usize = 512;

pub struct VirtualMachine {
    bytecode: Bytecode,
    stack: [Node; STACK_SIZE],
    stack_pointer: usize, // points to the next free space
}

impl VirtualMachine {
    pub const fn new(bytecode: Bytecode) -> Self {
        Self {
            bytecode,
            stack: unsafe { std::mem::zeroed() }, // exercise: This is UB as Node has non-zero discriminant!
            stack_pointer: 0,
        }
    }

    pub fn run(&mut self) -> Anyhow<()> {
        let mut ip = 0;

        while ip < self.bytecode.instructions.len() {
            let instruction = ip;
            ip += 1;

            match self.bytecode.instructions[instruction] {
                OpCode::CONSTANT => {
                    let const_idx = combine_u8s(
                        self.bytecode.instructions[ip],
                        self.bytecode.instructions[ip + 1],
                    );

                    ip += 2;
                    self.push(self.bytecode.constants[const_idx].clone())?;
                }
                OpCode::POP => {
                    self.pop()?;
                }
                OpCode::ADD => {
                    let right = self.pop_int()?;
                    let left = self.pop_int()?;
                    let val = left + right;
                    self.push(Node::Int(val))?;
                }
                OpCode::SUB => {
                    let right = self.pop_int()?;
                    let left = self.pop_int()?;
                    let val = left - right;
                    self.push(Node::Int(val))?;
                }
                OpCode::PLUS => {
                    let val = self.pop_int()?;
                    self.push(Node::Int(val))?;
                }
                OpCode::MINUS => {
                    let val = self.pop_int()?;
                    self.push(Node::Int(-val))?;
                }
                invalid => return Err(anyhow!("invalid instruction: {invalid:#04X}")),
            }
        }

        Ok(())
    }

    pub fn push(&mut self, node: Node) -> Anyhow<()> {
        self.stack[self.stack_pointer] = node;
        self.stack_pointer += 1;

        if self.stack_pointer >= STACK_SIZE {
            Err(anyhow!("stack overflow"))
        } else {
            Ok(())
        }
    }

    pub fn pop(&mut self) -> Anyhow<&Node> {
        if self.stack_pointer > 0 {
            self.stack_pointer -= 1;
            Ok(&self.stack[self.stack_pointer])
        } else {
            Err(anyhow!("stack underflow"))
        }
    }

    pub fn pop_int(&mut self) -> Anyhow<i32> {
        let node = self.pop()?;

        let Node::Int(value) = node else {
            return Err(anyhow!("expected integer: {node:?}"));
        };

        Ok(*value)
    }

    pub const fn pop_last(&self) -> &Node {
        &self.stack[self.stack_pointer]
    }
}

impl Compile for VirtualMachine {
    type Output = i32;

    fn from_ast(ast: Vec<Node>) -> Anyhow<Self::Output> {
        let bytecode = Interpreter::from_ast(ast)?;

        println!(
            "\nInstructions:\n{:?}\n\nConstants:\n{:#?}",
            bytecode.instructions, bytecode.constants
        );

        let mut vm = Self::new(bytecode);
        vm.run()?;

        match vm.pop_last() {
            Node::Int(n) => Ok(*n),
            _ => Err(anyhow!("expected integer result")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Case {
        input: &'static str,
        expected: i32,
    }

    fn run_test_cases<const N: usize>(tests: [Case; N]) {
        for case in tests {
            match VirtualMachine::from_source(case.input) {
                Ok(result) => assert_eq!(result, case.expected),
                Err(e) => panic!("{e}"),
            }
        }
    }

    #[test]
    fn unary() {
        run_test_cases([
            Case {
                input: "+1",
                expected: 1,
            },
            Case {
                input: "-2",
                expected: -2,
            },
        ]);
    }

    #[test]
    fn binary() {
        run_test_cases([
            Case {
                input: "1 + 2;",
                expected: 3,
            },
            Case {
                input: "1 - 2;",
                expected: -1,
            },
        ]);
    }

    #[test]
    fn compound() {
        run_test_cases([Case {
            input: "(1 + 2) - 1 + (-10);",
            expected: -8,
        }]);
    }
}
