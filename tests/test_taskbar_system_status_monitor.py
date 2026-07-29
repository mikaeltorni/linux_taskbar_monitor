#!/usr/bin/env python3
"""Tests for delegating taskbar system status monitor installation."""

import os
import subprocess
from pathlib import Path


ROOT_DIR = Path(__file__).resolve().parents[1]


def test_gnome_extension_module_installs_resource_monitor():
    """Resource Monitor core should patch and configure the downloaded extension."""
    source = (ROOT_DIR / "lib" / "gnome_extensions.sh").read_text(encoding="utf-8")

    assert "install_resource_monitor_core()" in source
    assert "rm_monitor patch-refresh" in source
    assert "glib-compile-schemas" in source
    # Refresh interval is configured through the persisted millisecond setting.
    assert 'resource_monitor_refresh_seconds' in source
    assert "curl -fL" in source
    assert "netethstatus false" not in source
    assert "rm_monitor configure-resource-monitor" in source


def test_refresh_interval_is_exposed_as_nested_installer_configuration():
    """Both standalone and master menus discover the same repository-owned field."""
    install = (ROOT_DIR / "install.sh").read_text()
    components = (ROOT_DIR / "installer" / "components.sh").read_text()

    assert "--list-configurable-components" in install
    assert "--list-component-config-values" in install
    assert "--configure-component" in install
    assert "rm_refresh_interval|Resource Monitor update time" in components
    assert "configure_resource_monitor_refresh_interval" in components
    assert "resource_monitor_refresh_interval_status" in components


def test_panel_spacing_is_exposed_as_nested_installer_configuration():
    """Panel spacing mode (stable/compact) is a repository-owned configurable component."""
    install = (ROOT_DIR / "install.sh").read_text()
    components = (ROOT_DIR / "installer" / "components.sh").read_text()
    lib = (ROOT_DIR / "lib" / "gnome_extensions.sh").read_text()

    assert "--list-configurable-components" in install
    assert "rm_panel_spacing|Resource Monitor panel spacing" in components
    assert "configure_resource_monitor_spacing" in components
    assert "resource_monitor_spacing_status" in components
    assert "apply_resource_monitor_spacing_mode" in lib
    assert "resource_monitor_spacing_mode" in lib
    # The persisted file and env override seed the compact choice.
    assert "RESOURCE_MONITOR_SPACING_MODE:-stable" in install
    assert "resource_monitor_spacing_file" in lib


def test_refresh_interval_configurator_has_requested_bounds_and_default():
    """The editable field uses milliseconds and rejects values outside 100..2000."""
    source = (ROOT_DIR / "lib" / "gnome_extensions.sh").read_text()

    assert 'RESOURCE_MONITOR_REFRESH_INTERVAL_MS:-500' in source
    assert "value < 100 || value > 2000" in source
    assert 'read -r -e -i "$current"' in source
    assert 'printf \'%s ms\\n\'' in source


def test_refresh_interval_persistence_validates_boundaries(tmp_path):
    """Only inclusive 100..2000 ms integer values are persisted."""
    script = f'''set -euo pipefail
TARGET_HOME="$HOME"
SCRIPT_DIR="{ROOT_DIR}"
RESOURCE_MONITOR_REFRESH_INTERVAL_MS=500
msg() {{ :; }}
run_as_target() {{ "$@"; }}
source "{ROOT_DIR / 'lib' / 'gnome_extensions.sh'}"
persist_resource_monitor_refresh_interval 100
test "$(resource_monitor_refresh_interval_ms)" = 100
persist_resource_monitor_refresh_interval 2000
test "$(resource_monitor_refresh_interval_ms)" = 2000
! persist_resource_monitor_refresh_interval 99
! persist_resource_monitor_refresh_interval 2001
! persist_resource_monitor_refresh_interval 500.5
test "$(resource_monitor_refresh_interval_ms)" = 2000
'''
    env = os.environ.copy()
    env["HOME"] = str(tmp_path)
    subprocess.run(["bash", "-c", script], check=True, env=env)


