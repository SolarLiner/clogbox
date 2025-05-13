import typing

import sympy as sp


class IntegratorInput(typing.NamedTuple):
    s: sp.Basic
    g: sp.Basic
    x: sp.Basic


Integrator: typing.TypeAlias = typing.Callable[[IntegratorInput], sp.Basic]


def linear_integrator(inp: IntegratorInput) -> sp.Basic:
    return inp.g * inp.x + inp.s


def ota_integrator(
    drive: sp.Basic, tanh=sp.tanh
) -> typing.Callable[[IntegratorInput], sp.Basic]:
    return lambda inp: inp.g * tanh(drive * inp.x) / drive + inp.s


Shaper: typing.TypeAlias = typing.Callable[[sp.Basic], sp.Basic]


def hyperbolic_bounded(x):
    return x / sp.sqrt(1 + x**2)


def hyperbolic_unbounded(x):
    return 2 * x / (1 + sp.sqrt(1 + sp.Abs(4 * x)))


def diode_clipper(
    num_fwd: sp.Basic = 1, num_rev: sp.Basic = 1, k: sp.Basic = 10, drop=0.707
) -> Shaper:
    a = drop * num_fwd
    b = drop * num_rev
    return lambda x: sp.Piecewise(
        (a + sp.log(1 + k * (x - a)) / k, x > a),
        (-b - sp.log(1 - k * (x + b)) / k, x < -b),
        (x, True),
    )


def drive(shaper: Shaper, drive: sp.Basic) -> Shaper:
    return lambda x: shaper(drive * x) / drive


def bias(shaper: Shaper, bias: sp.Basic) -> Shaper:
    return lambda x: shaper(x + bias)
