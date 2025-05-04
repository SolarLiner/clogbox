import io
import typing
from dataclasses import dataclass, field
from typing import Optional, Tuple, Union

import sympy as sp
from sympy.codegen.ast import value_const
from sympy.codegen import ast
from sympy.printing.codeprinter import requires
from sympy.printing.rust import RustCodePrinter, known_functions
import sympy.matrices.expressions as mat
from sympy.utilities import codegen

RUST_CODE_PREAMBLE = """#![allow(unused_imports, dead_code, non_snake_case, non_camel_case_types)]
"""


class ClogboxRustCodePrinter(RustCodePrinter):
    tab = " " * 4

    type_mappings = RustCodePrinter.type_mappings | {ast.float32: "T", ast.float64: "T"}

    _function_uses = {func: {"num_traits::Float"} for func in known_functions}

    _function_typeparams = {func: {"Float"} for func in known_functions}

    _default_uses = {"num_traits::Float"}
    _default_typeparams = {"Float"}

    def __init__(self, settings=None):
        if settings is None:
            settings = {}
        user_functions = settings.pop("user_functions", {})
        user_functions.update({"Determinant": "determinant"})
        settings["user_functions"] = user_functions
        super().__init__(settings)

        self.uses: set[str] = self._default_uses.copy()
        self.type_params: set[str] = self._default_typeparams.copy()

    def get_type_params(self, remove: Optional[set[str]] = None) -> str:
        if remove is None:
            remove = set()
        params = self.type_params - remove
        result = "<T{bounds}>".format(
            bounds=": " + " + ".join(sorted(params)) if len(params) > 0 else ""
        )
        self.type_params = self._default_typeparams.copy()
        return result

    def get_uses(self) -> str:
        def sort_uses(mod: str):
            if mod.startswith("std"):
                return ""
            return mod

        result = "\n".join(f"use {mod};" for mod in sorted(self.uses, key=sort_uses))
        self.uses = self._default_uses.copy()
        return result

    def _cast_to_float(self, expr):
        return expr

    @requires(type_params={"Float"}, uses={"num_traits::Float"})
    def _print_Zero(self, expr):
        return "T::zero()"

    @requires(type_params={"Float"}, uses={"num_traits::Float"})
    def _print_ZeroMatrix(self, expr: sp.ZeroMatrix):
        mtype = self._get_matrix_type(expr)
        return f"{mtype}::zero()"

    def _print_Function(self, expr):
        print("[_print_Function]", expr)
        if (use := self._function_uses.get(expr.func.__name__)) is not None:
            self.uses.update(use)
        if (use := self._function_typeparams.get(expr.func.__name__)) is not None:
            self.type_params.update(use)
        return super()._print_Function(expr)

    @requires(type_params={"Float"}, uses={"num_traits::Float"})
    def _print_sign(self, expr: sp.sign):
        print("[_print_sign]", expr)
        base = "({})".format(self._print(expr.args[0]))
        match expr.args[0]:
            case sp.MatrixBase():
                return f"{base}.map(T::signum)"
        return f"{base}.signum()"

    def _print_DiracDelta(self, expr: sp.DiracDelta):
        def dirac_delta(x):
            return sp.Piecewise((1, x == 0), (0, True))

        return self._print(dirac_delta(expr.args[0]))

    @requires(type_params={"Float"}, uses={"num_traits::Float"})
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
        mtype = self._get_matrix_type(mat)
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

    def _print_Piecewise(self, expr: sp.Piecewise):
        if len(expr.args) == 1:
            expr, _ = expr.args[0]
            return self._print(expr)
        arms = [
            self._indent_codestring(
                "_ if {cond} => {expr},".format(
                    cond=self._print(cond), expr=self._print(expr)
                )
            )
            for expr, cond in expr.args[:-1]
        ]
        arms.append(
            self._indent_codestring("_ => {}".format(self._print(expr.args[-1][0])))
        )
        arms = "\n".join(arms)
        return f"match () {{\n{arms}\n}}"

    def _print_FunctionDefinition(self, fdecl: ast.FunctionDefinition):
        params = ", ".join(
            self._print_Variable(var, _type=True) for var in fdecl.parameters
        )
        rtype = self._print_Type(fdecl.return_type)
        body = self._indent_codestring(self._print(fdecl.body))
        typeparams = self.get_type_params()

        return (
            f"""pub fn {fdecl.name}{typeparams}({params}) -> {rtype} {{\n{body}\n}}"""
        )

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
        return self._get_statement(
            f"let{mut} {self._print_Variable(decl.variable, _type=_type)} = {self._print(decl.variable.value)}"
        )

    def _print_Variable(self, x: ast.Variable, _type=False):
        if _type:
            return f"{x.symbol.name}: {self._print_Type(x.type)}"
        return x.symbol.name

    # def _print_Assignment(self, assign: ast.Assignment, _type=False):
    #     return self._get_statement(f"let {assign.lhs.name} = {self._print(assign.rhs)}")

    def _print_While(self, node: ast.While):
        condition = self._print(node.condition)
        body = self._indent_codestring(self._print(node.body))
        return f"while {condition} {{\n{body}\n}}"

    def _print_Return(self, node: ast.Return, is_last_statement=False):
        if is_last_statement:
            return self._print(getattr(node, "return"))
        return self._get_statement(f"return {self._print(getattr(node, 'return'))}")

    @requires(type_params={"na::Scalar"}, uses={"nalgebra as na"})
    def _print_Determinant(self, expr: mat.Determinant):
        return self._print_Function(expr)

    def _print_CodeBlock(self, expr: ast.CodeBlock):
        if isinstance(expr.args[-1], ast.Return):
            s = super()._print_CodeBlock(ast.CodeBlock(*expr.args[:-1]))
            s += "\n" + self._print_Return(expr.args[-1], is_last_statement=True)
            return s
        return super()._print_CodeBlock(expr)

    def _print_Scope(self, scope: ast.Scope):
        return "{{\n{body}\n}}".format(
            body=self._indent_codestring(self._print_CodeBlock(scope.body))
        )

    def _indent_codestring(self, codestring):
        return "\n".join([self.tab + line for line in codestring.split("\n")])

    def _get_matrix_type(self, mat):
        rdim = f"na::Const<{mat.rows}>"
        if mat.cols == 1:
            mtype = f"na::OVector::<T, {rdim}>"
        else:
            cdim = f"na::Const<{mat.cols}>"
            mtype = f"na::OMatrix::<T, {rdim}, {cdim}>"
        return mtype


