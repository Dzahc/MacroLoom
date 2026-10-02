"""Check Python prerequisites without installing or importing missing tools."""

from importlib.metadata import PackageNotFoundError, version
import sys


PYTHON_VERSION = (3, 12, 14)
PINS = {
    "lizard": "1.24.0", "pathspec": "1.1.1", "Pygments": "2.21.0",
    "tree-sitter": "0.25.2", "tree-sitter-rust": "0.24.2",
    "tree-sitter-typescript": "0.23.2",
}


def check():
    failures = []
    if sys.version_info[:3] != PYTHON_VERSION:
        failures.append(f"Python: expected {'.'.join(map(str, PYTHON_VERSION))}, found {sys.version.split()[0]}")
    for package, expected in PINS.items():
        try:
            actual = version(package)
        except PackageNotFoundError:
            actual = "missing"
        if actual != expected:
            failures.append(f"{package}: expected {expected}, found {actual}")
    if failures:
        print("\n".join(failures), file=sys.stderr)
        print("Follow docs/agents/quality-gate.md setup; install scripts/requirements-quality.txt in .venv.", file=sys.stderr)
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(check())
