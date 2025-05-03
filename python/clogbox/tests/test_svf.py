from io import StringIO

import sympy as sp

import pytest

from clogbox.codegen import ClogboxRustCodePrinter
from clogbox.filters import Integrator, Shaper, linear_integrator, ota_integrator
from clogbox.filters.svf import SvfInput

@pytest.mark.parametrize("integrator", [linear_integrator, ota_integrator(1)])
@pytest.mark.parametrize("shaper", [lambda x: x, sp.asinh, sp.tanh])
def test_svf(integrator: Integrator, shaper: Shaper, printer: ClogboxRustCodePrinter, snapshot: str):
    g, q = sp.symbols("g q", positive=True)
    svf = SvfInput(g, q, integrator, shaper).generate()
    actual = StringIO()
    svf.generate_module(actual, printer=printer)
    assert snapshot == actual.getvalue()
