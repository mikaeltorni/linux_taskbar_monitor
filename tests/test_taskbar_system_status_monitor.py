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


def test_dash_to_panel_is_installed_when_absent():
    """The dash_to_panel component must download/install dash-to-panel when it is
    not already present, rather than silently bailing. dash-to-panel is not in
    Ubuntu 24.04's apt sources, so the install goes through the pinned EGO build.
    Presence is checked on disk (metadata.json), never via the live
    `gnome-extensions list`, which omits a freshly installed extension until a
    Shell reload that deployment must not force."""
    features = (ROOT_DIR / "lib" / "extension_features.sh").read_text(encoding="utf-8")
    install = (ROOT_DIR / "install.sh").read_text(encoding="utf-8")

    # The pinned EGO build is configured in install.sh.
    assert "DASH_TO_PANEL_EXTENSION_URL" in install
    assert "DASH_TO_PANEL_EXTENSION_SHA256" in install

    # Missing dash-to-panel triggers an install instead of an early return.
    assert "install_dash_to_panel" in features
    assert "install_gnome_ext_zip" in features
    assert "$DASH_TO_PANEL_EXTENSION_URL" in features
    assert "$DASH_TO_PANEL_EXTENSION_SHA256" in features

    # Presence is detected on disk, not from the live extension list.
    assert "dash_to_panel_installed()" in features
    assert "metadata.json" in features
    assert "gnome-extensions list | grep -Fxq" not in features

    # Enablement is persisted via gsettings regardless of the live list.
    assert 'enable_shell_extension "$ext_id"' in features


def test_install_runs_core_before_component_selection():
    """install.sh installs the mandatory core regardless of selected components."""
    install = (ROOT_DIR / "install.sh").read_text(encoding="utf-8")

    assert "install_resource_monitor_core" in install
    assert "RESOURCE_MONITOR_REFRESH_TIME" in install
    # Core runs before the final (unconditional) component selection call.
    assert install.index("install_resource_monitor_core") < install.rindex(
        'component_main "$@"'
    )
