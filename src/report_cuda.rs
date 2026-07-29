//! Query `nvidia-smi` and print a GSettings GPU device list.
//!
//! Port of `scripts/report_cuda_devices.py`. The installer consumes stdout via
//! command substitution, so stdout carries exactly one line: the GSettings
//! string array (`[]` when no GPU is present).

use std::path::PathBuf;
use std::process::{Command, Stdio};

use regex::Regex;

use crate::logging;
use crate::settings::{format_gsettings_list, Device};

/// Locate `nvidia-smi` on `PATH`, mirroring Python's `shutil.which`.
///
/// Returns the resolved path, or `None` when the tool is not installed.
pub fn detect_nvidia_smi() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join("nvidia-smi"))
        .find(|candidate| is_executable(candidate))
}

#[cfg(unix)]
fn is_executable(path: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .map(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(path: &std::path::Path) -> bool {
    path.is_file()
}

/// Parse `nvidia-smi -L` output into Resource Monitor GPU device entries.
///
/// Each line is expected to match `GPU <N>: <Name> (UUID: <uuid>)`; lines that
/// do not match are skipped.
///
/// # Parameters
/// - `output`: Raw text from `nvidia-smi -L`.
pub fn parse_gpu_output(output: &str) -> Vec<Device> {
    // The pattern is a compile-time constant, so compilation cannot fail.
    let pattern = Regex::new(r"GPU\s+\d+:\s+(.*?)\s+\(UUID:\s+([^)]+)\)")
        .expect("GPU listing pattern is valid");

    let mut entries = Vec::new();
    for line in output.lines() {
        let Some(captures) = pattern.captures(line.trim()) else {
            continue;
        };
        let name = &captures[1];
        let uuid = &captures[2];
        entries.push(
            Device::new()
                .with("version", 2)
                .with("type", "gpu")
                .with("device", uuid)
                .with("name", name)
                .with("usage", true)
                .with("memory", true)
                .with("displayName", ""),
        );
    }
    entries
}

/// Query `nvidia-smi` and return structured GPU device information.
///
/// Returns an empty list when `nvidia-smi` is unavailable or the query fails.
pub fn get_gpu_devices() -> Vec<Device> {
    let Some(nvidia_smi) = detect_nvidia_smi() else {
        logging::info("nvidia-smi not found; no GPUs detected");
        return Vec::new();
    };

    let output = match Command::new(&nvidia_smi)
        .arg("-L")
        .stderr(Stdio::null())
        .output()
    {
        Ok(output) if output.status.success() => output.stdout,
        Ok(output) => {
            logging::warn(format!("nvidia-smi -L failed with status {}", output.status));
            return Vec::new();
        }
        Err(err) => {
            logging::warn(format!("nvidia-smi -L failed: {err}"));
            return Vec::new();
        }
    };

    let devices = parse_gpu_output(&String::from_utf8_lossy(&output));
    logging::info(format!("Detected {} GPU device(s)", devices.len()));
    devices
}

/// CLI entry point: print the GSettings GPU device array to stdout.
///
/// Returns `0` on success. A missing `nvidia-smi` is not an error; the command
/// prints `[]` and succeeds so the installer can use the value unconditionally.
pub fn run() -> i32 {
    println!("{}", format_gsettings_list(&get_gpu_devices()));
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_single_gpu_line() {
        let devices = parse_gpu_output("GPU 0: NVIDIA GeForce RTX 4090 (UUID: GPU-abc123)");
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].get_str("device"), Some("GPU-abc123"));
        assert_eq!(devices[0].get_str("name"), Some("NVIDIA GeForce RTX 4090"));
        assert_eq!(devices[0].get_str("type"), Some("gpu"));
    }

    #[test]
    fn parses_multiple_gpus_and_skips_noise() {
        let output = "GPU 0: NVIDIA A100 (UUID: GPU-1)\n\
warning: something happened\n\
GPU 1: NVIDIA A100 (UUID: GPU-2)\n";
        let devices = parse_gpu_output(output);
        assert_eq!(devices.len(), 2);
        assert_eq!(devices[1].get_str("device"), Some("GPU-2"));
    }

    #[test]
    fn empty_output_yields_the_empty_gsettings_array() {
        assert!(parse_gpu_output("").is_empty());
        assert_eq!(format_gsettings_list(&parse_gpu_output("")), "[]");
    }

    #[test]
    fn stdout_contract_is_a_repr_wrapped_json_array() {
        let devices = parse_gpu_output("GPU 0: NVIDIA A100 (UUID: GPU-1)");
        assert_eq!(
            format_gsettings_list(&devices),
            r#"['{"version": 2, "type": "gpu", "device": "GPU-1", "name": "NVIDIA A100", "usage": true, "memory": true, "displayName": ""}']"#
        );
    }
}
