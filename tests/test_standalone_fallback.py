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


def test_fallback_list_configurable_components():
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
            ["bash", str(INSTALL), "--list-configurable-components"],
            check=True,
            text=True,
            capture_output=True,
            env=env,
            cwd=str(ROOT),
        )
        ids = {ln.strip() for ln in completed.stdout.splitlines() if ln.strip()}
        assert "rm_refresh_interval" in ids
        assert "rm_panel_spacing" in ids


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


def test_fallback_configure_component_requires_framework():
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
            ["bash", str(INSTALL), "--configure-component", "rm_refresh_interval"],
            check=False,
            text=True,
            capture_output=True,
            env=env,
            cwd=str(ROOT),
        )
        assert completed.returncode == 1
        assert "requires the linux_installation_scripts_functions framework" in (
            completed.stderr + completed.stdout
        )


def test_fallback_preflight_aborts_not_continues():
    fallback = (ROOT / "lib" / "standalone_component_fallback.sh").read_text(
        encoding="utf-8"
    )
    assert "ERROR: preflight" in fallback
    assert "reported a problem (continuing)" not in fallback
    assert "refusing to clear receipt" in fallback


def test_default_components_run_with_errexit_and_last_component_off():
    """A trailing default-off row must not abort selection under installer flags."""
    script = f'''set -euo pipefail
msg() {{ :; }}
ISC_REPO_NAME=fixture
ISC_COMPONENTS=(
  'first|First|on|install_first'
  'second|Second|on|install_second'
  'optional|Optional|off|install_optional'
)
install_first() {{ printf 'first\\n'; }}
install_second() {{ printf 'second\\n'; }}
install_optional() {{ printf 'optional\\n'; }}
source "{ROOT / 'lib' / 'standalone_component_fallback.sh'}"
component_main --default
'''
    completed = subprocess.run(
        ["bash", "-c", script], text=True, capture_output=True, check=False
    )
    assert completed.returncode == 0, completed.stderr
    assert completed.stdout.splitlines() == ["first", "second"]
