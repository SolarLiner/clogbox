# /// script
# requires-python = ">=3.13"
# dependencies = [
#     "clogbox",
# ]
#
# [tool.uv.sources]
# clogbox = { path = "../../python/clogbox" }
# ///
from pathlib import Path

import sympy as sp

from clogbox.filters import (
    drive,
    ota_integrator,
    hyperbolic_unbounded,
    hyperbolic_bounded,
)
from clogbox.filters.svf import SvfInput


def main() -> None:
    this_dir = Path(__file__).parent
    dest = this_dir / "src" / "gen.rs"

    g, q = sp.symbols("g q", real=True, positive=True)
    k_drive = sp.Symbol("k_drive", real=True, positive=True)
    svf = SvfInput(
        g,
        q,
        ota_integrator(k_drive, hyperbolic_bounded),
        drive(hyperbolic_unbounded, k_drive),
    ).generate()

    with dest.open("wt") as f:
        svf.generate_module(f, evalf=0)


if __name__ == "__main__":
    main()