class ClogboxCodegen(codegen.RustCodeGen):
    printer: ClogboxRustCodePrinter

    def __init__(self, **kwargs):
        if "printer" not in kwargs:
            kwargs["printer"] = ClogboxRustCodePrinter()
        super().__init__(**kwargs)

    def get_prototype(self, routine: codegen.Routine):
        def print_arg(arg: codegen.InputArgument | codegen.InOutArgument):
            typ = get_type(arg.name)
            if typ.startswith("na::"):
                self.printer.uses.add("nalgebra as na")
                self.printer.type_params.add("na::Scalar")
            if isinstance(arg, codegen.InOutArgument):
                typ = "&mut " + typ
            return f"{arg.name}: {typ}"

        replacement, results = sp.cse(
            result.expr
            for result in typing.cast(list[codegen.Result], routine.results)
            if not isinstance(result, codegen.OutputArgument)
        )
        codeblock = ast.CodeBlock(
            *(ast.Assignment(lhs, rhs) for lhs, rhs in replacement),
            ast.Return(*results),
        )

        # Print the body to collect the necessary bounds
        self.printer.doprint(codeblock)

        args = ", ".join(print_arg(arg) for arg in routine.arguments)
        typeparams = self.printer.get_type_params()

        if len(routine.result_variables) > 1:
            rtype = "({types})".format(
                types=", ".join(get_type(var.expr) for var in routine.result_variables)
            )
        else:
            rtype = get_type(routine.result_variables[0].expr)
        return f"pub fn {routine.name}{typeparams}({args}) -> {rtype}"


def render_as_module(content, printer: Optional[ClogboxRustCodePrinter] = None) -> str:
    if not printer:
        printer = ClogboxRustCodePrinter()
    str = printer.doprint(content)

    def sort_uses(module: str):
        if module.startswith("std"):
            return ""
        return module

    imports = "\n".join(
        f"use {module};" for module in sorted(printer.uses, key=sort_uses)
    )
    return f"{RUST_CODE_PREAMBLE}\n{imports}\n\n{str}"


def codegen_module(
    routines,
    printer: Optional[ClogboxRustCodePrinter] = None,
    codegen: Optional[ClogboxCodegen] = None,
    project: str = "clogbox",
) -> str:
    if not printer:
        printer = ClogboxRustCodePrinter()
    if not codegen:
        codegen = ClogboxCodegen(printer=printer)

    def sort_uses(module: str):
        if module.startswith("std"):
            return ""
        return module

    [(_, contents)] = codegen.write(routines, "")  # type: str
    imports = codegen.printer.get_uses()
    end_of_comment = contents.find("*/") + 3  # 2 chars + newline
    return (
        contents[:end_of_comment]
        + "\n"
        + RUST_CODE_PREAMBLE
        + "\n"
        + imports
        + "\n\n"
        + contents[end_of_comment:]
    )


def get_type(e: sp.Basic):
    """
    Get the Rust type of sympy expression
    :param e: Expression
    :return: Rust type representation
    """
    match e:
        case sp.MatrixBase(cols=1, rows=r) | sp.MatrixSymbol(cols=1, rows=r):
            return f"na::OVector<T, na::Const<{r}>>"
        case sp.MatrixBase(cols=c, rows=r) | sp.MatrixSymbol(cols=c, rows=r):
            return f"na::OMatrix<T, na::Const<{r}>, na::Const<{c}>>"
        case ast.Tuple():
            return "({types})".format(types=", ".join(get_type(x) for x in e.args))
        case _:
            return "T"


