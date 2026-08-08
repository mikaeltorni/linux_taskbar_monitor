#!/usr/bin/env bash
# components.sh - Component manifest for Linux Taskbar Monitor.
#
# Maps each taskbar/extension component to the function (defined in lib/) that
# installs it. install.sh defines its inline helpers and sources the lib files
# first, then this manifest, then routes execution through component_main from
# the shared component_loader.sh (linux_installation_scripts_functions).
#
# Entry format (pipe-separated fields; trailing empties may be omitted):
#   id|label|default(on/off)|install_fn|detect_fn|uninstall_fn|section|requires|configure_fn|status_fn|select_configure
# Nested configurators are discovered when configure_fn (field 8) is non-empty.
# Field 6 is an optional menu section heading — never a keyword like
# "configurable". Field 10 may be "select_configure" for configure-on-select.
#
# ISC_REPO_NAME stays on the historical GitHub slug until the repository is
# renamed to linux_taskbar_monitor and the master orchestrator is updated in the
# same change. ISC_REPO_LABEL is the user-facing display name.

ISC_REPO_NAME="ubuntu_2404_taskbar_system_status_monitor"
ISC_REPO_LABEL="Linux Taskbar Monitor"

# Warn once before the first optional component when the session bus cannot be
# read — enable_shell_extension also fail-hards, but preflight surfaces the
# problem earlier. Framework preflight continues after a warning; core enable
# still aborts hard on rewrite refusal.
ISC_PREFLIGHT="isc_preflight_session_bus"
ISC_POSTFLIGHT="report_sudo_required"

# isc_preflight_session_bus — Verify org.gnome.shell enabled-extensions is readable.
isc_preflight_session_bus() {
  if ! run_as_target gsettings get org.gnome.shell enabled-extensions >/dev/null; then
    msg "ERROR: cannot read org.gnome.shell enabled-extensions (session bus unavailable?)"
    return 1
  fi
  return 0
}

# The Resource Monitor extension is the program's mandatory core: install.sh
# installs it unconditionally (install_resource_monitor_core) before this
# component selection runs, so the taskbar indicator works no matter which
# components the user picks. The refresh row exposes core configuration; the
# remaining entries are optional tweaks that layer on top of the core.
ISC_COMPONENTS=(
  "rm_refresh_interval|Resource Monitor update time|on|apply_resource_monitor_refresh_interval|detect_rm_refresh_interval||||configure_resource_monitor_refresh_interval|resource_monitor_refresh_interval_status"
  "rm_gradient_colors|Resource Monitor gradient indicator colors|on|patch_resource_monitor_gradient_colors|detect_rm_gradient_colors|uninstall_rm_gradient_colors"
  "rm_vram|Resource Monitor GPU VRAM display|on|patch_resource_monitor_vram|detect_rm_vram|uninstall_rm_vram"
  "rm_per_disk|Resource Monitor per-disk display|on|patch_resource_monitor_per_disk|detect_rm_per_disk|uninstall_rm_per_disk"
  "rm_panel_spacing|Resource Monitor panel spacing|on|apply_resource_monitor_spacing_mode|detect_rm_panel_spacing|uninstall_rm_panel_spacing|||configure_resource_monitor_spacing|resource_monitor_spacing_status"
  "rm_hide_eth_icon|Resource Monitor hide ethernet icon|on|patch_resource_monitor_eth_icon|detect_rm_hide_eth_icon|uninstall_rm_hide_eth_icon"
  "rm_process_popup|Resource Monitor per-process CPU popup (left-click)|on|patch_resource_monitor_process_popup|detect_rm_process_popup|uninstall_rm_process_popup"
  "window_rules|App window-rules extension (Wayland)|off|configure_window_rules_extension|detect_window_rules|uninstall_window_rules"
)
# Note: this repo owns only the Resource Monitor system-status extension and its
# window-rules helper. Features that are desktop-wide behavior were moved to their
# owning repositories to avoid cross-repo duplication: auto-move-windows placement
# -> linux_workspaces_setup, Chrome PWA icons/desktop entries and the Dash-to-Panel
# layout -> linux_configuration_setup.