def test_optional_resource_monitor_tweaks_are_separate_components():
    """The gradient/VRAM/per-disk tweaks must be selectable, not baked into core."""
    lib = (ROOT_DIR / "lib" / "gnome_extensions.sh").read_text(encoding="utf-8")
    components = (ROOT_DIR / "installer" / "components.sh").read_text(encoding="utf-8")

    for fn in (
        "patch_resource_monitor_gradient_colors()",
        "patch_resource_monitor_vram()",
        "patch_resource_monitor_per_disk()",
    ):
        assert fn in lib, fn

    # Each tweak is wired as its own component id.
    for comp in ("rm_gradient_colors|", "rm_vram|", "rm_per_disk|"):
        assert comp in components, comp

    # The mandatory core is not a deselectable component.
    assert "configure_resource_monitor_extension" not in components
    assert "resource_monitor|" not in components


def test_dash_to_panel_is_not_owned_by_system_monitor_repo():
    """Dash-to-Panel layout belongs to linux_configuration_setup, not here."""
    components = (ROOT_DIR / "installer" / "components.sh").read_text(encoding="utf-8")
    install = (ROOT_DIR / "install.sh").read_text(encoding="utf-8")

    assert "dash_to_panel|" not in components
    assert "DASH_TO_PANEL_EXTENSION_URL" not in install
    assert "layout -> linux_configuration_setup" in components


def test_install_runs_core_before_component_selection():
    """install.sh installs the mandatory core regardless of selected components."""
    install = (ROOT_DIR / "install.sh").read_text(encoding="utf-8")

    assert "install_resource_monitor_core" in install
    assert "RESOURCE_MONITOR_REFRESH_INTERVAL_MS" in install
    # Core runs before the final (unconditional) component selection call.
    assert install.index("install_resource_monitor_core") < install.rindex(
        'component_main "$@"'
    )


def test_core_reserves_tight_stable_per_value_widths():
    """Value labels get snug fixed widths in stable mode so the taskbar does not shift."""
    core = (ROOT_DIR / "lib" / "gnome_extensions.sh").read_text(encoding="utf-8")

    # The default (stable) spacing reserves a tight width per value: sized to the
    # widest reading at the configured units, not a generous buffer. CPU 0-100
    # (3 digits, "100"=24px) -> 24, RAM GB (2) -> 20, disk free GB (3) -> 36, GPU
    # usage 3 -> 24 (VRAM 2 is split off to its own snug width by rm_panel_spacing),
    # ethernet down|up (3|3) -> 60. compact mode sets these widths to 0 instead.
    stable_expected = {
        "cpuwidth 24",
        "ramwidth 20",
        "diskspacewidth 36",
        "netethwidth 60",
        "gpuwidth 24",
    }
    compact_expected = {"cpuwidth 0", "ramwidth 0", "diskspacewidth 0", "netethwidth 0", "gpuwidth 0"}
    for key_width in stable_expected:
        assert (
            f"org.gnome.shell.extensions.resource-monitor {key_width}" in core
        ), key_width
    for key_width in compact_expected:
        assert (
            f"org.gnome.shell.extensions.resource-monitor {key_width}" in core
        ), key_width

    # The spacing mode selects between them.
    assert "resource_monitor_spacing_mode" in core
    assert "case \"$(resource_monitor_spacing_mode)\" in" in core

    # Ethernet is placed leftmost so its rarer wider readings grow toward the
    # screen center instead of shifting the clock.
    assert "itemsposition \"[\'eth\', \'cpu\', \'ram\', \'stats\', \'space\', \'wlan\', \'gpu\']\"" in core

    # The reservations live in the mandatory core, not a deselectable component.
    components = (ROOT_DIR / "installer" / "components.sh").read_text(encoding="utf-8")
    for key in ("cpuwidth", "ramwidth", "netethwidth", "gpuwidth", "diskspacewidth"):
        assert key not in components, f"{key} must not be a component toggle"


