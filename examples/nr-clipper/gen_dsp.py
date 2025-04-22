# /// script
# requires-python = ">=3.13"
# dependencies = ["clogbox[nr]@${PROJECT_ROOT}/../../python/clogbox"]
# ///
from pathlib import Path

import sympy as sp

from clogbox.codegen import DIFFERENTIABLE_CODE_PREAMBLE, generate_differentiable, generate_functions

sat = sp.asinh

def main() -> None:
    this_dir = Path(__file__).parent
    dest = this_dir / "src" / "gen.rs"

    g, drive = sp.symbols("g k_drive", real=True, positive=True)
    x, u, y, s = sp.symbols("x u y s", real=True)

    integrator = lambda x: g * x + s
    y = integrator(x - u)
    eq = sp.Eq(sat(u), y)
    rust_equ_u = generate_differentiable(eq, u, "EqU")
    rust_fns = generate_functions(("y", y), ("s", sp.solve(eq, s)[0]))
    dest.write_text(
        "\n".join([DIFFERENTIABLE_CODE_PREAMBLE, *rust_equ_u, "", *rust_fns]))


if __name__ == "__main__":
    main()
