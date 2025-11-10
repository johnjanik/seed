"""Core utilities for working with Lie groups."""

import numpy as np
import scipy.linalg as la
from typing import Optional, Union


def random_su(n: int) -> np.ndarray:
    """
    Generate a random SU(n) matrix using QR decomposition.

    Parameters
    ----------
    n : int
        Dimension of the matrix

    Returns
    -------
    np.ndarray
        Random n x n unitary matrix with determinant 1
    """
    # Generate random complex matrix
    Z = np.random.randn(n, n) + 1j * np.random.randn(n, n)

    # QR decomposition
    Q, R = la.qr(Z)

    # Make unitary
    D = np.diag(np.exp(-1j * np.angle(np.diag(R))))
    U = Q @ D

    # Force det=1
    U /= la.det(U) ** (1/n)

    return U


def random_sun(n: int, num_samples: int = 1) -> Union[np.ndarray, list]:
    """
    Generate random SU(n) matrices.

    Parameters
    ----------
    n : int
        Dimension of the matrices
    num_samples : int, optional
        Number of samples to generate

    Returns
    -------
    np.ndarray or list
        Single matrix if num_samples=1, list otherwise
    """
    if num_samples == 1:
        return random_su(n)
    return [random_su(n) for _ in range(num_samples)]


def matrix_log(U: np.ndarray, hermitian: bool = False) -> np.ndarray:
    """
    Compute matrix logarithm via eigendecomposition.

    Parameters
    ----------
    U : np.ndarray
        Unitary matrix
    hermitian : bool, optional
        Whether the matrix is Hermitian

    Returns
    -------
    np.ndarray
        Matrix logarithm
    """
    if hermitian:
        # Real eigenvalues for Hermitian matrices
        w, V = la.eigh(U)
        L = V @ np.diag(np.log(w)) @ V.conj().T
    else:
        # Complex eigenvalues for general unitary
        w, V = la.eig(U)
        # Use principal branch
        ang = np.angle(w)  # in (-pi, pi]
        L = V @ np.diag(1j * ang) @ la.inv(V)

    return L


def bi_invariant_distance(U: np.ndarray, V: np.ndarray) -> float:
    """
    Compute bi-invariant distance between two unitary matrices.

    Uses the Frobenius norm of log(U† V) as a proxy for geodesic distance.

    Parameters
    ----------
    U : np.ndarray
        First unitary matrix
    V : np.ndarray
        Second unitary matrix

    Returns
    -------
    float
        Distance between U and V
    """
    W = U.conj().T @ V
    L = matrix_log(W)
    return la.norm(L, 'fro')


def random_sln_real(n: int) -> np.ndarray:
    """
    Generate a random SL(n, R) matrix.

    Parameters
    ----------
    n : int
        Dimension of the matrix

    Returns
    -------
    np.ndarray
        Random n x n real matrix with determinant 1
    """
    # Generate random matrix
    A = np.random.randn(n, n)

    # Force det=1
    A /= np.linalg.det(A) ** (1/n)

    return A


def polar_decomposition(A: np.ndarray) -> tuple[np.ndarray, np.ndarray]:
    """
    Compute polar decomposition A = UP.

    Parameters
    ----------
    A : np.ndarray
        Input matrix

    Returns
    -------
    tuple[np.ndarray, np.ndarray]
        U (unitary) and P (positive semidefinite) such that A = UP
    """
    U, S, Vh = la.svd(A)
    P = Vh.conj().T @ np.diag(S) @ Vh
    U = U @ Vh
    return U, P