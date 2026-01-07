#[derive(Debug, Copy, Clone)]
pub enum OpCode {
    Constant(u16), // pointer to constant table
    Pop,           // pop is needed for execution
    Add,
    Sub,
    Plus,
    Minus,
}

impl OpCode {
    pub const CONSTANT: u8 = 0x01;
    pub const POP: u8 = 0x02;
    pub const ADD: u8 = 0x03;
    pub const SUB: u8 = 0x04;
    pub const PLUS: u8 = 0x0A;
    pub const MINUS: u8 = 0x0B;

    pub fn make_op(self) -> Vec<u8> {
        match self {
            Self::Constant(arg) => vec![Self::CONSTANT, (arg >> 8) as u8, arg as u8], // split u16 arg into 2 bytes
            Self::Pop => vec![Self::POP],
            Self::Add => vec![Self::ADD],
            Self::Sub => vec![Self::SUB],
            Self::Plus => vec![Self::PLUS],
            Self::Minus => vec![Self::MINUS],
        }
    }
}

pub const fn combine_u8s(a: u8, b: u8) -> usize {
    ((a as usize) << 8) | b as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn make_op_constant() {
        assert_eq!(vec![0x01, 255, 254], OpCode::Constant(65534).make_op());
    }

    #[test]
    fn make_op_pop() {
        assert_eq!(vec![0x02], OpCode::Pop.make_op());
    }

    #[test]
    fn make_op_add() {
        assert_eq!(vec![0x03], OpCode::Add.make_op());
    }
}
