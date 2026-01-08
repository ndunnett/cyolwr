//! End-to-End Integration Tests for Firstlang
//!
//! These tests demonstrate the full capabilities of Firstlang
//! and serve as examples for the book.

use firstlang::{Value, run};

/// Construct a test case with a given source code and expected result.
/// The `Err` case must contain the expected string in the display formatted error, case insensitive.
enum Case {
    Ok {
        source: &'static str,
        expected: Value,
    },
    Err {
        source: &'static str,
        expected: &'static str,
    },
}

/// Run assertions for given array of test cases.
fn test_cases<const N: usize>(cases: [Case; N]) {
    for case in cases {
        match case {
            Case::Ok { source, expected } => match run(source) {
                Ok(value) => assert_eq!(expected, value, "expected value left, actual value right"),
                Err(e) => panic!("Unexpected error:\n{e}"),
            },
            Case::Err { source, expected } => match run(source) {
                Err(e) => {
                    assert!(
                        e.to_string()
                            .to_lowercase()
                            .contains(&expected.to_lowercase())
                    );
                }
                Ok(value) => panic!("Unexpected value: {value}"),
            },
        }
    }
}

// =============================================================================
// Basic Expressions
// =============================================================================

#[test]
fn test_integer_literals() {
    test_cases([
        Case::Ok {
            source: "0",
            expected: Value::Int(0),
        },
        Case::Ok {
            source: "42",
            expected: Value::Int(42),
        },
        Case::Ok {
            source: "999999",
            expected: Value::Int(999_999),
        },
    ]);
}

#[test]
fn test_boolean_literals() {
    test_cases([
        Case::Ok {
            source: "true",
            expected: Value::Bool(true),
        },
        Case::Ok {
            source: "false",
            expected: Value::Bool(false),
        },
    ]);
}

#[test]
fn test_arithmetic_operators() {
    test_cases([
        Case::Ok {
            source: "2 + 3",
            expected: Value::Int(5),
        },
        Case::Ok {
            source: "10 - 4",
            expected: Value::Int(6),
        },
        Case::Ok {
            source: "6 * 7",
            expected: Value::Int(42),
        },
        Case::Ok {
            source: "20 / 4",
            expected: Value::Int(5),
        },
        Case::Ok {
            source: "17 % 5",
            expected: Value::Int(2),
        },
    ]);
}

#[test]
fn test_operator_precedence() {
    test_cases([
        // Multiplication before addition
        Case::Ok {
            source: "2 + 3 * 4",
            expected: Value::Int(14),
        },
        // Parentheses override precedence
        Case::Ok {
            source: "(2 + 3) * 4",
            expected: Value::Int(20),
        },
        // Left-to-right for same precedence
        Case::Ok {
            source: "10 - 4 - 2",
            expected: Value::Int(4),
        },
    ]);
}

#[test]
fn test_unary_operators() {
    test_cases([
        Case::Ok {
            source: "-5",
            expected: Value::Int(-5),
        },
        Case::Ok {
            source: "--5",
            expected: Value::Int(5),
        },
        Case::Ok {
            source: "!true",
            expected: Value::Bool(false),
        },
        Case::Ok {
            source: "!false",
            expected: Value::Bool(true),
        },
        Case::Ok {
            source: "!!true",
            expected: Value::Bool(true),
        },
    ]);
}

#[test]
fn test_comparison_operators() {
    test_cases([
        Case::Ok {
            source: "1 < 2",
            expected: Value::Bool(true),
        },
        Case::Ok {
            source: "2 < 1",
            expected: Value::Bool(false),
        },
        Case::Ok {
            source: "2 > 1",
            expected: Value::Bool(true),
        },
        Case::Ok {
            source: "1 > 2",
            expected: Value::Bool(false),
        },
        Case::Ok {
            source: "1 <= 1",
            expected: Value::Bool(true),
        },
        Case::Ok {
            source: "1 <= 2",
            expected: Value::Bool(true),
        },
        Case::Ok {
            source: "2 >= 2",
            expected: Value::Bool(true),
        },
        Case::Ok {
            source: "1 >= 2",
            expected: Value::Bool(false),
        },
        Case::Ok {
            source: "42 == 42",
            expected: Value::Bool(true),
        },
        Case::Ok {
            source: "42 == 43",
            expected: Value::Bool(false),
        },
        Case::Ok {
            source: "42 != 43",
            expected: Value::Bool(true),
        },
        Case::Ok {
            source: "true == true",
            expected: Value::Bool(true),
        },
        Case::Ok {
            source: "true != false",
            expected: Value::Bool(true),
        },
    ]);
}

