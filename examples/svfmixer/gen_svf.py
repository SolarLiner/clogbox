# /// script
# requires-python = ">=3.13"
# dependencies = [
#     "clogbox[nr]",
# ]
#
# [tool.uv.sources]
# clogbox = { path = "../../python/clogbox" }
# ///
from pathlib import Path

import sympy as sp

from clogbox.filters import drive, linear_integrator
from clogbox.filters.svf import SvfInput


def main() -> None:
    this_dir = Path(__file__).parent
    dest = this_dir / "src" / "gen.rs"

    g, q = sp.symbols("g q", real=True, positive=True)
    k_drive = sp.Symbol("k_drive", real=True, positive=True)
    svf = SvfInput(g, q, linear_integrator, drive(sp.asinh, k_drive)).generate()

    with dest.open("wt") as f:
        svf.generate_module(f)


if __name__ == "__main__":
    main()
