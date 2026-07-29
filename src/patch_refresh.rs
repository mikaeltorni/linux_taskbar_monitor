//! Add sub-second refresh interval support to the Resource Monitor extension.
//!
//! Upstream Resource
//! Monitor only allows whole-second refresh intervals and throttles GPU polling
//! to a 5-second floor. This patcher rewrites the relevant source files so the
//! panel can refresh every 0.1 seconds (including GPU usage/VRAM), then
//! recompiles the GSettings schemas.

use std::fs;
use std::path::Path;
use std::process::Command;

use crate::logging;

/// A single `(old, new)` source substitution.
type Replacement = (&'static str, &'static str);

/// Canonical substitutions applied to each extension-relative file, in order.
const REPLACEMENTS: &[(&str, &[Replacement])] = &[
    (
        "extension.js",
        &[
            (
                "this._settings.get_int(REFRESH_TIME)",
                "this._settings.get_double(REFRESH_TIME)",
            ),
            ("GLib.timeout_add_seconds(", "GLib.timeout_add("),
            (
                "        this._refreshTime,\n",
                "        Math.round(this._refreshTime * 1000),\n",
            ),
            // Upstream throttles GPU polling to a 5-second minimum, which keeps
            // the GPU usage/VRAM values from refreshing at the configured
            // sub-second rate the way CPU, RAM, and ethernet do.
            (
                "const GPU_MIN_REFRESH_INTERVAL_SECONDS = 5;",
                "const GPU_MIN_REFRESH_INTERVAL_SECONDS = 0.1;",
            ),
        ],
    ),
    (
        "services/settings.js",
        &[(
            "indicator._settings.get_int(keys.REFRESH_TIME)",
            "indicator._settings.get_double(keys.REFRESH_TIME)",
        )],
    ),
    (
        "prefs.js",
        &[(
            "this._secondsSpinbutton = this._createSpinButton({\n        upper: 60,\n        step: 1,\n        page: 1,\n      });",
            "this._secondsSpinbutton = this._createSpinButton({\n        lower: 0.1,\n        upper: 60,\n        step: 0.1,\n        page: 1,\n        digits: 1,\n      });",
        )],
    ),
    (
        "schemas/org.gnome.shell.extensions.resource-monitor.gschema.xml",
        &[(
            "<key name=\"refreshtime\" type=\"i\">\n            <default>2</default>\n            <range min=\"1\" max=\"60\"/>",
            "<key name=\"refreshtime\" type=\"d\">\n            <default>0.5</default>\n            <range min=\"0.1\" max=\"60\"/>",
        )],
    ),
];

/// Upgrades for files produced by the earlier 500 ms patch, applied before the
/// canonical replacements so reconfiguration stays idempotent across versions.
const LEGACY_MINIMUM_REPLACEMENTS: &[(&str, &[Replacement])] = &[
    (
        "extension.js",
        &[(
            "const GPU_MIN_REFRESH_INTERVAL_SECONDS = 0.5;",
            "const GPU_MIN_REFRESH_INTERVAL_SECONDS = 0.1;",
        )],
    ),
    (
        "prefs.js",
        &[
            ("        lower: 0.5,", "        lower: 0.1,"),
            ("        step: 0.5,", "        step: 0.1,"),
        ],
    ),
    (
        "schemas/org.gnome.shell.extensions.resource-monitor.gschema.xml",
        &[(
            "            <range min=\"0.5\" max=\"60\"/>",
            "            <range min=\"0.1\" max=\"60\"/>",
        )],
    ),
];

fn legacy_replacements_for(relative_path: &str) -> Option<&'static [Replacement]> {
    LEGACY_MINIMUM_REPLACEMENTS
        .iter()
        .find(|(path, _)| *path == relative_path)
        .map(|(_, replacements)| *replacements)
}

