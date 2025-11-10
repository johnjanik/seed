"""Example: Geodesic-based embedding of SU(4) using Isomap."""

import numpy as np
import matplotlib.pyplot as plt
from lie_groups import (
    random_sun,
    isomap_embedding,
    plot_embedding,
    compute_distance_matrix,
    analyze_embedding_quality
)


def main():
    """Embed SU(4) elements in 2D using geodesic distances."""
    print("Generating 400 random SU(4) matrices...")
    n_samples = 400
    matrices = random_sun(4, num_samples=n_samples)

    print("Computing geodesic distance matrix...")
    # Get embedding and distance matrix
    embedding, distance_matrix = isomap_embedding(
        matrices,
        n_components=2,
        n_neighbors=12,
        return_distance_matrix=True
    )

    print("Creating 2D embedding using Isomap...")
    plot_embedding(
        embedding,
        title="SU(4) Geodesic Embedding via Isomap",
        save_path="su4_isomap_embedding.png"
    )

    # Analyze embedding quality
    print("\nAnalyzing embedding quality...")
    quality = analyze_embedding_quality(embedding, distance_matrix)

    print("\nEmbedding Quality Metrics:")
    print(f"  Pearson correlation: {quality['pearson_correlation']:.3f}")
    print(f"  Spearman correlation: {quality['spearman_correlation']:.3f}")
    print(f"  Normalized stress: {quality['normalized_stress']:.3f}")
    print(f"  Mean distortion: {quality['mean_distortion']:.3f}")
    print(f"  Max distortion: {quality['max_distortion']:.3f}")

    print("\nPlot saved as su4_isomap_embedding.png")
    print("The embedding preserves geodesic distances as much as possible in 2D.")


if __name__ == "__main__":
    main()