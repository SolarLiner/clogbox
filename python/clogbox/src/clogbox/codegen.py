import io
import typing
from dataclasses import dataclass
from typing import Optional, Tuple, Union

import sympy as sp
from sympy.codegen.ast import value_const
from sympy.codegen import ast
from sympy.printing.codeprinter import requires
from sympy.printing.rust import RustCodePrinter
import sympy.matrices.expressions as mat
from sympy.utilities import codegen

from clogbox.nodes import Tuple

RUST_CODE_PREAMBLE = """
//! Generated source code with the `clogbox` Python package, to use with the `clogbox` project.
#![allow(unused_imports, dead_code, non_snake_case, non_camel_case)]
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
    tab = " " * 4

    def __init__(self, settings=None):
        if settings is None:
            settings = {}
        user_functions = settings.pop("user_functions", {})
        user_functions.update({"Determinant": "determinant"})
        settings["user_functions"] = user_functions
        super().__init__(settings)

        self.uses: set[str] = set()
        self.type_params: set[str] = set()

    @requires(type_params={"Float"})
    def _print_Pow(self, expr):
        base_code = self._print(expr.base)
        exp = expr.exp
        if isinstance(exp, sp.Integer):
            return f"{base_code}.powi({int(exp)})"
        else:
            return super()._print_Pow(expr)

    @requires(type_params={"CastFrom<f64>"}, uses={"az::CastFrom"})
    def _print_Integer(self, expr, _type=False):
        return f"T::cast_from({float(expr)})"

    @requires(type_params={"CastFrom<f64>"}, uses={"az::CastFrom"})
    def _print_Float(self, expr, **kwargs):
        return f"T::cast_from({float(expr)})"

    def _print_Rational(self, expr):
        p, q = tuple(self._print(sp.Integer(x)) for x in (expr.p, expr.q))
        return f"{p}/{q}"

    @requires(type_params={"na::Scalar"}, uses={"nalgebra as na"})
    def _print_MatrixBase(self, mat: sp.MatrixBase):
        values = ", ".join(self._print(x) for x in mat)
        cdim = f"na::Const<{mat.cols}>"
        if mat.rows == 1:
            mtype = f"na::OVector::<T, {cdim}>"
        else:
            rdim = f"na::Const<{mat.rows}>"
            mtype = f"na::OMatrix::<T, {rdim}, {cdim}>"
        return f"{mtype}::new({values})"

    @requires(type_params={"FloatConst"}, uses={"num_traits::FloatConst"})
    def _print_NaN(self, expr, _type=False):
        return "T::nan()"

    @requires(type_params={"FloatConst"}, uses={"num_traits::FloatConst"})
    def _print_Exp1(self, expr, _type=False):
        return "T::E()"

    @requires(type_params={"FloatConst"}, uses={"num_traits::FloatConst"})
    def _print_Pi(self, expr, _type=False):
        return "T::PI()"

    @requires(type_params={"FloatConst"}, uses={"num_traits::FloatConst"})
    def _print_Infinity(self, expr, _type=False):
        return "T::INFINITY()"

    @requires(type_params={"FloatConst"}, uses={"num_traits::FloatConst"})
    def _print_NegativeInfinity(self, expr, _type=False):
        return "T::NEG_INFINITY()"

    def _print_Tuple(self, expr: Tuple):
        return "({values})".format(values=", ".join(self._print(x) for x in expr.args))

    def _print_FunctionDefinition(self, fdecl: ast.FunctionDefinition):
        self.type_params.clear()

        params = ", ".join(self._print_Variable(var, _type=True) for var in fdecl.parameters)
        rtype = self._print_Type(fdecl.return_type)
        body = self._indent_codestring(self._print(fdecl.body))
        typeparams = "<T{bounds}>".format(bounds=": " + " + ".join(self.type_params) if len(self.type_params) > 0 else "")
        return f"""fn {fdecl.name}{typeparams}({params}) -> {rtype} {{\n{body}\n}}"""

    def _print_Type(self, t: ast.Type):
        if isinstance(t, ast.ComplexBaseType):
            self.uses.add("nalgebra as na")
            return "na::Complex<T>"
        if isinstance(t, ast.SignedIntType):
            return "i64"
        if isinstance(t, ast.UnsignedIntType):
            return "u64"
        return "T"

    def _print_Declaration(self, decl: ast.Declaration, _type=False):
        if value_const in decl.variable.attrs:
            mut = ""
        else:
            mut = " mut"
        return self._get_statement(f"let{mut} {self._print_Variable(decl.variable, _type=_type)} = {self._print(decl.variable.value)}")

    def _print_Variable(self, x: ast.Variable, _type=False):
        if _type:
            return f"{x.symbol.name}: {self._print_Type(x.type)}"
        return x.symbol.name

    def _print_While(self, node: ast.While):
        condition = self._print(node.condition)
        body = self._indent_codestring(self._print(node.body))
        return f"while {condition} {{\n{body}\n}}"

    def _print_Return(self, node: ast.Return, is_last_statement=False):
        if is_last_statement:
            return self._print(getattr(node, "return"))
        return self._get_statement(f"return {self._print(getattr(node, 'return'))}")

    def _print_Determinant(self, expr: mat.Determinant):
        return self._print_Function(expr)

    def _indent_codestring(self, codestring):
        return '\n'.join([self.tab + line for line in codestring.split('\n')])


class ClogboxCodegen(codegen.RustCodeGen):
    printer: ClogboxRustCodePrinter
    def __init__(self, **kwargs):
        kwargs["printer"] = ClogboxRustCodePrinter()
        super().__init__(**kwargs)

    def get_prototype(self, routine: codegen.Routine):
        def print_arg(arg: codegen.InputArgument | codegen.InOutArgument):
            self.printer.type_params.clear()

            typ = get_type(arg.name)
            if typ.startswith("na::"):
                self.printer.uses.add("nalgebra as na")
                self.printer.type_params.add("na::Scalar")
            return f"{arg.name}: {typ}"

        replacement, results = sp.cse(result.expr for result in typing.cast(list[codegen.Result], routine.results))
        codeblock = ast.CodeBlock(*(ast.Assignment(lhs, rhs) for lhs, rhs in replacement), ast.Return(*results))
        _body = self.printer.indent_code(self.printer.doprint(codeblock))
        args = ", ".join(print_arg(arg) for arg in routine.arguments)
        typeparams = "<T{bounds}>".format(bounds=": " + " + ".join(self.printer.type_params) if len(
            self.printer.type_params) > 0 else "")
        return f"fn {routine.name}{typeparams}({args}) -> T"


def render_as_module(content, printer: Optional[ClogboxRustCodePrinter]=None) -> str:
    if not printer:
        printer = ClogboxRustCodePrinter()
    str = printer.doprint(content)
    imports = "\n".join(f"use {module};" for module in printer.uses)
    return f"{imports}\n\n{str}"


def codegen_module(routines, printer: Optional[ClogboxRustCodePrinter]=None, codegen: Optional[ClogboxCodegen]=None,
                   project: str = "clogbox") -> str:
    if not printer:
        printer = ClogboxRustCodePrinter()
    if not codegen:
        codegen = ClogboxCodegen(printer=printer)

    [(_, contents)] = codegen.write(routines, "")  # type: str
    imports = "\n".join(f"use {module};" for module in codegen.printer.uses)
    end_of_comment = contents.find("*/") + 3 # 2 chars + newline
    return contents[:end_of_comment] + "\n" + imports + "\n\n" + contents[end_of_comment:]


def get_type(e: sp.Basic):
    """
    Get the Rust type of a sympy expression
    :param e: Sympy expression
    :return: Rust type representation
    """
    match e:
        case sp.MatrixBase(cols=c, rows=r) | sp.MatrixSymbol(cols=c, rows=r):
            return f"na::OMatrix<T, na::Const<{r}>, na::Const<{c}>>"
        case _:
            return "T"


@dataclass()
class IndentStream(io.TextIOBase):
    stream: io.TextIOBase
    indent: str = " " * 4

    def write(self, s: str) -> int:
        return self.stream.write(s.replace("\n", "\n" + self.indent))

    def flush(self):
        self.stream.flush()

    def close(self):
        self.stream.close()


def generate_codeblock(exprs: list[sp.Expr]) -> ast.CodeBlock:
    replacements, exprs = sp.cse(exprs)
    return ast.CodeBlock(*(ast.Assignment(lhs, rhs) for lhs, rhs in replacements), Tuple(exprs))

def generate_differentiable(
    f: io.TextIOBase,
    expr: Union[sp.Expr, sp.Eq],
    variable: Union[sp.Symbol, sp.MatrixBase],
    struct_name="Equation",
    printer: Optional[RustCodePrinter] = None,
    codegen: Optional[ClogboxCodegen] = None,
    runtime_invert=False,
):
    """
    Generate Rust code for a specific equation that is differentiable once about a variable. This function will
    implement the `clogbox_math::root_eq::Differentiable` trait for the generated struct.

    The resulting source code needs the `clogbox_math` and `num-traits` crates. The final module must also have the
    contents of the `DIFFERENTIABLE_CODE_PREAMBLE` constant present.

    :param f: Stream to write the code to.
    :param expr: Expression to implement the trait for.
    :param variable: Variable about which the equation is differentiable.
    :param struct_name: Name of the struct to generate (defaults to "Equation").
    :param printer: Optional instance of `RustCodePrinter` used to print symbolic
        expressions as Rust-compatible code. If not provided, a default printer will
        be used.
    """
    if not printer:
        printer = ClogboxRustCodePrinter()
    if not codegen:
        codegen = ClogboxCodegen(printer=printer)

    if isinstance(expr, sp.Eq):
        expr = expr.rhs - expr.lhs

    if isinstance(variable, sp.MatrixBase):
        var_symbols = set(variable.free_symbols)
    else:
        var_symbols = {variable}
    fields = sorted(expr.free_symbols - var_symbols, key=lambda s: s.name)

    f.write(f"pub struct {struct_name}<T> {{")
    for field in fields:
        f.write(f"    pub {str(field)}: {get_type(field)},")
    f.write( "}")
    f.writelines([""])

    if isinstance(expr, sp.MatrixBase):
        vtype = "na::OVector<Self::Scalar, Self::Dim>"
        mtype = "na::OMatrix<Self::Scalar, Self::Dim, Self::Dim>"
        vview_type = "na::VectorView<Self::Scalar, Self::Dim>"
        f.write(
f"""
impl<T: Copy + na::Scalar + na::RealField + FloatConst + CastFrom<f64>> root_eq::MultiDifferentiable for {struct_name}<T> {{
    type Scalar = T;
    type Dim = na::Const<{expr.rows}>;
    
