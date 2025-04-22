import functools
import operator
from typing import Optional, Tuple, Iterable, List, Union

import sympy as sp
from sympy.printing.rust import RustCodePrinter

RUST_CODE_PREAMBLE = """
//! Generated source code with the `clogbox` Python package, to use with the `clogbox` project.
#![allow(unused_imports, dead_code)]
use num_traits::{Float, FloatConst};
use az::CastFrom;
"""

DIFFERENTIABLE_CODE_PREAMBLE = RUST_CODE_PREAMBLE + """
use clogbox_math::root_eq::Differentiable;
"""


class ClogboxRustCodePrinter(RustCodePrinter):
    def _print_Pow(self, expr, **kwargs):
        base_code = self.doprint(expr.base)
        exp = expr.exp
        if isinstance(exp, sp.Integer):
            return f"{base_code}.powi({int(exp)})"
        else:
            exp_code = self.doprint(exp)
            return f"{base_code}.powf({exp_code})"

    def _print_Integer(self, expr, **kwargs):
        return f"T::cast_from({float(expr)})"

    def _print_Float(self, expr, **kwargs):
        return f"T::cast_from({float(expr)})"

    def _print_Rational(self, expr):
        p, q = tuple(self._print(sp.Integer(x)) for x in (expr.p, expr.q))
        return f"{p}/{q}"

    def _print_NaN(self, expr, **kwargs):
        return "T::nan()"

    def _print_Exp1(self, expr, _type=False):
        return "T::E()"

    def _print_Pi(self, expr, _type=False):
        return 'T::PI()'

    def _print_Infinity(self, expr, _type=False):
        return 'T::INFINITY()'

    def _print_NegativeInfinity(self, expr, _type=False):
        return 'T::NEG_INFINITY()'


def generate_statements(*exprs: Tuple[str, sp.Expr], printer: Optional[RustCodePrinter] = None) -> Iterable[str]:
    """
    Generate Rust statements from a list of SymPy expressions with their assignments.

    To work properly, the resulting source code must be compiled with the `num-traits` crate and also be within a file
    that also has the contents of the `RUST_CODE_PREAMBLE` constant present.

    :param exprs: Expressions to be turned into statements, with their assignment bindings.
    :param printer: Printer instance to use, or None to create a new instance.
    :return: The generated code, yielding one line at a time (newlines not included).
    """
    if not printer:
        printer = ClogboxRustCodePrinter()
    extra_bindings, return_values = sp.cse([v for _, v in exprs])
    for expr, name in extra_bindings:
        yield f"let {name} = {printer.doprint(expr)};"
    for name, expr in zip((k for k, _ in exprs), return_values):
        yield f"let {name} = {printer.doprint(expr)};"


def generate_function(name: str, *exprs: sp.Expr, printer: Optional[RustCodePrinter] = None) -> Iterable[str]:
    """
    Generate a Rust function from a SymPy expression.

    To work properly, the resulting source code must be compiled with the `num-traits` crate and also be within a file
    that also has the contents of the `RUST_CODE_PREAMBLE` constant present.
    :param exprs: List of expressions. Will be returned as a tuple in-order.
    :param name: Name of the function to generate
    :param printer: Printer instance to use, or None to create a new instance.
    :return: The generated code, yielding one line at a time (newlines not included).
    """
    if not printer:
        printer = ClogboxRustCodePrinter()
    bindings, return_expr = sp.cse(exprs)

    free_args = functools.reduce(operator.or_, (e.free_symbols for e in exprs), set())
    args = ", ".join(f"{name}: T" for name in free_args)
    if len(exprs) == 1:
        rtype = "T"
        return_expr = printer.doprint(*return_expr)
    else:
        rtype = ", ".join("T" for _ in exprs)
        rtype = f"({rtype})"
        return_expr = ", ".join(printer.doprint(e) for e in return_expr)
        return_expr = f"({return_expr})"

    yield f"pub fn {name}<T: Float + FloatConst + CastFrom<f64>>({args}) -> {rtype} {{"
    for line in generate_statements(*((name, expr) for name, expr in bindings), printer=printer):
        yield "    " + line

    yield f"    {return_expr}"
    yield "}"


def generate_functions(*exprs: Tuple[str, sp.Expr], printer: Optional[RustCodePrinter] = None) -> Iterable[str]:
    for name, expr in exprs:
        yield from generate_function(name, expr, printer=printer)
        yield ""


def generate_module(exprs: List[Tuple[str, sp.Expr]], printer: Optional[RustCodePrinter] = None) -> Iterable[str]:
    """
    Generates Rust module code for a list of symbolic expressions.

    :param exprs: List of tuples where each tuple contains a function name (str) and
        its corresponding symbolic expression computed with SymPy.
    :param printer: Optional instance of `RustCodePrinter` used to print symbolic
        expressions as Rust-compatible code. If not provided, a default printer will
        be used.
    :return: Iterable of strings, each representing a line of the generated Rust
        module code.
    """
    yield from RUST_CODE_PREAMBLE.splitlines()
    yield ""
    yield from generate_functions(*exprs, printer=printer)


def generate_differentiable(expr: Union[sp.Expr, sp.Eq], variable: sp.Symbol, struct_name="Equation",
                            printer: Optional[RustCodePrinter] = None) -> Iterable[str]:
    """
    Generate Rust code for a specific equation that is differentiable once about a variable. This function will
    implement the `clogbox_math::root_eq::Differentiable` trait for the generated struct.

    The resulting source code needs the `clogbox_math` and `num-traits` crates. The final module must also have the
    contents of the `DIFFERENTIABLE_CODE_PREAMBLE` constant present.

    :param expr: Expression to implement the trait for.
    :param variable: Variable about which the equation is differentiable.
    :param struct_name: Name of the struct to generate (defaults to "Equation").
    :param printer: Optional instance of `RustCodePrinter` used to print symbolic
        expressions as Rust-compatible code. If not provided, a default printer will
        be used.
    :return: Generated lines of code
    """
    if not printer:
        printer = ClogboxRustCodePrinter()
    if isinstance(expr, sp.Eq):
        expr = expr.rhs - expr.lhs

    diff = sp.diff(expr, variable)

    fields = expr.free_symbols - {variable}

    yield f"pub struct {struct_name}<T> {{"
    for field in fields:
        yield f"    pub {str(field)}: T,"
    yield "}"
    yield ""
    yield f"impl<T: Float + FloatConst + CastFrom<f64>> Differentiable for {struct_name}<T> {{"
    yield f"    type Scalar = T;"
    yield ""
    yield f"    fn eval_with_derivative(&self, {str(variable)}: T) -> (T, T) {{"
    for field in fields:
        yield f"        let {str(field)} = self.{str(field)};"
    for line in generate_statements(("f", expr), ("df", diff), printer=printer):
        yield "        " + line
    yield "        (f, df)"
    yield "    }"
    yield "}"
