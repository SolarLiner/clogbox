import typing

import sympy as sp


class IntegratorInput(typing.NamedTuple):
    s: sp.Basic
    g: sp.Basic
    x: sp.Basic

Integrator: typing.TypeAlias = typing.Callable[[IntegratorInput], sp.Basic]


def linear_integrator(inp: IntegratorInput) -> sp.Basic:
    return inp.g * inp.x + inp.s


def ota_integrator(drive: sp.Basic, tanh=sp.tanh) -> typing.Callable[[IntegratorInput], sp.Basic]:
    return lambda inp: inp.g * tanh(drive * inp.x) / drive + inp.s


Shaper: typing.TypeAlias = typing.Callable[[sp.Basic], sp.Basic]

def drive(shaper: Shaper, drive: sp.Basic) -> Shaper:
    return lambda x: shaper(drive * x) / drive


def bias(shaper: Shaper, bias: sp.Basic) -> Shaper:
    return lambda x: shaper(x + bias)