    fn eval_with_inv_jacobian(&self, matrix: {vview_type}) -> ({vtype}, {mtype}) {{"""
        )
        for i, name in enumerate(variable):
            f.write(f"        let {str(name)} = matrix[{i}];")
        for field in fields:
            f.write(f"        let {str(field)} = self.{str(field)};")

        s = IndentStream(f, indent=" " * 8)
        if runtime_invert:
            j = expr.jacobian(variable)
            block = generate_codeblock([expr, j])
            s.write(printer.doprint(block, assign_to="(f, df)"))
            s.write("        (f, df.try_inverse().unwrap())")
        else:
            inv_j = expr.jacobian(variable).inverse()
            block = generate_codeblock([expr, inv_j])
            s.write(printer.doprint(block))

    else:
        f.write(
f"""
impl<T: Copy + na::Scalar + na::RealField + FloatConst + CastFrom<f64>> root_eq::Differentiable for {struct_name}<T> {{
    type Scalar = T;

    fn eval_with_derivative(&self, {str(variable)}: T) -> (T, T) {{
"""
        )
        for field in fields:
            f.write(f"        let {str(field)} = self.{str(field)};")

        diff = sp.diff(expr, variable)
        block = generate_codeblock([expr, diff])
        f.write(printer.doprint(block))
        f.write(
"""
    }
}
"""
        )