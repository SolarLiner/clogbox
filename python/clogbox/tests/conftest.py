import pytest

from clogbox.codegen import ClogboxRustCodePrinter


@pytest.fixture
def printer() -> ClogboxRustCodePrinter:
    return ClogboxRustCodePrinter()
