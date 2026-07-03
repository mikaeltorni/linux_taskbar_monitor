from pathlib import Path
from unittest.mock import patch

from scripts.patch_resource_monitor_refresh import patch_extension


def write_extension_fixture(tmp_path: Path) -> None:
    files = {
        "extension.js": (
            "this._settings.get_int(REFRESH_TIME);\n"
            "GLib.timeout_add_seconds(\n"
            "        GLib.PRIORITY_DEFAULT,\n"
            "        this._refreshTime,\n"
            "GLib.timeout_add_seconds(\n"
            "        GLib.PRIORITY_DEFAULT,\n"
            "        this._refreshTime,\n"
            "const GPU_MIN_REFRESH_INTERVAL_SECONDS = 5;\n"
        ),
        "services/settings.js": "indicator._settings.get_int(keys.REFRESH_TIME);\n",
        "prefs.js": (
            "this._secondsSpinbutton = this._createSpinButton({\n"
            "        upper: 60,\n"
            "        step: 1,\n"
            "        page: 1,\n"
            "      });\n"
        ),
        "schemas/org.gnome.shell.extensions.resource-monitor.gschema.xml": (
            '<key name="refreshtime" type="i">\n'
            "            <default>2</default>\n"
            '            <range min="1" max="60"/>\n'
        ),
    }
    for relative_path, content in files.items():
        path = tmp_path / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")


def test_patches_refresh_interval_to_one_tenth_second(tmp_path: Path):
    write_extension_fixture(tmp_path)

    with patch("scripts.patch_resource_monitor_refresh.subprocess.run") as run:
        patch_extension(tmp_path)

    extension = (tmp_path / "extension.js").read_text(encoding="utf-8")
    assert "get_double(REFRESH_TIME)" in extension
    assert extension.count("GLib.timeout_add(") == 2
    assert extension.count("Math.round(this._refreshTime * 1000)") == 2
    assert "const GPU_MIN_REFRESH_INTERVAL_SECONDS = 0.1;" in extension
    assert "get_double(keys.REFRESH_TIME)" in (
        tmp_path / "services/settings.js"
    ).read_text(encoding="utf-8")
    assert "lower: 0.1" in (tmp_path / "prefs.js").read_text(encoding="utf-8")
    assert "step: 0.1" in (tmp_path / "prefs.js").read_text(encoding="utf-8")
    schema = (
        tmp_path / "schemas/org.gnome.shell.extensions.resource-monitor.gschema.xml"
    ).read_text(encoding="utf-8")
    assert 'type="d"' in schema
    assert "<default>0.5</default>" in schema
    assert '<range min="0.1" max="60"/>' in schema
    run.assert_called_once_with(
        ["glib-compile-schemas", str(tmp_path / "schemas")],
        check=True,
    )


def test_patch_is_idempotent(tmp_path: Path):
    write_extension_fixture(tmp_path)

    with patch("scripts.patch_resource_monitor_refresh.subprocess.run"):
        patch_extension(tmp_path)
        patch_extension(tmp_path)


def test_upgrades_existing_half_second_patch_to_100ms_minimum(tmp_path: Path):
    """Existing installations patched by the older code must migrate in place."""
    write_extension_fixture(tmp_path)
    with patch("scripts.patch_resource_monitor_refresh.subprocess.run"):
        patch_extension(tmp_path)

    # Simulate the prior shipped 500 ms floor in all patched files.
    for relative_path in (
        "extension.js",
        "prefs.js",
        "schemas/org.gnome.shell.extensions.resource-monitor.gschema.xml",
    ):
        path = tmp_path / relative_path
        path.write_text(path.read_text().replace("0.1", "0.5"))

    with patch("scripts.patch_resource_monitor_refresh.subprocess.run"):
        patch_extension(tmp_path)

    assert "GPU_MIN_REFRESH_INTERVAL_SECONDS = 0.1" in (tmp_path / "extension.js").read_text()
    assert "lower: 0.1" in (tmp_path / "prefs.js").read_text()
    assert "step: 0.1" in (tmp_path / "prefs.js").read_text()
    assert 'range min="0.1"' in (
        tmp_path / "schemas/org.gnome.shell.extensions.resource-monitor.gschema.xml"
    ).read_text()