def test_panel_spacing_is_a_selectable_component():
    """Panel spacing (stable/compact) is a configurable component that drives the patch."""
    lib = (ROOT_DIR / "lib" / "gnome_extensions.sh").read_text(encoding="utf-8")
    components = (ROOT_DIR / "installer" / "components.sh").read_text(encoding="utf-8")

    assert "apply_resource_monitor_spacing_mode" in lib
    assert "resource_monitor_spacing_mode" in lib

    patch = (ROOT_DIR / "src" / "patch_stable_width.rs").read_text(encoding="utf-8")
    # The patcher reserves the secondary value width in stable mode, releases it
    # in compact mode, and is idempotent in both.
    assert "this._diskActivityWidth = 24" in patch
    assert "already reserved" in patch
    assert "compact" in patch

    assert "rm_panel_spacing|Resource Monitor panel spacing" in components
    assert "detect_rm_panel_spacing" in components
    # The apply function forwards the configured mode to rm-monitor.
    assert 'rm_monitor patch-stable-width --mode "$mode"' in lib
    # Uninstall reverts to the stable baseline spacing.
    lifecycle = (ROOT_DIR / "lib" / "lifecycle.sh").read_text(encoding="utf-8")
    assert "uninstall_rm_panel_spacing" in lifecycle


def test_default_json_lists_every_default_on_component():
    """installation_configs/default.json must match ISC_COMPONENTS default-on ids."""
    import json
    import re

    components = (ROOT_DIR / "installer" / "components.sh").read_text(encoding="utf-8")
    default = json.loads(
        (ROOT_DIR / "installation_configs" / "default.json").read_text(encoding="utf-8")
    )
    ids = re.findall(
        r'"(rm_[a-z_]+|window_rules)\|[^"]+\|on\|',
        components,
    )
    assert ids, "expected at least one default-on component in ISC_COMPONENTS"
    assert set(ids) == set(default["components"].keys())
    for cid in ids:
        assert default["components"][cid]["on"] == 1, cid


def test_dead_dash_to_panel_helpers_are_gone():
    """Dash-to-Panel ownership left this repo; leftover helpers must not return."""
    lib = (ROOT_DIR / "lib" / "gnome_extensions.sh").read_text(encoding="utf-8")
    assert "setup_dash_to_panel_integration" not in lib
    assert not (ROOT_DIR / "lib" / "extension_features.sh").exists()
    assert "need_cmd python3" not in lib


def test_source_patch_vram_has_no_fake_gsettings_uninstall():
    """rm_vram uninstall must not reset gpumemorymonitor (wrong key / no-op)."""
    components = (ROOT_DIR / "installer" / "components.sh").read_text(encoding="utf-8")
    lifecycle = (ROOT_DIR / "lib" / "lifecycle.sh").read_text(encoding="utf-8")
    vram_row = [
        line for line in components.splitlines() if line.strip().startswith('"rm_vram|')
    ][0]
    assert "uninstall_rm_vram" not in vram_row
    assert "uninstall_rm_vram" not in lifecycle
    assert "gpumemorymonitor" not in lifecycle


def test_refresh_interval_has_live_detect():
    """rm_refresh_interval is discoverable via detect_rm_refresh_interval."""
    components = (ROOT_DIR / "installer" / "components.sh").read_text(encoding="utf-8")
    lifecycle = (ROOT_DIR / "lib" / "lifecycle.sh").read_text(encoding="utf-8")
    assert "detect_rm_refresh_interval" in components
    assert "detect_rm_refresh_interval()" in lifecycle
    assert '_isc_mark_installed "rm_refresh_interval"' in (
        ROOT_DIR / "lib" / "gnome_extensions.sh"
    ).read_text(encoding="utf-8")
