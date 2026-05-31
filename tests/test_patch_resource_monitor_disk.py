#!/usr/bin/env python3
"""Tests for patch_resource_monitor_disk.js — fixture-based extension patching."""

import subprocess
import sys
from pathlib import Path


ROOT_DIR = Path(__file__).resolve().parents[1]
PATCH_SCRIPT = ROOT_DIR / "scripts" / "patch_resource_monitor_disk.js"


ORIGINAL_DISK_CONTAINER = """export const DiskContainerSpace = GObject.registerClass(
  class DiskContainerSpace extends DiskContainer {
    add_element(filesystem, label) {
      this._elementsPath.push(filesystem);

      this._elementsName[filesystem] = _createNameLabel(label);

      this._elementsValue[filesystem] = _createValueLabel("--");

      this._elementsUnit[filesystem] = _createUnitLabel("KB");

      this.add_child(this._elementsName[filesystem]);
      this.add_child(this._elementsValue[filesystem]);
      this.add_child(this._elementsUnit[filesystem]);
    }

    update_element_value(filesystem, value, unit, style = "") {
      if (this._elementsValue[filesystem]) {
        this._elementsValue[filesystem].text = value;
        this._elementsValue[filesystem].style = style;
        this._elementsUnit[filesystem].text = unit;
      }
    }
  }
);"""


ORIGINAL_EXTENSION_DISK_LIST = """      this._diskDevices.forEach((device) => {
        if (device.stats) {
          this._diskStatsBox.add_element(device.device, device.displayName);
        }

        if (device.space) {
          this._diskSpaceBox.add_element(device.device, device.displayName);
        }
      });"""


ORIGINAL_DISK_REFRESH = """          indicator._diskSpaceBox.update_element_value(
            entry.filesystem,
            `${indicator._getValueFixed(display.value, "diskSpace")}`,
            display.unit,
            indicator._getUsageColor(display.value, indicator._diskSpaceColors)
          );"""


CURRENT_UPSTREAM_DISK_REFRESH = """          const display = buildDiskSpaceDisplay(entry, {
            monitor: indicator._diskSpaceMonitor,
            unitType: indicator._diskSpaceUnitType,
            unitMeasure: indicator._diskSpaceUnitMeasure,
            scaleBase: indicator._dataScaleBase,
          });

          indicator._diskSpaceBox.update_element_value(
            entry.filesystem,
            display.isPercent
              ? `${display.value}`
              : `${indicator._getValueFixed(display.value)}`,
            display.unit,
            indicator._getUsageColor(display.value, indicator._diskSpaceColors)
          );"""


def _write_extension_fixture(tmp_path: Path, container_source: str, refresh_source: str) -> Path:
    """Create the minimal Resource Monitor file tree used by the patch script."""
    panel_dir = tmp_path / "panel"
    services_dir = tmp_path / "services"
    panel_dir.mkdir()
    services_dir.mkdir()

    containers_path = panel_dir / "containers.js"
    refreshers_path = services_dir / "refreshers.js"
    extension_path = tmp_path / "extension.js"

    containers_path.write_text(container_source, encoding="utf-8")
    refreshers_path.write_text(refresh_source, encoding="utf-8")
    extension_path.write_text(ORIGINAL_EXTENSION_DISK_LIST, encoding="utf-8")
    return containers_path


def _run_patch(containers_path: Path) -> subprocess.CompletedProcess:
    """Run the disk patch script against a fixture extension tree."""
    return subprocess.run(
        ["node", str(PATCH_SCRIPT), str(containers_path)],
        cwd=ROOT_DIR,
        capture_output=True,
        text=True,
        check=False,
    )


def test_patch_initializes_secondary_disk_labels(tmp_path):
    """Should initialize and clear secondary labels so disk rows render."""
    containers_path = _write_extension_fixture(
        tmp_path,
        ORIGINAL_DISK_CONTAINER,
        ORIGINAL_DISK_REFRESH,
    )

    result = _run_patch(containers_path)

    assert result.returncode == 0, result.stderr
    patched = containers_path.read_text(encoding="utf-8")
    assert "this._elementsSecondaryValue = [];" in patched
    assert "this._elementsSecondaryUnit = [];" in patched
    assert 'this._elementsSecondaryUnit[filesystem] = _createUnitLabel("%", [' in patched
    assert "cleanup_elements()" in patched


