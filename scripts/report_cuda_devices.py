#!/usr/bin/env python3
"""
report_cuda_devices — Query nvidia-smi and print a GSettings GPU device list.

Components:
  - log(level, msg): Timestamped logging helper (prints to stderr).
  - detect_nvidia_smi(): Check if nvidia-smi is available. Returns path or None.
  - parse_gpu_output(output): Parse nvidia-smi -L output into a list of device dicts.
  - format_gsettings_list(devices): Format GPU dicts for Resource Monitor's as schema.
  - get_gpu_devices(): Query nvidia-smi and return structured GPU info.
  - main(): CLI entry point — prints a GSettings string array to stdout.

Called via stdin heredoc from bash:
  gpu_devices="$(python3 scripts/report_cuda_devices.py)"
"""

import json
import re
import shutil
import subprocess
import sys


# ── Logging helper ───────────────────────────────────────────────────────────

def log(level: str, msg: str) -> None:
    """Print a timestamped log message to stderr.

    Args:
        level: Log level string (info, warn, error).
        msg: Message text.
    """
    print(f"[{level.upper()}] {msg}", file=sys.stderr)


# ── GPU detection ────────────────────────────────────────────────────────────

def detect_nvidia_smi() -> str | None:
    """Check if nvidia-smi is available in PATH.

    Returns:
        Path to nvidia-smi binary, or None if not found.
    """
    return shutil.which("nvidia-smi")


def parse_gpu_output(output: str) -> list[dict]:
    """Parse nvidia-smi -L output into a list of GPU device dicts.

    Each line is expected to match the pattern:
        GPU <N>: <Name> (UUID: <uuid>)

    Args:
        output: Raw text from `nvidia-smi -L`.

    Returns:
        List of dicts with keys: version, type, device, name, usage, memory, displayName.

    Example:
        >>> lines = "GPU 0: NVIDIA GeForce RTX 4090 (UUID: GPU-abc123)"
        >>> parse_gpu_output(lines)
        [{'version': 2, 'type': 'gpu', ...}]
    """
    entries = []
    for line in output.splitlines():
        match = re.search(
            r"GPU\s+\d+:\s+(.*?)\s+\(UUID:\s+([^)]+)\)",
            line.strip(),
        )
        if not match:
            continue
        name, uuid = match.groups()
        entries.append({
            "version": 2,
            "type": "gpu",
            "device": uuid,
            "name": name,
            "usage": True,
            "memory": True,
            "displayName": "",
        })
    return entries


def get_gpu_devices() -> list[dict]:
    """Query nvidia-smi and return structured GPU device information.

    Returns:
        List of GPU device dicts, or empty list if nvidia-smi is unavailable
        or an error occurred.
    """
    nvidia_smi = detect_nvidia_smi()
    if nvidia_smi is None:
        log("info", "nvidia-smi not found — no GPUs detected")
        return []

    try:
        output = subprocess.check_output(
            [nvidia_smi, "-L"],
            text=True,
            stderr=subprocess.DEVNULL,
        )
    except Exception as exc:
        log("warn", f"nvidia-smi -L failed: {exc}")
        return []

    return parse_gpu_output(output)


def format_gsettings_list(devices: list[dict]) -> str:
    """Format GPU devices as a GSettings string array of JSON objects.

    Resource Monitor stores ``gpudeviceslist`` as ``as``: an array of strings.
    Each string is a JSON object. Printing Python dict reprs looks close, but
    GSettings rejects it because the array elements are not strings.

    Args:
        devices: GPU device dictionaries from ``parse_gpu_output``.

    Returns:
        GSettings-formatted string array, e.g. ``['{"device": "GPU-abc"}']``.

    Example:
        >>> format_gsettings_list([{"device": "GPU-abc"}])
        '[\'{"device": "GPU-abc"}\']'
    """
    if not devices:
        return "[]"
    return "[" + ", ".join(repr(json.dumps(device)) for device in devices) + "]"


# ── CLI entry point ─────────────────────────────────────────────────────────

def main() -> int:
    """CLI entry point for report_cuda_devices.py.

    Queries nvidia-smi and prints a JSON array of GPU devices to stdout.
    Exits cleanly (return 0) if no GPUs are found or nvidia-smi is unavailable.

    Returns:
        0 on success, non-zero on error.
    """
    try:
        devices = get_gpu_devices()
        print(format_gsettings_list(devices))
        return 0
    except Exception as exc:
        log("error", f"Unexpected error: {exc}")
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
