"""Cartan decomposition and projection for non-compact Lie groups."""

import numpy as np
import scipy.linalg as la
from typing import Tuple, Optional
import matplotlib.pyplot as plt


def cartan_projection(
    g: np.ndarray,
    return_decomposition: bool = False
) -> np.ndarray:
    """
    Compute Cartan projection for a matrix in SL(n,R).

    Uses SVD to find g = kak' where k, k' are orthogonal and a is diagonal.
    Returns log of the diagonal part (sorted to positive chamber).

    Parameters
    ----------
    g : np.ndarray
        Matrix in SL(n,R) or GL(n,R)
    return_decomposition : bool
        Whether to return full decomposition (k, a, k')

    Returns
    -------
    np.ndarray or tuple
        Cartan coordinates (log singular values) or full decomposition
    """
    # SVD decomposition
    U, S, Vh = la.svd(g)

    # Sort singular values to positive chamber
    S_sorted = np.sort(S)[::-1]
    log_S = np.log(S_sorted)

    if return_decomposition:
        # Reconstruct sorted diagonal matrix
        A = np.diag(S_sorted)
        # Find permutation
        perm = np.argsort(S)[::-1]
        K1 = U[:, perm]
        K2 = Vh[perm, :]
        return K1, A, K2
    else:
        return log_S


def iwasawa_decomposition(A: np.ndarray) -> Tuple[np.ndarray, np.ndarray, np.ndarray]:
    """
    Compute Iwasawa decomposition A = KAN.

    For GL(n,R): K is orthogonal, A is diagonal positive, N is upper triangular
    with ones on diagonal.

    Parameters
    ----------
    A : np.ndarray
        Matrix in GL(n,R)

    Returns
    -------
    tuple
        K (orthogonal), A (diagonal), N (upper triangular)
    """
    # QR decomposition gives us most of what we need
    Q, R = la.qr(A)

    # Q is orthogonal (our K)
    K = Q

    # Extract diagonal part of R (positive due to QR properties)
    diag_R = np.abs(np.diag(R))
    A_diag = np.diag(diag_R)

    # N is R with normalized diagonal
    N = R / diag_R[:, np.newaxis]

    return K, A_diag, N


def polar_decomposition(A: np.ndarray) -> Tuple[np.ndarray, np.ndarray]:
    """
    Compute polar decomposition A = UP.

    Parameters
    ----------
    A : np.ndarray
        Input matrix

    Returns
    -------
    tuple
        U (unitary/orthogonal) and P (positive semidefinite)
    """
    U, S, Vh = la.svd(A)
    P = Vh.conj().T @ np.diag(S) @ Vh
    U_polar = U @ Vh
    return U_polar, P


def plot_cartan_chamber(
    projections: np.ndarray,
    title: str = "Cartan Chamber Projection",
    figsize: tuple = (8, 8),
    save_path: Optional[str] = None
) -> None:
    """
    Plot projections in the positive Weyl chamber.

    For rank 2, plots directly. For higher rank, plots first 2 coordinates.

    Parameters
    ----------
    projections : np.ndarray
        Array of Cartan projections (each row is one projection)
    title : str
        Plot title
    figsize : tuple
        Figure size
    save_path : str, optional
        Path to save figure
    """
    fig, ax = plt.subplots(figsize=figsize)

    if projections.shape[1] >= 2:
        # Plot first two coordinates
        ax.scatter(projections[:, 0], projections[:, 1], s=8, alpha=0.6)
        ax.set_xlabel(r'$\log \sigma_1$', fontsize=12)
        ax.set_ylabel(r'$\log \sigma_2$', fontsize=12)
    else:
        # 1D case
        ax.hist(projections[:, 0], bins=50, alpha=0.7)
        ax.set_xlabel(r'$\log \sigma_1$', fontsize=12)
        ax.set_ylabel('Count', fontsize=12)

    ax.set_title(title, fontsize=14)
    ax.grid(True, alpha=0.3)

    if save_path:
        plt.savefig(save_path, dpi=150, bbox_inches='tight')

    plt.show()


def cartan_subalgebra_basis(lie_type: str, rank: int) -> np.ndarray:
    """
    Return a basis for the Cartan subalgebra.

    Parameters
    ----------
    lie_type : str
        Type of Lie algebra (A, B, C, D, etc.)
    rank : int
        Rank of the Lie algebra

    Returns
    -------
    np.ndarray
        Basis matrices for the Cartan subalgebra
    """
    if lie_type == "A":
        # Type A_n corresponds to sl(n+1, R)
        # Cartan subalgebra: diagonal traceless matrices
        n = rank + 1
        basis = []
        for i in range(rank):
            H = np.zeros((n, n))
            H[i, i] = 1
            H[i+1, i+1] = -1
            basis.append(H)
        return np.array(basis)

    elif lie_type == "B":
        # Type B_n corresponds to so(2n+1, R)
        # TODO: Implement for other types
        raise NotImplementedError(f"Type {lie_type} not yet implemented")

    else:
        raise ValueError(f"Unknown Lie type: {lie_type}")


def restricted_root_system(g: np.ndarray, theta: np.ndarray) -> dict:
    """
    Compute restricted root system for a real semisimple Lie algebra.

    Parameters
    ----------
    g : np.ndarray
        Lie algebra element
    theta : np.ndarray
        Cartan involution

    Returns
    -------
    dict
        Information about restricted roots
    """
    # This is a placeholder for more sophisticated root system calculations
    # Would need the full Lie algebra structure
    return {
        'roots': None,
        'root_spaces': None,
        'multiplicities': None
    }