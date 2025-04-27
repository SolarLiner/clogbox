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

DIFFERENTIABLE_CODE_PREAMBLE = (
    RUST_CODE_PREAMBLE
    + """
use clogbox_math::root_eq;
use nalgebra as na;
"""
)


class ClogboxRustCodePrinter(RustCodePrinter):
    def _print_Pow(self, expr):
        base_code = self._print(expr.base)
        exp = expr.exp
        if isinstance(exp, sp.Integer):
            return f"{base_code}.powi({int(exp)})"
        else:
            return super()._print_Pow(expr)

    def _print_Integer(self, expr, _type=False):
        return f"T::cast_from({float(expr)})"

    def _print_Float(self, expr, **kwargs):
        return f"T::cast_from({float(expr)})"

    def _print_Rational(self, expr):
        p, q = tuple(self._print(sp.Integer(x)) for x in (expr.p, expr.q))
        return f"{p}/{q}"

    def _print_MatrixBase(self, mat: sp.MatrixBase):
        values = ", ".join(self._print(x) for x in mat)
        cdim = f"na::Const<{mat.cols}>"
        if mat.rows == 1:
            mtype = f"na::OVector::<T, {cdim}>"
        else:
            rdim = f"na::Const<{mat.rows}>"
            mtype = f"na::OMatrix::<T, {rdim}, {cdim}>"
        return f"{mtype}::new({values})"

    def _print_NaN(self, expr, _type=False):
        return "T::nan()"

    def _print_Exp1(self, expr, _type=False):
        return "T::E()"

    def _print_Pi(self, expr, _type=False):
        return "T::PI()"

    def _print_Infinity(self, expr, _type=False):
        return "T::INFINITY()"

    def _print_NegativeInfinity(self, expr, _type=False):
        return "T::NEG_INFINITY()"


def get_type(e: sp.Basic):
    """
    Get the Rust type of a sympy expression
    :param e: Sympy expression
    :return: Rust type representation
    """
    match e:
        case sp.MatrixBase(cols=c, rows=r):
            return f"na::OMatrix<T, na::Const<{r}>, na::Const<{c}>>"
        case _:
            return "T"


def generate_statements(
    *exprs: Tuple[str, sp.Expr], printer: Optional[RustCodePrinter] = None
) -> Iterable[str]:
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
    for name, expr in extra_bindings:
        yield f"let {name} = {printer.doprint(expr)};"
    for name, expr in zip((k for k, _ in exprs), return_values):
        yield f"let {name} = {printer.doprint(expr)};"


def generate_function(
    name: str, *exprs: sp.Expr, printer: Optional[RustCodePrinter] = None
) -> Iterable[str]:
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

    free_args = sorted(
        functools.reduce(operator.or_, (e.free_symbols for e in exprs), set()),
        key=lambda s: s.name,
    )

    args = ", ".join(f"{name}: {get_type(name)}" for name in free_args)
    if len(exprs) == 1:
        rtype = get_type(*return_expr)
        return_expr = printer.doprint(*return_expr)
    else:
        rtype = ", ".join(get_type(e) for e in exprs)
        rtype = f"({rtype})"
        return_expr = ", ".join(printer.doprint(e) for e in return_expr)
        return_expr = f"({return_expr})"

    if any(isinstance(x, sp.MatrixBase) for x in exprs):
        na_traits = "na::Scalar + "
    else:
        na_traits = ""

    yield f"pub fn {name}<T: {na_traits}Float + FloatConst + CastFrom<f64>>({args}) -> {rtype} {{"
    for line in generate_statements(
        *((name, expr) for name, expr in bindings), printer=printer
    ):
        yield "    " + line

    yield f"    {return_expr}"
    yield "}"


def generate_functions(
    *exprs: Tuple[str, sp.Expr], printer: Optional[RustCodePrinter] = None
) -> Iterable[str]:
    for name, expr in exprs:
        yield from generate_function(name, expr, printer=printer)
        yield ""


def generate_module(
    exprs: List[Tuple[str, sp.Expr]], printer: Optional[RustCodePrinter] = None
) -> Iterable[str]:
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


def generate_differentiable(
    expr: Union[sp.Expr, sp.Eq],
    variable: Union[sp.Symbol, sp.MatrixBase],
    struct_name="Equation",
    printer: Optional[RustCodePrinter] = None,
    runtime_invert=False,
) -> Iterable[str]:
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

    if isinstance(variable, sp.MatrixBase):
        var_symbols = set(variable.free_symbols)
    else:
        var_symbols = {variable}
    fields = sorted(expr.free_symbols - var_symbols, key=lambda s: s.name)

    yield f"pub struct {struct_name}<T> {{"
    for field in fields:
        yield f"    pub {str(field)}: {get_type(field)},"
    yield "}"
    yield ""
    if isinstance(expr, sp.MatrixBase):
        vtype = "na::OVector<Self::Scalar, Self::Dim>"
        mtype = "na::OMatrix<Self::Scalar, Self::Dim, Self::Dim>"
        vview_type = "na::VectorView<Self::Scalar, Self::Dim>"
        yield f"impl<T: Copy + na::Scalar + na::RealField + FloatConst + CastFrom<f64>> root_eq::MultiDifferentiable for {struct_name}<T> {{"
        yield f"    type Scalar = T;"
        yield f"    type Dim = na::Const<{expr.rows}>;"
        yield ""
        yield f"    fn eval_with_inv_jacobian(&self, matrix: {vview_type}) -> ({vtype}, {mtype}) {{"
        for i, name in enumerate(variable):
            yield f"        let {str(name)} = matrix[{i}];"
        for field in fields:
            yield f"        let {str(field)} = self.{str(field)};"

        if runtime_invert:
            j = expr.jacobian(variable)
            for line in generate_statements(("f", expr), ("df", j), printer=printer):
                yield "        " + line
            yield "        let df = df.try_inverse().unwrap();"
        else:
            inv_j = expr.jacobian(variable).inverse()
            for line in generate_statements(("f", expr), ("df", inv_j), printer=printer):
                yield "        " + line

    else:
        yield f"impl<T: Float + FloatConst + CastFrom<f64>> root_eq::Differentiable for {struct_name}<T> {{"
        yield f"    type Scalar = T;"
        yield ""
        yield f"    fn eval_with_derivative(&self, {str(variable)}: T) -> (T, T) {{"
        for field in fields:
            yield f"        let {str(field)} = self.{str(field)};"

        diff = sp.diff(expr, variable)
        for line in generate_statements(("f", expr), ("df", diff), printer=printer):
            yield "        " + line
    yield "        (f, df)"
    yield "    }"
    yield "}"
