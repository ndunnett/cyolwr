#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operator {
    Plus,
    Minus,
}

impl std::fmt::Display for Operator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match &self {
            Self::Plus => write!(f, "+"),
            Self::Minus => write!(f, "-"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    Int(i32),
    UnaryExpr {
        op: Operator,
        child: Box<Self>,
    },
    BinaryExpr {
        op: Operator,
        left: Box<Self>,
        right: Box<Self>,
    },
}

impl std::fmt::Display for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match &self {
            Self::Int(n) => write!(f, "{n}"),
            Self::UnaryExpr { op, child } => write!(f, "{op}{child}"),
            Self::BinaryExpr { op, left, right } => write!(f, "{left} {op} {right}"),
        }
    }
}
