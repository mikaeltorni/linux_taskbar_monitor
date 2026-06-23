#!/usr/bin/env python3
"""
patch_resource_monitor_refresh.py — Add sub-second refresh interval support to
the Resource Monitor extension.

Upstream Resource Monitor only allows whole-second refresh intervals and
throttles GPU polling to a 5-second floor. This script rewrites the relevant
source files so the panel can refresh every 0.5 seconds (including GPU
usage/VRAM), then recompiles the GSettings schemas.

Components:
  - REPLACEMENTS: Mapping of extension-relative file paths to the (old, new)
    source substitutions applied to each file.
  - patch_extension(extension_dir): Apply all replacements and recompile schemas.
  - main(): CLI entry point — patches the extension directory given as argv[1].

Usage (called from bash):
  python3 scripts/patch_resource_monitor_refresh.py <extension-directory>
"""

from pathlib import Path
import subprocess
import sys

from rm_logging import LOGGER, log_call


REPLACEMENTS = {
    "extension.js": (
        ("this._settings.get_int(REFRESH_TIME)", "this._settings.get_double(REFRESH_TIME)"),
        ("GLib.timeout_add_seconds(", "GLib.timeout_add("),
        ("        this._refreshTime,\n", "        Math.round(this._refreshTime * 1000),\n"),
        # Upstream throttles GPU polling to a 5-second minimum, which keeps the
        # GPU usage/VRAM values from refreshing at the configured 0.5 s rate the
        # way CPU, RAM, and ethernet do. Lower the floor so GPU follows suit.
        (
            "const GPU_MIN_REFRESH_INTERVAL_SECONDS = 5;",
            "const GPU_MIN_REFRESH_INTERVAL_SECONDS = 0.5;",
        ),
    ),
    "services/settings.js": (
        (
            "indicator._settings.get_int(keys.REFRESH_TIME)",
            "indicator._settings.get_double(keys.REFRESH_TIME)",
        ),
    ),
    "prefs.js": (
        (
            "this._secondsSpinbutton = this._createSpinButton({\n"
            "        upper: 60,\n"
            "        step: 1,\n"
            "        page: 1,\n"
            "      });",
            "this._secondsSpinbutton = this._createSpinButton({\n"
            "        lower: 0.5,\n"
            "        upper: 60,\n"
            "        step: 0.5,\n"
            "        page: 1,\n"
            "        digits: 1,\n"
            "      });",
        ),
    ),
    "schemas/org.gnome.shell.extensions.resource-monitor.gschema.xml": (
        (
            '<key name="refreshtime" type="i">\n'
            "            <default>2</default>\n"
            '            <range min="1" max="60"/>',
            '<key name="refreshtime" type="d">\n'
            "            <default>0.5</default>\n"
            '            <range min="0.5" max="60"/>',
        ),
    ),
}


@log_call(LOGGER)
def patch_extension(extension_dir: str | Path) -> None:
    """Apply the sub-second refresh patches and recompile the schemas.

    Each file in REPLACEMENTS is read, its (old, new) substitutions are applied
    (skipping any already present so the patch is idempotent), and written back.
    Finally ``glib-compile-schemas`` is run so the modified gschema takes effect.

    Args:
        extension_dir: Path to the installed Resource Monitor extension directory
            (the folder containing extension.js, prefs.js, schemas/, etc.).

    Returns:
        None.

    Raises:
        RuntimeError: If a target file does not contain the expected upstream
            source to patch.
        OSError: If a source file cannot be read or written.
        subprocess.CalledProcessError: If schema compilation fails.
    """
    root = Path(extension_dir)
    for relative_path, replacements in REPLACEMENTS.items():
        path = root / relative_path
        content = path.read_text(encoding="utf-8")
        for old, new in replacements:
            if new in content:
                continue
            if old not in content:
                raise RuntimeError(f"Unsupported Resource Monitor source in {relative_path}")
            content = content.replace(old, new)
        path.write_text(content, encoding="utf-8")

    LOGGER.info("Compiling Resource Monitor schemas in %s", root / "schemas")
    subprocess.run(
        ["glib-compile-schemas", str(root / "schemas")],
        check=True,
    )


@log_call(LOGGER)
def main() -> int:
    """CLI entry point for patch_resource_monitor_refresh.py.

    Expects exactly one positional argument: the path to the installed
    Resource Monitor extension directory.

    Returns:
        0 on success, 1 on a usage error or when patching fails.
    """
    if len(sys.argv) != 2:
        LOGGER.error("Invalid refresh patcher usage: %r", sys.argv)
        print(f"Usage: {sys.argv[0]} <extension-directory>", file=sys.stderr)
        return 1
    try:
        patch_extension(sys.argv[1])
    except (OSError, RuntimeError, subprocess.CalledProcessError) as exc:
        LOGGER.error("Failed to patch Resource Monitor refresh interval: %s", exc)
        print(f"Failed to patch Resource Monitor refresh interval: {exc}", file=sys.stderr)
        return 1
    print("Patched Resource Monitor for a 0.5-second refresh interval")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
