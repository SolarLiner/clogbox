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
from functools import cached_property
from pathlib import Path

import sympy as sp

from clogbox.codegen import generate_function, generate_differentiable, ClogboxRustCodePrinter, \
    DIFFERENTIABLE_CODE_PREAMBLE


class Svf:
    def __init__(self) -> None:
        self.x, self.lp, self.bp, self.hp, self.r = sp.symbols("x y_lp y_bp y_hp R", real=True)
        self.g = sp.Symbol("g", real=True, positive=True)
        self.s = sp.symbols("s:2", real=True)
        self.bp1 = 2 * self.r * self.bp
        self.fb_sum = self.lp + self.bp1

    @cached_property
    def Y(self):
        return sp.Matrix([self.lp, self.bp, self.hp])

    @cached_property
    def S(self):
        return sp.Matrix(self.s)

    @cached_property
    def output_equations(self):
        return sp.Eq(self.Y, sp.Matrix([
            self._integrator(self.bp, self.s[1]),
            self._integrator(self.hp, self.s[0]),
            self.x - self.fb_sum
        ]))

    @cached_property
    def state_equations(self):
        return sp.Eq(self.S, sp.Matrix([
            self._integrator(self.hp, self.bp),
            self._integrator(self.bp, self.lp),
        ]))

    def _integrator(self, x: sp.Basic, s: sp.Basic):
        return self.g * x + s


class NonlinearSvf(Svf):
    def __init__(self, nonlinearity: typing.Callable[[sp.Basic], sp.Basic]) -> None:
        super().__init__()
        self.drive = sp.Symbol("k_drive", real=True, positive=True)
        self.nonlinearity = nonlinearity

    def _integrator(self, x: sp.Basic, s: sp.Basic):
        return super()._integrator(self.nonlinearity(self.drive * x) / self.drive, s)


class DrivenSvf(NonlinearSvf):
    def __init__(self, *args, **kwargs) -> None:
        super().__init__(*args, **kwargs)
        self.sat = sp.Function("sat", real=True)
        self.asat = sp.Function("sat^{-1}", real=True)
        self.bpp = self.asat(self.bp)
        self.bp1 = 2 * (self.bpp + (self.r - 1) * self.bp)
        self.fb_sum = self.lp + self.bp1

    @cached_property
    def Y(self):
        return sp.Matrix([self.lp, self.bp, self.hp])

    @cached_property
    def sat_replacement(self):
        k, v = next(iter(sp.solve(super().output_equations, self.asat(self.bp), implicit=True, dict=True)[0].items()))
        return sp.Eq(k, v)

    @cached_property
    def bp_equation(self) -> sp.Eq:
        return sp.Eq(self.bp, self.sat(self.sat_replacement.rhs))

    @cached_property
    def output_equations(self) -> sp.Eq:
        eq = super().output_equations.subs({self.bp_equation.lhs: self.bp_equation.rhs})
        return self._simplify_sat(eq)

    def _simplify_sat(self, e: sp.Basic) -> sp.Basic:
        w = sp.Wild('w')
        return e.replace(self.sat(self.asat(w)), w).replace(self.asat(self.sat(w)), w).simplify()


def main() -> None:
    this_dir = Path(__file__).parent
    dest = this_dir / "src" / "svf" / "gen.rs"

    equ = DrivenSvf(sp.tanh)
    # a,b = map(sp.Float, ("1e-12", "4e-2"))
    # fb_sat = lambda x: (1-a/b)*x + a*sp.asinh(x/b)
    fb_sat = lambda x: sp.asinh(equ.drive * x) / equ.drive
    def replace_fb_sat(e: sp.Basic) -> sp.Basic:
        w = sp.Wild('w')
        return e.replace(equ.sat(w), fb_sat(w))

    printer = ClogboxRustCodePrinter(settings={"strict": False})
    dest.write_text("\n".join([DIFFERENTIABLE_CODE_PREAMBLE, "",
                               *generate_differentiable(replace_fb_sat(equ.output_equations), equ.Y,
                                                        printer=printer, runtime_invert=True), "",
                               *generate_function("s", replace_fb_sat(equ.state_equations.rhs), printer=printer), ]))


if __name__ == "__main__":
    main()
