#!/usr/bin/env bash
set -euo pipefail

# Mimic the setup from install.sh
TARGET_USER="${SUDO_USER:-$USER}"
TARGET_UID="$(id -u "$TARGET_USER")"
TARGET_HOME="$(getent passwd "$TARGET_USER" | cut -d: -f6)"
TEST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SCRIPT_DIR="$(dirname "$TEST_DIR")"
CONFIG_DIR="$SCRIPT_DIR/configs"

# Source lib files in the same order as install.sh
source "$SCRIPT_DIR/lib/helpers.sh"
source "$SCRIPT_DIR/lib/hardware.sh"
source "$SCRIPT_DIR/lib/system_services.sh"
source "$SCRIPT_DIR/lib/system_config.sh"
source "$SCRIPT_DIR/lib/gnome_desktop.sh"
source "$SCRIPT_DIR/lib/gsettings_helpers.sh"
source "$SCRIPT_DIR/lib/apt_helpers.sh"
source "$SCRIPT_DIR/lib/autostart.sh"
source "$SCRIPT_DIR/lib/gnome_extensions.sh"
source "$SCRIPT_DIR/lib/agent_command_center.sh"
source "$SCRIPT_DIR/lib/packages.sh"
source "$SCRIPT_DIR/lib/window_manager.sh"

# Initialize git submodules (linux_hotkey_setup contains all hotkey setup code).
if [ -d "$SCRIPT_DIR/.git" ]; then
  git -c protocol.file.allow=always submodule update --init linux_hotkey_setup 2>/dev/null || true
fi

source "$SCRIPT_DIR/linux_hotkey_setup/lib/hotkeys_setup.sh"
source "$SCRIPT_DIR/lib/systemd_helpers.sh"
source "$SCRIPT_DIR/lib/logging.sh"
source "$SCRIPT_DIR/lib/app_installer.sh"

# Now test the functions
echo "Testing lock_primary_paste:"
lock_primary_paste

echo "Testing build_xmousepasteblock:"
if build_xmousepasteblock; then
  echo "xmousepasteblock binary is available."
else
  echo "xmousepasteblock binary is unavailable; service setup should skip safely."
fi

echo "Testing configure_xmousepasteblock_service:"
configure_xmousepasteblock_service || true

echo "Done."
