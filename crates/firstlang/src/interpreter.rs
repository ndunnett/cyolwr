use std::collections::HashMap;

use crate::{
    Anyhow, anyhow,
    ast::{BinaryOp, Expression, Program, Statement, UnaryOp},
};

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Bool(bool),
    Function {
        params: Vec<String>,
        body: Vec<Statement>,
    },
    Unit,
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Int(n) => write!(f, "{n}"),
            Self::Bool(b) => write!(f, "{b}"),
            Self::Function { params, .. } => write!(f, "<function({})>", params.join(", ")),
            Self::Unit => write!(f, "()"),
        }
    }
}

#[derive(Debug, Clone)]
struct Frame {
    locals: HashMap<String, Value>,
}

impl Frame {
    fn new() -> Self {
        Self {
            locals: HashMap::new(),
        }
    }
}

enum ControlFlow {
    Continue(Value),
    Return(Value),
}

pub struct Interpreter {
    globals: HashMap<String, Value>,
    call_stack: Vec<Frame>,
}

impl Interpreter {
    pub fn new() -> Self {
        Self {
            globals: HashMap::new(),
            call_stack: vec![Frame::new()],
        }
    }

    pub fn run(&mut self, program: &Program) -> Anyhow<Value> {
        let mut result = Value::Unit;

        for statement in program {
            match self.statement(statement)? {
                ControlFlow::Continue(v) => result = v,
                ControlFlow::Return(v) => return Ok(v),
            }
        }

        Ok(result)
    }

    fn lookup(&self, name: &str) -> Anyhow<Value> {
        if let Some(val) = self.current_frame().locals.get(name) {
            return Ok(val.clone());
        }

        if let Some(val) = self.globals.get(name) {
            return Ok(val.clone());
        }

        Err(anyhow!("undefined variable: {name}"))
    }

    fn current_frame(&self) -> &Frame {
        let last = self.call_stack.len() - 1;
        &self.call_stack[last]
    }

    fn current_frame_mut(&mut self) -> &mut Frame {
        let last = self.call_stack.len() - 1;
        &mut self.call_stack[last]
    }

    fn statement(&mut self, statement: &Statement) -> Anyhow<ControlFlow> {
        match statement {
            Statement::Function { name, params, body } => {
                self.globals.insert(
                    name.clone(),
                    Value::Function {
                        params: params.clone(),
                        body: body.clone(),
                    },
                );

                Ok(ControlFlow::Continue(Value::Unit))
            }
            Statement::Return(expression) => Ok(ControlFlow::Return(self.expression(expression)?)),
            Statement::Assignment { name, value } => {
                let value = self.expression(value)?;
                self.current_frame_mut().locals.insert(name.clone(), value);
                Ok(ControlFlow::Continue(Value::Unit))
            }
            Statement::Expression(expression) => {
                Ok(ControlFlow::Continue(self.expression(expression)?))
            }
        }
    }

