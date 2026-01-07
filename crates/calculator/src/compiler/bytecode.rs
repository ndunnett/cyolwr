use crate::{
    Anyhow, Compile, anyhow,
    ast::{Node, Operator},
    compiler::opcode::OpCode,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bytecode {
    pub instructions: Vec<u8>,
    pub constants: Vec<Node>,
}

impl Bytecode {
    const fn new() -> Self {
        Self {
            instructions: Vec::new(),
            constants: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub struct Interpreter {
    bytecode: Bytecode,
}

impl Compile for Interpreter {
    type Output = Bytecode;

    fn from_ast(ast: Vec<Node>) -> Anyhow<Self::Output> {
        let mut interpreter = Self {
            bytecode: Bytecode::new(),
        };

        for node in ast {
            println!("compiling node {node:?}");
            interpreter.interpret_node(node)?;
            // pop one element from the stack after
            // each expression statement to clean up
            interpreter.add_instruction(OpCode::Pop)?;
        }

        Ok(interpreter.bytecode)
    }
}

impl Interpreter {
    fn add_constant(&mut self, node: Node) -> Anyhow<u16> {
        let Ok(index) = u16::try_from(self.bytecode.constants.len()) else {
            return Err(anyhow!("exceeded constants pool size"));
        };

        self.bytecode.constants.push(node);
        Ok(index)
    }

    fn add_instruction(&mut self, op_code: OpCode) -> Anyhow<u16> {
        let Ok(index) = u16::try_from(self.bytecode.instructions.len()) else {
            return Err(anyhow!("exceeded instruction pool size"));
        };

        self.bytecode.instructions.extend(op_code.make_op());

        println!(
            "added instructions {:?} from opcode {:?}",
            self.bytecode.instructions, op_code
        );

        Ok(index)
    }

    fn interpret_node(&mut self, node: Node) -> Anyhow<()> {
        match node {
            Node::Int(num) => {
                let const_index = self.add_constant(Node::Int(num))?;
                self.add_instruction(OpCode::Constant(const_index))?;
            }
            Node::UnaryExpr { op, child } => {
                self.interpret_node(*child)?;

                match op {
                    Operator::Plus => self.add_instruction(OpCode::Plus)?,
                    Operator::Minus => self.add_instruction(OpCode::Minus)?,
                };
            }
            Node::BinaryExpr { op, left, right } => {
                self.interpret_node(*left)?;
                self.interpret_node(*right)?;

                match op {
                    Operator::Plus => self.add_instruction(OpCode::Add)?,
                    Operator::Minus => self.add_instruction(OpCode::Sub)?,
                };
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn infix_template(infix_str: &str, op_code: OpCode) {
        let input = format!("1 {infix_str} 2;");

        match Interpreter::from_source(&input) {
            Ok(bytecode) => {
                let expected_instructions = vec![
                    OpCode::Constant(0),
                    OpCode::Constant(1),
                    op_code,
                    OpCode::Pop,
                ]
                .into_iter()
                .flat_map(OpCode::make_op)
                .collect();

                assert_eq!(
                    Bytecode {
                        instructions: expected_instructions,
                        constants: vec![Node::Int(1), Node::Int(2)]
                    },
                    bytecode
                );
            }
            Err(e) => panic!("{e}"),
        }
    }

    #[test]
    fn basics() {
        infix_template("+", OpCode::Add);
        infix_template("-", OpCode::Sub);
    }
}
