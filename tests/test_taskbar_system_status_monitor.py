#!/usr/bin/env python3
"""Tests for delegating taskbar system status monitor installation."""

import os
import subprocess
import tempfile
from pathlib import Path


ROOT_DIR = Path(__file__).resolve().parents[1]


def test_core_fails_when_metadata_pin_fails():
    """Version pin must not be soft-skipped; EGO overwrite depends on it."""
    core = (ROOT_DIR / "lib" / "gnome_extensions.sh").read_text(encoding="utf-8")
    helper = (ROOT_DIR / "lib" / "extension_installation.sh").read_text(
        encoding="utf-8"
    )
    assert 'patch_extension_metadata "$staging" metadata.json "$shell_version" 9999' in core
    assert "9999 || true" not in core
    # Empty/zero shell version must hard-fail before the pin call.
    assert 'could not parse GNOME Shell version' in core
    assert '[ "$shell_version" = "0" ]' in core
    # Parse and staged patch/pin must happen before the destructive live replace.
    assert core.index("could not parse GNOME Shell version") < core.index(
        'mv "$ext_dir" "$backup_dir"'
    )
    assert core.index('patch_extension_metadata "$staging"') < core.index(
        'mv "$ext_dir" "$backup_dir"'
    )
    assert 'staging="$tmpdir/staging"' in core
    assert "ERROR: metadata.json not found" in helper
    assert "WARNING: metadata.json not found" not in helper
    readme = (ROOT_DIR / "README.md").read_text(encoding="utf-8")
    assert "gnome-shell" in readme
    assert "9999" in readme
    assert "staging directory" in readme
    assert "Linux Taskbar Monitor" in readme
    assert "Supported platforms" in readme
    assert "Ubuntu 24.04 LTS" in readme
    assert "Not guaranteed elsewhere" in readme
    assert "GNOME Shell 46" in readme
    # Machine id stays historical until the GitHub rename + orchestrator update.
    components = (ROOT_DIR / "installer" / "components.sh").read_text(
        encoding="utf-8"
    )
    assert 'ISC_REPO_NAME="ubuntu_2404_taskbar_system_status_monitor"' in components
    assert 'ISC_REPO_LABEL="Linux Taskbar Monitor"' in components


def test_core_publishes_via_new_dir_then_rename():
    """Live replace must move the old tree aside, not rm it before publish."""
    core = (ROOT_DIR / "lib" / "gnome_extensions.sh").read_text(encoding="utf-8")
    assert 'publish_dir="${ext_dir}.new"' in core
    assert 'backup_dir="${ext_dir}.old"' in core
    assert 'mv "$publish_dir" "$ext_dir"' in core
    assert 'mv "$ext_dir" "$backup_dir"' in core
    # Never delete the only good tree before the replacement is in place.
    assert 'rm -rf "$ext_dir"' not in core.split("sync_resource_monitor_user_schema")[0]
    assert core.index('cp -a "$staging/." "$publish_dir/"') < core.index(
        'mv "$ext_dir" "$backup_dir"'
    )
    assert core.index('mv "$ext_dir" "$backup_dir"') < core.index(
        'mv "$publish_dir" "$ext_dir"'
    )


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