    fn expression(&mut self, expression: &Expression) -> Anyhow<Value> {
        match expression {
            Expression::Int(n) => Ok(Value::Int(*n)),
            Expression::Bool(b) => Ok(Value::Bool(*b)),
            Expression::Var(name) => self.lookup(name),
            Expression::Unary { op, expr } => Self::unary(*op, self.expression(expr)?),
            Expression::Binary { op, left, right } => {
                Self::binary(*op, self.expression(left)?, self.expression(right)?)
            }

            Expression::Call { name, args } => {
                let Value::Function { params, body } = self.lookup(name)? else {
                    return Err(anyhow!("{name} is not a function"));
                };

                if params.len() != args.len() {
                    return Err(anyhow!(
                        "function {} expects {} arguments, got {}",
                        name,
                        params.len(),
                        args.len()
                    ));
                }

                let args = args
                    .iter()
                    .map(|a| self.expression(a))
                    .collect::<Anyhow<Vec<_>>>()?;

                let mut frame = Frame::new();

                for (param, arg) in params.iter().zip(args) {
                    frame.locals.insert(param.clone(), arg);
                }

                self.call_stack.push(frame);
                let mut result = Value::Unit;

                for statement in &body {
                    match self.statement(statement)? {
                        ControlFlow::Continue(v) => result = v,
                        ControlFlow::Return(v) => {
                            result = v;
                            break;
                        }
                    }
                }

                self.call_stack.pop();
                Ok(result)
            }

            Expression::If {
                cond,
                then_branch,
                else_branch,
            } => {
                let Value::Bool(b) = self.expression(cond)? else {
                    return Err(anyhow!("condition must be a boolean expression: {cond:?}"));
                };

                let branch = if b { then_branch } else { else_branch };
                let mut result = Value::Unit;

                for statement in branch {
                    match self.statement(statement)? {
                        ControlFlow::Continue(v) => result = v,
                        ControlFlow::Return(v) => {
                            result = v;
                            break;
                        }
                    }
                }

                Ok(result)
            }

            Expression::While { cond, body } => loop {
                let Value::Bool(b) = self.expression(cond)? else {
                    return Err(anyhow!("condition must be a boolean expression: {cond:?}"));
                };

                if !b {
                    return Ok(Value::Unit);
                }

                for statement in body {
                    if let ControlFlow::Return(v) = self.statement(statement)? {
                        return Ok(v);
                    }
                }
            },

            Expression::Block(statements) => {
                let mut result = Value::Unit;

                for stmt in statements {
                    match self.statement(stmt)? {
                        ControlFlow::Continue(v) => result = v,
                        ControlFlow::Return(v) => return Ok(v),
                    }
                }

                Ok(result)
            }
        }
    }

