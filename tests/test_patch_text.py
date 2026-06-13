#!/usr/bin/env python3
"""Tests for scripts/lib/patch_text.js — shared snippet-replacement helpers.

These helpers are pure text transforms used by the JavaScript patchers. They are
exercised here by requiring the CommonJS module from a small Node harness and
asserting on the returned content and process exit behavior.
"""

import json
import subprocess
from pathlib import Path

ROOT_DIR = Path(__file__).resolve().parents[1]
MODULE = ROOT_DIR / "scripts" / "lib" / "patch_text.js"


def _run(call: str) -> subprocess.CompletedProcess:
    """Run a Node snippet that requires the module as `patch`.

    The snippet should print the function's return value (a string) to stdout
    via `process.stdout.write`. Process exits and stderr are captured so callers
    can assert on the exit-on-failure behavior of the helpers.
    """
    # Route the helpers' progress logs to stderr so only the explicit return
    # value written by `call` lands on stdout.
    harness = f"console.log = (...a) => console.error(...a);\n"
    script = f"{harness}const patch = require({json.dumps(str(MODULE))});\n{call}"
    return subprocess.run(
        ["node", "-e", script],
        cwd=ROOT_DIR,
        capture_output=True,
        text=True,
        check=False,
    )


def test_replace_known_snippet_replaces_first_match():
    call = (
        'process.stdout.write(patch.replaceKnownSnippet('
        '"a OLD b", ["MISSING", "OLD"], "NEW", "MARKER", "thing"));'
    )
    result = _run(call)
    assert result.returncode == 0, result.stderr
    assert result.stdout == "a NEW b"


def test_replace_known_snippet_already_patched_is_noop():
    call = (
        'process.stdout.write(patch.replaceKnownSnippet('
        '"has MARKER already", ["OLD"], "NEW", "MARKER", "thing"));'
    )
    result = _run(call)
    assert result.returncode == 0, result.stderr
    assert result.stdout == "has MARKER already"


def test_replace_known_snippet_exits_when_no_match():
    call = (
        'patch.replaceKnownSnippet('
        '"nothing here", ["OLD"], "NEW", "MARKER", "thing");'
    )
    result = _run(call)
    assert result.returncode == 1
    assert "Could not find target code in thing" in result.stderr


def test_replace_known_snippet_with_migration_applies_migrations():
    call = (
        'process.stdout.write(patch.replaceKnownSnippetWithMigration('
        '"keep FROM here FROM", ["OLD"], "NEW", "DONE", '
        '[{from: "FROM", to: "TO", all: true}], "thing"));'
    )
    result = _run(call)
    assert result.returncode == 0, result.stderr
    assert result.stdout == "keep TO here TO"


def test_replace_known_snippet_with_migration_respects_required_marker():
    # requiredMarker absent -> migration skipped -> falls through to snippet patch.
    call = (
        'process.stdout.write(patch.replaceKnownSnippetWithMigration('
        '"OLD body", ["OLD"], "NEW", "DONE", '
        '[{from: "body", to: "X", requiredMarker: "NOPE"}], "thing"));'
    )
    result = _run(call)
    assert result.returncode == 0, result.stderr
    assert result.stdout == "NEW body"


def test_replace_known_snippet_or_patterns_prefers_patterns():
    call = (
        'process.stdout.write(patch.replaceKnownSnippetOrPatterns('
        '"value=42", ["value=42"], [/value=\\d+/], "value=NEW", "DONE", "thing"));'
    )
    result = _run(call)
    assert result.returncode == 0, result.stderr
    assert result.stdout == "value=NEW"


def test_replace_known_snippet_or_patterns_falls_back_to_snippet():
    call = (
        'process.stdout.write(patch.replaceKnownSnippetOrPatterns('
        '"literal here", ["literal"], [/nomatch/], "LIT", "DONE", "thing"));'
    )
    result = _run(call)
    assert result.returncode == 0, result.stderr
    assert result.stdout == "LIT here"


def test_replace_known_snippet_or_patterns_exits_when_no_match():
    call = (
        'patch.replaceKnownSnippetOrPatterns('
        '"x", ["y"], [/z/], "N", "DONE", "thing");'
    )
    result = _run(call)
    assert result.returncode == 1
    assert "Could not find target code in thing" in result.stderr


def test_replace_known_snippet_optional_returns_unchanged_when_absent():
    call = (
        'process.stdout.write(patch.replaceKnownSnippetOptional('
        '"unchanged", ["OLD"], "NEW", "MARKER", "thing"));'
    )
    result = _run(call)
    assert result.returncode == 0, result.stderr
    assert result.stdout == "unchanged"


def test_replace_known_snippet_optional_replaces_when_present():
    call = (
        'process.stdout.write(patch.replaceKnownSnippetOptional('
        '"a OLD b", ["OLD"], "NEW", "MARKER", "thing"));'
    )
    result = _run(call)
    assert result.returncode == 0, result.stderr
    assert result.stdout == "a NEW b"


if __name__ == "__main__":
    import pytest

    raise SystemExit(pytest.main([__file__, "-v"]))