def test_spacing_mode_persistence_validates_values(tmp_path):
    """Only stable/compact are persisted; invalid values return 2."""
    script = f'''set -euo pipefail
TARGET_HOME="$HOME"
SCRIPT_DIR="{ROOT_DIR}"
RESOURCE_MONITOR_SPACING_MODE=stable
msg() {{ :; }}
run_as_target() {{ "$@"; }}
source "{ROOT_DIR / 'lib' / 'gnome_extensions.sh'}"
persist_resource_monitor_spacing_mode compact
test "$(resource_monitor_spacing_mode)" = compact
persist_resource_monitor_spacing_mode stable
test "$(resource_monitor_spacing_mode)" = stable
! persist_resource_monitor_spacing_mode wide
! persist_resource_monitor_spacing_mode ""
test "$(resource_monitor_spacing_mode)" = stable
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
    compact_expected = {
        "cpuwidth 0",
        "ramwidth 0",
        "diskspacewidth 0",
        "netethwidth 0",
        "gpuwidth 0",
    }
    for key_width in stable_expected:
        assert (
            f"org.gnome.shell.extensions.resource-monitor {key_width}" in core
        ), key_width
    for key_width in compact_expected:
        assert (
            f"org.gnome.shell.extensions.resource-monitor {key_width}" in core
        ), key_width

    # The spacing mode selects between them via the shared width helper.
    assert "resource_monitor_spacing_mode" in core
    assert "apply_resource_monitor_width_gsettings" in core
    assert 'apply_resource_monitor_width_gsettings "$ext_dir" "$(resource_monitor_spacing_mode)"' in core

    # The primary network column is placed first (leftmost), so its rarer wider
    # readings grow toward the screen center instead of shifting the clock.
    assert "itemsposition \"[\'eth\', \'cpu\', \'ram\', \'stats\', \'space\', \'wlan\', \'gpu\']\"" in core

    # The reservations live in the mandatory core, not a deselectable component.
    components = (ROOT_DIR / "installer" / "components.sh").read_text(encoding="utf-8")
    for key in (
        "cpuwidth",
        "ramwidth",
        "netethwidth",
        "gpuwidth",
        "diskspacewidth",
    ):
        assert key not in components, f"{key} must not be a component toggle"


def test_core_keeps_only_the_primary_network_column():
    """The core keeps the existing primary network reading without a duplicate Wi-Fi column."""
    core = (ROOT_DIR / "lib" / "gnome_extensions.sh").read_text(encoding="utf-8")

    schema = "org.gnome.shell.extensions.resource-monitor"
    assert f"{schema} netethstatus true" in core
    assert f"{schema} netwlanstatus false" in core
    assert f"{schema} netautohidestatus true" in core
    assert f"{schema} netwlanstatus true" not in core


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
    apply_fn = lib.split("apply_resource_monitor_spacing_mode() {")[1].split("\n}\n")[0]
    assert "if ! rm_monitor patch-stable-width" in apply_fn
    # Uninstall fails hard: stable-width markers are a source patch until core re-extract.
    lifecycle = (ROOT_DIR / "lib" / "lifecycle.sh").read_text(encoding="utf-8")
    assert "uninstall_rm_panel_spacing() { uninstall_rm_source_patch rm_panel_spacing; }" in lifecycle


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
    # window_rules stays optional / default-off and is not in the default snapshot.
    assert "window_rules" not in default["components"]
    assert re.search(
        r'"window_rules\|[^"]+\|off\|',
        components,
    ), "window_rules must default off for public installs"


def test_build_script_reclaims_sudo_owned_artifacts():
    script = (ROOT_DIR / "scripts" / "build_rm_monitor.sh").read_text(encoding="utf-8")
    assert "reclaim_build_artifacts_for_invoker" in script
    assert 'chown -R "$owner:$owner"' in script
    assert "SUDO_USER" in script
    reclaim_fn = script.split("reclaim_build_artifacts_for_invoker() {")[1].split("\n}\n")[0]
    assert "WARNING: could not chown" not in reclaim_fn
    assert "ERROR: could not chown" in reclaim_fn
    assert "return 1" in reclaim_fn
    # have_binary must stay defined — reclaim commit once deleted it and every
    # build exited 1 after a successful cargo compile.
    assert "have_binary() { [ -x \"$1\" ]; }" in script or "have_binary()" in script
    assert script.index("have_binary()") < script.index('have_binary "$DIST_BIN"')


def test_installer_apt_installs_runtime_deps():
    install = (ROOT_DIR / "install.sh").read_text(encoding="utf-8")
    assert "ensure_runtime_deps" in install
    assert "apt_install curl unzip libglib2.0-bin" in install
    core = (ROOT_DIR / "lib" / "gnome_extensions.sh").read_text(encoding="utf-8")
    assert "skipping zip integrity check" in core


def test_core_syncs_user_schema_for_refreshtime():
    """Patched double refreshtime schema must be mirrored into user glib schemas."""
    core = (ROOT_DIR / "lib" / "gnome_extensions.sh").read_text(encoding="utf-8")
    assert "sync_resource_monitor_user_schema() {" in core
    # Hard-fail on missing src / compile failure (no soft WARN+return 0).
    sync_fn = core.split("sync_resource_monitor_user_schema() {")[1].split("\n}\n")[0]
    assert "return 1" in sync_fn
    assert "WARN: extension schema missing" not in sync_fn
    # Scope to install_resource_monitor_core so apply_* call sites do not confuse order.
    core_fn = core.split("install_resource_monitor_core()")[1].split(
        "# ── Optional Resource Monitor tweaks"
    )[0]
    publish_mv = core_fn.index('mv "$publish_dir" "$ext_dir"')
    sync_call = core_fn.index('sync_resource_monitor_user_schema "$ext_dir" || return 1')
    assert publish_mv < sync_call
    # Live refreshtime is owned by rm_refresh_interval, not core.
    assert (
        'ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor refreshtime'
        not in core_fn
    )
    apply_fn = core.split("apply_resource_monitor_refresh_interval() {")[1].split("\n}\n")[0]
    assert (
        'set org.gnome.shell.extensions.resource-monitor refreshtime' in apply_fn
    )
    assert "persist_resource_monitor_refresh_interval" in apply_fn
    assert "glib-2.0/schemas" in core
    assert "org.gnome.shell.extensions.resource-monitor.gschema.xml" in core
    assert "uninstall_rm_refresh_interval" in (
        ROOT_DIR / "lib" / "lifecycle.sh"
    ).read_text(encoding="utf-8")
    assert "uninstall_rm_refresh_interval" in (
        ROOT_DIR / "installer" / "components.sh"
    ).read_text(encoding="utf-8")
    agents = (ROOT_DIR / "AGENTS.md").read_text(encoding="utf-8")
    assert "glib-2.0/schemas" in agents


def test_select_unknown_aborts_at_runtime_before_core_banner():
    """Unknown --select must exit 1 without starting the install banner."""
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
        curl = bin_dir / "curl"
        curl.write_text("#!/bin/sh\nexit 1\n", encoding="utf-8")
        curl.chmod(0o755)
        result = subprocess.run(
            ["bash", str(ROOT_DIR / "install.sh"), "--select", "not_a_component"],
            cwd=ROOT_DIR,
            env=env,
            capture_output=True,
            text=True,
            check=False,
        )
        assert result.returncode == 1
        combined = result.stdout + result.stderr
        assert "unknown component id" in combined
        assert "Linux Taskbar Monitor & GNOME Extensions Setup" not in combined


def test_select_ids_validated_before_core_install():
    """Unknown/empty --select must abort before install_resource_monitor_core."""
    install = (ROOT_DIR / "install.sh").read_text(encoding="utf-8")
    main = install.split("# ── Main installer logic")[1]
    early, _after = main.split("ensure_rm_monitor_tools", 1)
    assert "_validate_select_ids" in early
    assert 'unknown component id' in early
    assert "--select needs component ids" in early
    # Unknown bare tokens (no leading dash) must also abort.
    assert 'unknown argument: $1' in early


def test_dead_dash_to_panel_helpers_are_gone():
    """Dash-to-Panel ownership left this repo; leftover helpers must not return."""
    lib = (ROOT_DIR / "lib" / "gnome_extensions.sh").read_text(encoding="utf-8")
    assert "setup_dash_to_panel_integration" not in lib
    assert not (ROOT_DIR / "lib" / "extension_features.sh").exists()
    assert "need_cmd python3" not in lib


def test_source_patch_vram_has_no_fake_gsettings_uninstall():
    """rm_vram uninstall must fail hard (core re-extract), not reset gsettings."""
    components = (ROOT_DIR / "installer" / "components.sh").read_text(encoding="utf-8")
    lifecycle = (ROOT_DIR / "lib" / "lifecycle.sh").read_text(encoding="utf-8")
    vram_row = [
        line for line in components.splitlines() if line.strip().startswith('"rm_vram|')
    ][0]
    assert "uninstall_rm_vram" in vram_row
    assert "uninstall_rm_source_patch" in lifecycle
    assert "gpumemorymonitor" not in lifecycle
    assert "core re-extract" in lifecycle


def test_refresh_interval_has_live_detect():
    """rm_refresh_interval is discoverable via detect_rm_refresh_interval."""
    components = (ROOT_DIR / "installer" / "components.sh").read_text(encoding="utf-8")
    lifecycle = (ROOT_DIR / "lib" / "lifecycle.sh").read_text(encoding="utf-8")
    assert "detect_rm_refresh_interval" in components
    assert "detect_rm_refresh_interval()" in lifecycle
    detect_fn = lifecycle.split("detect_rm_refresh_interval() {")[1].split("\n}\n")[0]
    assert "resource_monitor_refresh_interval_file" in detect_fn
    assert '_isc_mark_installed "rm_refresh_interval"' in (
        ROOT_DIR / "lib" / "gnome_extensions.sh"
    ).read_text(encoding="utf-8")
    readme = (ROOT_DIR / "README.md").read_text(encoding="utf-8")
    assert "bash scripts/check.sh" in readme


def test_readonly_cli_flags_early_exit_before_core_install():
    """List/detect/help/uninstall/reconfigure must not wipe/re-extract Resource Monitor."""
    install = (ROOT_DIR / "install.sh").read_text(encoding="utf-8")
    # The early-exit case lives in the main installer block, before the
    # mandatory core call (not the ensure_rm_monitor_tools function definition).
    main = install.split("# ── Main installer logic")[1]
    early, after = main.split("ensure_rm_monitor_tools", 1)
    for flag in (
        "--list-components",
        "--list-configurable-components",
        "--list-select-configure-components",
        "--list-component-config-values",
        "--configure-component",
        "--export-selection",
        "--detect",
        "--help",
        "--uninstall",
        "--reconfigure",
    ):
        assert flag in early, f"{flag} must early-exit before ensure_rm_monitor_tools"
    assert "install_resource_monitor_core" in after
    assert "install_resource_monitor_core" not in early


def test_unknown_dash_args_abort_before_core_install():
    """Typos like --list-configurable must not re-extract the live extension."""
    install = (ROOT_DIR / "install.sh").read_text(encoding="utf-8")
    main = install.split("# ── Main installer logic")[1]
    early, _after = main.split("ensure_rm_monitor_tools", 1)
    assert '-*)' in early
    assert "unknown argument" in early
    assert "_validate_select_ids" in early
    assert '--default|--all|""' in early.replace("\n", "")
    # Bare unknown tokens (no dash) also abort before core.
    assert "*)" in early


def test_panel_spacing_manifest_section_is_empty():
    """Field 6 is a menu section heading, not the word 'configurable'."""
    components = (ROOT_DIR / "installer" / "components.sh").read_text(encoding="utf-8")
    row = [
        line
        for line in components.splitlines()
        if line.strip().startswith('"rm_panel_spacing|')
    ][0]
    # uninstall | empty section | empty requires | configure | status
    assert (
        "uninstall_rm_panel_spacing|||configure_resource_monitor_spacing|"
        "resource_monitor_spacing_status"
    ) in row
    assert "|configurable|" not in row


def test_top_users_window_is_a_persisted_configurable_component():
    """U2TSSM seeds the window, the repo persists it, and the component exposes it."""
    core = (ROOT_DIR / "lib" / "gnome_extensions.sh").read_text(encoding="utf-8")
    installer = (ROOT_DIR / "install.sh").read_text(encoding="utf-8")
    components = (ROOT_DIR / "installer" / "components.sh").read_text(encoding="utf-8")

    # The env var seeds a clean install and defaults to a 10 minute window.
    assert 'U2TSSM="${U2TSSM:-10}"' in installer
    assert 'value="${U2TSSM:-10}"' in core

    # A persisted file outranks the env var so re-running install.sh without
    # re-exporting U2TSSM keeps the window the user configured.
    assert "top-users-window-minutes" in core
    assert "resource_monitor_top_window_file" in core
    for fn in (
        "resource_monitor_top_window_minutes",
        "persist_resource_monitor_top_window_minutes",
        "configure_resource_monitor_top_window",
        "resource_monitor_top_window_status",
    ):
        assert f"{fn}()" in core, f"{fn} must be defined"

    # Out-of-range values must not reach the injected JavaScript, which only
    # accepts 1..1440 minutes.
    assert "value < 1 || value > 1440" in core

    # The patcher receives the resolved window rather than the raw env var.
    assert 'rm_monitor patch-process-popup --window-minutes "$window"' in core

    # The component row wires the configure/status pair, making the window
    # reachable from --configure-component and the interactive menu.
    row = next(
        line for line in components.splitlines() if line.strip().startswith('"rm_process_popup|')
    )
    assert "configure_resource_monitor_top_window" in row
    assert "resource_monitor_top_window_status" in row


def test_top_users_popup_covers_every_panel_metric():
    """The popup ranks users of each metric the panel shows, not just CPU and RAM."""
    popup = (ROOT_DIR / "src" / "patch_process_popup.rs").read_text(encoding="utf-8")

    # One section per panel metric, each naming the top processes for it.
    for label in ("CPU", "RAM", "Disk", "Network", "GPU", "VRAM"):
        assert label in popup, f"popup must have a {label} section"

    # The window is injected, not hard-coded, and the extension re-reads it from
    # the environment so exporting U2TSSM changes the window without a repatch.
    assert "WINDOW_PLACEHOLDER" in popup
    assert "U2TSSM" in popup
    assert "MAX_WINDOW_MINUTES" in popup
