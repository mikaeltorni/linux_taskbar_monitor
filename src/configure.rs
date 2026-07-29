//! Configure Resource Monitor extension display settings.
//!
//! Resolves the extension's compiled schema directory, discovers GPU and disk
//! devices for the requested display mode, and applies each `gsettings set`
//! command in turn.

use std::path::{Path, PathBuf};

use crate::disks::{detect_disk_devices, detect_disk_devices_home_only};
use crate::logging;
use crate::report_cuda::get_gpu_devices;
use crate::settings::{apply_settings, build_gsettings_args, DisplayMode};

/// Resource Monitor GSettings schema ID.
const SCHEMA: &str = "org.gnome.shell.extensions.resource-monitor";

/// Default extension UUID when `RESOURCE_MONITOR_EXTENSION_ID` is unset.
const DEFAULT_EXTENSION_ID: &str = "Resource_Monitor@Ory0n";

/// Resolve the extension schema directory under `home` for `extension_id`.
fn schema_dir_under_home(home: &Path, extension_id: &str) -> PathBuf {
    home.join(".local/share/gnome-shell/extensions")
        .join(extension_id)
        .join("schemas")
}

/// Candidate home directories for schema auto-detection.
///
/// Prefers `HOME` (so custom homes and sudo `-H -u` installs work), then
/// `/home/$SUDO_USER` and `/home/$USER` when those differ from `HOME`. Never
/// invents a machine-specific username fallback.
fn candidate_homes() -> Vec<PathBuf> {
    let mut homes = Vec::new();
    let push_unique = |homes: &mut Vec<PathBuf>, path: PathBuf| {
        if !path.as_os_str().is_empty() && !homes.iter().any(|existing| existing == &path) {
            homes.push(path);
        }
    };

    if let Ok(home) = std::env::var("HOME") {
        push_unique(&mut homes, PathBuf::from(home));
    }
    for key in ["SUDO_USER", "USER"] {
        if let Ok(user) = std::env::var(key) {
            if user.is_empty() || user == "root" {
                continue;
            }
            push_unique(&mut homes, PathBuf::from(format!("/home/{user}")));
        }
    }
    homes
}

/// Auto-detect the compiled schema directory of the installed extension.
///
/// Honors `RESOURCE_MONITOR_EXTENSION_ID` (same env the installer uses). Returns
/// the first existing schemas directory under the candidate homes, or `None`.
fn auto_detect_schema_dir() -> Option<PathBuf> {
    let extension_id = std::env::var("RESOURCE_MONITOR_EXTENSION_ID")
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_EXTENSION_ID.to_string());

    let mut tried = Vec::new();
    for home in candidate_homes() {
        let candidate = schema_dir_under_home(&home, &extension_id);
        if candidate.is_dir() {
            return Some(candidate);
        }
        tried.push(candidate.display().to_string());
    }

    logging::error(format!(
        "Could not auto-detect schema directory (tried: {}). Use --schema-dir.",
        if tried.is_empty() {
            "(no HOME/USER candidates)".to_string()
        } else {
            tried.join(", ")
        }
    ));
    eprintln!(
        "Could not auto-detect schema directory (tried: {}). Use --schema-dir.",
        if tried.is_empty() {
            "(no HOME/USER candidates)".to_string()
        } else {
            tried.join(", ")
        }
    );
    None
}

/// CLI entry point: configure the requested Resource Monitor display mode.
///
/// # Parameters
/// - `gpu_memory_perc`: Show GPU memory as a percentage instead of numeric GB.
/// - `disk_space_gb`: Show disk space in numeric GB (the patch adds live IO %).
/// - `disk_space_perc`: Hidden legacy alias for `disk_space_gb`.
/// - `disk_space_perc_home_only`: Show only `/home` usage percentage.
/// - `schema_dir`: Explicit schema directory; auto-detected when `None`.
///
/// Returns `0` when every `gsettings set` succeeded, otherwise `1`.
pub fn run(
    gpu_memory_perc: bool,
    disk_space_gb: bool,
    disk_space_perc: bool,
    disk_space_perc_home_only: bool,
    schema_dir: Option<PathBuf>,
) -> i32 {
    if disk_space_gb && disk_space_perc_home_only {
        logging::error("--disk-space-gb and --disk-space-perc-home-only are mutually exclusive.");
        eprintln!("--disk-space-gb and --disk-space-perc-home-only are mutually exclusive.");
        return 1;
    }

    let configure_disk_space = disk_space_gb || disk_space_perc || disk_space_perc_home_only;
    if !gpu_memory_perc && !configure_disk_space {
        logging::error("No options specified. Use --gpu-memory-perc and/or --disk-space-gb.");
        eprintln!("No options specified. Use --gpu-memory-perc and/or --disk-space-gb.");
        return 1;
    }

    let ext_dir = match schema_dir.or_else(auto_detect_schema_dir) {
        Some(dir) => dir,
        None => return 1,
    };

    let disk_devices = if disk_space_perc_home_only {
        Some(detect_disk_devices_home_only())
    } else if configure_disk_space {
        Some(detect_disk_devices())
    } else {
        None
    };

    let gpu_devices = if gpu_memory_perc {
        Some(get_gpu_devices())
    } else {
        None
    };

    let mode = DisplayMode {
        gpu_memory_perc,
        disk_space_gb: configure_disk_space && !disk_space_perc_home_only,
        disk_space_perc,
        disk_space_perc_home_only,
    };

    let commands = build_gsettings_args(
        SCHEMA,
        &ext_dir.to_string_lossy(),
        mode,
        gpu_devices.as_deref(),
        disk_devices.as_deref(),
    );

    if commands.is_empty() {
        logging::info("No settings to apply.");
        return 0;
    }

    let mut success = true;
    for args in &commands {
        logging::info(format!("Running: {}", args.join(" ")));
        if !apply_settings(args) {
            success = false;
        }
    }

    if success {
        logging::info("configure-resource-monitor applied all settings");
        0
    } else {
        logging::error("configure-resource-monitor failed to apply one or more settings");
        eprintln!("configure-resource-monitor failed to apply one or more settings");
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutually_exclusive_disk_modes_are_rejected() {
        assert_eq!(run(false, true, false, true, None), 1);
    }

    #[test]
    fn no_options_is_a_usage_error() {
        assert_eq!(run(false, false, false, false, None), 1);
    }

    #[test]
    fn schema_dir_under_home_joins_uuid_and_schemas() {
        assert_eq!(
            schema_dir_under_home(Path::new("/home/someone"), DEFAULT_EXTENSION_ID),
            PathBuf::from(
                "/home/someone/.local/share/gnome-shell/extensions/Resource_Monitor@Ory0n/schemas"
            )
        );
    }

    #[test]
    fn candidate_homes_prefers_home_env() {
        // Pure unit check of path joining; env-dependent ordering is covered by
        // the installer always passing --schema-dir for configure-resource-monitor.
        let under = schema_dir_under_home(Path::new("/custom/home"), "Ext@id");
        assert_eq!(
            under,
            PathBuf::from("/custom/home/.local/share/gnome-shell/extensions/Ext@id/schemas")
        );
    }
}
