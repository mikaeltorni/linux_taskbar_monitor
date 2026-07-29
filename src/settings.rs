//! Build and apply GSettings values for the Resource Monitor extension.
//!
//! Owns device serialization
//! (byte-compatible with Python's `repr(json.dumps(device))`), display-mode
//! command construction, and execution of individual `gsettings set` commands.

use std::io::{self, Read, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Map, Value};

use crate::disks::filter_disk_devices_to_mount_point;
use crate::logging;

/// Wall-clock budget for a single `gsettings set` invocation.
const GSETTINGS_TIMEOUT: Duration = Duration::from_secs(10);

/// A Resource Monitor GPU or disk device entry.
///
/// Wraps an order-preserving JSON object so serialization matches Python's
/// `json.dumps`, which emits keys in dict insertion order.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Device(Map<String, Value>);

impl Device {
    /// Create an empty device entry.
    pub fn new() -> Self {
        Self(Map::new())
    }

    /// Append a field, preserving insertion order.
    ///
    /// # Parameters
    /// - `key`: JSON key, e.g. `"mountPoint"`.
    /// - `value`: Any value convertible into a JSON scalar.
    pub fn with(mut self, key: &str, value: impl Into<Value>) -> Self {
        self.0.insert(key.to_string(), value.into());
        self
    }

    /// Read a string field, or `None` when absent or not a string.
    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(Value::as_str)
    }

    /// Overwrite (or add) a string field in place.
    pub fn set_str(&mut self, key: &str, value: &str) {
        self.0.insert(key.to_string(), Value::from(value));
    }

    /// Serialize as a JSON object using Python's `json.dumps` separators.
    pub fn to_json(&self) -> String {
        python_json_dumps(&Value::Object(self.0.clone()))
    }
}

/// `serde_json` formatter reproducing Python's default `json.dumps` separators
/// (`", "` between items and `": "` between key and value).
struct PythonFormatter;

impl serde_json::ser::Formatter for PythonFormatter {
    fn begin_array_value<W>(&mut self, writer: &mut W, first: bool) -> io::Result<()>
    where
        W: ?Sized + Write,
    {
        if first {
            Ok(())
        } else {
            writer.write_all(b", ")
        }
    }

    fn begin_object_key<W>(&mut self, writer: &mut W, first: bool) -> io::Result<()>
    where
        W: ?Sized + Write,
    {
        if first {
            Ok(())
        } else {
            writer.write_all(b", ")
        }
    }

    fn begin_object_value<W>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + Write,
    {
        writer.write_all(b": ")
    }
}

/// Serialize a JSON value exactly like Python's default `json.dumps`.
///
/// # Parameters
/// - `value`: The value to render.
pub fn python_json_dumps(value: &Value) -> String {
    let mut buffer = Vec::new();
    let mut serializer = serde_json::Serializer::with_formatter(&mut buffer, PythonFormatter);
    // Serializing an in-memory `Value` into a `Vec` cannot fail.
    serde::Serialize::serialize(value, &mut serializer).expect("serializing JSON value");
    String::from_utf8(buffer).expect("serde_json emits UTF-8")
}

/// Render a string the way Python's `repr()` does.
///
/// Prefers single quotes, switching to double quotes only when the text
/// contains a single quote but no double quote — matching CPython.
///
/// # Parameters
/// - `text`: The string to quote.
pub fn python_repr(text: &str) -> String {
    let quote = if text.contains('\'') && !text.contains('"') {
        '"'
    } else {
        '\''
    };

    let mut out = String::with_capacity(text.len() + 2);
    out.push(quote);
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            c => out.push(c),
        }
    }
    out.push(quote);
    out
}

/// Format device entries as a GSettings array of JSON strings.
///
/// # Parameters
/// - `devices`: Resource Monitor GPU or disk device entries.
///
/// Returns a value accepted by GSettings for an `as` schema key, or `"[]"`.
pub fn format_gsettings_list(devices: &[Device]) -> String {
    if devices.is_empty() {
        return "[]".to_string();
    }
    let parts: Vec<String> = devices
        .iter()
        .map(|device| python_repr(&device.to_json()))
        .collect();
    format!("[{}]", parts.join(", "))
}

/// Requested Resource Monitor display mode, mirroring the Python keyword args.
#[derive(Debug, Clone, Copy, Default)]
pub struct DisplayMode {
    /// Display GPU memory as a percentage instead of numeric GB.
    pub gpu_memory_perc: bool,
    /// Display free disk space in numeric GB.
    pub disk_space_gb: bool,
    /// Legacy alias for `disk_space_gb`, kept as a compatibility surface.
    pub disk_space_perc: bool,
    /// Display only `/home` usage percentage.
    pub disk_space_perc_home_only: bool,
}

