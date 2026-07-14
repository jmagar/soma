"""Plain Python provider used for Soma provider runtime smoke checks."""

import platform


PROVIDER = {
    "name": "local-python-tools",
    "kind": "python",
    "title": "Local Python Tools",
    "description": "Self-contained Python sidecar tools for Soma provider smoke tests.",
}


def python_runtime_check(message: str = "ok", repeat: int = 1) -> dict:
    """Return a compact proof that the Python sidecar executed the provider."""
    repeat = max(1, min(int(repeat), 5))
    return {
        "ok": True,
        "runtime": "python-sidecar",
        "message": " ".join([message] * repeat),
        "python": platform.python_version(),
    }
