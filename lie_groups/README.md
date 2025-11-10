# Lie Groups

A Python library for visualizing and working with Lie groups using geometric and algebraic techniques.

## Features

This library provides three main approaches for visualizing Lie groups in 2D:

### 1. Cartan/Weyl Reduction
- Best for compact or semisimple groups
- Maps elements to fundamental domains (Weyl alcoves)
- Visualizes conjugacy classes efficiently
- Example: SU(n) eigen-angle parametrization

### 2. Geodesic-Distance Embedding
- Works for any matrix Lie group
- Uses bi-invariant Riemannian metrics
- Applies manifold learning (Isomap, MDS, PCA)
- Preserves geometric structure in low dimensions

### 3. Cartan Decomposition
- For non-compact semisimple groups
- Uses polar/SVD decompositions
- Projects to positive Weyl chambers
- Natural for groups like SL(n,R)

## Installation

### Using UV (recommended)

```bash
# Install UV if you haven't already
curl -LsSf https://astral.sh/uv/install.sh | sh

# Install the package and dependencies
uv sync

# Run examples
uv run python examples/su3_weyl_alcove.py
```

### Using pip

```bash
# Create virtual environment
python -m venv .venv
source .venv/bin/activate  # On Windows: .venv\Scripts\activate

# Install the package
pip install -e .

# For development
pip install -e ".[dev]"
```

## Quick Start

### Example 1: Visualizing SU(3) Conjugacy Classes

```python
from lie_groups import random_sun, plot_weyl_alcove

# Generate random SU(3) matrices
matrices = random_sun(3, num_samples=1000)

# Plot in Weyl alcove
plot_weyl_alcove(matrices, title="SU(3) Conjugacy Classes")
```

### Example 2: Geodesic Embedding of SU(4)

```python
from lie_groups import random_sun, isomap_embedding, plot_embedding

# Generate SU(4) samples
matrices = random_sun(4, num_samples=400)

# Create 2D embedding preserving geodesic distances
embedding = isomap_embedding(matrices, n_components=2)

# Visualize
plot_embedding(embedding, title="SU(4) Manifold Structure")
```

### Example 3: Cartan Projection for SL(n,R)

```python
import numpy as np
from lie_groups.cartan import cartan_projection, plot_cartan_chamber

# Generate random SL(3,R) matrices
matrices = []
for _ in range(100):
    A = np.random.randn(3, 3)
    A /= np.linalg.det(A)**(1/3)  # Force det=1
    matrices.append(A)

# Compute Cartan projections
projections = np.array([cartan_projection(A) for A in matrices])

# Visualize in positive chamber
plot_cartan_chamber(projections, title="SL(3,R) in Cartan Chamber")
```

## API Reference

### Core Functions

- `random_su(n)`: Generate random SU(n) matrix
- `random_sun(n, num_samples)`: Generate multiple SU(n) matrices
- `matrix_log(U)`: Compute matrix logarithm
- `bi_invariant_distance(U, V)`: Compute geodesic distance

### Weyl Alcove Functions

- `su3_alcove_coords(U)`: Map SU(3) to Weyl alcove coordinates
- `sun_alcove_coords(U)`: General SU(n) alcove coordinates
- `plot_weyl_alcove(matrices)`: Visualize in Weyl alcove

### Geodesic Embedding Functions

- `compute_distance_matrix(elements)`: Build pairwise distances
- `isomap_embedding(elements)`: Isomap manifold learning
- `geodesic_embedding(elements, method)`: General embedding (Isomap/MDS/PCA)
- `plot_embedding(embedding)`: Visualize 2D embedding
- `analyze_embedding_quality(embedding, distances)`: Assess embedding quality

### Cartan Decomposition Functions

- `cartan_projection(g)`: Project to positive Weyl chamber
- `polar_decomposition(A)`: Compute A = UP decomposition
- `iwasawa_decomposition(A)`: Compute A = KAN decomposition
- `plot_cartan_chamber(projections)`: Visualize projections

## Mathematical Background

### Compact Groups (e.g., SU(n))
Every element is conjugate to a maximal torus element. The conjugacy classes are parametrized by the Weyl group quotient T/W, visualized as a fundamental domain (Weyl alcove).

### Riemannian Geometry
Lie groups admit left-invariant metrics. For compact groups with bi-invariant metrics, geodesics through identity are one-parameter subgroups, enabling distance calculations via matrix logarithms.

### Non-Compact Groups (e.g., SL(n,R))
Use Cartan decomposition G = KAK where K is maximal compact and A is abelian. The Cartan projection extracts "size" coordinates in the positive Weyl chamber.

## Running Tests

```bash
# Using UV
uv run pytest

# Using pytest directly
pytest tests/

# With coverage
pytest tests/ --cov=lie_groups --cov-report=html
```

## Examples

See the `examples/` directory for complete working examples:
- `su3_weyl_alcove.py`: Visualize SU(3) conjugacy classes
- `geodesic_embedding.py`: Embed SU(4) using Isomap

## Development

### Code Quality

```bash
# Format code
uv run black src/ tests/

# Lint
uv run ruff check src/ tests/

# Type checking
uv run mypy src/
```

### Contributing

1. Fork the repository
2. Create a feature branch
3. Add tests for new functionality
4. Ensure all tests pass
5. Submit a pull request

## References

- [Maximal Torus Theory](https://en.wikipedia.org/wiki/Maximal_torus)
- [Cartan Decomposition](https://en.wikipedia.org/wiki/Cartan_decomposition)
- [Bi-invariant Metrics on Lie Groups](https://www.lehman.edu/faculty/rbettiol/old_teaching/661files/Chap2.pdf)
- [Isomap: A Global Geometric Framework](https://www.robots.ox.ac.uk/~az/lectures/ml/tenenbaum-isomap-Science2000.pdf)

## License

MIT License - see LICENSE file for details

## Citation

If you use this library in your research, please cite:

```bibtex
@software{lie_groups,
  title = {Lie Groups: Python Library for Lie Group Visualization},
  author = {Your Name},
  year = {2024},
  url = {https://github.com/yourusername/lie_groups}
}
```