@dataclass()
class IndentStream(io.TextIOBase):
    stream: io.TextIOBase
    indent: str = " " * 4
    _written: bool = field(init=False, default=False)

    def write(self, s: str) -> int:
        i = 0
        if not self._written:
            i = self.stream.write(self.indent)
            self._written = True
        i += self.stream.write(s.replace("\n", "\n" + self.indent))
        return i

    def flush(self):
        self.stream.flush()

    def close(self):
        pass


def generate_scope(exprs: list[sp.Expr]) -> ast.Scope:
    replacements, exprs = sp.cse(exprs)
    codeblock = ast.CodeBlock(
        *(
            ast.Variable(lhs, attrs={value_const}).as_Declaration(value=rhs)
            for lhs, rhs in replacements
        ),
        ast.Return(sp.Tuple(*exprs)),
    )
    return ast.Scope(codeblock)


def generate_differentiable(
    f: io.TextIOBase,
    expr: Union[sp.Expr, sp.Eq],
    variable: Union[sp.Symbol, sp.MatrixBase],
    struct_name="Equation",
    printer: Optional[RustCodePrinter] = None,
    codegen: Optional[ClogboxCodegen] = None,
    runtime_invert=False,
    evalf=23,
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
    :param evalf: Do partial numerical evaluation where possible (set to zero to disable)
    """
    if not codegen:
        if not printer:
            printer = ClogboxRustCodePrinter()
        codegen = ClogboxCodegen(printer=printer)
    del printer

    codegen.printer.uses.add("clogbox_math::root_eq")

    if isinstance(expr, sp.Eq):
        expr = expr.rhs - expr.lhs

    if evalf > 0:
        expr = expr.evalf(n=evalf)

    if isinstance(variable, sp.MatrixBase):
        var_symbols = set(variable.free_symbols)
    else:
        var_symbols = {variable}
    fields = sorted(expr.free_symbols - var_symbols, key=lambda s: s.name)

    fields_str: list[str] = list()
    for field in fields:
        type_ = get_type(field)
        if type_.startswith("na::"):
            codegen.printer.uses.add("nalgebra as na")
            codegen.printer.type_params.add("na::Scalar")
        fields_str.append(f"pub {str(field)}: {type_},")

    typeparams = codegen.printer.get_type_params(remove={"Float"})

    f.write(f"pub struct {struct_name}{typeparams} {{\n")
    for field in fields_str:
        f.write(f"    {field}\n")
    f.write("}\n")
    f.writelines([""])

    codegen.printer.uses.update(
        {
            "num_traits::Float",
            "num_traits::FloatConst",
            "az::CastFrom",
            "clogbox_math::root_eq",
        }
    )
    if isinstance(expr, sp.MatrixBase):
        codegen.printer.uses.add("nalgebra as na")
        vtype = "na::OVector<Self::Scalar, Self::Dim>"
        mtype = "na::OMatrix<Self::Scalar, Self::Dim, Self::Dim>"
        vview_type = "na::VectorView<Self::Scalar, Self::Dim>"
        f.write(
            f"""
impl<T: Copy + na::Scalar + na::RealField + FloatConst + CastFrom<f64>> root_eq::MultiDifferentiable for {struct_name}<T> {{
    type Scalar = T;
    type Dim = na::Const<{expr.rows}>;
    
    fn eval_with_inv_jacobian(&self, matrix: {vview_type}) -> ({vtype}, {mtype}) {{\n"""
        )
        for i, name in enumerate(variable):
            f.write(f"        let {str(name)} = matrix[{i}];\n")
        for field in fields:
            f.write(f"        let {str(field)} = self.{str(field)};\n")

        indented = IndentStream(f, indent=" " * 8)
        if runtime_invert:
            j = expr.jacobian(variable)
            block = generate_scope([expr, j])
            indented.write(codegen.printer.doprint(block, assign_to="let (f, df)"))
            indented.write("\n(f, df.try_inverse().unwrap())\n")
            f.write("    }\n}\n")
        else:
            inv_j = expr.jacobian(variable).inverse()
            block = generate_scope([expr, inv_j])
            indented.write(codegen.printer.doprint(block))

    else:
        f.write(
            f"""
impl<T: Copy + Float + FloatConst + CastFrom<f64>> root_eq::Differentiable for {struct_name}<T> {{
    type Scalar = T;

    fn eval_with_derivative(&self, {str(variable)}: T) -> (T, T) {{
"""
        )
        s = IndentStream(f, indent=" " * 8)
        for field in fields:
            s.write(f"let {str(field)} = self.{str(field)};\n")

        diff = sp.diff(expr, variable)
        block = generate_scope([expr, diff])
        s.write(codegen.printer.doprint(block))

        f.write("""
    }
}""")
