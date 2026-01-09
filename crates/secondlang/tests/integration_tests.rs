//! Integration tests for Secondlang
//!
//! These tests verify the full compilation pipeline:
//! parsing -> type checking -> LLVM codegen -> JIT execution

use secondlang::{Anyhow, ast::Program, compile, create_context, optimize, parse, typecheck};

// =============================================================================
// Type Checking Tests
// =============================================================================

fn typecheck_source(source: &str) -> Anyhow<Program> {
    let mut program = parse(source)?;
    typecheck(&mut program)?;
    Ok(optimize(program))
}

fn ir_string(source: &str) -> Anyhow<String> {
    let program = typecheck_source(source)?;
    let ctx = create_context();
    let module = compile(&ctx, &program, "test")?;
    Ok(module.to_string())
}

fn execute(source: &str) -> Anyhow<i64> {
    let program = typecheck_source(source)?;
    let ctx = create_context();
    let module = compile(&ctx, &program, "test")?;
    module.execute()
}

#[test]
fn test_typecheck_literals() {
    assert!(typecheck_source("42").is_ok());
    assert!(typecheck_source("true").is_ok());
    assert!(typecheck_source("false").is_ok());
}

#[test]
fn test_typecheck_arithmetic() {
    assert!(typecheck_source("1 + 2").is_ok());
    assert!(typecheck_source("10 - 5").is_ok());
    assert!(typecheck_source("3 * 4").is_ok());
    assert!(typecheck_source("20 / 4").is_ok());
}

#[test]
fn test_typecheck_comparison() {
    assert!(typecheck_source("1 < 2").is_ok());
    assert!(typecheck_source("1 > 2").is_ok());
    assert!(typecheck_source("1 == 1").is_ok());
    assert!(typecheck_source("1 != 2").is_ok());
}

#[test]
fn test_typecheck_type_error_arithmetic() {
    assert!(typecheck_source("1 + true").is_err());
}

#[test]
fn test_typecheck_typed_function() {
    let source = r"
        def add(a: int, b: int) -> int {
            return a + b
        }
        add(1, 2)
    ";
    assert!(typecheck_source(source).is_ok());
}

#[test]
fn test_typecheck_fibonacci() {
    let source = r"
        def fib(n: int) -> int {
            if (n < 2) {
                return n
            } else {
                return fib(n - 1) + fib(n - 2)
            }
        }
        fib(10)
    ";
    assert!(typecheck_source(source).is_ok());
}

#[test]
fn test_typecheck_wrong_argument_type() {
    let source = r"
        def add(a: int, b: int) -> int {
            return a + b
        }
        add(1, true)
    ";
    assert!(typecheck_source(source).is_err());
}

// =============================================================================
// LLVM IR Generation Tests
// =============================================================================

#[test]
fn test_compile_simple() {
    let source = r"
        def answer() -> int {
            return 42
        }
        answer()
    ";
    let ir = ir_string(source).unwrap();
    assert!(ir.contains("define i64 @answer"));
    assert!(ir.contains("ret i64 42"));
}

#[test]
fn test_compile_add() {
    let source = r"
        def add(a: int, b: int) -> int {
            return a + b
        }
        add(3, 4)
    ";
    let ir = ir_string(source).unwrap();
    assert!(ir.contains("define i64 @add"));
    assert!(ir.contains("add"));
}

#[test]
fn test_compile_fibonacci() {
    let source = r"
        def fib(n: int) -> int {
            if (n < 2) {
                return n
            } else {
                return fib(n - 1) + fib(n - 2)
            }
        }
        fib(10)
    ";
    let ir = ir_string(source).unwrap();
    assert!(ir.contains("define i64 @fib"));
    assert!(ir.contains("call i64 @fib")); // Recursive call
}

// =============================================================================
// JIT Execution Tests
// =============================================================================

#[test]
fn test_jit_simple() {
    let source = r"
        def answer() -> int {
            return 42
        }
        answer()
    ";
    assert_eq!(execute(source).unwrap(), 42);
}

#[test]
fn test_jit_arithmetic() {
    let source = r"
        def compute() -> int {
            return 2 + 3 * 4
        }
        compute()
    ";
    assert_eq!(execute(source).unwrap(), 14);
}

#[test]
fn test_jit_add() {
    let source = r"
        def add(a: int, b: int) -> int {
            return a + b
        }
        add(3, 4)
    ";
    assert_eq!(execute(source).unwrap(), 7);
}

#[test]
fn test_jit_conditional() {
    let source = r"
        def max(a: int, b: int) -> int {
            if (a > b) {
                return a
            } else {
                return b
            }
        }
        max(10, 20)
    ";
    assert_eq!(execute(source).unwrap(), 20);
}

#[test]
fn test_jit_while_loop() {
    let source = r"
        def sum_to(n: int) -> int {
            result: int = 0
            i: int = 1
            while (i <= n) {
                result = result + i
                i = i + 1
            }
            return result
        }
        sum_to(10)
    ";
    assert_eq!(execute(source).unwrap(), 55);
}

#[test]
fn test_jit_factorial_recursive() {
    let source = r"
        def factorial(n: int) -> int {
            if (n <= 1) {
                return 1
            } else {
                return n * factorial(n - 1)
            }
        }
        factorial(5)
    ";
    assert_eq!(execute(source).unwrap(), 120);
}

#[test]
fn test_jit_fibonacci() {
    let source = r"
        def fib(n: int) -> int {
            if (n < 2) {
                return n
            } else {
                return fib(n - 1) + fib(n - 2)
            }
        }
        fib(10)
    ";
    assert_eq!(execute(source).unwrap(), 55);
}

#[test]
fn test_jit_fibonacci_larger() {
    let source = r"
        def fib(n: int) -> int {
            if (n < 2) {
                return n
            } else {
                return fib(n - 1) + fib(n - 2)
            }
        }
        fib(20)
    ";
    assert_eq!(execute(source).unwrap(), 6765);
}

#[test]
fn test_jit_multiple_functions() {
    let source = r"
        def double(x: int) -> int {
            return x * 2
        }
        def quadruple(x: int) -> int {
            return double(double(x))
        }
        quadruple(5)
    ";
    assert_eq!(execute(source).unwrap(), 20);
}

#[test]
fn test_jit_gcd() {
    let source = r"
        def gcd(a: int, b: int) -> int {
            while (b != 0) {
                temp: int = b
                b = a % b
                a = temp
            }
            return a
        }
        gcd(48, 18)
    ";
    assert_eq!(execute(source).unwrap(), 6);
}

#[test]
fn test_jit_power() {
    let source = r"
        def power(base: int, exp: int) -> int {
            if (exp == 0) {
                return 1
            } else {
                return base * power(base, exp - 1)
            }
        }
        power(2, 10)
    ";
    assert_eq!(execute(source).unwrap(), 1024);
}
