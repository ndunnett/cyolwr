use inkwell::{
    OptimizationLevel,
    builder::Builder,
    context::Context,
    execution_engine::JitFunction,
    types::IntType,
    values::{AnyValue, IntValue},
};

use crate::{
    Anyhow, Compile, anyhow,
    ast::{Node, Operator},
};

type JitFn = unsafe extern "C" fn() -> i32;

pub struct Jit;

impl Compile for Jit {
    type Output = i32;

    fn from_ast(ast: Vec<Node>) -> Anyhow<Self::Output> {
        let context = Context::create();
        let module = context.create_module("calculator");

        let i32_type = context.i32_type();
        let fn_type = i32_type.fn_type(&[], false);

        let function = module.add_function("jit", fn_type, None);
        let basic_block = context.append_basic_block(function, "entry");

        let builder = context.create_builder();
        builder.position_at_end(basic_block);

        for node in ast {
            let recursive_builder = RecursiveBuilder::new(i32_type, &builder);
            let return_value = recursive_builder.build(&node)?;
            builder.build_return(Some(&return_value))?;
        }

        println!(
            "\nLLVM IR:\n{}",
            function.print_to_string().to_string().trim()
        );

        let execution_engine = module
            .create_jit_execution_engine(OptimizationLevel::None)
            .map_err(|e| anyhow!(e.to_string()))?;

        unsafe {
            let jit_function: JitFunction<JitFn> = execution_engine.get_function("jit")?;
            Ok(jit_function.call())
        }
    }
}

struct RecursiveBuilder<'a> {
    i32_type: IntType<'a>,
    builder: &'a Builder<'a>,
}

impl<'a> RecursiveBuilder<'a> {
    pub const fn new(i32_type: IntType<'a>, builder: &'a Builder) -> Self {
        Self { i32_type, builder }
    }

    pub fn build(&self, ast: &Node) -> Anyhow<IntValue<'a>> {
        match ast {
            Node::Int(n) => Ok(self.i32_type.const_int(*n as u64, true)),
            Node::UnaryExpr { op, child } => {
                let child = self.build(child)?;

                match op {
                    Operator::Minus => Ok(child.const_neg()),
                    Operator::Plus => Ok(child),
                }
            }
            Node::BinaryExpr {
                op,
                left: lhs,
                right: rhs,
            } => {
                let left = self.build(lhs)?;
                let right = self.build(rhs)?;

                match op {
                    Operator::Plus => Ok(self.builder.build_int_add(left, right, "plus_temp")?),
                    Operator::Minus => Ok(self.builder.build_int_sub(left, right, "minus_temp")?),
                }
            }
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
            match Jit::from_source(case.input) {
                Ok(result) => assert_eq!(result, case.expected),
                Err(e) => panic!("{e}"),
            }
        }
    }

    #[test]
    fn basics() {
        run_test_cases([
            Case {
                input: "1",
                expected: 1,
            },
            Case {
                input: "1 + 2",
                expected: 3,
            },
            Case {
                input: "2 + (2 - 1)",
                expected: 3,
            },
            Case {
                input: "(2 + 3) - 1",
                expected: 4,
            },
            Case {
                input: "1 + ((2 + 3) - (2 + 3))",
                expected: 1,
            },
        ]);
    }
}
