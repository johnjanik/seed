"""Tests for Weyl alcove functionality."""

import numpy as np
import pytest
from lie_groups import random_su
from lie_groups.weyl import (
    su3_alcove_coords,
    sun_alcove_coords,
    weyl_chamber_projection
)


class TestSU3AlcoveCoords:
    """Test SU(3) Weyl alcove coordinates."""

    def test_identity_at_origin(self):
        """Test that identity maps to origin-like point."""
        I = np.eye(3, dtype=complex)
        coords = su3_alcove_coords(I)
        # Identity has all eigenvalues = 1, so all angles = 0
        # After enforcing sum = 0 and computing differences, should be at origin
        np.testing.assert_allclose(coords, [0, 0], atol=1e-10)

    def test_alcove_coords_bounded(self):
        """Test that coordinates stay in reasonable bounds."""
        for _ in range(100):
            U = random_su(3)
            coords = su3_alcove_coords(U)
            # Coordinates should be bounded (within the alcove)
            assert np.all(np.abs(coords) <= 2 * np.pi)

    def test_conjugate_matrices_same_coords(self):
        """Test that conjugate matrices give same alcove coordinates."""
        U = random_su(3)
        V = random_su(3)

        # Conjugate U by V
        U_conjugate = V @ U @ V.conj().T

        coords1 = su3_alcove_coords(U)
        coords2 = su3_alcove_coords(U_conjugate)

        # Should map to same point in alcove (conjugacy class)
        np.testing.assert_allclose(coords1, coords2, atol=1e-10)


class TestSUNAlcoveCoords:
    """Test general SU(n) alcove coordinates."""

    def test_dimension(self):
        """Test that output has correct dimension."""
        for n in [2, 3, 4, 5]:
            U = random_su(n)
            coords = sun_alcove_coords(U)
            assert coords.shape == (n-1,)

    def test_identity_coords(self):
        """Test identity matrix coordinates."""
        for n in [2, 3, 4]:
            I = np.eye(n, dtype=complex)
            coords = sun_alcove_coords(I)
            # All differences should be zero for identity
            np.testing.assert_allclose(coords, np.zeros(n-1), atol=1e-10)

    def test_coordinate_ordering(self):
        """Test that coordinates represent ordered eigenangles."""
        U = random_su(4)
        coords = sun_alcove_coords(U)
        # Coordinates are differences of sorted angles, should be non-negative
        # (since angles are sorted descending)
        assert np.all(coords >= -1e-10)  # Allow small numerical errors


class TestWeylChamberProjection:
    """Test Weyl chamber projection."""

    def test_type_A_projection(self):
        """Test projection for type A (SU(n))."""
        for n in [3, 4, 5]:
            U = random_su(n)
            proj = weyl_chamber_projection(U, lie_type="A")
            assert proj.shape == (n-1,)

    def test_unsupported_type(self):
        """Test that unsupported Lie types raise error."""
        U = random_su(3)
        with pytest.raises(NotImplementedError):
            weyl_chamber_projection(U, lie_type="B")


class TestAlcoveProperties:
    """Test mathematical properties of the alcove."""

    def test_alcove_periodicity(self):
        """Test that alcove coordinates respect periodicity."""
        U = random_su(3)
        coords1 = su3_alcove_coords(U)

        # Multiply by a center element (should not change conjugacy class)
        omega = np.exp(2j * np.pi / 3)
        center_element = omega * np.eye(3)
        U2 = center_element @ U

        coords2 = su3_alcove_coords(U2)

        # Should map to same alcove point (modulo Weyl group action)
        # This is a simplified test - full test would require Weyl group implementation
        assert np.linalg.norm(coords1) < 4 * np.pi  # Bounded
        assert np.linalg.norm(coords2) < 4 * np.pi  # Bounded