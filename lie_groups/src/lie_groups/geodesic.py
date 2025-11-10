"""Geodesic distance-based embeddings for Lie groups."""

import numpy as np
from sklearn.manifold import Isomap, MDS
from sklearn.decomposition import PCA
from typing import Optional, Literal, Callable
import matplotlib.pyplot as plt

from .core import bi_invariant_distance


def compute_distance_matrix(
    elements: list,
    distance_func: Optional[Callable] = None
) -> np.ndarray:
    """
    Compute pairwise distance matrix for a list of group elements.

    Parameters
    ----------
    elements : list
        List of group elements (matrices)
    distance_func : Callable, optional
        Distance function to use (defaults to bi_invariant_distance)

    Returns
    -------
    np.ndarray
        Symmetric distance matrix
    """
    if distance_func is None:
        distance_func = bi_invariant_distance

    n = len(elements)
    D = np.zeros((n, n))

    for i in range(n):
        for j in range(i+1, n):
            d = distance_func(elements[i], elements[j])
            D[i, j] = D[j, i] = d

    return D


def isomap_embedding(
    elements: list,
    n_components: int = 2,
    n_neighbors: int = 12,
    distance_func: Optional[Callable] = None,
    return_distance_matrix: bool = False
) -> np.ndarray:
    """
    Embed Lie group elements in Euclidean space using Isomap.

    Isomap preserves global geometric structure by approximating
    geodesic distances through a neighborhood graph.

    Parameters
    ----------
    elements : list
        List of group elements
    n_components : int, optional
        Dimension of the embedding space
    n_neighbors : int, optional
        Number of neighbors for graph construction
    distance_func : Callable, optional
        Distance function to use
    return_distance_matrix : bool, optional
        Whether to also return the distance matrix

    Returns
    -------
    np.ndarray or tuple
        Embedded coordinates (and distance matrix if requested)
    """
    # Compute distance matrix
    D = compute_distance_matrix(elements, distance_func)

    # Apply Isomap
    embedding = Isomap(
        n_neighbors=n_neighbors,
        n_components=n_components,
        metric='precomputed'
    ).fit_transform(D)

    if return_distance_matrix:
        return embedding, D
    return embedding


def geodesic_embedding(
    elements: list,
    method: Literal["isomap", "mds", "pca"] = "isomap",
    n_components: int = 2,
    distance_func: Optional[Callable] = None,
    **kwargs
) -> np.ndarray:
    """
    General geodesic-based embedding for Lie group elements.

    Parameters
    ----------
    elements : list
        List of group elements
    method : str
        Embedding method: "isomap", "mds", or "pca"
    n_components : int
        Dimension of the embedding
    distance_func : Callable, optional
        Distance function to use
    **kwargs
        Additional arguments for the specific method

    Returns
    -------
    np.ndarray
        Embedded coordinates
    """
    if method == "isomap":
        n_neighbors = kwargs.get('n_neighbors', 12)
        return isomap_embedding(
            elements,
            n_components=n_components,
            n_neighbors=n_neighbors,
            distance_func=distance_func
        )

    elif method == "mds":
        D = compute_distance_matrix(elements, distance_func)
        mds = MDS(
            n_components=n_components,
            dissimilarity='precomputed',
            random_state=kwargs.get('random_state', 42)
        )
        return mds.fit_transform(D)

    elif method == "pca":
        # Flatten matrices and apply PCA
        X = np.array([g.flatten() for g in elements])
        if np.iscomplexobj(X):
            # Handle complex matrices by stacking real and imaginary parts
            X = np.hstack([X.real, X.imag])
        pca = PCA(n_components=n_components)
        return pca.fit_transform(X)

    else:
        raise ValueError(f"Unknown embedding method: {method}")


def plot_embedding(
    embedding: np.ndarray,
    title: str = "Lie Group Embedding",
    labels: Optional[np.ndarray] = None,
    figsize: tuple = (8, 8),
    save_path: Optional[str] = None
) -> None:
    """
    Plot 2D embedding of Lie group elements.

    Parameters
    ----------
    embedding : np.ndarray
        2D embedded coordinates
    title : str
        Plot title
    labels : np.ndarray, optional
        Labels for coloring points
    figsize : tuple
        Figure size
    save_path : str, optional
        Path to save figure
    """
    fig, ax = plt.subplots(figsize=figsize)

    if labels is not None:
        scatter = ax.scatter(
            embedding[:, 0],
            embedding[:, 1],
            c=labels,
            cmap='viridis',
            s=8,
            alpha=0.6
        )
        plt.colorbar(scatter, ax=ax)
    else:
        ax.scatter(
            embedding[:, 0],
            embedding[:, 1],
            s=8,
            alpha=0.6
        )

    ax.set_xlabel('Component 1', fontsize=12)
    ax.set_ylabel('Component 2', fontsize=12)
    ax.set_title(title, fontsize=14)
    ax.set_aspect('equal')
    ax.grid(True, alpha=0.3)

    if save_path:
        plt.savefig(save_path, dpi=150, bbox_inches='tight')

    plt.show()


def analyze_embedding_quality(
    embedding: np.ndarray,
    distance_matrix: np.ndarray
) -> dict:
    """
    Analyze the quality of a 2D embedding.

    Parameters
    ----------
    embedding : np.ndarray
        2D embedded coordinates
    distance_matrix : np.ndarray
        Original distance matrix

    Returns
    -------
    dict
        Quality metrics including stress, correlation, etc.
    """
    from scipy.spatial.distance import pdist, squareform
    from scipy.stats import spearmanr, pearsonr

    # Compute embedding distances
    embed_dist = squareform(pdist(embedding))

    # Flatten for correlation (exclude diagonal)
    mask = np.triu_indices_from(distance_matrix, k=1)
    orig_flat = distance_matrix[mask]
    embed_flat = embed_dist[mask]

    # Compute metrics
    pearson_r, _ = pearsonr(orig_flat, embed_flat)
    spearman_r, _ = spearmanr(orig_flat, embed_flat)

    # Stress (normalized)
    stress = np.sqrt(np.sum((orig_flat - embed_flat)**2) / np.sum(orig_flat**2))

    return {
        'pearson_correlation': pearson_r,
        'spearman_correlation': spearman_r,
        'normalized_stress': stress,
        'mean_distortion': np.mean(np.abs(orig_flat - embed_flat)),
        'max_distortion': np.max(np.abs(orig_flat - embed_flat))
    }