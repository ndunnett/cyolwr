use crate::types::Type;

pub type Program = Vec<TopLevel>;

#[derive(Debug, Clone, PartialEq)]
pub enum TopLevel {
    Class(ClassDef),
    Stmt(Statement),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClassDef {
    pub name: String,
    pub fields: Vec<FieldDef>,
    pub methods: Vec<MethodDef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldDef {
    pub name: String,
    pub ty: Type,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MethodDef {
    pub name: String,
    pub params: Vec<(String, Type)>,
    pub return_type: Type,
    pub body: Vec<Statement>,
}

impl MethodDef {
    pub fn is_constructor(&self) -> bool {
        self.name == "__init__"
    }

    pub fn is_destructor(&self) -> bool {
        self.name == "__del__"
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AssignTarget {
    Var(String),
    Field {
        object: Box<TypedExpr>,
        field: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Function {
        name: String,
        params: Vec<(String, Type)>,
        return_type: Type,
        body: Vec<Self>,
    },
    Return(TypedExpr),
    Assignment {
        target: AssignTarget,
        type_ann: Option<Type>,
        value: TypedExpr,
    },
    Expression(TypedExpr),
    Delete(TypedExpr),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    Int(i64),
    Bool(bool),
    Var(String),
    SelfRef,
    Unary {
        op: UnaryOp,
        expr: Box<TypedExpr>,
    },
    Binary {
        op: BinaryOp,
        left: Box<TypedExpr>,
        right: Box<TypedExpr>,
    },
    Call {
        name: String,
        args: Vec<TypedExpr>,
    },
    MethodCall {
        object: Box<TypedExpr>,
        method: String,
        args: Vec<TypedExpr>,
    },
    FieldAccess {
        object: Box<TypedExpr>,
        field: String,
    },
    New {
        class: String,
        args: Vec<TypedExpr>,
    },
    If {
        cond: Box<TypedExpr>,
        then_branch: Vec<Statement>,
        else_branch: Vec<Statement>,
    },
    While {
        cond: Box<TypedExpr>,
        body: Vec<Statement>,
    },
    Block(Vec<Statement>),
}

impl Expression {
    pub const fn typed(self, ty: Type) -> TypedExpr {
        TypedExpr { expr: self, ty }
    }

    pub const fn untyped(self) -> TypedExpr {
        self.typed(Type::Unknown)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedExpr {
    pub expr: Expression,
    pub ty: Type,
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

impl std::fmt::Display for TopLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Class(class) => write!(f, "class {} {{ ... }}", class.name),
            Self::Stmt(stmt) => write!(f, "{stmt}"),
        }
    }
}

impl std::fmt::Display for ClassDef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "class {} {{", self.name)?;

        for field in &self.fields {
            writeln!(f, "    {}: {}", field.name, field.ty)?;
        }

        for method in &self.methods {
            writeln!(f, "    def {}(...) {{ ... }}", method.name)?;
        }

        write!(f, "}}")
    }
}

impl std::fmt::Display for Statement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Function {
                name,
                params,
                return_type,
                body,
            } => {
                let params = params
                    .iter()
                    .map(|(param, ty)| format!("{param}: {ty}"))
                    .collect::<Vec<_>>()
                    .join(", ");

                write!(f, "def {name}({params}) -> {return_type} {{ ")?;

                for stmt in body {
                    write!(f, "{stmt} ")?;
                }

                write!(f, "}}")
            }
            Self::Return(expr) => write!(f, "return {expr}"),
            Self::Assignment {
                target,
                type_ann,
                value,
            } => {
                let target_str = match target {
                    AssignTarget::Var(name) => name.clone(),
                    AssignTarget::Field { object, field } => format!("{object}.{field}"),
                };

                if let Some(ty) = type_ann {
                    write!(f, "{target_str}: {ty} = {value}")
                } else {
                    write!(f, "{target_str} = {value}")
                }
            }
            Self::Delete(expr) => write!(f, "delete {expr}"),
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
            Self::SelfRef => write!(f, "self"),
            Self::Unary { op, expr } => write!(f, "({op}{expr})"),
            Self::Binary { op, left, right } => write!(f, "({left} {op} {right})"),
            Self::Call { name, args } => {
                let args = args
                    .iter()
                    .map(|arg| arg.expr.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");

                write!(f, "{name}({args})")
            }
            Self::MethodCall {
                object,
                method,
                args,
            } => {
                let args = args
                    .iter()
                    .map(|arg| arg.expr.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");

                write!(f, "{}.{method}({args})", object.expr)
            }
            Self::FieldAccess { object, field } => {
                write!(f, "{}.{field}", object.expr)
            }
            Self::New { class, args } => {
                let args = args
                    .iter()
                    .map(|arg| arg.expr.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");

                write!(f, "new {class}({args})")
            }
            Self::If { cond, .. } => write!(f, "if ({cond}) {{ ... }} else {{ ... }}"),
            Self::While { cond, .. } => write!(f, "while ({cond}) {{ ... }}"),
            Self::Block(_) => write!(f, "{{ ... }}"),
        }
    }
}

impl std::fmt::Display for TypedExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.expr, self.ty)
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
