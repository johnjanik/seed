"""Example: Visualizing SU(3) conjugacy classes via the Weyl alcove."""

import numpy as np
import matplotlib.pyplot as plt
from lie_groups import random_sun, plot_weyl_alcove


def main():
    """Generate and plot random SU(3) elements in the Weyl alcove."""
    print("Generating 5000 random SU(3) matrices...")

    # Generate random SU(3) matrices
    matrices = random_sun(3, num_samples=5000)

    # Plot in Weyl alcove
    print("Plotting Weyl alcove projection...")
    plot_weyl_alcove(
        matrices,
        title="SU(3) Conjugacy Classes in Weyl Alcove",
        save_path="su3_weyl_alcove.png"
    )

    print("Plot saved as su3_weyl_alcove.png")
    print("\nThe triangular region shows the fundamental domain for SU(3) conjugacy classes.")
    print("Each point represents a conjugacy class, with the alcove boundaries in red.")


if __name__ == "__main__":
    main()