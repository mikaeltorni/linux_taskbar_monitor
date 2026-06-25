#!/usr/bin/env bash
# extension_features.sh — GNOME Shell extension feature configurations  
#
# This repo owns only the Resource Monitor system-status extension and its
# window-rules helper. Two features that used to live here moved to their owning
# repositories:
#   - Chrome PWA icons/desktop entries -> linux_configuration_setup (lib/pwa_icons.sh)
#   - Dash-to-Panel layout             -> linux_configuration_setup (lib/dash_to_panel.sh)  
#   - Taskbar system status indicators -> this repo (system-status extension)
#
# Shutdown/Date Controls Configuration:
# These are now handled by dash_to_panel.sh in bottom panel position with
# proper system-status integration for shutdown, date, and calendar controls.

# System Status Extension Features
SYS_STATUS_REFRESH_TIME="${SYSTEM_STATUS_REFRESH_TIME:-5}"  
SYS_STATUS_MONITOR_POSITION="top"  # Where the monitor appears (left/center/right)  
SYS_STATUS_SHOW_CPU=true
SYS_STATUS_SHOW_MEMORY=true  
SYS_STATUS_SHOW_DISK=false
SYS_STATUS_UPDATE_INTERVAL=30

# Shutdown/Date Controls Integration with Dash-to-Panel
# When dash-to-panel is active, these controls appear in bottom panel  
D2P_PANEL_POSITION="bottom"  # Where dash-to-panel puts the taskbar
D2P_SYSTEM_STATUS_INTEGRATION=true  # Enable system-status in bottom panel

