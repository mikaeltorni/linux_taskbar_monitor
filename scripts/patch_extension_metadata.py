#!/usr/bin/env python3
"""
patch_extension_metadata.py — Patch GNOME extension metadata.json to include a
shell-version entry.

Components:
  - patch_shell_version(metadata_path, shell_version): Add shell-version to
    metadata if not already present. Returns True if modified, False otherwise.
  - main(): CLI entry point that reads args and calls patch_shell_version().

Usage (called from bash):
  python3 scripts/patch_extension_metadata.py <metadata.json> <shell-version>

If the shell-version is already listed, no changes are made.
"""

import json
import sys
from pathlib import Path


def patch_shell_version(metadata_path: str | Path, shell_version: str) -> bool:
    """Patch a GNOME extension metadata.json to include a shell-version entry.

    Reads the JSON file at *metadata_path*, ensures *shell_version* is present
    in the ``"shell-version"`` list (creating it if necessary), writes back
    only when a change was made, and returns whether the file was modified.

    Args:
        metadata_path: Path to the extension's metadata.json file.
        shell_version: Shell version string to add (e.g. "46").

    Returns:
        True if the file was modified, False if no change was needed or an
        error occurred.

    Raises:
        FileNotFoundError: If *metadata_path* does not exist.
        json.JSONDecodeError: If the file contains invalid JSON.

    Example:
        >>> import tempfile, pathlib
        >>> with tempfile.NamedTemporaryFile(suffix=".json", mode="w") as f:
        ...     f.write('{"uuid": "test@ext"}')
        ...     path = pathlib.Path(f.name)
        >>> patch_shell_version(path, "46")
        True
    """
    path = Path(metadata_path)

    if not path.is_file():
        raise FileNotFoundError(f"Metadata file not found: {path}")

    with open(path, "r", encoding="utf-8") as f:
        data = json.load(f)

    sv = data.get("shell-version")
    modified = False

    if isinstance(sv, list):
        if shell_version not in sv:
            sv.append(shell_version)
            data["shell-version"] = sv
            modified = True
    else:
        # Not a list — replace with a single-element list
        data["shell-version"] = [shell_version]
        modified = True

    if modified:
        with open(path, "w", encoding="utf-8") as f:
            json.dump(data, f)
        print(f"Patched shell-version in metadata.json to include {shell_version}")

    return modified


def main() -> int:
    """CLI entry point for patch_extension_metadata.py.

    Expects exactly two positional arguments:
      1. Path to metadata.json
      2. Shell version string

    Returns:
        0 on success, 1 on error.
    """
    if len(sys.argv) != 3:
        print(f"Usage: {sys.argv[0]} <metadata.json> <shell-version>", file=sys.stderr)
        return 1

    metadata_path = sys.argv[1]
    shell_version = sys.argv[2]

    try:
        modified = patch_shell_version(metadata_path, shell_version)
        if not modified:
            print(f"Shell version '{shell_version}' already present — no changes made.")
        return 0
    except FileNotFoundError as exc:
        print(f"Error: {exc}", file=sys.stderr)
        return 1
    except json.JSONDecodeError as exc:
        print(f"Error: Invalid JSON in {metadata_path}: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
