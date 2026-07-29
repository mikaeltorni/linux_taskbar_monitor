#!/usr/bin/env bash
# components.sh - Component manifest for ubuntu_2404_taskbar_system_status_monitor.
#
# Maps each taskbar/extension component to the function (defined in lib/) that
# installs it. install.sh defines its inline helpers and sources the lib files
# first, then this manifest, then routes execution through component_main from
# the shared component_loader.sh (linux_installation_scripts_functions).
#
# Entry format (pipe-separated fields; trailing empties may be omitted):
#   id|label|default(on/off)|install_fn|detect_fn|uninstall_fn|section|requires|configure_fn|status_fn
# Nested configurators are discovered when configure_fn (field 8) is non-empty.
# Field 6 is an optional menu section heading — never a keyword like
# "configurable".

ISC_REPO_NAME="ubuntu_2404_taskbar_system_status_monitor"
ISC_REPO_LABEL="Taskbar system status monitor"

ISC_POSTFLIGHT="report_sudo_required"

# The Resource Monitor extension is the program's mandatory core: install.sh
# installs it unconditionally (install_resource_monitor_core) before this
# component selection runs, so the taskbar indicator works no matter which
# components the user picks. The refresh row exposes core configuration; the
# remaining entries are optional tweaks that layer on top of the core.
ISC_COMPONENTS=(
  "rm_refresh_interval|Resource Monitor update time|on|apply_resource_monitor_refresh_interval|detect_rm_refresh_interval||||configure_resource_monitor_refresh_interval|resource_monitor_refresh_interval_status"
  "rm_gradient_colors|Resource Monitor gradient indicator colors|on|patch_resource_monitor_gradient_colors|detect_rm_gradient_colors"
  "rm_vram|Resource Monitor GPU VRAM display|on|patch_resource_monitor_vram|detect_rm_vram"
  "rm_per_disk|Resource Monitor per-disk display|on|patch_resource_monitor_per_disk|detect_rm_per_disk"
  "rm_panel_spacing|Resource Monitor panel spacing|on|apply_resource_monitor_spacing_mode|detect_rm_panel_spacing|uninstall_rm_panel_spacing|||configure_resource_monitor_spacing|resource_monitor_spacing_status"
  "rm_hide_eth_icon|Resource Monitor hide ethernet icon|on|patch_resource_monitor_eth_icon|detect_rm_hide_eth_icon"
  "rm_process_popup|Resource Monitor per-process CPU popup (left-click)|on|patch_resource_monitor_process_popup|detect_rm_process_popup"
  "window_rules|App window-rules extension (Wayland)|on|configure_window_rules_extension|detect_window_rules|uninstall_window_rules"
)
# Note: this repo owns only the Resource Monitor system-status extension and its
# window-rules helper. Features that are desktop-wide behavior were moved to their
# owning repositories to avoid cross-repo duplication: auto-move-windows placement
# -> linux_workspaces_setup, Chrome PWA icons/desktop entries and the Dash-to-Panel
# layout -> linux_configuration_setup.
