#!/usr/bin/env python3
"""Tests for patch_resource_monitor_eth_icon.js — hide ethernet display icon."""

import subprocess
from pathlib import Path


ROOT_DIR = Path(__file__).resolve().parents[1]
PATCH_SCRIPT = ROOT_DIR / "scripts" / "patch_resource_monitor_eth_icon.js"

# Minimal upstream mainGui.js excerpt: just enough of the ethernet group wiring
# for the patcher to find and edit its snippet. The icon is dropped but the
# value and unit labels stay.
ORIGINAL_MAIN_GUI = """import Clutter from "gi://Clutter";
import St from "gi://St";

function _replaceGroupChildren(group, appendChildren) {
  group.remove_all_children();
  appendChildren((child) => group.add_child(child));
}

function _appendSimpleChildren(icon, value, unit, addChild, iconsPosition) {
  if (iconsPosition === "left") {
    addChild(icon);
  }

  addChild(value);
  addChild(unit);

  if (iconsPosition !== "left") {
    addChild(icon);
  }
}

  _replaceGroupChildren(indicator._ethGroup, (addChild) =>
    _appendSimpleChildren(
      indicator._ethIcon,
      indicator._ethValue,
      indicator._ethUnit,
      addChild,
      iconsPosition
    )
  );

export { _appendSimpleChildren };
"""


EXPECTED_MARKER = "Ethernet icon removed: value/unit kept, icon omitted"


def _write_main_gui_fixture(tmp_path: Path) -> Path:
    panel_dir = tmp_path / "panel"
    panel_dir.mkdir()
    main_gui = panel_dir / "mainGui.js"
    main_gui.write_text(ORIGINAL_MAIN_GUI, encoding="utf-8")
    return main_gui


def _run_patch(main_gui_path: Path) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["node", str(PATCH_SCRIPT), str(main_gui_path)],
        cwd=ROOT_DIR,
        capture_output=True,
        text=True,
        check=False,
    )


def test_patch_removes_eth_icon(tmp_path):
    """Should drop the ethernet icon argument while keeping value/unit."""
    main_gui = _write_main_gui_fixture(tmp_path)

    result = _run_patch(main_gui)
    assert result.returncode == 0, f"Patch failed: {result.stderr}"

    patched = main_gui.read_text(encoding="utf-8")
    assert EXPECTED_MARKER in patched
    # Icon argument must be gone; value and unit references must remain.
    assert "indicator._ethIcon," not in patched
    assert "indicator._ethValue," in patched
    assert "indicator._ethUnit," in patched


def test_patch_is_idempotent(tmp_path):
    """Running the patch twice should be a no-op on the second run."""
    main_gui = _write_main_gui_fixture(tmp_path)

    first = _run_patch(main_gui)
    assert first.returncode == 0, f"First patch failed: {first.stderr}"

    second = _run_patch(main_gui)
    assert second.returncode == 0, f"Second patch failed: {second.stderr}"
    assert "already removed" in second.stdout

    again_dir = tmp_path / "other"
    again_dir.mkdir()
    again = _write_main_gui_fixture(again_dir)
    _run_patch(again)
    assert main_gui.read_text() == again.read_text()
