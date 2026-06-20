#!/usr/bin/env python3
"""Tests for the centralized rm_logging utility."""

import sys
from pathlib import Path


sys.path.insert(0, str(Path("scripts").resolve()))

import rm_logging


def test_log_writes_uppercase_level_to_stderr(capsys):
    """A log call should emit ``[LEVEL] message`` to stderr with a trailing newline."""
    rm_logging.log("warn", "df command failed")

    captured = capsys.readouterr()
    assert captured.err == "[WARN] df command failed\n"
    assert captured.out == ""


def test_log_accepts_every_documented_level(capsys):
    """Each documented severity name should render as its upper-cased label."""
    for level in rm_logging.LOG_LEVELS:
        rm_logging.log(level, "msg")

    err_lines = capsys.readouterr().err.strip().splitlines()
    assert err_lines == [f"[{level.upper()}] msg" for level in rm_logging.LOG_LEVELS]


def test_shared_log_is_reexported_by_settings_module():
    """resource_monitor_settings.log must be the centralized rm_logging.log."""
    import resource_monitor_settings

    assert resource_monitor_settings.log is rm_logging.log


def test_shared_log_is_reused_by_disks_module():
    """resource_monitor_disks.log must be the centralized rm_logging.log."""
    import resource_monitor_disks

    assert resource_monitor_disks.log is rm_logging.log
