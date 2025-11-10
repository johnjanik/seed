"""Tests for geodesic embedding functionality."""

import numpy as np
import pytest
from lie_groups import random_sun
from lie_groups.geodesic import (
    compute_distance_matrix,
    isomap_embedding,
    geodesic_embedding,
    analyze_embedding_quality
)


class TestDistanceMatrix:
    """Test distance matrix computation."""

    def test_distance_matrix_symmetry(self):
        """Test that distance matrix is symmetric."""
        matrices = random_sun(3, num_samples=10)
        D = compute_distance_matrix(matrices)

        # Check symmetry
        np.testing.assert_allclose(D, D.T, atol=1e-10)

    def test_distance_matrix_diagonal(self):
        """Test that diagonal is zero."""
        matrices = random_sun(3, num_samples=10)
        D = compute_distance_matrix(matrices)

        # Diagonal should be zero (distance to self)
        np.testing.assert_allclose(np.diag(D), 0, atol=1e-10)

    def test_distance_matrix_nonnegative(self):
        """Test that all distances are non-negative."""
        matrices = random_sun(3, num_samples=10)
        D = compute_distance_matrix(matrices)

        assert np.all(D >= -1e-10)  # Allow small numerical errors


class TestIsomapEmbedding:
    """Test Isomap embedding."""

    def test_isomap_embedding_shape(self):
        """Test that embedding has correct shape."""
        n_samples = 50
        n_components = 2
        matrices = random_sun(4, num_samples=n_samples)

        embedding = isomap_embedding(matrices, n_components=n_components)

        assert embedding.shape == (n_samples, n_components)

    def test_isomap_with_distance_matrix(self):
        """Test returning distance matrix."""
        matrices = random_sun(3, num_samples=20)

        embedding, D = isomap_embedding(
            matrices,
            n_components=2,
            return_distance_matrix=True
        )

        assert embedding.shape == (20, 2)
        assert D.shape == (20, 20)
        assert np.all(D >= -1e-10)


class TestGeodesicEmbedding:
    """Test general geodesic embedding function."""

    def test_isomap_method(self):
        """Test Isomap method."""
        matrices = random_sun(3, num_samples=30)
        embedding = geodesic_embedding(matrices, method="isomap", n_components=2)
        assert embedding.shape == (30, 2)

    def test_mds_method(self):
        """Test MDS method."""
        matrices = random_sun(3, num_samples=30)
        embedding = geodesic_embedding(matrices, method="mds", n_components=2)
        assert embedding.shape == (30, 2)

    def test_pca_method(self):
        """Test PCA method."""
        matrices = random_sun(3, num_samples=30)
        embedding = geodesic_embedding(matrices, method="pca", n_components=2)
        assert embedding.shape == (30, 2)

    def test_invalid_method(self):
        """Test that invalid method raises error."""
        matrices = random_sun(3, num_samples=10)
        with pytest.raises(ValueError):
            geodesic_embedding(matrices, method="invalid", n_components=2)


class TestEmbeddingQuality:
    """Test embedding quality analysis."""

    def test_quality_metrics(self):
        """Test that quality metrics are computed correctly."""
        # Create simple test case
        n_samples = 20
        matrices = random_sun(3, num_samples=n_samples)

        # Get embedding and distance matrix
        embedding, D = isomap_embedding(
            matrices,
            n_components=2,
            return_distance_matrix=True
        )

        # Analyze quality
        quality = analyze_embedding_quality(embedding, D)

        # Check that all metrics are present and reasonable
        assert 'pearson_correlation' in quality
        assert 'spearman_correlation' in quality
        assert 'normalized_stress' in quality
        assert 'mean_distortion' in quality
        assert 'max_distortion' in quality

        # Correlations should be between -1 and 1
        assert -1 <= quality['pearson_correlation'] <= 1
        assert -1 <= quality['spearman_correlation'] <= 1

        # Stress and distortions should be non-negative
        assert quality['normalized_stress'] >= 0
        assert quality['mean_distortion'] >= 0
        assert quality['max_distortion'] >= 0

    def test_perfect_embedding(self):
        """Test quality metrics for a perfect 2D embedding."""
        # Create points already in 2D
        points_2d = np.random.randn(10, 2)

        # Compute distance matrix
        from scipy.spatial.distance import pdist, squareform
        D = squareform(pdist(points_2d))

        # "Embedding" is just the original points
        quality = analyze_embedding_quality(points_2d, D)

        # Should have perfect correlation and zero stress
        np.testing.assert_allclose(quality['pearson_correlation'], 1.0, atol=1e-10)
        np.testing.assert_allclose(quality['normalized_stress'], 0.0, atol=1e-10)