#!/usr/bin/env python3
"""Add sub-second refresh interval support to Resource Monitor."""

from pathlib import Path
import subprocess
import sys


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


def patch_extension(extension_dir: str | Path) -> None:
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

    subprocess.run(
        ["glib-compile-schemas", str(root / "schemas")],
        check=True,
    )


def main() -> int:
    if len(sys.argv) != 2:
        print(f"Usage: {sys.argv[0]} <extension-directory>", file=sys.stderr)
        return 1
    try:
        patch_extension(sys.argv[1])
    except (OSError, RuntimeError, subprocess.CalledProcessError) as exc:
        print(f"Failed to patch Resource Monitor refresh interval: {exc}", file=sys.stderr)
        return 1
    print("Patched Resource Monitor for a 0.5-second refresh interval")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
