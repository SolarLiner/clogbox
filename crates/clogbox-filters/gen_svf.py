# /// script
# requires-python = ">=3.13"
# dependencies = [
#     "clogbox[nr]",
# ]
#
# [tool.uv.sources]
# clogbox = { path = "../../python/clogbox" }
# ///
import typing
from io import StringIO
from pathlib import Path
from typing import TypeAlias, NamedTuple

import sympy as sp

from clogbox.codegen import generate_differentiable, ClogboxRustCodePrinter, \
    ClogboxCodegen, codegen_module


class IntegratorInput(NamedTuple):
    s: sp.Basic
    g: sp.Basic
    x: sp.Basic

Integrator: TypeAlias = typing.Callable[[IntegratorInput], sp.Basic]
Shaper: TypeAlias = typing.Callable[[sp.Basic], sp.Basic]


def linear_integrator(inp: IntegratorInput) -> sp.Basic:
    return inp.g * inp.x + inp.s


def ota_integrator(inp: IntegratorInput, tanh=sp.tanh) -> sp.Basic:
    return inp.g * tanh(inp.x) + inp.s


def make_matrix_equation(eqs: typing.Iterable[sp.Eq]) -> sp.Eq:
    return sp.Eq(sp.Matrix([e.lhs for e in eqs]), sp.Matrix([e.rhs for e in eqs]))


class Svf(NamedTuple):
    x: sp.Symbol
    lp: sp.Symbol
    bp: sp.Symbol
    hp: sp.Symbol
    output_equations: sp.Eq
    state_equations: sp.Eq


def svf(integrator: Integrator, damping_resonance: Shaper) -> Svf:
    x, lp, bp, hp, r = sp.symbols("x y_lp y_bp y_hp R", real=True)
    g = sp.Symbol("g", real=True, positive=True)
    s = sp.MatrixSymbol('S', 2, 1)
    sat = sp.Function("sat", real=True)
    asat = sp.Function("sat^{-1}", real=True)
    bpp = asat(bp)
    bp1 = 2 * (bpp + (r - 1) * bp)
    fb_sum = lp + bp1

    out_eq = [
        sp.Eq(hp, x - fb_sum),
        sp.Eq(bp, integrator(IntegratorInput(s[0], g, hp))),
        sp.Eq(lp, integrator(IntegratorInput(s[1], g, bp))),
    ]

    sat_inner = sp.solve(out_eq, asat(bp), implicit=True, dict=True)[0][asat(bp)]
    bp_replacement = {bp: sat(sat_inner)}

    w = sp.Wild("w")
    out_eq = [e.subs(bp_replacement).replace(sat(asat(w)), w).replace(asat(sat(w)), w).replace(sat(w), damping_resonance(w)) for e in out_eq]
    state_eq = [integrator(IntegratorInput(bp, g, hp)), integrator(IntegratorInput(lp, g, bp))]

    return Svf(x, lp, bp, hp, make_matrix_equation(out_eq), sp.Eq(s, sp.Matrix(state_eq)))


def main() -> None:
    this_dir = Path(__file__).parent
    dest = this_dir / "src" / "svf" / "gen.rs"

    drive = sp.Symbol("k_drive", real=True, positive=True)
    equ = svf(ota_integrator, lambda x: sp.asinh(drive * x) / drive)

    printer = ClogboxRustCodePrinter(settings={"strict": False})
    codegen = ClogboxCodegen(printer=printer)
    wrt = sp.Matrix([equ.lp, equ.bp, equ.hp])
    with dest.open("wt") as f:
        s = StringIO()
        generate_differentiable(s, equ.output_equations, wrt, "SvfEquation", printer=printer, codegen=codegen,
                                runtime_invert=True)
        state_routine_args = sorted(equ.state_equations.free_symbols, key=lambda s: s.name)
        routine = codegen.routine("state", equ.state_equations, state_routine_args, [])
        f.write(codegen_module([routine], printer=printer, codegen=codegen))
        f.write("\n\n")
        f.write(s.getvalue())


if __name__ == "__main__":
    main()
