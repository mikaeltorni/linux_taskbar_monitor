//! Configure Resource Monitor extension display settings.
//!
//! Port of `scripts/configure_resource_monitor.py`. Resolves the extension's
//! compiled schema directory, discovers GPU and disk devices for the requested
//! display mode, and applies each `gsettings set` command in turn.

use std::path::PathBuf;

use crate::disks::{detect_disk_devices, detect_disk_devices_home_only};
use crate::logging;
use crate::report_cuda::get_gpu_devices;
use crate::settings::{apply_settings, build_gsettings_args, DisplayMode};

/// Resource Monitor GSettings schema ID.
const SCHEMA: &str = "org.gnome.shell.extensions.resource-monitor";

/// Resolve the extension schema directory for `username`.
fn schema_dir_candidate(username: &str) -> PathBuf {
    PathBuf::from(format!(
        "/home/{username}/.local/share/gnome-shell/extensions/Resource_Monitor@Ory0n/schemas"
    ))
}

/// Auto-detect the compiled schema directory of the installed extension.
///
/// Prefers `SUDO_USER` so a sudo-wrapped installer still targets the invoking
/// user's home, falling back to `USER` and finally the historical `mk` default.
///
/// Returns the directory, or `None` when it does not exist.
fn auto_detect_schema_dir() -> Option<PathBuf> {
    let username = std::env::var("SUDO_USER")
        .ok()
        .filter(|value| !value.is_empty())
        .or_else(|| std::env::var("USER").ok())
        .unwrap_or_else(|| "mk".to_string());

    let candidate = schema_dir_candidate(&username);
    if candidate.is_dir() {
        return Some(candidate);
    }

    logging::error(format!(
        "Could not auto-detect schema directory at {}. Use --schema-dir.",
        candidate.display()
    ));
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
        0
    } else {
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
    fn schema_dir_candidate_uses_the_extension_uuid_path() {
        assert_eq!(
            schema_dir_candidate("someone"),
            PathBuf::from(
                "/home/someone/.local/share/gnome-shell/extensions/Resource_Monitor@Ory0n/schemas"
            )
        );
    }
}
