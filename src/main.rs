//! `rm-monitor` — standalone helpers for the Linux Taskbar Monitor Resource
//! Monitor installer (tested on Ubuntu 24.04 / GNOME Shell 46).
//!
//! One binary replaces the Python and Node helper scripts the installer used to
//! shell out to, so a clean install needs no `python3` or `node` runtime. Each
//! subcommand keeps the exit codes and stdout contract of the script it
//! replaces: machine-readable installer payloads (`report-cuda-devices`,
//! `gsettings-strv`) stay on stdout alone; patchers also print a short human
//! progress line on stdout for interactive runs. Structured diagnostics go to
//! the repository `.log/` sink (and errors also to stderr).

mod configure;
mod disks;
#[cfg(test)]
mod env_test_lock;
mod gradient_colors;
mod gsettings_strv;
mod logging;
mod patch_colors;
mod patch_disk;
mod patch_eth_icon;
mod patch_metadata;
mod patch_process_popup;
mod patch_refresh;
mod patch_stable_width;
mod patch_text;
mod patch_vram;
mod report_cuda;
mod settings;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "rm-monitor",
    about = "Helpers for installing and patching the Resource Monitor GNOME Shell extension",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Append or remove a value in a GSettings string array read from `CURRENT`.
    GsettingsStrv {
        /// Either `append` or `remove`.
        action: String,
        /// Value to add or drop.
        value: String,
    },

    /// Print the GSettings GPU device array reported by `nvidia-smi`.
    ReportCudaDevices,

    /// Configure Resource Monitor extension display settings.
    ConfigureResourceMonitor {
        /// Set GPU memory to percentage mode (used/total %).
        #[arg(long)]
        gpu_memory_perc: bool,
        /// Free `/home` space in GB (installer default); live IO % comes from patch-disk.
        #[arg(long)]
        disk_space_gb: bool,
        /// Legacy alias for `--disk-space-gb`, kept for installer compatibility.
        #[arg(long, hide = true)]
        disk_space_perc: bool,
        /// Show /home disk space as percentage of total.
        #[arg(long)]
        disk_space_perc_home_only: bool,
        /// Path to the extension schemas directory (auto-detected if omitted).
        #[arg(long, value_name = "PATH")]
        schema_dir: Option<PathBuf>,
    },

    /// Add a shell version to metadata.json and pin the extension version.
    PatchExtensionMetadata {
        /// Path to the extension's metadata.json.
        metadata: PathBuf,
        /// Shell version string to add (e.g. `46`).
        shell_version: String,
        /// Optional version-pin integer that outranks extensions.gnome.org.
        pin: Option<String>,
    },

    /// Enable 0.1-60 second refresh intervals in an installed extension.
    PatchRefresh {
        /// Installed Resource Monitor extension directory.
        extension_dir: PathBuf,
    },

    /// Remove the brackets around the GPU VRAM value.
    PatchVram {
        /// Path to the extension's panel/containers.js.
        containers: PathBuf,
    },

    /// Replace `_getUsageColor` with 256-step RGB gradients.
    PatchColors {
        /// Path to the extension's extension.js.
        extension: PathBuf,
    },

    /// Show free disk space plus live IO activity percentage.
    PatchDisk {
        /// Path to the extension's panel/containers.js.
        containers: PathBuf,
    },

    /// Drop the ethernet display icon, keeping its value and unit.
    PatchEthIcon {
        /// Path to the extension's panel/mainGui.js.
        main_gui: PathBuf,
    },

    /// Make left-click open a per-process CPU/RAM popup.
    PatchProcessPopup {
        /// Path to the extension's extension.js.
        extension: PathBuf,
    },

    /// Reserve (or release) fixed widths for the activity and VRAM labels.
    PatchStableWidth {
        /// `stable` reserves the widths; `compact` releases them.
        #[arg(long, default_value = "stable")]
        mode: String,
        /// Path to the extension's panel/containers.js.
        containers: PathBuf,
    },
}

