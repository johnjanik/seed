"""Main entry point for the lie-groups library."""

import argparse
import sys


def main():
    """Main CLI for lie-groups library."""
    parser = argparse.ArgumentParser(
        description="Lie Groups: Python library for Lie group visualization"
    )
    parser.add_argument(
        "--version",
        action="store_true",
        help="Show version information"
    )
    parser.add_argument(
        "--examples",
        action="store_true",
        help="Show available examples"
    )

    args = parser.parse_args()

    if args.version:
        from lie_groups import __version__
        print(f"lie-groups version {__version__}")
    elif args.examples:
        print("Available examples:")
        print("  python examples/su3_weyl_alcove.py")
        print("  python examples/geodesic_embedding.py")
        print("\nRun with UV:")
        print("  uv run python examples/su3_weyl_alcove.py")
    else:
        print("Lie Groups Library")
        print("==================")
        print("A Python library for visualizing and working with Lie groups.")
        print("\nQuick start:")
        print("  import lie_groups")
        print("  help(lie_groups)")
        print("\nFor examples, run: python main.py --examples")
        print("For version info: python main.py --version")


if __name__ == "__main__":
    main()
