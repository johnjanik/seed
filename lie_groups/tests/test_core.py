"""Tests for core Lie group utilities."""

import numpy as np
import pytest
from lie_groups.core import (
    random_su,
    random_sun,
    matrix_log,
    bi_invariant_distance,
    polar_decomposition
)


class TestRandomSU:
    """Test random SU(n) generation."""

    def test_random_su_unitary(self):
        """Test that random_su produces unitary matrices."""
        for n in [2, 3, 4, 5]:
            U = random_su(n)
            # Check unitarity: U† U = I
            product = U.conj().T @ U
            np.testing.assert_allclose(product, np.eye(n), atol=1e-10)

    def test_random_su_determinant(self):
        """Test that random_su produces determinant 1."""
        for n in [2, 3, 4, 5]:
            U = random_su(n)
            det = np.linalg.det(U)
            np.testing.assert_allclose(det, 1.0, atol=1e-10)

    def test_random_sun_multiple(self):
        """Test generation of multiple SU(n) matrices."""
        matrices = random_sun(3, num_samples=10)
        assert len(matrices) == 10
        for U in matrices:
            assert U.shape == (3, 3)
            np.testing.assert_allclose(np.linalg.det(U), 1.0, atol=1e-10)


class TestMatrixLog:
    """Test matrix logarithm computation."""

    def test_matrix_log_identity(self):
        """Test log of identity matrix."""
        I = np.eye(3)
        L = matrix_log(I, hermitian=True)
        # log(I) should give zero matrix (up to branch cuts)
        np.testing.assert_allclose(L, np.zeros((3, 3)), atol=1e-10)

    def test_matrix_log_inverse(self):
        """Test that exp(log(U)) = U for small rotations."""
        # Create a small rotation
        theta = 0.1
        U = np.array([
            [np.cos(theta), -np.sin(theta)],
            [np.sin(theta), np.cos(theta)]
        ])
        L = matrix_log(U)
        U_recovered = scipy.linalg.expm(L)
        np.testing.assert_allclose(U_recovered, U, atol=1e-10)


class TestBiInvariantDistance:
    """Test bi-invariant distance computation."""

    def test_distance_identity(self):
        """Test distance from matrix to itself is zero."""
        U = random_su(3)
        d = bi_invariant_distance(U, U)
        assert abs(d) < 1e-10

    def test_distance_symmetry(self):
        """Test that distance is symmetric."""
        U = random_su(3)
        V = random_su(3)
        d1 = bi_invariant_distance(U, V)
        d2 = bi_invariant_distance(V, U)
        np.testing.assert_allclose(d1, d2)

    def test_distance_invariance(self):
        """Test left and right invariance."""
        U = random_su(3)
        V = random_su(3)
        W = random_su(3)

        # Original distance
        d_original = bi_invariant_distance(U, V)

        # Left invariance: d(WU, WV) = d(U, V)
        d_left = bi_invariant_distance(W @ U, W @ V)
        np.testing.assert_allclose(d_left, d_original, rtol=1e-10)

        # Right invariance: d(UW, VW) = d(U, V)
        d_right = bi_invariant_distance(U @ W, V @ W)
        np.testing.assert_allclose(d_right, d_original, rtol=1e-10)


class TestPolarDecomposition:
    """Test polar decomposition."""

    def test_polar_decomposition_properties(self):
        """Test that polar decomposition has correct properties."""
        # Generate random matrix
        A = np.random.randn(4, 4)

        U, P = polar_decomposition(A)

        # Check that A = UP
        np.testing.assert_allclose(A, U @ P, atol=1e-10)

        # Check that U is unitary
        np.testing.assert_allclose(U @ U.conj().T, np.eye(4), atol=1e-10)

        # Check that P is positive semidefinite
        eigenvalues = np.linalg.eigvalsh(P)
        assert np.all(eigenvalues >= -1e-10)  # Allow small numerical errors


# Add this import at the top if running tests that need scipy
import scipy.linalg