    fn binary(op: BinaryOp, left: Value, right: Value) -> Anyhow<Value> {
        match (op, left, right) {
            // Arithmetic operations (integers only)
            (BinaryOp::Add, Value::Int(a), Value::Int(b)) => Ok(Value::Int(a + b)),
            (BinaryOp::Subtract, Value::Int(a), Value::Int(b)) => Ok(Value::Int(a - b)),
            (BinaryOp::Multiply, Value::Int(a), Value::Int(b)) => Ok(Value::Int(a * b)),
            (BinaryOp::Divide, Value::Int(a), Value::Int(b)) => {
                if b == 0 {
                    Err(anyhow!("division by zero"))
                } else {
                    Ok(Value::Int(a / b))
                }
            }
            (BinaryOp::Modulo, Value::Int(a), Value::Int(b)) => {
                if b == 0 {
                    Err(anyhow!("modulo by zero"))
                } else {
                    Ok(Value::Int(a % b))
                }
            }

            // Comparison operations (integers)
            (BinaryOp::LessThan, Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a < b)),
            (BinaryOp::GreaterThan, Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a > b)),
            (BinaryOp::LessEqual, Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a <= b)),
            (BinaryOp::GreaterEqual, Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a >= b)),
            (BinaryOp::Equal, Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a == b)),
            (BinaryOp::NotEqual, Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a != b)),

            // Boolean equality
            (BinaryOp::Equal, Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(a == b)),
            (BinaryOp::NotEqual, Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(a != b)),

            // Type mismatch
            (op, a, b) => Err(anyhow!(
                "type mismatch: cannot apply {op:?} to {a:?} and {b:?}"
            )),
        }
    }

    fn unary(op: UnaryOp, operand: Value) -> Anyhow<Value> {
        match (op, operand) {
            (UnaryOp::Negative, Value::Int(n)) => Ok(Value::Int(-n)),
            (UnaryOp::Not, Value::Bool(b)) => Ok(Value::Bool(!b)),
            (op, operand) => Err(anyhow!("type mismatch: cannot apply {op:?} to {operand:?}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Case {
        input: &'static str,
        expected: Value,
    }

    fn run(source: &str) -> Anyhow<Value> {
        let program = crate::parser::parse(source)?;
        Interpreter::new().run(&program)
    }

    fn test_cases<const N: usize>(tests: [Case; N]) {
        for case in tests {
            match run(case.input) {
                Ok(value) => assert_eq!(case.expected, value, "expected left, actual right"),
                Err(e) => panic!("Unexpected error:\n{e}"),
            }
        }
    }

    #[test]
    fn test_literal() {
        test_cases([
            Case {
                input: "42",
                expected: Value::Int(42),
            },
            Case {
                input: "true",
                expected: Value::Bool(true),
            },
            Case {
                input: "false",
                expected: Value::Bool(false),
            },
        ]);
    }

    #[test]
    fn test_arithmetic() {
        test_cases([
            Case {
                input: "1 + 2",
                expected: Value::Int(3),
            },
            Case {
                input: "10 - 3",
                expected: Value::Int(7),
            },
            Case {
                input: "4 * 5",
                expected: Value::Int(20),
            },
            Case {
                input: "15 / 3",
                expected: Value::Int(5),
            },
            Case {
                input: "17 % 5",
                expected: Value::Int(2),
            },
        ]);
    }

    #[test]
    fn test_comparison() {
        test_cases([
            Case {
                input: "1 < 2",
                expected: Value::Bool(true),
            },
            Case {
                input: "2 > 1",
                expected: Value::Bool(true),
            },
            Case {
                input: "1 == 1",
                expected: Value::Bool(true),
            },
            Case {
                input: "1 != 2",
                expected: Value::Bool(true),
            },
        ]);
    }

    #[test]
    fn test_variables() {
        test_cases([Case {
            input: "x = 42\nx",
            expected: Value::Int(42),
        }]);
    }

    #[test]
    fn test_function() {
        test_cases([Case {
            input: r"
                def add(a, b) {
                    return a + b
                }
                add(3, 4)
            ",
            expected: Value::Int(7),
        }]);
    }

    #[test]
    fn test_conditional() {
        test_cases([
            Case {
                input: r"
                    if (true) {
                        42
                    } else {
                        0
                    }
                ",
                expected: Value::Int(42),
            },
            Case {
                input: r"
                    if (false) {
                        42
                    } else {
                        0
                    }
                ",
                expected: Value::Int(0),
            },
        ]);
    }

    #[test]
    fn test_while_loop() {
        test_cases([Case {
            input: r"
                x = 0
                while (x < 5) {
                    x = x + 1
                }
                x
            ",
            expected: Value::Int(5),
        }]);
    }

    #[test]
    fn test_factorial_iterative() {
        test_cases([Case {
            input: r"
                def factorial(n) {
                    result = 1
                    while (n > 1) {
                        result = result * n
                        n = n - 1
                    }
                    return result
                }
                factorial(5)
            ",
            expected: Value::Int(120),
        }]);
    }

    #[test]
    fn test_factorial_recursive() {
        test_cases([Case {
            input: r"
                def factorial(n) {
                    if (n <= 1) {
                        return 1
                    } else {
                        return n * factorial(n - 1)
                    }
                }
                factorial(5)
            ",
            expected: Value::Int(120),
        }]);
    }

    #[test]
    fn test_fibonacci_recursive() {
        test_cases([Case {
            input: r"
                def fib(n) {
                    if (n < 2) {
                        return n
                    } else {
                        return fib(n - 1) + fib(n - 2)
                    }
                }
                fib(10)
            ",
            expected: Value::Int(55),
        }]);
    }

    #[test]
    fn test_fibonacci_iterative() {
        test_cases([Case {
            input: r"
                def fib(n) {
                    if (n < 2) {
                        return n
                    } else {
                        a = 0
                        b = 1
                        i = 2
                        while (i <= n) {
                            temp = a + b
                            a = b
                            b = temp
                            i = i + 1
                        }
                        return b
                    }
                }
                fib(10)
            ",
            expected: Value::Int(55),
        }]);
    }

    #[test]
    fn test_nested_calls() {
        test_cases([Case {
            input: r"
                def double(x) {
                    return x * 2
                }
                def quadruple(x) {
                    return double(double(x))
                }
                quadruple(5)
            ",
            expected: Value::Int(20),
        }]);
    }
}
