#!/usr/bin/env python3
"""Tests for the centralized rm_logging utility."""

import logging
import sys
from pathlib import Path


sys.path.insert(0, str(Path("scripts").resolve()))

import rm_logging


def test_get_logger_creates_repo_log_file(tmp_path, monkeypatch):
    """The project logger should create its sink under repository .log."""
    monkeypatch.setattr(rm_logging, "_repo_root", lambda: tmp_path)

    logger = rm_logging.get_logger("resource-monitor-test-file", "resource-monitor.log")
    logger.info("configured")

    assert (tmp_path / ".log" / "resource-monitor.log").read_text(encoding="utf-8")
    assert logger.handlers


def test_get_logger_is_idempotent(tmp_path, monkeypatch):
    """Repeated logger requests should not attach duplicate handlers."""
    monkeypatch.setattr(rm_logging, "_repo_root", lambda: tmp_path)

    logger = rm_logging.get_logger("resource-monitor-test-idempotent", "resource-monitor.log")
    again = rm_logging.get_logger("resource-monitor-test-idempotent", "resource-monitor.log")

    assert again is logger
    assert len(logger.handlers) == 1


def test_get_logger_falls_back_to_null_handler(monkeypatch):
    """Logging setup should not raise when the file sink cannot be created."""

    def fail_repo_root() -> Path:
        raise OSError("unwritable")

    monkeypatch.setattr(rm_logging, "_repo_root", fail_repo_root)

    logger = rm_logging.get_logger("resource-monitor-test-null", "resource-monitor.log")

    assert any(isinstance(handler, logging.NullHandler) for handler in logger.handlers)


def test_log_call_records_arguments_and_return(caplog):
    """The tracing decorator should log bound arguments and the return value."""
    logger = logging.getLogger("resource-monitor-test-decorator")
    logger.handlers.clear()
    logger.propagate = True
    logger.setLevel(logging.DEBUG)

    @rm_logging.log_call(logger)
    def add(left: int, right: int = 1) -> int:
        return left + right

    with caplog.at_level(logging.DEBUG, logger="resource-monitor-test-decorator"):
        assert add(2, right=3) == 5

    messages = [record.getMessage() for record in caplog.records]
    assert any("ENTER test_rm_logging.py:test_log_call_records_arguments_and_return.<locals>.add(left=2, right=3)" in message for message in messages)
    assert any("EXIT test_rm_logging.py:test_log_call_records_arguments_and_return.<locals>.add -> 5" in message for message in messages)


def test_legacy_log_helper_is_removed():
    """Feature modules should use LOGGER directly rather than a log shim."""
    assert not hasattr(rm_logging, "log")


def test_settings_module_reuses_central_logger():
    """resource_monitor_settings should expose the project logger for compatibility."""
    import resource_monitor_settings

    assert resource_monitor_settings.LOGGER is rm_logging.LOGGER


def test_disks_module_reuses_central_logger():
    """resource_monitor_disks should expose the project logger for compatibility."""
    import resource_monitor_disks

    assert resource_monitor_disks.LOGGER is rm_logging.LOGGER
