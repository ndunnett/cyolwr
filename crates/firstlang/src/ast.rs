pub type Program = Vec<Statement>;

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Function {
        name: String,
        params: Vec<String>,
        body: Vec<Self>,
    },
    Return(Expression),
    Assignment {
        name: String,
        value: Expression,
    },
    Expression(Expression),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    Int(i64),
    Bool(bool),
    Var(String),
    Unary {
        op: UnaryOp,
        expr: Box<Self>,
    },
    Binary {
        op: BinaryOp,
        left: Box<Self>,
        right: Box<Self>,
    },
    Call {
        name: String,
        args: Vec<Self>,
    },
    If {
        cond: Box<Self>,
        then_branch: Vec<Statement>,
        else_branch: Vec<Statement>,
    },
    While {
        cond: Box<Self>,
        body: Vec<Statement>,
    },
    Block(Vec<Statement>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Negative,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
    LessThan,
    GreaterThan,
    LessEqual,
    GreaterEqual,
    Equal,
    NotEqual,
}

impl std::fmt::Display for Statement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Function { name, params, body } => {
                write!(f, "def {}({}) {{ ", name, params.join(", "))?;

                for stmt in body {
                    write!(f, "{stmt} ")?;
                }

                write!(f, "}}")
            }
            Self::Return(expr) => write!(f, "return {expr}"),
            Self::Assignment { name, value } => write!(f, "{name} = {value}"),
            Self::Expression(expr) => write!(f, "{expr}"),
        }
    }
}

impl std::fmt::Display for Expression {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Int(n) => write!(f, "{n}"),
            Self::Bool(b) => write!(f, "{b}"),
            Self::Var(name) => write!(f, "{name}"),
            Self::Unary { op, expr } => write!(f, "({op}{expr})"),
            Self::Binary { op, left, right } => write!(f, "({left} {op} {right})"),
            Self::Call { name, args } => {
                let args = args
                    .iter()
                    .map(Self::to_string)
                    .collect::<Vec<_>>()
                    .join(", ");

                write!(f, "{name}({args})")
            }
            Self::If { cond, .. } => write!(f, "if ({cond}) {{ ... }} else {{ ... }}"),
            Self::While { cond, .. } => write!(f, "while ({cond}) {{ ... }}"),
            Self::Block(_) => write!(f, "{{ ... }}"),
        }
    }
}

impl std::fmt::Display for UnaryOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Negative => write!(f, "-"),
            Self::Not => write!(f, "!"),
        }
    }
}

impl std::fmt::Display for BinaryOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Add => write!(f, "+"),
            Self::Subtract => write!(f, "-"),
            Self::Multiply => write!(f, "*"),
            Self::Divide => write!(f, "/"),
            Self::Modulo => write!(f, "%"),
            Self::LessThan => write!(f, "<"),
            Self::GreaterThan => write!(f, ">"),
            Self::LessEqual => write!(f, "<="),
            Self::GreaterEqual => write!(f, ">="),
            Self::Equal => write!(f, "=="),
            Self::NotEqual => write!(f, "!="),
        }
    }
}
