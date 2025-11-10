"""Weyl alcove and conjugacy class visualization for compact Lie groups."""

import numpy as np
import matplotlib.pyplot as plt
from typing import Optional, Tuple
import scipy.linalg as la


def su3_alcove_coords(U: np.ndarray) -> np.ndarray:
    """
    Compute Weyl alcove coordinates for an SU(3) matrix.

    Maps an SU(3) element to its conjugacy class representative
    in the Weyl alcove, then projects to 2D coordinates.

    Parameters
    ----------
    U : np.ndarray
        3x3 SU(3) matrix

    Returns
    -------
    np.ndarray
        2D coordinates in the alcove
    """
    # Get eigenvalues (unit-modulus)
    w, _ = la.eig(U)
    angles = np.angle(w)  # in (-pi, pi]

    # Enforce trace zero (sum of angles = 0 mod 2π)
    angles -= angles.mean()

    # Sort descending to land in alcove
    angles = np.sort(angles)[::-1]

    # Barycentric coordinates of 2-simplex: project to 2D
    e1 = angles[0] - angles[1]
    e2 = angles[1] - angles[2]

    return np.array([e1, e2])


def sun_alcove_coords(U: np.ndarray) -> np.ndarray:
    """
    Compute Weyl alcove coordinates for a general SU(n) matrix.

    Parameters
    ----------
    U : np.ndarray
        nxn SU(n) matrix

    Returns
    -------
    np.ndarray
        Alcove coordinates (n-1 dimensional)
    """
    n = U.shape[0]

    # Get eigenvalues
    w, _ = la.eig(U)
    angles = np.angle(w)

    # Enforce trace zero
    angles -= angles.mean()

    # Sort descending
    angles = np.sort(angles)[::-1]

    # Return differences (n-1 dimensional)
    return np.diff(angles)


def plot_weyl_alcove(
    matrices: list,
    title: str = "Weyl Alcove Projection",
    figsize: Tuple[int, int] = (8, 8),
    save_path: Optional[str] = None
) -> None:
    """
    Plot SU(3) matrices in the Weyl alcove.

    Parameters
    ----------
    matrices : list
        List of SU(3) matrices
    title : str, optional
        Plot title
    figsize : tuple, optional
        Figure size
    save_path : str, optional
        Path to save figure
    """
    # Compute alcove coordinates
    pts = np.array([su3_alcove_coords(U) for U in matrices])

    # Create plot
    fig, ax = plt.subplots(figsize=figsize)

    # Plot points
    ax.scatter(pts[:, 0], pts[:, 1], s=3, alpha=0.5)

    # Draw alcove boundary (equilateral triangle)
    # Vertices of the fundamental alcove for SU(3)
    vertices = np.array([
        [0, 0],
        [2*np.pi/3, 0],
        [np.pi/3, np.pi/np.sqrt(3)]
    ])
    triangle = plt.Polygon(vertices, fill=False, edgecolor='red', linewidth=2)
    ax.add_patch(triangle)

    # Labels and formatting
    ax.set_xlabel(r'$\theta_1 - \theta_2$', fontsize=12)
    ax.set_ylabel(r'$\theta_2 - \theta_3$', fontsize=12)
    ax.set_title(title, fontsize=14)
    ax.set_aspect('equal')
    ax.grid(True, alpha=0.3)

    if save_path:
        plt.savefig(save_path, dpi=150, bbox_inches='tight')

    plt.show()


def weyl_chamber_projection(
    g: np.ndarray,
    lie_type: str = "A"
) -> np.ndarray:
    """
    Project a Lie group element to the positive Weyl chamber.

    Parameters
    ----------
    g : np.ndarray
        Lie group element
    lie_type : str
        Type of Lie algebra (A, B, C, D, etc.)

    Returns
    -------
    np.ndarray
        Coordinates in the positive Weyl chamber
    """
    if lie_type == "A":
        # Type A corresponds to SU(n)
        return sun_alcove_coords(g)
    else:
        raise NotImplementedError(f"Lie type {lie_type} not yet implemented")