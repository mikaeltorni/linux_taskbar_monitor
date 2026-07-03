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
    assert "patch_resource_monitor_refresh.py" in source
    assert "glib-compile-schemas" in source
    # Refresh interval is configured through the persisted millisecond setting.
    assert 'resource_monitor_refresh_seconds' in source
    assert "curl -fL" in source
    assert "netethstatus false" not in source
    assert "scripts/configure_resource_monitor.py" in source


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
