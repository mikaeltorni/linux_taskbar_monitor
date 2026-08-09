//! Detect and format disk devices for Resource Monitor.
//!
//! Discovers mounted block devices
//! through `df -P`, always exposes a `/home` row (even when `/home` lives on the
//! root filesystem), and shapes each row as a Resource Monitor v2 disk entry.

use std::process::{Command, Stdio};

use crate::logging;
use crate::settings::Device;

/// Build a Resource Monitor v2 disk device entry.
///
/// # Parameters
/// - `filesystem`: Block device path reported by `df` (e.g. `/dev/sda2`).
/// - `mount_point`: Mount point the device is mounted at (e.g. `/`).
///
/// Returns an entry whose `space` flag is enabled and `stats` flag disabled,
/// keyed for the disk-space row.
pub fn build_disk_device_entry(filesystem: &str, mount_point: &str) -> Device {
    Device::new()
        .with("version", 2)
        .with("type", "disk")
        .with("device", filesystem)
        .with("stableId", "")
        .with("mountPoint", mount_point)
        .with("stats", false)
        .with("space", true)
        .with("displayName", mount_point)
}

/// Parse POSIX `df` output into Resource Monitor disk entries.
///
/// Skips the header row and any line that does not describe a real block device
/// (only rows whose first column starts with `/dev/` are kept).
///
/// # Parameters
/// - `output`: Raw text from `df -P` (or `df -P <path>`).
pub fn parse_df_output(output: &str) -> Vec<Device> {
    let trimmed = output.trim();
    let lines: Vec<&str> = trimmed.lines().collect();
    if lines.len() < 2 {
        return Vec::new();
    }

    let mut entries = Vec::new();
    for line in &lines[1..] {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 6 {
            continue;
        }

        let filesystem = parts[0];
        let mount_point = parts[5..].join(" ");
        if !filesystem.starts_with("/dev/") {
            continue;
        }

        entries.push(build_disk_device_entry(filesystem, &mount_point));
    }

    entries
}

/// Run a `df` command and return its stdout, or `None` when it fails.
fn run_df(args: &[&str]) -> Option<String> {
    let output = Command::new("df")
        .args(args)
        .stderr(Stdio::null())
        .output()
        .map_err(|err| {
            logging::warn(format!("df command failed: {err}"));
            err
        })
        .ok()?;

    if !output.status.success() {
        logging::warn(format!("df command failed with status {}", output.status));
        return None;
    }

    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Append a `/home` row when it is not already a separate mount.
///
/// When `/home` lives on its own mount it is already present and the list is
/// returned unchanged. Otherwise `df -P /home` resolves the backing device and a
/// dedicated `/home` entry is appended so the panel can show home usage.
///
/// # Parameters
/// - `entries`: Existing disk entries from [`parse_df_output`].
pub fn append_home_directory_entry(mut entries: Vec<Device>) -> Vec<Device> {
    if entries
        .iter()
        .any(|entry| entry.get_str("mountPoint") == Some("/home"))
    {
        return entries;
    }

    let Some(output) = run_df(&["-P", "/home"]) else {
        return entries;
    };

    let home_entries = parse_df_output(&output);
    let Some(mut home_entry) = home_entries.into_iter().next() else {
        return entries;
    };

    home_entry.set_str("mountPoint", "/home");
    home_entry.set_str("displayName", "/home");
    logging::info(format!(
        "Added /home disk entry backed by {}",
        home_entry.get_str("device").unwrap_or("?")
    ));
    entries.push(home_entry);
    entries
}

/// Query `df` and return mounted block devices plus a `/home` row.
///
/// Returns an empty list when `df` fails.
pub fn detect_disk_devices() -> Vec<Device> {
    let Some(output) = run_df(&["-P"]) else {
        return Vec::new();
    };
    let devices = append_home_directory_entry(parse_df_output(&output));
    logging::info(format!("Detected {} disk device(s)", devices.len()));
    devices
}

/// Return disk entries matching `mount_point`, or all entries for `None`.
///
/// # Parameters
/// - `devices`: Disk entries to filter.
/// - `mount_point`: Mount point to keep (e.g. `/home`); `None` disables filtering.
pub fn filter_disk_devices_to_mount_point(
    devices: Vec<Device>,
    mount_point: Option<&str>,
) -> Vec<Device> {
    let Some(mount_point) = mount_point else {
        return devices;
    };
    devices
        .into_iter()
        .filter(|device| device.get_str("mountPoint") == Some(mount_point))
        .collect()
}

/// Detect and return only the Resource Monitor entry for `/home`.
///
/// Returns a single-element list, or an empty list when `df` fails or `/home`
/// cannot be resolved.
pub fn detect_disk_devices_home_only() -> Vec<Device> {
    let Some(output) = run_df(&["-P"]) else {
        return Vec::new();
    };
    let entries = append_home_directory_entry(parse_df_output(&output));
    let filtered = filter_disk_devices_to_mount_point(entries, Some("/home"));
    logging::info(format!("Detected {} /home disk entry", filtered.len()));
    filtered
}

#[cfg(test)]
mod tests {
    use super::*;

    const DF_OUTPUT: &str = "Filesystem     1024-blocks      Used Available Capacity Mounted on\n\
tmpfs              3276800     22000   3254800       1% /run\n\
/dev/nvme0n1p2   982820000 411000000 521000000      45% /\n\
/dev/nvme0n1p1      523248      6300    516948       2% /boot/efi\n";

    #[test]
    fn parse_keeps_only_block_devices() {
        let entries = parse_df_output(DF_OUTPUT);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].get_str("device"), Some("/dev/nvme0n1p2"));
        assert_eq!(entries[0].get_str("mountPoint"), Some("/"));
        assert_eq!(entries[1].get_str("mountPoint"), Some("/boot/efi"));
    }

    #[test]
    fn parse_preserves_mount_points_with_spaces() {
        let output = "\
Filesystem     1024-blocks Used Available Capacity Mounted on
/dev/sda2           100000  50   99950      1% /mnt/my data
";
        let entries = parse_df_output(output);
        assert_eq!(entries.len(), 1);
        assert!(entries[0]
            .to_json()
            .contains(r#""mountPoint": "/mnt/my data""#));
    }

    #[test]
    fn entry_shape_matches_the_schema_version_two_layout() {
        let entry = build_disk_device_entry("/dev/sda2", "/home");
        assert_eq!(
            entry.to_json(),
            r#"{"version": 2, "type": "disk", "device": "/dev/sda2", "stableId": "", "mountPoint": "/home", "stats": false, "space": true, "displayName": "/home"}"#
        );
    }

    #[test]
    fn existing_home_mount_is_left_untouched() {
        let entries = vec![
            build_disk_device_entry("/dev/sda1", "/"),
            build_disk_device_entry("/dev/sda2", "/home"),
        ];
        let result = append_home_directory_entry(entries.clone());
        assert_eq!(result, entries);
    }

    #[test]
    fn filtering_by_mount_point_is_optional() {
        let entries = vec![
            build_disk_device_entry("/dev/sda1", "/"),
            build_disk_device_entry("/dev/sda2", "/home"),
        ];
        assert_eq!(
            filter_disk_devices_to_mount_point(entries.clone(), None).len(),
            2
        );
        let home = filter_disk_devices_to_mount_point(entries, Some("/home"));
        assert_eq!(home.len(), 1);
        assert_eq!(home[0].get_str("device"), Some("/dev/sda2"));
    }
}