def test_patch_uses_free_disk_gb_and_activity_percent(tmp_path):
    """Should render remaining disk space in GB with live activity percent."""
    refresh_source = (
        'import {\n'
        '  getBaseStorageUnit,\n'
        '  getFixedDataUnitForMeasure,\n'
        '} from "../runtime/metrics.js";\n'
        '\n'
        'export function refreshDiskSpaceValue(indicator) {\n'
        '        return {\n'
        '          filesystem: device.device,\n'
        '          usedBytes: Math.max(0, size - free),\n'
        '          availableBytes: Math.max(0, free),\n'
        '          usedPercent: size > 0 ? Math.round((100 * (size - free)) / size) : 0,\n'
        '        };\n'
        '  const display = buildDiskSpaceDisplay(entry, {});\n'
        f'{ORIGINAL_DISK_REFRESH}\n'
        '}\n'
    )
    containers_path = _write_extension_fixture(
        tmp_path,
        ORIGINAL_DISK_CONTAINER,
        refresh_source,
    )

    result = _run_patch(containers_path)

    assert result.returncode == 0, result.stderr
    refreshers = (tmp_path / "services" / "refreshers.js").read_text(encoding="utf-8")
    assert "getDataScaleFactor" not in refreshers
    assert "function getDiskSpaceActivityPercent(indicator, filesystem)" in refreshers
    assert "_diskSpaceActivitySamples" in refreshers
    assert "ioTimeMs" in refreshers
    assert "activityPercent" in refreshers
    assert 'monitor: "free"' in refreshers
    assert 'unitType: "numeric"' in refreshers
    assert 'unitMeasure: "g"' in refreshers
    assert 'indicator._getUsageColor(entry.usedPercent, indicator._diskSpaceColors)' in refreshers
    assert '`${indicator._getValueFixed(activityPercent, "diskSpace")}`' in refreshers
    assert '"%"' in refreshers
    assert 'monitor: "used"' not in refreshers
    assert "devicePath: device.device" in refreshers
    assert "filesystem: device.mountPoint || device.device" in refreshers
    assert "getDiskSpaceActivityPercent(indicator, entry.devicePath)" in refreshers


def test_patch_current_upstream_disk_refresh_shape(tmp_path):
    """Should patch Resource Monitor v27 refreshers without exact old text."""
    refresh_source = (
        'import GLib from "gi://GLib";\n'
        'import { buildDiskSpaceDisplay } from "../runtime/disk.js";\n'
        '\n'
        'export function refreshDiskSpaceValue(indicator) {\n'
        '  results.forEach((result) => {\n'
        '    const entry = result.value;\n'
        f'{CURRENT_UPSTREAM_DISK_REFRESH}\n'
        '  });\n'
        '}\n'
    )
    containers_path = _write_extension_fixture(
        tmp_path,
        ORIGINAL_DISK_CONTAINER,
        refresh_source,
    )

    result = _run_patch(containers_path)

    assert result.returncode == 0, result.stderr
    refreshers = (tmp_path / "services" / "refreshers.js").read_text(encoding="utf-8")
    assert "const display = buildDiskSpaceDisplay(entry" not in refreshers
    assert "getDiskSpaceActivityPercent(indicator, entry.devicePath)" in refreshers
    assert "update_element_secondary_value" in refreshers


def test_patch_keys_disk_space_rows_by_mount_point(tmp_path):
    """Should let / and /home render as separate rows on the same device."""
    containers_path = _write_extension_fixture(
        tmp_path,
        ORIGINAL_DISK_CONTAINER,
        ORIGINAL_DISK_REFRESH,
    )

    result = _run_patch(containers_path)

    assert result.returncode == 0, result.stderr
    extension = (tmp_path / "extension.js").read_text(encoding="utf-8")
    assert "const diskSpaceKey = device.mountPoint || device.device;" in extension
    assert "this._diskSpaceBox.add_element(diskSpaceKey, device.displayName);" in extension


if __name__ == "__main__":
    import pytest

    raise SystemExit(pytest.main([__file__, "-v"]))