fn gsettings_set(ext_dir: &str, schema: &str, key: &str, value: &str) -> Vec<String> {
    vec![
        "gsettings".to_string(),
        "--schemadir".to_string(),
        ext_dir.to_string(),
        "set".to_string(),
        schema.to_string(),
        key.to_string(),
        value.to_string(),
    ]
}

/// Build the Resource Monitor `gsettings set` command arguments.
///
/// # Parameters
/// - `schema`: Resource Monitor GSettings schema ID.
/// - `ext_dir`: Path to the extension's compiled schema directory.
/// - `mode`: Requested display mode flags.
/// - `gpu_devices`: Optional GPU entries for `gpudeviceslist`.
/// - `disk_devices`: Optional disk entries for `diskdeviceslist`.
///
/// Returns command argument lists ready for [`apply_settings`].
pub fn build_gsettings_args(
    schema: &str,
    ext_dir: &str,
    mode: DisplayMode,
    gpu_devices: Option<&[Device]>,
    disk_devices: Option<&[Device]>,
) -> Vec<Vec<String>> {
    let mut commands: Vec<Vec<String>> = Vec::new();
    let configure_disk_space = mode.disk_space_gb || mode.disk_space_perc;
    let mut disk_devices: Option<Vec<Device>> = disk_devices.map(<[Device]>::to_vec);

    if mode.gpu_memory_perc {
        commands.push(gsettings_set(ext_dir, schema, "gpumemoryunit", "'perc'"));
    } else {
        commands.push(gsettings_set(ext_dir, schema, "gpumemoryunit", "'numeric'"));
    }

    if mode.disk_space_perc_home_only {
        commands.push(gsettings_set(ext_dir, schema, "diskstatsstatus", "false"));
        commands.push(gsettings_set(ext_dir, schema, "diskspaceunit", "'perc'"));
        commands.push(gsettings_set(ext_dir, schema, "diskspacemonitor", "'used'"));
        disk_devices =
            disk_devices.map(|devices| filter_disk_devices_to_mount_point(devices, Some("/home")));
    } else if configure_disk_space {
        commands.push(gsettings_set(ext_dir, schema, "diskstatsstatus", "false"));
        commands.push(gsettings_set(ext_dir, schema, "diskspaceunit", "'numeric'"));
        commands.push(gsettings_set(
            ext_dir,
            schema,
            "diskspaceunitmeasure",
            "'g'",
        ));
        commands.push(gsettings_set(ext_dir, schema, "diskspacemonitor", "'free'"));
        disk_devices =
            disk_devices.map(|devices| filter_disk_devices_to_mount_point(devices, Some("/home")));
    }

    if let Some(devices) = gpu_devices.filter(|devices| !devices.is_empty()) {
        commands.push(gsettings_set(
            ext_dir,
            schema,
            "gpudeviceslist",
            &format_gsettings_list(devices),
        ));
    }

    if let Some(devices) = disk_devices.as_deref().filter(|devices| !devices.is_empty()) {
        commands.push(gsettings_set(
            ext_dir,
            schema,
            "diskdeviceslist",
            &format_gsettings_list(devices),
        ));
    }

    commands
}

