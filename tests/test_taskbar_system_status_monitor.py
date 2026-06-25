#!/usr/bin/env python3
"""Tests for delegating taskbar system status monitor installation."""

from pathlib import Path


ROOT_DIR = Path(__file__).resolve().parents[1]


def test_gnome_extension_module_installs_resource_monitor():
    """Resource Monitor core should patch and configure the downloaded extension."""
    source = (ROOT_DIR / "lib" / "gnome_extensions.sh").read_text(encoding="utf-8")

    assert "install_resource_monitor_core()" in source
    assert "patch_resource_monitor_refresh.py" in source
    assert "glib-compile-schemas" in source
    # Refresh interval is configurable (0.5 is only the default).
    assert 'refreshtime "${RESOURCE_MONITOR_REFRESH_TIME:-0.5}"' in source
    assert "curl -fL" in source
    assert "netethstatus false" not in source
    assert "scripts/configure_resource_monitor.py" in source


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
    assert "RESOURCE_MONITOR_REFRESH_TIME" in install
    # Core runs before the final (unconditional) component selection call.
    assert install.index("install_resource_monitor_core") < install.rindex(
        'component_main "$@"'
    )
