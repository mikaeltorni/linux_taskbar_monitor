//! Add sub-second refresh interval support to the Resource Monitor extension.
//!
//! Upstream Resource
//! Monitor only allows whole-second refresh intervals and throttles GPU polling
//! to a 5-second floor. This patcher rewrites the relevant source files so the
//! panel can refresh every 0.1 seconds (including GPU usage/VRAM), then
//! recompiles the GSettings schemas.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::logging;

/// Name of the Resource Monitor GSettings schema file, shared between the
/// extension's own `schemas/` directory and the user's GSettings schema
/// directory synced by [`sync_user_schemas`].
const GSCHEMA_FILE_NAME: &str = "org.gnome.shell.extensions.resource-monitor.gschema.xml";

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
/// Each configured file is read and transformed first; writes happen only after
/// every target succeeds so a mid-run unsupported file cannot leave a
/// half-patched tree. Already-applied files are left untouched. Schemas are
/// always compiled after a successful transform pass so a prior
/// write-then-compile-failure can recover on re-run.
///
/// # Parameters
/// - `extension_dir`: Installed Resource Monitor extension directory.
///
/// # Returns
/// `Ok(true)` when at least one file changed, `Ok(false)` when every file was
/// already patched (schemas are still compiled in both cases).
pub fn patch_extension(extension_dir: &Path) -> Result<bool, RefreshPatchError> {
    let mut pending: Vec<(PathBuf, String, &'static str)> = Vec::new();

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
            // Prefer "old still present → replace" over "new substring exists →
            // skip". Short tokens like `GLib.timeout_add(` can appear elsewhere
            // while the refresh timer still uses timeout_add_seconds.
            if content.contains(old) {
                content = content.replace(old, new);
            } else if !content.contains(new) {
                return Err(RefreshPatchError::Unsupported((*relative_path).to_string()));
            }
        }

        if content != original {
            pending.push((path, content, relative_path));
        } else {
            logging::info(format!(
                "{relative_path} already patched for sub-second refresh"
            ));
        }
    }

    let any_changed = !pending.is_empty();
    if any_changed {
        let batch: Vec<(PathBuf, String)> = pending
            .iter()
            .map(|(path, content, _)| (path.clone(), content.clone()))
            .collect();
        crate::patch_text::write_atomic_batch(&batch)?;
        for (_, _, relative_path) in &pending {
            logging::info(format!("Patched {relative_path} for sub-second refresh"));
        }
    }

    let schemas_dir = extension_dir.join("schemas");
    if any_changed {
        logging::info(format!(
            "Compiling Resource Monitor schemas in {}",
            schemas_dir.display()
        ));
    } else {
        logging::info(format!(
            "Refresh sources already patched; ensuring schemas in {}",
            schemas_dir.display()
        ));
    }
    let status = Command::new("glib-compile-schemas")
        .arg(&schemas_dir)
        .status()
        .map_err(|err| RefreshPatchError::SchemaCompile(err.to_string()))?;
    if !status.success() {
        return Err(RefreshPatchError::SchemaCompile(format!(
            "exited with {status}"
        )));
    }

    // The extension's own schema compiled successfully; a problem syncing it
    // to the user's GSettings schema directory is logged but must not fail
    // this patch (gsettings consumers outside the extension process are a
    // secondary concern to the extension itself working).
    sync_user_schemas(&schemas_dir);

    Ok(any_changed)
}

