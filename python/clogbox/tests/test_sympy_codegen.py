from io import StringIO

import sympy as sp

from clogbox.codegen import ClogboxRustCodePrinter, ClogboxCodegen, render_as_module, codegen_module, generate_differentiable

import pytest


@pytest.fixture
def codegen(printer: ClogboxRustCodePrinter) -> ClogboxCodegen:
    return ClogboxCodegen(printer=printer)


def test_abs_diff(printer: ClogboxRustCodePrinter, snapshot: str):
    x = sp.Symbol("x", real=True)
    expr = sp.Abs(x).diff(x)
    actual = printer.doprint(expr)
    assert snapshot == actual


def test_hyperbolic_diff(printer: ClogboxRustCodePrinter, snapshot: str):
    x = sp.Symbol("x", real=True)
    hyperbolic = x / (1 + sp.Abs(x))
    actual = StringIO()
    generate_differentiable(actual, hyperbolic, x, printer=printer)
    assert snapshot == actual.getvalue()


def test_piecewise(printer: ClogboxRustCodePrinter, snapshot: str):
    x = sp.Symbol("x", real=True)
    expr = sp.Piecewise((x, x < 0), (x ** 2, True))
    actual = printer.doprint(expr)
    assert snapshot == actual


def test_asinh_derivative(printer: ClogboxRustCodePrinter, snapshot: str):
    u = sp.Symbol('u', real=True)
    expr = 1 / sp.sqrt(u ** 2 + 1)
    actual = printer.doprint(expr)
    # expected = "(u.powi(2) + T::cast_from(1.0)).sqrt().recip()"
    assert snapshot == actual


def test_generate_differentiable(codegen: ClogboxCodegen, snapshot: str):
    u = sp.Symbol('u', real=True)
    expr = sp.asinh(u) - sp.tanh(u)

    actual = StringIO()
    generate_differentiable(actual, expr, u, codegen=codegen)
    assert snapshot == actual.getvalue()


def test_newton_rhapson_function(printer: ClogboxRustCodePrinter, snapshot: str):
    from sympy.codegen.algorithms import newtons_method_function

    u = sp.Symbol('u', real=True)
    expr = sp.asinh(u) - sp.tanh(u)
    code = newtons_method_function(expr, u, cse=True)
    actual = printer.doprint(code)
    assert snapshot == actual


def test_matrix_expression(printer: ClogboxRustCodePrinter, snapshot: str):
    u = sp.Symbol("u")
    X = sp.Matrix([[u, u ** 2], [u ** 3, u ** 4]])
    root_eq = sp.Determinant(X) - sp.tanh(u)
    actual = printer.doprint(root_eq)
    assert snapshot == actual


def test_matrix_routine(codegen: ClogboxCodegen, snapshot: str):
    X = sp.MatrixSymbol('X', 2, 2)
    y = sum(X) / sp.Determinant(X)

    routine = codegen.routine("matrix_routine", y, [X], [])
    actual = codegen_module([routine], codegen=codegen)
    assert snapshot == actual
