import sympy as sp

from clogbox.codegen import ClogboxRustCodePrinter

NR_RUST_PREAMBLE = """
use clogbox_math::root_eq::Differentiable;
"""


def generate_root_eq(equation, variable, struct_name ="Equation"):
    """
    Generate Rust code for a specific equation to be solved with the Newton-Raphson method,
    optimized using common subexpression elimination.

    Parameters:
    -----------
    equation : sympy.Expr
        A sympy expression that will be solved for zero (i.e., equation = 0)
    variable : sympy.Symbol
        The variable in the equation

    Returns:
    --------
    str
        Rust source code implementing the specific equation using the Differentiable trait,
        including all free variables as fields in the struct and using generics with Float bound.
    """
    # "Normalize" an equality to a root equation
    if isinstance(equation, sp.Eq):
        equation = equation.rhs - equation.lhs

    # Calculate the derivative
    derivative = sp.diff(equation, variable)

    # Perform common subexpression elimination on both expressions
    replacements, reduced_exprs = sp.cse([equation, derivative])

    # Use the custom printer
    printer = ClogboxRustCodePrinter()

    # Generate Rust code for the common subexpressions
    cse_code = ""
    for idx, (sym, expr) in enumerate(replacements):
        cse_code += f"let {printer.doprint(sym)} = {printer.doprint(expr)};\n        "

    # Generate Rust code for the reduced expressions
    eq_str = printer.doprint(reduced_exprs[0])
    derivative_str = printer.doprint(reduced_exprs[1])

    # Identify all free symbols except the main variable
    free_vars = sorted(equation.free_symbols - {variable}, key=lambda s: s.name)

    # Create struct fields for each free variable
    struct_fields = "\n    ".join(f"pub {str(var)}: T," for var in free_vars)
    param_inserts = "\n        ".join(f"let {str(var)} = self.{str(var)};" for var in free_vars)

    x = variable.name
    # Generate the Rust code
    rust_code = f"""/// Structure representing the specific equation to solve
#[derive(Debug, Clone, Copy)]
pub struct {struct_name}<T> {{
    {struct_fields}
}}

impl<T: Float + FloatConst + CastFrom<f64>> Differentiable for {struct_name}<T> {{
    type Scalar = T;

    fn eval(&self, {x}: T) -> T {{
        {param_inserts}
        {cse_code}
        {eq_str}
    }}

    fn derivative(&self, {x}: T) -> T {{
        {param_inserts}
        {cse_code}
        {derivative_str}
    }}

    fn eval_with_derivative(&self, {x}: T) -> (T, T) {{
        {param_inserts}
        {cse_code}
        let fx = {eq_str};
        let dfx = {derivative_str};
        (fx, dfx)
    }}
}}
"""

    return rust_code