/// Execute one `gsettings set` command.
///
/// # Parameters
/// - `args`: Complete command arguments, program name first.
///
/// Returns `true` when the process exits successfully, otherwise `false`. The
/// child is killed and `false` returned when it outlives [`GSETTINGS_TIMEOUT`].
pub fn apply_settings(args: &[String]) -> bool {
    let Some((program, rest)) = args.split_first() else {
        logging::error("apply_settings called without a command");
        return false;
    };

    let spawned = Command::new(program)
        .args(rest)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();

    let mut child = match spawned {
        Ok(child) => child,
        Err(err) => {
            logging::error(format!("Unexpected error running gsettings: {err}"));
            return false;
        }
    };

    let deadline = Instant::now() + GSETTINGS_TIMEOUT;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    logging::error("gsettings command timed out");
                    return false;
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(err) => {
                logging::error(format!("Unexpected error running gsettings: {err}"));
                return false;
            }
        }
    };

    if status.success() {
        return true;
    }

    let mut stderr = String::new();
    if let Some(mut pipe) = child.stderr.take() {
        let _ = pipe.read_to_string(&mut stderr);
    }
    logging::error(format!("gsettings failed: {}", stderr.trim()));
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gpu_device() -> Device {
        Device::new()
            .with("version", 2)
            .with("type", "gpu")
            .with("device", "GPU-abc123")
            .with("name", "NVIDIA GeForce RTX 4090")
            .with("usage", true)
            .with("memory", true)
            .with("displayName", "")
    }

    #[test]
    fn json_uses_python_separators_and_insertion_order() {
        assert_eq!(
            gpu_device().to_json(),
            r#"{"version": 2, "type": "gpu", "device": "GPU-abc123", "name": "NVIDIA GeForce RTX 4090", "usage": true, "memory": true, "displayName": ""}"#
        );
    }

    #[test]
    fn python_repr_matches_cpython_quoting() {
        assert_eq!(python_repr("plain"), "'plain'");
        assert_eq!(python_repr(r#"{"a": 1}"#), r#"'{"a": 1}'"#);
        assert_eq!(python_repr("it's"), "\"it's\"");
        assert_eq!(python_repr("both\" and '"), r#"'both" and \''"#);
        assert_eq!(python_repr("back\\slash"), r"'back\\slash'");
    }

    #[test]
    fn empty_device_list_formats_as_empty_array() {
        assert_eq!(format_gsettings_list(&[]), "[]");
    }

    #[test]
    fn device_list_wraps_each_json_string_in_repr() {
        let formatted = format_gsettings_list(&[gpu_device()]);
        assert!(formatted.starts_with("['{\"version\": 2"));
        assert!(formatted.ends_with("\"displayName\": \"\"}']"));
    }

    fn disk_device(mount_point: &str) -> Device {
        crate::disks::build_disk_device_entry("/dev/sda2", mount_point)
    }

    #[test]
    fn gpu_numeric_mode_is_the_default_command() {
        let commands = build_gsettings_args("schema", "/ext", DisplayMode::default(), None, None);
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0][5], "gpumemoryunit");
        assert_eq!(commands[0][6], "'numeric'");
    }

    #[test]
    fn gpu_percentage_mode_switches_the_unit() {
        let mode = DisplayMode {
            gpu_memory_perc: true,
            ..DisplayMode::default()
        };
        let commands = build_gsettings_args("schema", "/ext", mode, None, None);
        assert_eq!(commands[0][6], "'perc'");
    }

    #[test]
    fn disk_space_gb_mode_emits_the_free_gigabyte_keys() {
        let mode = DisplayMode {
            disk_space_gb: true,
            ..DisplayMode::default()
        };
        let commands = build_gsettings_args("schema", "/ext", mode, None, None);
        let keys: Vec<&str> = commands.iter().map(|c| c[5].as_str()).collect();
        assert_eq!(
            keys,
            vec![
                "gpumemoryunit",
                "diskstatsstatus",
                "diskspaceunit",
                "diskspaceunitmeasure",
                "diskspacemonitor",
            ]
        );
        assert_eq!(commands[4][6], "'free'");
    }

    #[test]
    fn legacy_disk_space_perc_alias_still_configures_disk_space() {
        let mode = DisplayMode {
            disk_space_perc: true,
            ..DisplayMode::default()
        };
        let commands = build_gsettings_args("schema", "/ext", mode, None, None);
        assert!(commands.iter().any(|c| c[5] == "diskspaceunitmeasure"));
    }

    #[test]
    fn home_only_mode_uses_used_percentage_and_filters_devices() {
        let mode = DisplayMode {
            disk_space_perc_home_only: true,
            ..DisplayMode::default()
        };
        let devices = vec![disk_device("/"), disk_device("/home")];
        let commands = build_gsettings_args("schema", "/ext", mode, None, Some(&devices));
        let keys: Vec<&str> = commands.iter().map(|c| c[5].as_str()).collect();
        assert_eq!(
            keys,
            vec![
                "gpumemoryunit",
                "diskstatsstatus",
                "diskspaceunit",
                "diskspacemonitor",
                "diskdeviceslist",
            ]
        );
        let device_list = &commands[4][6];
        assert!(device_list.contains("/home"));
        assert_eq!(device_list.matches("\\\"mountPoint").count(), 0);
        assert_eq!(device_list.matches("mountPoint").count(), 1);
    }

    #[test]
    fn empty_device_lists_are_not_written() {
        let mode = DisplayMode {
            gpu_memory_perc: true,
            ..DisplayMode::default()
        };
        let commands = build_gsettings_args("schema", "/ext", mode, Some(&[]), Some(&[]));
        assert_eq!(commands.len(), 1);
    }

    #[test]
    fn apply_settings_reports_process_success_and_failure() {
        assert!(apply_settings(&["true".to_string()]));
        assert!(!apply_settings(&["false".to_string()]));
        assert!(!apply_settings(&[
            "rm-monitor-no-such-binary".to_string()
        ]));
        assert!(!apply_settings(&[]));
    }
}