/// Failure while applying the refresh-interval patch.
#[derive(Debug, thiserror::Error)]
pub enum RefreshPatchError {
    /// A target file could not be read or written.
    #[error("{0}")]
    Io(#[from] std::io::Error),
    /// A target file does not contain the expected upstream source.
    #[error("Unsupported Resource Monitor source in {0}")]
    Unsupported(String),
    /// `glib-compile-schemas` could not be run or reported failure.
    #[error("glib-compile-schemas failed: {0}")]
    SchemaCompile(String),
}

/// Apply the sub-second refresh patches and recompile the GSettings schemas.
///
/// Each configured file is read, its legacy upgrades and canonical `(old, new)`
/// substitutions are applied (skipping any already present so the patch is
/// idempotent), and written back only when content changes.
///
/// # Parameters
/// - `extension_dir`: Installed Resource Monitor extension directory.
pub fn patch_extension(extension_dir: &Path) -> Result<bool, RefreshPatchError> {
    let mut any_changed = false;
    for (relative_path, replacements) in REPLACEMENTS {
        let path = extension_dir.join(relative_path);
        let original = fs::read_to_string(&path)?;
        let mut content = original.clone();

        if let Some(legacy) = legacy_replacements_for(relative_path) {
            for (old, new) in legacy {
                if content.contains(old) {
                    logging::info(format!("Upgrading legacy 500 ms patch in {relative_path}"));
                    content = content.replace(old, new);
                }
            }
        }

        for (old, new) in *replacements {
            if content.contains(new) {
                continue;
            }
            if !content.contains(old) {
                return Err(RefreshPatchError::Unsupported((*relative_path).to_string()));
            }
            content = content.replace(old, new);
        }

        if content != original {
            fs::write(&path, content)?;
            logging::info(format!("Patched {relative_path} for sub-second refresh"));
            any_changed = true;
        } else {
            logging::info(format!(
                "{relative_path} already patched for sub-second refresh"
            ));
        }
    }

    if !any_changed {
        logging::info("Refresh patch already applied; skipping schema compile");
        return Ok(false);
    }

    let schemas_dir = extension_dir.join("schemas");
    logging::info(format!(
        "Compiling Resource Monitor schemas in {}",
        schemas_dir.display()
    ));
    let status = Command::new("glib-compile-schemas")
        .arg(&schemas_dir)
        .status()
        .map_err(|err| RefreshPatchError::SchemaCompile(err.to_string()))?;
    if !status.success() {
        return Err(RefreshPatchError::SchemaCompile(format!(
            "exited with {status}"
        )));
    }

    Ok(true)
}

/// CLI entry point for the refresh-interval patcher.
///
/// # Parameters
/// - `extension_dir`: Installed Resource Monitor extension directory.
///
/// Returns `0` on success and `1` when patching or schema compilation fails.
pub fn run(extension_dir: &Path) -> i32 {
    logging::info(format!("patch-refresh dir={}", extension_dir.display()));
    match patch_extension(extension_dir) {
        Ok(true) => {
            println!("Patched Resource Monitor for configurable 0.1–60 second refresh intervals");
            0
        }
        Ok(false) => {
            println!("Resource Monitor refresh intervals already patched — skipping");
            0
        }
        Err(err) => {
            logging::error(format!(
                "Failed to patch Resource Monitor refresh interval: {err}"
            ));
            eprintln!("Failed to patch Resource Monitor refresh interval: {err}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Indentation is load-bearing: several substitutions match on the exact
    // leading whitespace of the upstream source.
    const UPSTREAM_EXTENSION_JS: &str = r#"const GPU_MIN_REFRESH_INTERVAL_SECONDS = 5;
this._refreshTime = this._settings.get_int(REFRESH_TIME);
    this._timeout = GLib.timeout_add_seconds(
        GLib.PRIORITY_DEFAULT,
        this._refreshTime,
        () => {}
    );
"#;

    const UPSTREAM_SETTINGS_JS: &str =
        "  indicator._refreshTime = indicator._settings.get_int(keys.REFRESH_TIME);\n";

    const UPSTREAM_PREFS_JS: &str = r#"      this._secondsSpinbutton = this._createSpinButton({
        upper: 60,
        step: 1,
        page: 1,
      });
"#;

    // A complete schema so `glib-compile-schemas` succeeds when it is installed.
    const UPSTREAM_GSCHEMA: &str = r#"<schemalist>
    <schema id="org.gnome.shell.extensions.resource-monitor" path="/org/gnome/shell/extensions/resource-monitor/">
        <key name="refreshtime" type="i">
            <default>2</default>
            <range min="1" max="60"/>
        </key>
    </schema>
</schemalist>
"#;

    fn upstream_extension() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("temp dir");
        fs::create_dir_all(dir.path().join("services")).expect("services dir");
        fs::create_dir_all(dir.path().join("schemas")).expect("schemas dir");
        fs::write(dir.path().join("extension.js"), UPSTREAM_EXTENSION_JS).expect("extension.js");
        fs::write(
            dir.path().join("services/settings.js"),
            UPSTREAM_SETTINGS_JS,
        )
        .expect("settings.js");
        fs::write(dir.path().join("prefs.js"), UPSTREAM_PREFS_JS).expect("prefs.js");
        fs::write(
            dir.path()
                .join("schemas/org.gnome.shell.extensions.resource-monitor.gschema.xml"),
            UPSTREAM_GSCHEMA,
        )
        .expect("gschema");
        dir
    }

    /// Patch the sources without invoking `glib-compile-schemas`, which is not
    /// guaranteed to exist in a test environment.
    fn patch_sources_only(root: &Path) -> Result<(), RefreshPatchError> {
        match patch_extension(root) {
            Err(RefreshPatchError::SchemaCompile(_)) | Ok(_) => Ok(()),
            Err(other) => Err(other),
        }
    }

    #[test]
    fn rewrites_every_target_file() {
        let dir = upstream_extension();
        patch_sources_only(dir.path()).expect("patch");

        let extension = fs::read_to_string(dir.path().join("extension.js")).expect("read");
        assert!(extension.contains("this._settings.get_double(REFRESH_TIME)"));
        assert!(extension.contains("GLib.timeout_add("));
        assert!(!extension.contains("GLib.timeout_add_seconds("));
        assert!(extension.contains("Math.round(this._refreshTime * 1000),"));
        assert!(extension.contains("GPU_MIN_REFRESH_INTERVAL_SECONDS = 0.1;"));

        let settings =
            fs::read_to_string(dir.path().join("services/settings.js")).expect("read settings");
        assert!(settings.contains("get_double(keys.REFRESH_TIME)"));

        let prefs = fs::read_to_string(dir.path().join("prefs.js")).expect("read prefs");
        assert!(prefs.contains("lower: 0.1,"));
        assert!(prefs.contains("digits: 1,"));

        let gschema = fs::read_to_string(
            dir.path()
                .join("schemas/org.gnome.shell.extensions.resource-monitor.gschema.xml"),
        )
        .expect("read gschema");
        assert!(gschema.contains("type=\"d\""));
        assert!(gschema.contains("<range min=\"0.1\" max=\"60\"/>"));
    }

    #[test]
    fn re_running_the_patch_is_idempotent() {
        let dir = upstream_extension();
        patch_sources_only(dir.path()).expect("first patch");
        let first = fs::read_to_string(dir.path().join("extension.js")).expect("read");
        patch_sources_only(dir.path()).expect("second patch");
        let second = fs::read_to_string(dir.path().join("extension.js")).expect("read");
        assert_eq!(first, second);
    }

    #[test]
    fn legacy_five_hundred_millisecond_patch_is_upgraded() {
        let dir = upstream_extension();
        fs::write(
            dir.path().join("extension.js"),
            UPSTREAM_EXTENSION_JS
                .replace("GPU_MIN_REFRESH_INTERVAL_SECONDS = 5;", "GPU_MIN_REFRESH_INTERVAL_SECONDS = 0.5;"),
        )
        .expect("write legacy");
        fs::write(
            dir.path().join("prefs.js"),
            r#"      this._secondsSpinbutton = this._createSpinButton({
        lower: 0.5,
        upper: 60,
        step: 0.5,
        page: 1,
        digits: 1,
      });
"#,
        )
        .expect("write legacy prefs");

        patch_sources_only(dir.path()).expect("patch");

        let extension = fs::read_to_string(dir.path().join("extension.js")).expect("read");
        assert!(extension.contains("GPU_MIN_REFRESH_INTERVAL_SECONDS = 0.1;"));
        let prefs = fs::read_to_string(dir.path().join("prefs.js")).expect("read prefs");
        assert!(prefs.contains("lower: 0.1,"));
        assert!(prefs.contains("step: 0.1,"));
    }

    #[test]
    fn unsupported_source_fails_fast() {
        let dir = upstream_extension();
        fs::write(dir.path().join("prefs.js"), "// unrelated\n").expect("write");
        assert!(matches!(
            patch_sources_only(dir.path()),
            Err(RefreshPatchError::Unsupported(_))
        ));
    }

    #[test]
    fn missing_extension_directory_exits_non_zero() {
        assert_eq!(run(Path::new("/nonexistent/rm-monitor-extension")), 1);
    }
}
