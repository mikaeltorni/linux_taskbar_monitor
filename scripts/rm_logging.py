#!/usr/bin/env python3
"""rm_logging.py — Centralized logging utility for the Resource Monitor scripts.

This is the single definition of the installer-compatible ``log`` helper shared
by every Python script under ``scripts/``. Centralizing it keeps the on-screen
format identical to the bash installer's ``msg`` output and avoids duplicating
the same function in each module.

Components:
  - LOG_LEVELS: Recognized severity names, ordered from most to least verbose.
  - log(level, message): Write a single ``[LEVEL] message`` line to stderr.

Usage:
  from rm_logging import log
  log("warn", "df command failed")
"""

from __future__ import annotations

import sys

# Severity names recognized by :func:`log`, ordered from most to least verbose.
# The format mirrors the bash installer's output so CLI logs read consistently
# whether they originate from shell or Python.
LOG_LEVELS = ("verbose", "debug", "info", "warn", "error")


def log(level: str, message: str) -> None:
    """Write a single log line to stderr in the installer-compatible format.

    The output format is ``[LEVEL] message`` followed by a newline, matching the
    bash installer's ``msg`` style so mixed shell/Python logs stay uniform. The
    level is upper-cased for display but is not otherwise validated, so callers
    may pass any of :data:`LOG_LEVELS`.

    Args:
        level: Severity name such as ``verbose``, ``debug``, ``info``, ``warn``,
            or ``error`` (see :data:`LOG_LEVELS`). Rendered in upper case.
        message: Human-readable message text to write.

    Returns:
        None. The line is written to ``sys.stderr`` as a side effect.
    """
    print(f"[{level.upper()}] {message}", file=sys.stderr)
