#!/usr/bin/env python3
"""
patch_extension_metadata.py — Patch GNOME extension metadata.json to include a
shell-version entry and pin the version field above upstream.

Components:
  - patch_shell_version(metadata_path, shell_version): Add shell-version to
    metadata if not already present. Returns True if modified, False otherwise.
  - pin_version(metadata_path, min_version): Raise the "version" field to at
    least *min_version* so GNOME never sees a newer release on
    extensions.gnome.org and therefore never auto-updates over the local
    patches. Returns True if modified, False otherwise.
  - main(): CLI entry point that reads args and calls both patchers.

Usage (called from bash):
  python3 scripts/patch_extension_metadata.py <metadata.json> <shell-version> [pin-version]

If the shell-version is already listed and the version is already pinned high
enough, no changes are made.
"""

import json
import sys
from pathlib import Path

from rm_logging import LOGGER, log_call


@log_call(LOGGER)
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
        LOGGER.info("Patched shell-version in %s to include %s", path, shell_version)
        print(f"Patched shell-version in metadata.json to include {shell_version}")

    return modified


@log_call(LOGGER)
def pin_version(metadata_path: str | Path, min_version: int) -> bool:
    """Raise a GNOME extension's ``"version"`` field to at least *min_version*.

    GNOME Shell auto-updates extensions installed from extensions.gnome.org
    whenever the remote version is greater than the locally installed
    ``"version"``. Because this project patches an EGO-sourced extension in
    place, a later EGO release silently overwrites the local patches on the
    next shell reload. Pinning the local version above any plausible upstream
    release makes the remote version never appear newer, so the auto-update is
    never staged.

    Reads the JSON file at *metadata_path*, sets ``"version"`` to *min_version*
    when the current value is missing or lower, writes back only when a change
    was made, and returns whether the file was modified.

    Args:
        metadata_path: Path to the extension's metadata.json file.
        min_version: Minimum version integer to pin to (e.g. 9999).

    Returns:
        True if the file was modified, False if no change was needed or an
        error occurred.

    Raises:
        FileNotFoundError: If *metadata_path* does not exist.
        json.JSONDecodeError: If the file contains invalid JSON.

    Example:
        >>> import tempfile, pathlib
        >>> with tempfile.NamedTemporaryFile(suffix=".json", mode="w") as f:
        ...     _ = f.write('{"uuid": "test@ext", "version": 27}')
        ...     f.flush()
        ...     pin_version(pathlib.Path(f.name), 9999)
        True
    """
    path = Path(metadata_path)

    if not path.is_file():
        raise FileNotFoundError(f"Metadata file not found: {path}")

    with open(path, "r", encoding="utf-8") as f:
        data = json.load(f)

    try:
        current = int(data.get("version", 0))
    except (TypeError, ValueError):
        current = 0

    if current >= min_version:
        return False

    data["version"] = min_version
    with open(path, "w", encoding="utf-8") as f:
        json.dump(data, f)
    LOGGER.info("Pinned %s version to %s from %s", path, min_version, current)
    print(f"Pinned metadata.json version to {min_version} (was {current})")
    return True


@log_call(LOGGER)
def main() -> int:
    """CLI entry point for patch_extension_metadata.py.

    Expects two or three positional arguments:
      1. Path to metadata.json
      2. Shell version string
      3. Optional version-pin integer (raises "version" to outrank EGO)

    Returns:
        0 on success, 1 on error.
    """
    if len(sys.argv) not in (3, 4):
        LOGGER.error("Invalid metadata patcher usage: %r", sys.argv)
        print(
            f"Usage: {sys.argv[0]} <metadata.json> <shell-version> [pin-version]",
            file=sys.stderr,
        )
        return 1

    metadata_path = sys.argv[1]
    shell_version = sys.argv[2]
    pin = sys.argv[3] if len(sys.argv) == 4 else None

    try:
        modified = patch_shell_version(metadata_path, shell_version)
        if not modified:
            LOGGER.info("Shell version %s already present in %s", shell_version, metadata_path)
            print(f"Shell version '{shell_version}' already present — no changes made.")
        if pin is not None:
            pinned = pin_version(metadata_path, int(pin))
            if not pinned:
                LOGGER.info("Version already pinned at or above %s in %s", pin, metadata_path)
                print(f"Version already pinned at or above {pin} — no changes made.")
        return 0
    except FileNotFoundError as exc:
        LOGGER.error("Metadata file not found: %s", exc)
        print(f"Error: {exc}", file=sys.stderr)
        return 1
    except json.JSONDecodeError as exc:
        LOGGER.error("Invalid JSON in %s: %s", metadata_path, exc)
        print(f"Error: Invalid JSON in {metadata_path}: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
