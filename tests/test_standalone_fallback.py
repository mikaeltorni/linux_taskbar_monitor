#!/usr/bin/env python3
"""Standalone installer fallback when the shared component framework is missing."""

from __future__ import annotations

import os
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
INSTALL = ROOT / "install.sh"


def test_install_sh_sources_standalone_fallback():
    text = INSTALL.read_text(encoding="utf-8")
    assert "lib/standalone_component_fallback.sh" in text
    assert "ISC_FRAMEWORK_ACTIVE" in text
    # Must not hard-abort on a failed curl of the private framework.
    assert "source <(curl -fsSL" not in text


def test_fallback_list_components_stdout_is_clean():
    """Force framework miss; --list-components stdout must stay machine-readable."""
    with tempfile.TemporaryDirectory() as tmp:
        home = Path(tmp) / "home"
        home.mkdir()
        env = {
            **os.environ,
            "HOME": str(home),
            # Point past any real sibling / override; empty dir has no loader.
            "ISC_FUNCTIONS_DIR": str(Path(tmp) / "missing-framework"),
            # Avoid network: fake curl that always fails.
            "PATH": str(Path(tmp) / "bin") + ":" + os.environ.get("PATH", ""),
        }
        bin_dir = Path(tmp) / "bin"
        bin_dir.mkdir()
        curl = bin_dir / "curl"
        curl.write_text("#!/bin/sh\nexit 1\n", encoding="utf-8")
        curl.chmod(0o755)

        completed = subprocess.run(
            ["bash", str(INSTALL), "--list-components"],
            check=True,
            text=True,
            capture_output=True,
            env=env,
            cwd=str(ROOT),
        )
        # Warnings belong on stderr only.
        assert "standalone fallback" in completed.stderr
        lines = [ln for ln in completed.stdout.splitlines() if ln.strip()]
        assert lines, "expected component rows on stdout"
        for line in lines:
            parts = line.split("\t")
            assert len(parts) == 3, line
            assert parts[2] in ("on", "off"), line
        ids = {ln.split("\t")[0] for ln in lines}
        assert "rm_refresh_interval" in ids
        assert "rm_gradient_colors" in ids


def test_fallback_detect_contract():
    with tempfile.TemporaryDirectory() as tmp:
        home = Path(tmp) / "home"
        home.mkdir()
        env = {
            **os.environ,
            "HOME": str(home),
            "ISC_FUNCTIONS_DIR": str(Path(tmp) / "missing-framework"),
            "PATH": str(Path(tmp) / "bin") + ":" + os.environ.get("PATH", ""),
        }
        bin_dir = Path(tmp) / "bin"
        bin_dir.mkdir()
        (bin_dir / "curl").write_text("#!/bin/sh\nexit 1\n", encoding="utf-8")
        (bin_dir / "curl").chmod(0o755)

        completed = subprocess.run(
            ["bash", str(INSTALL), "--detect"],
            check=True,
            text=True,
            capture_output=True,
            env=env,
            cwd=str(ROOT),
        )
        rows = [ln for ln in completed.stdout.splitlines() if ln.strip()]
        assert rows
        for line in rows:
            cid, state = line.split("\t")
            assert state in ("installed", "absent"), line
            assert cid
