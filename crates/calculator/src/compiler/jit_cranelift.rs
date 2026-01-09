use cranelift::{
    codegen::{ir::BlockArg, write_function},
    prelude::*,
};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{DataDescription, Linkage, Module};

use crate::{
    Anyhow, Compile, anyhow,
    ast::{Node, Operator},
};

// https://github.com/bytecodealliance/cranelift-jit-demo/

pub struct Jit {
    builder_context: FunctionBuilderContext,
    ctx: codegen::Context,
    data_description: DataDescription,
    module: JITModule,
}

impl Compile for Jit {
    type Output = i32;

    fn from_ast(ast: Vec<Node>) -> Anyhow<Self::Output> {
        let mut flag_builder = settings::builder();
        flag_builder.set("use_colocated_libcalls", "false")?;
        flag_builder.set("is_pic", "false")?;
        let isa_builder = cranelift_native::builder().map_err(|e| anyhow!("{e}"))?;
        let isa = isa_builder.finish(settings::Flags::new(flag_builder))?;
        let builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());
        let module = JITModule::new(builder);

        let mut jit = Self {
            builder_context: FunctionBuilderContext::new(),
            ctx: module.make_context(),
            data_description: DataDescription::new(),
            module,
        };

        let code_ptr = jit.compile(&ast)?;
        print!("\nCranelift IR:\n{}", jit.ctx.func);

        unsafe {
            let code_fn = std::mem::transmute::<*const u8, fn() -> i32>(code_ptr);
            let result = Ok(code_fn());
            jit.module.free_memory();
            result
        }
    }
}

impl Jit {
    fn compile(&mut self, ast: &[Node]) -> Anyhow<*const u8> {
        let int = Type::int(32).unwrap();
        self.ctx.func.signature.returns.push(AbiParam::new(int));
        let mut builder = FunctionBuilder::new(&mut self.ctx.func, &mut self.builder_context);
        let entry_block = builder.create_block();
        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);

        let mut t = Translator {
            int,
            builder,
            module: &mut self.module,
        };

        for node in ast {
            let return_value = t.translate_node(node)?;
            t.builder.ins().return_(&[return_value]);
        }

        t.builder.finalize();

        let id =
            self.module
                .declare_function("__main", Linkage::Export, &self.ctx.func.signature)?;

        self.module.define_function(id, &mut self.ctx)?;
        self.module.finalize_definitions()?;
        Ok(self.module.get_finalized_function(id))
    }
}

struct Translator<'a> {
    int: types::Type,
    builder: FunctionBuilder<'a>,
    module: &'a mut JITModule,
}

impl Translator<'_> {
    pub fn translate_node(&mut self, node: &Node) -> Anyhow<Value> {
        match node {
            Node::Int(n) => Ok(self.builder.ins().iconst(self.int, *n as i64)),
            Node::UnaryExpr { op, child } => {
                let child = self.translate_node(child)?;

                match op {
                    Operator::Minus => Ok(self.builder.ins().ineg(child)),
                    Operator::Plus => Ok(child),
                }
            }
            Node::BinaryExpr {
                op,
                left: lhs,
                right: rhs,
            } => {
                let left = self.translate_node(lhs)?;
                let right = self.translate_node(rhs)?;

                match op {
                    Operator::Plus => Ok(self.builder.ins().iadd(left, right)),
                    Operator::Minus => Ok(self.builder.ins().isub(left, right)),
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