// =============================================================================
// Variables
// =============================================================================

#[test]
fn test_variable_assignment() {
    test_cases([Case::Ok {
        source: "x = 42\nx",
        expected: Value::Int(42),
    }]);
}

#[test]
fn test_variable_reassignment() {
    test_cases([Case::Ok {
        source: r"
            x = 10
            x = 20
            x
        ",
        expected: Value::Int(20),
    }]);
}

#[test]
fn test_variable_in_expressions() {
    test_cases([Case::Ok {
        source: r"
            a = 10
            b = 20
            a + b * 2
        ",
        expected: Value::Int(50),
    }]);
}

#[test]
fn test_compound_assignment_pattern() {
    test_cases([Case::Ok {
        source: r"
            x = 1
            x = x + 1
            x = x * 2
            x
        ",
        expected: Value::Int(4),
    }]);
}

// =============================================================================
// Functions
// =============================================================================

#[test]
fn test_function_no_params() {
    test_cases([Case::Ok {
        source: r"
            def answer() {
                return 42
            }
            answer()
        ",
        expected: Value::Int(42),
    }]);
}

#[test]
fn test_function_with_parameters() {
    test_cases([Case::Ok {
        source: r"
            def add(a, b) {
                return a + b
            }
            add(3, 4)
        ",
        expected: Value::Int(7),
    }]);
}

#[test]
fn test_function_with_local_variables() {
    test_cases([Case::Ok {
        source: r"
            def compute(x) {
                doubled = x * 2
                tripled = x * 3
                return doubled + tripled
            }
            compute(10)
        ",
        expected: Value::Int(50),
    }]);
}

#[test]
fn test_multiple_functions() {
    test_cases([Case::Ok {
        source: r"
            def square(x) {
                return x * x
            }
            def cube(x) {
                return x * x * x
            }
            square(3) + cube(2)
        ",
        expected: Value::Int(17),
    }]);
}

