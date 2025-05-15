# /// script
# requires-python = ">=3.13"
# dependencies = [
#     "clogbox",
# ]
#
# [tool.uv.sources]
# clogbox = { path = "../../python/clogbox" }
# ///
from io import StringIO
from pathlib import Path

import sympy as sp

from clogbox.codegen import generate_differentiable, ClogboxCodegen, codegen_module
from clogbox.filters import (
    linear_integrator,
    IntegratorInput,
    drive,
    bias,
    hyperbolic_unbounded,
)


def main() -> None:
    this_dir = Path(__file__).parent
    dest = this_dir / "src" / "gen.rs"

    g, k_drive = sp.symbols("g k_drive", real=True, positive=True)
    x, u, y, s, k_bias = sp.symbols("x u y s k_bias", real=True)

    sat = bias(drive(hyperbolic_unbounded, k_drive), k_bias)
    eq_y = linear_integrator(IntegratorInput(s, g, x - u))
    eq = sp.Eq(sat(u), eq_y)
    eq_s = linear_integrator(IntegratorInput(y, g, x - u))

    codegen = ClogboxCodegen()

    with dest.open("wt") as f:
        diff_impl = StringIO()
        generate_differentiable(diff_impl, eq, u, codegen=codegen)
        routine_y = codegen.routine("y", eq_y, [g, x, s, u], [])
        routine_s = codegen.routine("s", eq_s, [g, x, y, u], [])
        module = codegen_module([routine_y, routine_s], codegen=codegen)
        f.writelines([module, "", "", diff_impl.getvalue()])


if __name__ == "__main__":
    main()
