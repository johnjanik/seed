"""
lie_groups: Python library for visualizing and working with Lie groups.

This package provides tools for:
- Cartan/Weyl reduction for compact and semisimple groups
- Geodesic-distance embedding for matrix Lie groups
- Coadjoint orbit analysis
"""

from .core import (
    random_su,
    random_sun,
    matrix_log,
    bi_invariant_distance,
)

from .weyl import (
    su3_alcove_coords,
    sun_alcove_coords,
    plot_weyl_alcove,
)

from .geodesic import (
    geodesic_embedding,
    isomap_embedding,
)

from .cartan import (
    cartan_projection,
    polar_decomposition,
)

__version__ = "0.1.0"

__all__ = [
    "random_su",
    "random_sun",
    "matrix_log",
    "bi_invariant_distance",
    "su3_alcove_coords",
    "sun_alcove_coords",
    "plot_weyl_alcove",
    "geodesic_embedding",
    "isomap_embedding",
    "cartan_projection",
    "polar_decomposition",
]