#[test]
fn test_function_calling_function() {
    test_cases([Case::Ok {
        source: r"
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

// =============================================================================
// Control Flow - Conditionals
// =============================================================================

#[test]
fn test_if_true_branch() {
    test_cases([Case::Ok {
        source: r"
            if (true) {
                42
            } else {
                0
            }
        ",
        expected: Value::Int(42),
    }]);
}

#[test]
fn test_if_false_branch() {
    test_cases([Case::Ok {
        source: r"
            if (false) {
                42
            } else {
                0
            }
        ",
        expected: Value::Int(0),
    }]);
}

#[test]
fn test_if_with_comparison() {
    test_cases([Case::Ok {
        source: r"
            x = 10
            if (x > 5) {
                1
            } else {
                0
            }
        ",
        expected: Value::Int(1),
    }]);
}

#[test]
fn test_nested_conditionals() {
    test_cases([Case::Ok {
        source: r"
            def classify(n) {
                if (n < 0) {
                    return -1
                } else {
                    if (n == 0) {
                        return 0
                    } else {
                        return 1
                    }
                }
            }
            classify(-5) + classify(0) + classify(10)
        ",
        expected: Value::Int(0),
    }]);
}

#[test]
fn test_max_function() {
    test_cases([Case::Ok {
        source: r"
            def max(a, b) {
                if (a > b) {
                    return a
                } else {
                    return b
                }
            }
            max(10, 20)
        ",
        expected: Value::Int(20),
    }]);
}

#[test]
fn test_abs_function() {
    test_cases([Case::Ok {
        source: r"
            def abs(x) {
                if (x < 0) {
                    return -x
                } else {
                    return x
                }
            }
            abs(-42)
        ",
        expected: Value::Int(42),
    }]);
}

// =============================================================================
// Control Flow - Loops
// =============================================================================

#[test]
fn test_while_loop() {
    test_cases([Case::Ok {
        source: r"
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
fn test_while_loop_sum() {
    test_cases([Case::Ok {
        source: r"
            sum = 0
            i = 1
            while (i <= 10) {
                sum = sum + i
                i = i + 1
            }
            sum
        ",
        expected: Value::Int(55),
    }]);
}

#[test]
fn test_while_never_executes() {
    test_cases([Case::Ok {
        source: r"
            x = 0
            while (false) {
                x = x + 1
            }
            x
        ",
        expected: Value::Int(0),
    }]);
}

#[test]
fn test_countdown() {
    test_cases([Case::Ok {
        source: r"
            def countdown(n) {
                count = 0
                while (n > 0) {
                    count = count + 1
                    n = n - 1
                }
                return count
            }
            countdown(10)
        ",
        expected: Value::Int(10),
    }]);
}

// =============================================================================
// Recursion
// =============================================================================

#[test]
fn test_factorial_recursive() {
    test_cases([Case::Ok {
        source: r"
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
fn test_factorial_iterative() {
    test_cases([Case::Ok {
        source: r"
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
fn test_fibonacci_recursive() {
    test_cases([Case::Ok {
        source: r"
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
    test_cases([Case::Ok {
        source: r"
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
fn test_fibonacci_larger() {
    test_cases([Case::Ok {
        source: r"
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
            fib(20)
        ",
        expected: Value::Int(6765),
    }]);
}

#[test]
fn test_sum_to_n_recursive() {
    test_cases([Case::Ok {
        source: r"
            def sum_to(n) {
                if (n <= 0) {
                    return 0
                } else {
                    return n + sum_to(n - 1)
                }
            }
            sum_to(10)
        ",
        expected: Value::Int(55),
    }]);
}

#[test]
fn test_power_function() {
    test_cases([Case::Ok {
        source: r"
            def power(base, exp) {
                if (exp == 0) {
                    return 1
                } else {
                    return base * power(base, exp - 1)
                }
            }
            power(2, 10)
        ",
        expected: Value::Int(1024),
    }]);
}

#[test]
fn test_mutual_recursion_even_odd() {
    test_cases([Case::Ok {
        source: r"
            def is_even(n) {
                if (n == 0) {
                    return true
                } else {
                    return is_odd(n - 1)
                }
            }
            def is_odd(n) {
                if (n == 0) {
                    return false
                } else {
                    return is_even(n - 1)
                }
            }
            is_even(10)
        ",
        expected: Value::Bool(true),
    }]);
}

// =============================================================================
// Complex Programs
// =============================================================================

#[test]
fn test_gcd_euclidean() {
    test_cases([Case::Ok {
        source: r"
            def gcd(a, b) {
                while (b != 0) {
                    temp = b
                    b = a % b
                    a = temp
                }
                return a
            }
            gcd(48, 18)
        ",
        expected: Value::Int(6),
    }]);
}

#[test]
fn test_is_prime() {
    test_cases([Case::Ok {
        source: r"
            def is_prime(n) {
                if (n < 2) {
                    return false
                } else {
                    result = true
                    i = 2
                    while (i * i <= n) {
                        if (n % i == 0) {
                            result = false
                        } else {
                            result = result
                        }
                        i = i + 1
                    }
                    return result
                }
            }
            is_prime(17)
        ",
        expected: Value::Bool(true),
    }]);
}

#[test]
fn test_is_not_prime() {
    test_cases([Case::Ok {
        source: r"
            def is_prime(n) {
                if (n < 2) {
                    return false
                } else {
                    result = true
                    i = 2
                    while (i * i <= n) {
                        if (n % i == 0) {
                            result = false
                        } else {
                            result = result
                        }
                        i = i + 1
                    }
                    return result
                }
            }
            is_prime(15)
        ",
        expected: Value::Bool(false),
    }]);
}

#[test]
fn test_collatz_steps() {
    // Count steps to reach 1 in Collatz sequence
    test_cases([Case::Ok {
        source: r"
        def collatz_steps(n) {
            steps = 0
            while (n != 1) {
                if (n % 2 == 0) {
                    n = n / 2
                } else {
                    n = n * 3 + 1
                }
                steps = steps + 1
            }
            return steps
        }
        collatz_steps(27)
        ",
        expected: Value::Int(111),
    }]);
}

// =============================================================================
// Error Cases
// =============================================================================

#[test]
fn test_undefined_variable_error() {
    test_cases([Case::Err {
        source: "x",
        expected: "Undefined variable",
    }]);
}

#[test]
fn test_undefined_function_error() {
    test_cases([Case::Err {
        source: "foo()",
        expected: "Undefined",
    }]);
}

#[test]
fn test_division_by_zero_error() {
    test_cases([Case::Err {
        source: "10 / 0",
        expected: "Division by zero",
    }]);
}

#[test]
fn test_wrong_argument_count_error() {
    test_cases([Case::Err {
        source: r"
            def add(a, b) {
                return a + b
            }
            add(1)
        ",
        expected: "expects",
    }]);
}

#[test]
fn test_type_error_in_conditional() {
    test_cases([Case::Err {
        source: r"
            if (42) {
                1
            } else {
                0
            }
        ",
        expected: "boolean",
    }]);
}
