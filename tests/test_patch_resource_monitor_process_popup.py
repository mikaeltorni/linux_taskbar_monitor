#!/usr/bin/env python3
"""Tests for patch_resource_monitor_process_popup.js — left-click process popup.

The patcher must rewire the extension's left-click (and Enter/Space keyboard
activation) from launching the configured task manager to opening an in-panel
popup menu that aggregates CPU%/RAM% per process name.
"""

import subprocess
from pathlib import Path


ROOT_DIR = Path(__file__).resolve().parents[1]
PATCH_SCRIPT = ROOT_DIR / "scripts" / "patch_resource_monitor_process_popup.js"

# Minimal upstream extension.js excerpt: imports, _setupAccessibility tooltip,
# _clickManager and _onKeyPressEvent — just enough for the patcher to find and
# edit each snippet it targets.
ORIGINAL_EXTENSION = """import Gio from "gi://Gio";
import Clutter from "gi://Clutter";

import * as Main from "resource:///org/gnome/shell/ui/main.js";
import * as PanelMenu from "resource:///org/gnome/shell/ui/panelMenu.js";
import * as Util from "resource:///org/gnome/shell/misc/util.js";

  class ResourceMonitor extends PanelMenu.Button {
    _setupAccessibility() {
      this._setPanelTooltip(
        _("Left-click launches the configured action.")
      );
    }

    _clickManager(actor, event) {
      switch (event.get_button()) {
        case 3: // Right-click
          if (this._rightClickStatus) {
            this._openPreferences();
          }

          return Clutter.EVENT_STOP;

        case 1: // Left-click
          this._launchPrimaryAction();

          return Clutter.EVENT_STOP;

        default:
          return Clutter.EVENT_PROPAGATE;
      }
    }

    _onKeyPressEvent(actor, event) {
      switch (event.get_key_symbol()) {
        case Clutter.KEY_Return:
        case Clutter.KEY_KP_Enter:
        case Clutter.KEY_space:
          this._launchPrimaryAction();
          return Clutter.EVENT_STOP;

        default:
          return Clutter.EVENT_PROPAGATE;
      }
    }

    _launchPrimaryAction() {
      Util.spawnCommandLine("gnome-system-monitor");
    }
  }
"""

EXPECTED_MARKER = "Process popup: total CPU/RAM aggregated per process name"


def _write_extension_fixture(tmp_path: Path) -> Path:
    extension = tmp_path / "extension.js"
    extension.write_text(ORIGINAL_EXTENSION, encoding="utf-8")
    return extension


def _run_patch(extension_path: Path) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["node", str(PATCH_SCRIPT), str(extension_path)],
        cwd=ROOT_DIR,
        capture_output=True,
        text=True,
        check=False,
    )


def test_patch_rewires_left_click_to_process_popup(tmp_path):
    """Left-click and keyboard activation must open the popup, not spawn."""
    extension = _write_extension_fixture(tmp_path)

    result = _run_patch(extension)
    assert result.returncode == 0, f"Patch failed: {result.stderr}"

    patched = extension.read_text(encoding="utf-8")
    assert EXPECTED_MARKER in patched
    # PopupMenu import added exactly once, next to PanelMenu.
    assert (
        patched.count(
            'import * as PopupMenu from "resource:///org/gnome/shell/ui/popupMenu.js";'
        )
        == 1
    )
    # Left-click case now toggles the menu; primary action no longer wired to
    # click or keyboard (its definition may remain, unused).
    assert "case 1: // Left-click\n          // Show per-process" in patched
    assert "this._toggleProcessMenu();" in patched
    assert (
        "case 1: // Left-click\n          this._launchPrimaryAction();"
        not in patched
    )
    assert (
        "case Clutter.KEY_space:\n          this._launchPrimaryAction();"
        not in patched
    )
    # Popup aggregates per name from ps output.
    assert '"ps", "-eo", "comm=,%cpu=,%mem="' in patched
    assert "_populateProcessMenu" in patched
    # Tooltip reflects the new behavior.
    assert 'Left-click shows per-process CPU and RAM usage.' in patched
    assert 'Left-click launches the configured action.' not in patched


def test_patch_is_idempotent(tmp_path):
    """Running the patch twice should be a no-op on the second run."""
    extension = _write_extension_fixture(tmp_path)

    first = _run_patch(extension)
    assert first.returncode == 0, f"First patch failed: {first.stderr}"

    second = _run_patch(extension)
    assert second.returncode == 0, f"Second patch failed: {second.stderr}"
    assert "already applied" in second.stdout

    again_dir = tmp_path / "other"
    again_dir.mkdir()
    again = _write_extension_fixture(again_dir)
    _run_patch(again)
    _run_patch(again)
    assert extension.read_text() == again.read_text()


def test_patch_fails_fast_on_unexpected_layout(tmp_path):
    """Missing upstream snippets must abort with a non-zero exit."""
    extension = tmp_path / "extension.js"
    extension.write_text("// not the extension\n", encoding="utf-8")

    result = _run_patch(extension)
    assert result.returncode == 1
    assert "aborting" in result.stderr