fn main() -> ExitCode {
    logging::init();

    let cli = Cli::parse();
    let code = match cli.command {
        Command::GsettingsStrv { action, value } => match gsettings_strv::run(&action, &value) {
            Ok(()) => 0,
            Err(code) => code,
        },
        Command::ReportCudaDevices => report_cuda::run(),
        Command::ConfigureResourceMonitor {
            gpu_memory_perc,
            disk_space_gb,
            disk_space_perc,
            disk_space_perc_home_only,
            schema_dir,
        } => configure::run(
            gpu_memory_perc,
            disk_space_gb,
            disk_space_perc,
            disk_space_perc_home_only,
            schema_dir,
        ),
        Command::PatchExtensionMetadata {
            metadata,
            shell_version,
            pin,
        } => patch_metadata::run(&metadata, &shell_version, pin.as_deref()),
        Command::PatchRefresh { extension_dir } => patch_refresh::run(&extension_dir),
        Command::PatchVram { containers } => patch_vram::run(&containers),
        Command::PatchColors { extension } => patch_colors::run(&extension),
        Command::PatchDisk { containers } => patch_disk::run(&containers),
        Command::PatchEthIcon { main_gui } => patch_eth_icon::run(&main_gui),
        Command::PatchProcessPopup { extension } => patch_process_popup::run(&extension),
        Command::PatchStableWidth { mode, containers } => {
            patch_stable_width::run(&mode, &containers)
        }
    };

    ExitCode::from(u8::try_from(code).unwrap_or(1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn every_documented_subcommand_parses() {
        let invocations: Vec<Vec<&str>> = vec![
            vec!["rm-monitor", "gsettings-strv", "append", "value"],
            vec!["rm-monitor", "gsettings-strv", "remove", "value"],
            vec!["rm-monitor", "report-cuda-devices"],
            vec![
                "rm-monitor",
                "configure-resource-monitor",
                "--gpu-memory-perc",
            ],
            vec![
                "rm-monitor",
                "configure-resource-monitor",
                "--disk-space-gb",
            ],
            vec![
                "rm-monitor",
                "configure-resource-monitor",
                "--disk-space-perc-home-only",
                "--schema-dir",
                "/tmp/schemas",
            ],
            vec![
                "rm-monitor",
                "patch-extension-metadata",
                "metadata.json",
                "46",
            ],
            vec![
                "rm-monitor",
                "patch-extension-metadata",
                "metadata.json",
                "46",
                "9999",
            ],
            vec!["rm-monitor", "patch-refresh", "/ext"],
            vec!["rm-monitor", "patch-vram", "containers.js"],
            vec!["rm-monitor", "patch-colors", "extension.js"],
            vec!["rm-monitor", "patch-disk", "containers.js"],
            vec!["rm-monitor", "patch-eth-icon", "mainGui.js"],
            vec!["rm-monitor", "patch-process-popup", "extension.js"],
            vec!["rm-monitor", "patch-stable-width", "containers.js"],
            vec![
                "rm-monitor",
                "patch-stable-width",
                "--mode",
                "compact",
                "containers.js",
            ],
        ];

        for invocation in invocations {
            Cli::try_parse_from(&invocation)
                .unwrap_or_else(|err| panic!("failed to parse {invocation:?}: {err}"));
        }
    }

    #[test]
    fn stable_width_defaults_to_stable_mode() {
        let cli = Cli::try_parse_from(["rm-monitor", "patch-stable-width", "containers.js"])
            .expect("parse");
        match cli.command {
            Command::PatchStableWidth { mode, .. } => assert_eq!(mode, "stable"),
            _ => panic!("unexpected subcommand"),
        }
    }

    #[test]
    fn legacy_disk_space_perc_flag_is_still_accepted() {
        let cli = Cli::try_parse_from([
            "rm-monitor",
            "configure-resource-monitor",
            "--disk-space-perc",
        ])
        .expect("parse");
        match cli.command {
            Command::ConfigureResourceMonitor {
                disk_space_perc, ..
            } => assert!(disk_space_perc),
            _ => panic!("unexpected subcommand"),
        }
    }
}
