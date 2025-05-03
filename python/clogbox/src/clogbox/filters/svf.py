import io
import typing
from dataclasses import dataclass

import sympy as sp

from clogbox.codegen import generate_differentiable, ClogboxRustCodePrinter, ClogboxCodegen, codegen_module
from clogbox.filters import Integrator, linear_integrator, Shaper, IntegratorInput


def make_matrix_equation(eqs: typing.Iterable[sp.Eq]) -> sp.Eq:
    return sp.Eq(sp.Matrix([e.lhs for e in eqs]), sp.Matrix([e.rhs for e in eqs]))


@dataclass
class SvfOutput:
    x: sp.Symbol
    lp: sp.Symbol
    bp: sp.Symbol
    hp: sp.Symbol
    output_equations: sp.Eq
    state_equations: sp.Eq

    def generate_module(self, f: io.TextIOBase, printer: typing.Optional[ClogboxRustCodePrinter] = None,
                        codegen: typing.Optional[ClogboxCodegen] = None, evalf=23, runtime_invert=True):
        if not printer:
            printer = ClogboxRustCodePrinter()
        if not codegen:
            codegen = ClogboxCodegen(printer=printer)

        s = io.StringIO()
        wrt = sp.Matrix([self.lp, self.bp, self.hp])
        generate_differentiable(s, self.output_equations, wrt, "SvfEquation", printer=printer, codegen=codegen,
                                runtime_invert=runtime_invert, evalf=evalf)
        state_routine_args = sorted(self.state_equations.free_symbols, key=lambda s: s.name)
        routine = codegen.routine("state", self.state_equations, state_routine_args, [])
        f.write(codegen_module([routine], printer=printer, codegen=codegen))
        f.write("\n\n")
        f.write(s.getvalue())


@dataclass
class SvfInput:
    g: sp.Basic = sp.Symbol("g", real=True, positive=True)
    q: sp.Basic = sp.Symbol("q", real=True, positive=True)
    integrator: Integrator = linear_integrator
    damping_resonance: Shaper = sp.asinh

    def generate(self) -> SvfOutput:
        x, lp, bp, hp = sp.symbols("x y_lp y_bp y_hp", real=True)
        r = 2 * (1 - self.q)
        s = sp.MatrixSymbol('S', 2, 1)
        sat = sp.Function("sat", real=True)
        asat = sp.Function("sat^{-1}", real=True)
        bpp = asat(bp)
        bp1 = 2 * (bpp + (r - 1) * bp)
        fb_sum = lp + bp1

        out_eq = [
            sp.Eq(hp, x - fb_sum),
            sp.Eq(bp, self.integrator(IntegratorInput(s[0], self.g, hp))),
            sp.Eq(lp, self.integrator(IntegratorInput(s[1], self.g, bp))),
        ]

        sat_inner = sp.solve(out_eq, asat(bp), implicit=True, dict=True)[0][asat(bp)]
        bp_replacement = {bp: sat(sat_inner)}

        w = sp.Wild("w")
        out_eq = [e.subs(bp_replacement).replace(sat(asat(w)), w).replace(asat(sat(w)), w).replace(sat(w),
                                                                                                   self.damping_resonance(
                                                                                                       w))
                  for e in out_eq]
        state_eq = [self.integrator(IntegratorInput(bp, self.g, hp)), self.integrator(IntegratorInput(lp, self.g, bp))]

        return SvfOutput(x, lp, bp, hp, make_matrix_equation(out_eq), sp.Eq(s, sp.Matrix(state_eq)))
