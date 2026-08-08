//! Patch a GNOME extension `metadata.json`.
//!
//! Adds the running shell version
//! to the `shell-version` list and pins `version` above any plausible upstream
//! release so GNOME never auto-updates over the local source patches.

use std::fs;
use std::path::Path;

use serde_json::Value;
use thiserror::Error;

use crate::logging;
use crate::settings::python_json_dumps;

/// Failure while reading, parsing, or writing `metadata.json`.
#[derive(Debug, Error)]
pub enum MetadataError {
    /// The metadata file does not exist.
    #[error("Metadata file not found: {0}")]
    NotFound(String),
    /// The metadata file could not be read or written.
    #[error("{0}")]
    Io(#[from] std::io::Error),
    /// The metadata file does not contain valid JSON.
    #[error("Invalid JSON in {path}: {source}")]
    InvalidJson {
        /// Path of the offending file.
        path: String,
        /// Underlying parse failure.
        source: serde_json::Error,
    },
}

fn load(path: &Path) -> Result<Value, MetadataError> {
    if !path.is_file() {
        return Err(MetadataError::NotFound(path.display().to_string()));
    }
    let text = fs::read_to_string(path)?;
    serde_json::from_str(&text).map_err(|source| MetadataError::InvalidJson {
        path: path.display().to_string(),
        source,
    })
}

fn store(path: &Path, data: &Value) -> Result<(), MetadataError> {
    crate::patch_text::write_atomic(path, &python_json_dumps(data))?;
    Ok(())
}

/// Ensure `shell_version` is listed in the metadata's `shell-version` array.
///
/// A non-list `shell-version` value is replaced with a single-element list. The
/// file is written back only when a change was made.
///
/// # Parameters
/// - `metadata_path`: Path to the extension's `metadata.json`.
/// - `shell_version`: Shell version string to add (e.g. `"46"`).
///
/// Returns `true` when the file was modified.
pub fn patch_shell_version(
    metadata_path: &Path,
    shell_version: &str,
) -> Result<bool, MetadataError> {
    let mut data = load(metadata_path)?;

    let modified = match data.get_mut("shell-version") {
        Some(Value::Array(versions)) => {
            let present = versions
                .iter()
                .any(|entry| entry.as_str() == Some(shell_version));
            if present {
                false
            } else {
                versions.push(Value::from(shell_version));
                true
            }
        }
        _ => {
            data["shell-version"] = Value::from(vec![shell_version]);
            true
        }
    };

    if modified {
        store(metadata_path, &data)?;
        logging::info(format!(
            "Patched shell-version in {} to include {shell_version}",
            metadata_path.display()
        ));
        println!("Patched shell-version in metadata.json to include {shell_version}");
    }

    Ok(modified)
}

/// Raise the extension's `version` field to at least `min_version`.
///
/// GNOME Shell auto-updates EGO-sourced extensions whenever the remote version
/// outranks the installed one, which would silently overwrite the local source
/// patches. Pinning the local version high keeps the remote release from ever
/// appearing newer.
///
/// # Parameters
/// - `metadata_path`: Path to the extension's `metadata.json`.
/// - `min_version`: Minimum version integer to pin to (e.g. `9999`).
///
/// Returns `true` when the file was modified.
pub fn pin_version(metadata_path: &Path, min_version: i64) -> Result<bool, MetadataError> {
    let mut data = load(metadata_path)?;

    let current = data
        .get("version")
        .and_then(|value| match value {
            Value::Number(number) => number.as_i64(),
            Value::String(text) => text.parse::<i64>().ok(),
            _ => None,
        })
        .unwrap_or(0);

    if current >= min_version {
        return Ok(false);
    }

    data["version"] = Value::from(min_version);
    store(metadata_path, &data)?;
    logging::info(format!(
        "Pinned {} version to {min_version} from {current}",
        metadata_path.display()
    ));
    println!("Pinned metadata.json version to {min_version} (was {current})");
    Ok(true)
}

/// CLI entry point for the metadata patcher.
///
/// # Parameters
/// - `metadata_path`: Path to the extension's `metadata.json`.
/// - `shell_version`: Shell version string to add.
/// - `pin`: Optional version-pin integer, as passed on the command line.
///
/// Returns `0` on success and `1` on any error, matching the Python script.
pub fn run(metadata_path: &Path, shell_version: &str, pin: Option<&str>) -> i32 {
    logging::info(format!(
        "patch-extension-metadata path={} shell_version={shell_version} pin={pin:?}",
        metadata_path.display()
    ));

    let modified = match patch_shell_version(metadata_path, shell_version) {
        Ok(modified) => modified,
        Err(err) => return report(err),
    };
    if !modified {
        logging::info(format!(
            "Shell version {shell_version} already present in {}",
            metadata_path.display()
        ));
        println!("Shell version '{shell_version}' already present — no changes made.");
    }

    if let Some(pin) = pin {
        let Ok(min_version) = pin.parse::<i64>() else {
            logging::error(format!("Invalid pin version: {pin}"));
            eprintln!("Error: invalid pin version: {pin}");
            return 1;
        };
        match pin_version(metadata_path, min_version) {
            Ok(true) => {}
            Ok(false) => {
                logging::info(format!(
                    "Version already pinned at or above {pin} in {}",
                    metadata_path.display()
                ));
                println!("Version already pinned at or above {pin} — no changes made.");
            }
            Err(err) => return report(err),
        }
    }

    0
}

fn report(err: MetadataError) -> i32 {
    logging::error(err.to_string());
    eprintln!("Error: {err}");
    1
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_metadata(contents: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("metadata.json");
        let mut file = fs::File::create(&path).expect("create metadata");
        file.write_all(contents.as_bytes()).expect("write metadata");
        (dir, path)
    }

    #[test]
    fn adds_a_missing_shell_version_list() {
        let (_dir, path) = write_metadata(r#"{"uuid": "test@ext"}"#);
        assert!(patch_shell_version(&path, "46").expect("patch"));
        let written = fs::read_to_string(&path).expect("read back");
        assert_eq!(written, r#"{"uuid": "test@ext", "shell-version": ["46"]}"#);
    }

    #[test]
    fn appends_to_an_existing_shell_version_list() {
        let (_dir, path) = write_metadata(r#"{"shell-version": ["45"]}"#);
        assert!(patch_shell_version(&path, "46").expect("patch"));
        assert_eq!(
            fs::read_to_string(&path).expect("read back"),
            r#"{"shell-version": ["45", "46"]}"#
        );
    }

    #[test]
    fn already_present_shell_version_is_a_no_op() {
        let (_dir, path) = write_metadata(r#"{"shell-version": ["46"]}"#);
        assert!(!patch_shell_version(&path, "46").expect("patch"));
    }

    #[test]
    fn pins_the_version_upwards_only() {
        let (_dir, path) = write_metadata(r#"{"uuid": "t", "version": 27}"#);
        assert!(pin_version(&path, 9999).expect("pin"));
        assert_eq!(
            fs::read_to_string(&path).expect("read back"),
            r#"{"uuid": "t", "version": 9999}"#
        );
        assert!(!pin_version(&path, 9999).expect("pin again"));
        assert!(!pin_version(&path, 10).expect("lower pin"));
    }

    #[test]
    fn missing_version_field_is_treated_as_zero() {
        let (_dir, path) = write_metadata(r#"{"uuid": "t"}"#);
        assert!(pin_version(&path, 1).expect("pin"));
    }

    #[test]
    fn missing_file_and_invalid_json_report_errors() {
        let missing = Path::new("/nonexistent/rm-monitor/metadata.json");
        assert!(matches!(
            patch_shell_version(missing, "46"),
            Err(MetadataError::NotFound(_))
        ));
        assert_eq!(run(missing, "46", None), 1);

        let (_dir, path) = write_metadata("{not json");
        assert!(matches!(
            patch_shell_version(&path, "46"),
            Err(MetadataError::InvalidJson { .. })
        ));
        assert_eq!(run(&path, "46", None), 1);
    }

    #[test]
    fn run_applies_both_patches_and_preserves_key_order() {
        let (_dir, path) = write_metadata(r#"{"uuid": "t", "version": 1, "name": "Res"}"#);
        assert_eq!(run(&path, "46", Some("9999")), 0);
        assert_eq!(
            fs::read_to_string(&path).expect("read back"),
            r#"{"uuid": "t", "version": 9999, "name": "Res", "shell-version": ["46"]}"#
        );
        assert_eq!(run(&path, "46", Some("9999")), 0);
    }

    #[test]
    fn invalid_pin_argument_is_rejected() {
        let (_dir, path) = write_metadata(r#"{"uuid": "t"}"#);
        assert_eq!(run(&path, "46", Some("not-a-number")), 1);
    }
}
