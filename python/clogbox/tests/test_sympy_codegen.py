import sympy as sp

from clogbox.codegen import ClogboxRustCodePrinter


def test_asinh_derivative():
    printer = ClogboxRustCodePrinter()
    u = sp.Symbol('u', real=True)
    expr = 1 / sp.sqrt(u ** 2 + 1)
    actual = printer.doprint(expr)
    expected = "(u.powi(2) + T::cast_from(1.0)).sqrt().recip()"
    assert expected == actual


