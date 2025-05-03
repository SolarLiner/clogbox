import sympy as sp

from clogbox.codegen import ClogboxRustCodePrinter
from clogbox.filters import linear_integrator, IntegratorInput

sat = sp.asinh

def test_diode_clipper(printer: ClogboxRustCodePrinter, snapshot: str) -> None:
    g = sp.symbols("g", real=True, positive=True)
    x, u, y = sp.symbols("x u y", real=True)

    eq_s = linear_integrator(IntegratorInput(y, g, x - u))
    actual = printer.doprint(eq_s)
    assert snapshot == actual