/// Copy the compiled extension's schema XML into the user's GSettings schema
/// directory (`$HOME/.local/share/glib-2.0/schemas/`) and recompile it there.
///
/// `gsettings` and other GSettings consumers outside the extension process
/// resolve schemas through the user's XDG data dirs, not the extension's own
/// `schemas/` directory; without this sync those consumers can keep seeing a
/// stale (e.g. integer) `refreshtime` type after the extension's schema has
/// already moved to the sub-second `double` type. Every failure here is a
/// soft warning — see the call site in [`patch_extension`].
///
/// # Parameters
/// - `extension_schemas_dir`: The extension's own `schemas/` directory,
///   already compiled successfully by the caller.
fn sync_user_schemas(extension_schemas_dir: &Path) {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|home| !home.as_os_str().is_empty());
    let Some(home) = home else {
        logging::warn("HOME is not set; skipping user GSettings schema sync");
        return;
    };

    let user_schemas_dir = home.join(".local/share/glib-2.0/schemas");
    if let Err(err) = fs::create_dir_all(&user_schemas_dir) {
        logging::warn(format!(
            "Could not create {}: {err}",
            user_schemas_dir.display()
        ));
        return;
    }

    let src = extension_schemas_dir.join(GSCHEMA_FILE_NAME);
    let dest = user_schemas_dir.join(GSCHEMA_FILE_NAME);

    let src_bytes = match fs::read(&src) {
        Ok(bytes) => bytes,
        Err(err) => {
            logging::warn(format!("Could not read {}: {err}", src.display()));
            return;
        }
    };

    let already_identical = fs::read(&dest)
        .map(|dest_bytes| dest_bytes == src_bytes)
        .unwrap_or(false);
    if already_identical {
        logging::info(format!(
            "User GSettings schema already up to date at {}",
            dest.display()
        ));
    } else if let Err(err) = fs::write(&dest, &src_bytes) {
        logging::warn(format!(
            "Could not copy schema to {}: {err}",
            dest.display()
        ));
        return;
    } else {
        logging::info(format!(
            "Synced Resource Monitor schema to {}",
            dest.display()
        ));
    }

    match Command::new("glib-compile-schemas")
        .arg(&user_schemas_dir)
        .status()
    {
        Ok(status) if status.success() => {
            logging::info(format!(
                "Compiled user GSettings schemas in {}",
                user_schemas_dir.display()
            ));
        }
        Ok(status) => {
            logging::warn(format!(
                "glib-compile-schemas for user schemas exited with {status}"
            ));
        }
        Err(err) => {
            logging::warn(format!(
                "Could not run glib-compile-schemas for user schemas: {err}"
            ));
        }
    }
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

    /// Serializes tests that mutate the process-wide `PATH`/`HOME` env vars,
    /// so they cannot race each other under cargo's default parallel test
    /// execution within this binary.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

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
            UPSTREAM_EXTENSION_JS.replace(
                "GPU_MIN_REFRESH_INTERVAL_SECONDS = 5;",
                "GPU_MIN_REFRESH_INTERVAL_SECONDS = 0.5;",
            ),
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
        let extension_js = dir.path().join("extension.js");
        let settings_js = dir.path().join("services/settings.js");
        let before_extension = fs::read_to_string(&extension_js).expect("extension before");
        let before_settings = fs::read_to_string(&settings_js).expect("settings before");
        fs::write(dir.path().join("prefs.js"), "// unrelated\n").expect("write prefs");
        assert!(matches!(
            patch_sources_only(dir.path()),
            Err(RefreshPatchError::Unsupported(_))
        ));
        assert_eq!(
            fs::read_to_string(&extension_js).expect("extension after"),
            before_extension,
            "extension.js must stay untouched when a later refresh target is unsupported"
        );
        assert_eq!(
            fs::read_to_string(&settings_js).expect("settings after"),
            before_settings,
            "settings.js must stay untouched when a later refresh target is unsupported"
        );
    }

    #[test]
    fn already_patched_sources_still_compile_schemas() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = upstream_extension();
        // First pass writes patched sources even if schema compile is unavailable.
        let _ = patch_sources_only(dir.path());

        let bin_dir = tempfile::tempdir().expect("bin dir");
        let marker = bin_dir.path().join("compiled.marker");
        let fake = bin_dir.path().join("glib-compile-schemas");
        fs::write(
            &fake,
            format!("#!/bin/sh\nprintf 'ok' > '{}'\nexit 0\n", marker.display()),
        )
        .expect("fake compiler");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&fake).expect("meta").permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&fake, perms).expect("chmod");
        }

        // A real HOME must never be touched by a test; sync_user_schemas now
        // runs unconditionally after a successful extension schema compile.
        let home_dir = tempfile::tempdir().expect("home dir");
        let original_path = std::env::var_os("PATH").unwrap_or_default();
        let original_home = std::env::var_os("HOME");
        let mut path = std::ffi::OsString::from(bin_dir.path());
        path.push(":");
        path.push(&original_path);
        // SAFETY: test-only PATH/HOME override, restored below before this
        // test returns (and serialized via ENV_LOCK across the whole file).
        unsafe {
            std::env::set_var("PATH", &path);
            std::env::set_var("HOME", home_dir.path());
        }
        let result = patch_extension(dir.path());
        unsafe {
            std::env::set_var("PATH", original_path);
            match original_home {
                Some(home) => std::env::set_var("HOME", home),
                None => std::env::remove_var("HOME"),
            }
        }

        assert!(!result.expect("patch"), "sources should already be patched");
        assert!(
            marker.is_file(),
            "already-patched re-run must still invoke glib-compile-schemas"
        );
    }

    #[test]
    fn missing_extension_directory_exits_non_zero() {
        assert_eq!(run(Path::new("/nonexistent/rm-monitor-extension")), 1);
    }

    #[test]
    fn syncs_compiled_schema_to_user_glib_schema_dir() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = upstream_extension();

        let bin_dir = tempfile::tempdir().expect("bin dir");
        let fake = bin_dir.path().join("glib-compile-schemas");
        fs::write(&fake, "#!/bin/sh\nexit 0\n").expect("fake compiler");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&fake).expect("meta").permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&fake, perms).expect("chmod");
        }

        let home_dir = tempfile::tempdir().expect("home dir");
        let original_path = std::env::var_os("PATH").unwrap_or_default();
        let original_home = std::env::var_os("HOME");
        let mut path = std::ffi::OsString::from(bin_dir.path());
        path.push(":");
        path.push(&original_path);
        // SAFETY: test-only PATH/HOME override, restored below (serialized
        // across the file via ENV_LOCK).
        unsafe {
            std::env::set_var("PATH", &path);
            std::env::set_var("HOME", home_dir.path());
        }
        let result = patch_extension(dir.path());
        unsafe {
            std::env::set_var("PATH", original_path);
            match original_home {
                Some(home) => std::env::set_var("HOME", home),
                None => std::env::remove_var("HOME"),
            }
        }
        result.expect("patch");

        let extension_schema = dir.path().join("schemas").join(GSCHEMA_FILE_NAME);
        let user_schema = home_dir
            .path()
            .join(".local/share/glib-2.0/schemas")
            .join(GSCHEMA_FILE_NAME);
        assert!(
            user_schema.is_file(),
            "schema must be copied into the user's GSettings schema dir"
        );
        assert_eq!(
            fs::read(&user_schema).expect("read user schema"),
            fs::read(&extension_schema).expect("read extension schema"),
            "synced schema must match the extension's compiled schema byte-for-byte"
        );
    }

    #[test]
    fn re_syncing_an_identical_user_schema_is_a_no_op_copy() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let extension_dir = upstream_extension();
        // Skip the extension's own patch entirely: sync_user_schemas only
        // cares about the already-compiled schemas/ directory contents.
        let schemas_dir = extension_dir.path().join("schemas");

        let home_dir = tempfile::tempdir().expect("home dir");
        let user_schemas_dir = home_dir.path().join(".local/share/glib-2.0/schemas");
        fs::create_dir_all(&user_schemas_dir).expect("user schemas dir");
        let extension_schema_bytes =
            fs::read(schemas_dir.join(GSCHEMA_FILE_NAME)).expect("read extension schema");
        let user_schema_path = user_schemas_dir.join(GSCHEMA_FILE_NAME);
        fs::write(&user_schema_path, &extension_schema_bytes).expect("seed identical schema");
        let before_mtime = fs::metadata(&user_schema_path)
            .expect("meta")
            .modified()
            .expect("mtime");

        let original_home = std::env::var_os("HOME");
        // SAFETY: test-only HOME override, restored below (serialized across
        // the file via ENV_LOCK). PATH is left as-is on purpose: an absent or
        // failing glib-compile-schemas here must stay a soft warning, not a
        // panic, since this test does not assert on the compile step.
        unsafe { std::env::set_var("HOME", home_dir.path()) };
        sync_user_schemas(&schemas_dir);
        unsafe {
            match original_home {
                Some(home) => std::env::set_var("HOME", home),
                None => std::env::remove_var("HOME"),
            }
        }

        let after = fs::read(&user_schema_path).expect("read after sync");
        assert_eq!(after, extension_schema_bytes);
        let after_mtime = fs::metadata(&user_schema_path)
            .expect("meta")
            .modified()
            .expect("mtime");
        assert_eq!(
            before_mtime, after_mtime,
            "an identical destination schema must not be rewritten"
        );
    }

    #[test]
    fn missing_home_env_is_a_soft_no_op() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let extension_dir = upstream_extension();
        let schemas_dir = extension_dir.path().join("schemas");

        let original_home = std::env::var_os("HOME");
        // SAFETY: test-only HOME removal, restored below (serialized via
        // ENV_LOCK). Must not panic or otherwise abort the caller.
        unsafe { std::env::remove_var("HOME") };
        sync_user_schemas(&schemas_dir);
        unsafe {
            if let Some(home) = original_home {
                std::env::set_var("HOME", home);
            }
        }
    }
}
