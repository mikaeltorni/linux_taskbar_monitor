"""Installer behavior tests for Chrome PWA icon setup."""

import subprocess
from pathlib import Path


def test_install_pwa_icons_creates_one_directory_per_icon_size(tmp_path):
    """Multiple icon sizes must not be collapsed into one malformed path."""
    target_home = tmp_path / "home"
    scripts_dir = Path(__file__).resolve().parents[1]
    icons_dir = (
        target_home
        / ".config/google-chrome/Default/Web Applications/Manifest Resources/chatgpt/Icons"
    )
    icons_dir.mkdir(parents=True)
    (icons_dir / "128.png").write_bytes(b"png")
    (icons_dir / "256.png").write_bytes(b"png")

    fake_bin = tmp_path / "bin"
    fake_bin.mkdir()
    gtk_cache = fake_bin / "gtk-update-icon-cache"
    gtk_cache.write_text("#!/usr/bin/env bash\nexit 0\n", encoding="utf-8")
    gtk_cache.chmod(0o755)

    bash_script = f"""
set -euo pipefail
SCRIPT_DIR={scripts_dir}
TARGET_HOME={target_home}
TARGET_USER=$(id -un)
CHROME_PWAS=("chatgpt|ChatGPT|chatgpt.desktop|0")
msg() {{ printf '%s\\n' "$*"; }}
run_as_target() {{ "$@"; }}
source "$SCRIPT_DIR/lib/extension_features.sh"
install_pwa_icons
"""

    result = subprocess.run(
        ["bash", "-c", bash_script],
        check=False,
        text=True,
        capture_output=True,
        env={"PATH": f"{fake_bin}:/usr/bin:/bin"},
    )

    assert result.returncode == 0, result.stderr
    assert (
        target_home / ".local/share/icons/hicolor/128x128/apps"
    ).is_dir()
    assert (
        target_home / ".local/share/icons/hicolor/256x256/apps"
    ).is_dir()
    assert not (
        target_home / ".local/share/icons/hicolor/128x128 256x256/apps"
    ).exists()
